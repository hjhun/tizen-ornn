// Keep QEMU's serial and guest control channels independent of terminal windows.
use std::{
    io::{self, Read},
    process::Command,
    thread,
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use windows_sys::Win32::{Foundation::HANDLE, System::Console::*};

pub(super) struct Mode(HANDLE, u32);

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

pub(super) fn terminal_modes() -> io::Result<(Option<Mode>, Option<Mode>)> {
    Ok((
        Mode::set(
            STD_INPUT_HANDLE,
            ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT,
            ENABLE_VIRTUAL_TERMINAL_INPUT,
        )?,
        Mode::set(STD_OUTPUT_HANDLE, 0, ENABLE_VIRTUAL_TERMINAL_PROCESSING)?,
    ))
}

pub(super) fn keyboard() -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel(8);
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
    rx
}

pub(super) fn size() -> Option<(u16, u16)> {
    let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
    if unsafe { GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), &mut info) } == 0 {
        return None;
    }
    let rect = info.srWindow;
    Some((
        (rect.Bottom - rect.Top + 1) as u16,
        (rect.Right - rect.Left + 1) as u16,
    ))
}

// Only the QEMU child may take these two short-lived loopback connections.
fn belongs_to_child(peer: u16, local: u16, pid: u32) -> io::Result<bool> {
    use windows_sys::Win32::{
        Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR},
        NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
        },
        Networking::WinSock::AF_INET,
    };
    let mut table = vec![0u32; 1024];
    loop {
        let mut bytes = (table.len() * 4) as u32;
        let result = unsafe {
            GetExtendedTcpTable(
                table.as_mut_ptr().cast(),
                &mut bytes,
                0,
                AF_INET.into(),
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if result == ERROR_INSUFFICIENT_BUFFER {
            table.resize((bytes as usize).div_ceil(4), 0);
            continue;
        }
        if result != NO_ERROR {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
        // MIB_TCPTABLE_OWNER_PID starts with a DWORD row count followed by DWORD-aligned rows.
        let count = table[0] as usize;
        if count > (table.len() - 1) * 4 / std::mem::size_of::<MIB_TCPROW_OWNER_PID>() {
            return Err(io::Error::other("Invalid Windows TCP table."));
        }
        let rows = unsafe {
            std::slice::from_raw_parts(table[1..].as_ptr().cast::<MIB_TCPROW_OWNER_PID>(), count)
        };
        let localhost = u32::from_ne_bytes([127, 0, 0, 1]);
        return Ok(rows.iter().any(|row| {
            row.dwOwningPid == pid
                && row.dwLocalAddr == localhost
                && row.dwRemoteAddr == localhost
                && u16::from_be(row.dwLocalPort as u16) == peer
                && u16::from_be(row.dwRemotePort as u16) == local
        }));
    }
}

async fn accept_qemu(listener: TcpListener, pid: u32) -> io::Result<TcpStream> {
    let local = listener.local_addr()?.port();
    loop {
        let (stream, peer) = listener.accept().await?;
        if belongs_to_child(peer.port(), local, pid)? {
            stream.set_nodelay(true)?;
            return Ok(stream);
        }
    }
}

async fn control(
    stream: TcpStream,
    stop: &std::path::Path,
    sizes: tokio::sync::watch::Receiver<Option<(u16, u16)>>,
) -> io::Result<()> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut pipe = BufReader::new(stream);
    let mut applied = None;
    let mut sequence = 0u64;
    loop {
        tokio::time::sleep(Duration::from_millis(350)).await;
        let size = *sizes.borrow();
        let shutdown = stop.is_file();
        if !shutdown && size == applied {
            continue;
        }
        sequence += 1;
        let id = format!("{sequence:032x}");
        let request = if shutdown {
            serde_json::json!({"op":"poweroff","id":id})
        } else if let Some((rows, cols)) = size {
            serde_json::json!({"op":"resize","id":id,"names":[rows.to_string(),cols.to_string()]})
        } else {
            continue;
        };
        pipe.get_mut()
            .write_all(format!("{request}\n").as_bytes())
            .await?;
        let mut line = String::new();
        loop {
            line.clear();
            match tokio::time::timeout(Duration::from_secs(4), pipe.read_line(&mut line)).await {
                Err(_) => break,
                Ok(result) => {
                    if result? == 0 {
                        return Ok(());
                    }
                }
            }
            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line)
                && msg["id"] == id
                && msg["type"] == "exit"
            {
                if msg["code"] == 0 {
                    applied = size;
                }
                break;
            }
        }
    }
}

pub(super) fn run(
    cmd: &mut Command,
    stop: &std::path::Path,
    image: &std::path::Path,
) -> io::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let (serial, control_listener) = rt.block_on(async {
        Ok::<_, io::Error>((
            TcpListener::bind(("127.0.0.1", 0)).await?,
            TcpListener::bind(("127.0.0.1", 0)).await?,
        ))
    })?;
    for (id, listener) in [("console", &serial), ("control", &control_listener)] {
        cmd.args([
            "-chardev",
            &format!(
                "socket,id={id},host=127.0.0.1,port={},server=off",
                listener.local_addr()?.port()
            ),
        ]);
    }
    cmd.args([
        "-device",
        "virtserialport,bus=guestbus.0,chardev=control,name=org.tizen.vm.packages",
    ]);
    let listener = {
        let _guard = rt.enter();
        super::session::Listener::new(image)?
    };
    let (size_tx, size_rx) = tokio::sync::watch::channel(None);
    let mut child = cmd.spawn()?;
    let result=rt.block_on(async {
        let pid=child.id();
        let connect=async {tokio::try_join!(accept_qemu(serial,pid),accept_qemu(control_listener,pid))};
        let exited=async {
            loop {
                if let Some(status)=child.try_wait()? { return Err(io::Error::other(format!("QEMU exited during startup: {status}"))); }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        let (pipe,control_stream)=tokio::select!{
            result=connect=>result?,
            result=exited=>return result,
            _=tokio::time::sleep(Duration::from_secs(60))=>return Err(io::Error::new(io::ErrorKind::TimedOut,"QEMU did not connect its guest console.")),
        };
        let forward = listener.serve(pipe, size_tx);
        tokio::pin!(forward);
        tokio::select! {
            result = &mut forward => result,
            result = control(control_stream, stop,size_rx) => {
                if let Err(error) = result { eprintln!("Guest control channel: {error}"); }
                forward.await
            }
        }
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
