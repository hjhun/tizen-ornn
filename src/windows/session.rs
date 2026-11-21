// The VM owns the QEMU serial connection. Viewer processes can disconnect without closing it.
use super::console;
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Child,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
    sync::{mpsc, watch},
};

fn descriptor(image: &Path) -> io::Result<PathBuf> {
    use std::os::windows::ffi::OsStrExt;
    let mut hash = Sha256::new();
    for unit in image.canonicalize()?.as_os_str().encode_wide() {
        hash.update(unit.to_le_bytes());
    }
    Ok(std::env::temp_dir().join(format!("ornn-console-{:x}.json", hash.finalize())))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Endpoint {
    pid: u32,
    pipe: String,
}

pub struct Listener {
    name: String,
    server: NamedPipeServer,
    _descriptor: tempfile::TempPath,
    _nonce: tempfile::TempDir,
}
impl Listener {
    pub fn new(image: &Path) -> io::Result<Self> {
        let nonce = tempfile::tempdir()?;
        let name = format!(
            r"\\.\pipe\ornn-view-{}",
            nonce.path().file_name().unwrap().to_string_lossy()
        );
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(&name)?;
        let path = descriptor(image)?;
        let mut tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        serde_json::to_writer(
            &mut tmp,
            &Endpoint {
                pid: std::process::id(),
                pipe: name.clone(),
            },
        )?;
        tmp.into_temp_path().persist(&path).map_err(|e| e.error)?;
        Ok(Self {
            name,
            server,
            _descriptor: tempfile::TempPath::try_from_path(path)?,
            _nonce: nonce,
        })
    }

    pub async fn serve(
        mut self,
        pipe: tokio::net::TcpStream,
        size: watch::Sender<Option<(u16, u16)>>,
    ) -> io::Result<()> {
        let (mut reader, mut writer) = tokio::io::split(pipe);
        let (output, _) = watch::channel((0u64, VecDeque::<u8>::new()));
        let (input, mut keys) = mpsc::channel::<Vec<u8>>(16);
        let mut buf = [0u8; 8192];
        // A busy guest may stop reading input while its output pipe is full.
        // Keep draining the display even when a viewer's paste is backpressured.
        let write_input = async {
            while let Some(bytes) = keys.recv().await {
                writer.write_all(&bytes).await?;
            }
            Ok(())
        };
        tokio::pin!(write_input);
        loop {
            tokio::select! {
                result = &mut write_input => return result,
                count = reader.read(&mut buf) => {
                    let count = match count {
                        Ok(0) => break,
                        Ok(n) => n,
                        // Windows QEMU can reset its serial socket during poweroff.
                        // The worker checks the actual QEMU exit status after EOF.
                        Err(e) if matches!(e.kind(),
                            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
                        ) => break,
                        Err(e) => return Err(e),
                    };
                    output.send_modify(|(end, history)| {
                        *end += count as u64;
                        history.extend(&buf[..count]);
                        history.drain(..history.len().saturating_sub(65536));
                    });
                }
                connected = self.server.connect() => {
                    connected?;
                    let next = ServerOptions::new().reject_remote_clients(true).create(&self.name)?;
                    let client = std::mem::replace(&mut self.server, next);
                    tokio::spawn(viewer(client, output.subscribe(), input.clone(), size.clone()));
                }
            }
        }
        Ok(())
    }
}

async fn viewer(
    client: NamedPipeServer,
    mut output: watch::Receiver<(u64, VecDeque<u8>)>,
    input: mpsc::Sender<Vec<u8>>,
    size: watch::Sender<Option<(u16, u16)>>,
) -> io::Result<()> {
    let (mut reader, mut writer) = tokio::io::split(client);
    let display = async {
        // QEMU may deliver the boot log a byte at a time. Keep a cursor in
        // the byte history so a slow console can catch up across many reads.
        let mut cursor = 0;
        loop {
            let bytes: Vec<u8> = {
                let state = output.borrow_and_update();
                let (end, history) = &*state;
                let pending = (end - cursor).min(history.len() as u64) as usize;
                cursor = *end;
                history.range(history.len() - pending..).copied().collect()
            };
            writer.write_all(&bytes).await?;
            if output.changed().await.is_err() {
                break;
            }
        }
        Ok(())
    };
    let keyboard = async {
        loop {
            let count = reader.read_u16().await?;
            if count == 0 {
                let rows = reader.read_u16().await?;
                let cols = reader.read_u16().await?;
                if rows > 0 && cols > 0 {
                    size.send_replace(Some((rows, cols)));
                }
            } else {
                let mut bytes = vec![0; usize::from(count)];
                reader.read_exact(&mut bytes).await?;
                if input.send(bytes).await.is_err() {
                    return Ok(());
                }
            }
        }
    };
    tokio::select! { result=display=>result,result=keyboard=>result }
}

pub fn attach(image: &Path, mut worker: Option<&mut Child>) -> io::Result<()> {
    let path = descriptor(image)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let start = Instant::now();
        let pipe = loop {
            if let Some(child) = worker.as_deref_mut()
                && let Some(status) = child.try_wait()?
            {
                return Err(io::Error::other(format!("VM startup failed ({status}).")));
            }
            if let Ok(data) = fs::read(&path)
                && let Ok(endpoint) = serde_json::from_slice::<Endpoint>(&data)
                && worker
                    .as_ref()
                    .is_none_or(|child| child.id() == endpoint.pid)
                && let Ok(pipe) = ClientOptions::new().open(endpoint.pipe)
            {
                break pipe;
            }
            if start.elapsed() > Duration::from_secs(if worker.is_some() { 65 } else { 5 }) {
                return Err(io::Error::other("No console is available for this image."));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        let _modes = console::terminal_modes()?;
        let (mut reader, mut writer) = tokio::io::split(pipe);
        let mut input = console::keyboard();
        let mut buf = [0u8; 8192];
        let mut applied = None;
        let mut timer = tokio::time::interval(Duration::from_millis(250));
        let mut output = io::stdout().lock();
        let mut input_open = true;
        loop {
            tokio::select! {
                count=reader.read(&mut buf)=>match count {
                    Ok(0)=>break,
                    Ok(n)=>{output.write_all(&buf[..n])?;output.flush()?;}
                    Err(e) if e.kind()==io::ErrorKind::BrokenPipe=>break,
                    Err(e)=>return Err(e)
                },
                bytes=input.recv(),if input_open=>{
                    if let Some(bytes)=bytes {
                        writer.write_u16(bytes.len() as u16).await?;
                        writer.write_all(&bytes).await?;
                    } else {input_open=false;}
                },
                _=timer.tick()=>{
                    if let Some(current)=console::size() && applied!=Some(current) {
                        writer.write_u16(0).await?;
                        writer.write_u16(current.0).await?;
                        writer.write_u16(current.1).await?;
                        applied=Some(current);
                    }
                }
            }
        }
        Ok(())
    })
}
