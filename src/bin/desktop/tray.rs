//! One monitor owns the notification icon for each running image. Its lifetime
//! follows the VM lock, independently of launcher and terminal windows.
use super::{Launcher, cli};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub fn running(dir: &Path) -> io::Result<bool> {
    #[cfg(windows)]
    let lock = dir.join(".ornn-session");
    #[cfg(target_os = "linux")]
    let lock = if tizen_ornn::arm_arch(dir).is_some() {
        dir.join("build.lock")
    } else if dir.join("tizen-rootfs.ext4").is_file() {
        dir.join("tizen-rootfs.ext4")
    } else {
        dir.join("tizen-rootfs.qcow2")
    };
    match File::open(lock) {
        Ok(file) => match file.try_lock_shared() {
            Ok(()) => Ok(false),
            Err(std::fs::TryLockError::WouldBlock) => Ok(true),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

pub fn start(dir: &Path) -> io::Result<std::process::Child> {
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.arg("--tray-monitor")
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

pub fn monitor(dir: &Path) -> Result<(), String> {
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(".ornn-tray.lock"))
        .map_err(|e| e.to_string())?;
    match lock.try_lock() {
        Ok(()) => (),
        Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
        Err(e) => return Err(e.to_string()),
    }
    let until = Instant::now() + Duration::from_secs(60);
    while !running(dir).map_err(|e| e.to_string())? {
        if Instant::now() >= until {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }

    #[cfg(windows)]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new("powershell.exe");
        cmd.creation_flags(0x08000000).args([
            "-NoProfile",
            "-NonInteractive",
            "-STA",
            "-Command",
            include_str!("tray.ps1"),
        ]);
        cmd
    };
    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut cmd = Command::new("/usr/bin/python3");
        cmd.args(["-c", include_str!("tray.py")]);
        cmd
    };
    let language = tizen_ornn::workdir::language().unwrap_or_default();
    cmd.env("ORNN_TRAY_RUNNING", language.text("running", "실행 중"))
        .env(
            "ORNN_TRAY_TERMINAL",
            language.text("Open terminal", "터미널 열기"),
        )
        .env("ORNN_TRAY_SETTINGS", language.text("Settings", "설정"))
        .env(
            "ORNN_TRAY_SHUTDOWN",
            language.text("Shut down guest", "게스트 종료"),
        )
        .env(
            "ORNN_TRAY_IMAGE",
            dir.file_name().unwrap_or(dir.as_os_str()),
        )
        .env(
            "ORNN_TRAY_GUI",
            std::env::current_exe().map_err(|e| e.to_string())?,
        )
        .env("ORNN_TRAY_PARENT", std::process::id().to_string());
    let mut helper = cmd
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let output = helper.stdout.take().unwrap();
    let (tx, events) = mpsc::channel();
    thread::spawn(move || {
        for line in io::BufReader::new(output).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut launcher = Launcher::default();
    let result = (|| {
        let path = dir.to_str().ok_or("Invalid image path")?;
        let mut shutdown = None::<std::process::Child>;
        while running(dir).map_err(|e| e.to_string())? {
            launcher
                .processes
                .retain_mut(|p| matches!(p.try_wait(), Ok(None)));
            if let Some(child) = shutdown.as_mut()
                && child.try_wait().map_err(|e| e.to_string())?.is_some()
            {
                shutdown = None;
            }
            match events.recv_timeout(Duration::from_secs(1)) {
                Ok(action) => match action.as_str() {
                    "settings" => {
                        launcher.processes.push(
                            Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                                .arg("--settings")
                                .arg(dir)
                                .spawn()
                                .map_err(|e| e.to_string())?,
                        );
                    }
                    "terminal" => launcher.terminal(&["console", "--image-dir", path])?,
                    "shutdown" if shutdown.is_none() => {
                        let mut cmd = cli()?;
                        launcher.configure(&mut cmd)?;
                        shutdown = Some(
                            cmd.args(["shutdown", "--image-dir", path])
                                .spawn()
                                .map_err(|e| e.to_string())?,
                        );
                    }
                    _ => (),
                },
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Desktop notification service unavailable".into());
                }
            }
        }
        Ok(())
    })();
    // The adapter disposes its icon when this monitor exits.
    result
}
