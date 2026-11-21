// QEMU's Windows stdio backend can discard input while the emulated UART is
// full. Its named-pipe backend reads only as many bytes as the guest accepts.
use std::{
    io::{self, Read, Write},
    process::Command,
    thread,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient},
    sync::mpsc,
};
use windows_sys::Win32::{Foundation::HANDLE, System::Console::*};

struct Mode(HANDLE, u32);

impl Mode {
    fn set(stream: u32, remove: u32, add: u32) -> io::Result<Option<Self>> {
        // GetStdHandle returns a borrowed process handle; never close it.
        unsafe {
            let h = GetStdHandle(stream);
            let mut old = 0;
            if GetConsoleMode(h, &mut old) == 0 {
                return Ok(None);
            }
            if SetConsoleMode(h, (old & !remove) | add) == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Some(Self(h, old)))
        }
    }
}
impl Drop for Mode {
    fn drop(&mut self) {
        unsafe {
            SetConsoleMode(self.0, self.1);
        }
    }
}

async fn forward(pipe: NamedPipeClient) -> io::Result<()> {
    let _input = Mode::set(
        STD_INPUT_HANDLE,
        ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT,
        ENABLE_VIRTUAL_TERMINAL_INPUT,
    )?;
    let _output = Mode::set(STD_OUTPUT_HANDLE, 0, ENABLE_VIRTUAL_TERMINAL_PROCESSING)?;
    let (mut reader, mut writer) = tokio::io::split(pipe);
    let (tx, mut rx) = mpsc::channel(8);
    // Keep blocking console reads off the event loop. When the guest powers off
    // the CLI exits even when its terminal is still waiting for a keystroke.
    thread::spawn(move || {
        let mut input = io::stdin().lock();
        let mut buf = [0u8; 1024];
        while let Ok(n) = input.read(&mut buf) {
            if n == 0 || tx.blocking_send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut buf = [0u8; 8192];
    let mut input_open = true;
    let mut output = io::stdout().lock();
    loop {
        tokio::select! {
            count=reader.read(&mut buf)=>{
                match count {
                    Ok(0)=>break,
                    Ok(n)=>{output.write_all(&buf[..n])?;output.flush()?;}
                    Err(e) if e.kind()==io::ErrorKind::BrokenPipe=>break,
                    Err(e)=>return Err(e),
                }
            }
            data=rx.recv(), if input_open=>{
                if let Some(data)=data {writer.write_all(&data).await?;}
                else {input_open=false;}
            }
        }
    }
    Ok(())
}

pub(super) fn run(cmd: &mut Command) -> io::Result<()> {
    let name = tempfile::Builder::new()
        .prefix("tizen-ornn-")
        .rand_bytes(16)
        .tempdir()?;
    let leaf = name.path().file_name().unwrap().to_string_lossy();
    let path = format!(r"\\.\pipe\{leaf}");
    cmd.args(["-chardev", &format!("pipe,id=console,path={leaf}")]);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    let mut child = cmd.spawn()?;
    let start = Instant::now();
    let result = rt.block_on(async {
        let pipe = loop {
            if let Some(exit) = child.try_wait()? {
                return Err(io::Error::other(format!(
                    "QEMU exited before opening the guest console: {exit}"
                )));
            }
            match ClientOptions::new().open(&path) {
                Ok(pipe) => break pipe,
                Err(e)
                    if e.kind() == io::ErrorKind::NotFound
                        && start.elapsed() < Duration::from_secs(60) =>
                {
                    thread::sleep(Duration::from_millis(25))
                }
                Err(e) => return Err(e),
            }
        };
        forward(pipe).await
    });
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait()?;
    result?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("QEMU exited with {status}")))
    }
}
