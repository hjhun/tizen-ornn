use super::{Action, Cli};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};
use tizen_ornn::Image;
mod build;
mod console;
mod session;

fn home() -> io::Result<PathBuf> {
    let dir = env::var_os("TIZEN_ORNN_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Tizen Ornn")))
        .ok_or_else(|| io::Error::other("Set TIZEN_ORNN_HOME to a writable directory."))?;
    std::path::absolute(dir)
}

fn qemu(tool: &str) -> io::Result<Command> {
    let file = format!("{tool}.exe");
    let mut dirs = Vec::new();
    if let Some(p) = env::var_os("TIZEN_ORNN_QEMU_DIR") {
        dirs.push(PathBuf::from(p));
    } else {
        if let Some(runtime) = env::current_exe()?.parent() {
            dirs.push(runtime.join("qemu"));
        }
        if let Some(p) = env::var_os("ProgramFiles") {
            dirs.push(PathBuf::from(p).join("qemu"));
        }
        if let Some(paths) = env::var_os("PATH") {
            dirs.extend(env::split_paths(&paths).filter(|p| p.is_absolute()));
        }
    }
    let path=dirs.into_iter().map(|p|p.join(&file)).find(|p|p.is_file())
        .ok_or_else(||io::Error::other(format!("Install 64-bit QEMU for Windows or set TIZEN_ORNN_QEMU_DIR to the directory containing {file}.")))?;
    Ok(Command::new(path))
}

fn checked(cmd: &mut Command, label: &str) -> io::Result<()> {
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{label} exited with {status}")))
    }
}

fn disk(image: &Path) -> io::Result<(&'static str, &'static str)> {
    if !image.join("bzImage").is_file() {
        return Err(io::Error::other(format!(
            "Missing bzImage in {}",
            image.display()
        )));
    }
    for (name, format) in [
        ("tizen-rootfs.qcow2", "qcow2"),
        ("tizen-rootfs.ext4", "raw"),
    ] {
        if image.join(name).is_file() {
            return Ok((name, format));
        }
    }
    Err(io::Error::other(format!(
        "Missing Tizen disk in {}",
        image.display()
    )))
}

fn images() -> io::Result<Vec<Image>> {
    let root = home()?.join("images");
    let mut result = Vec::new();
    if root.is_dir() {
        for item in fs::read_dir(root)? {
            let item = item?;
            if !item.file_name().to_string_lossy().starts_with('.') && disk(&item.path()).is_ok() {
                result.push(Image {
                    arch: "x86_64".into(),
                    path: item.path(),
                });
            }
        }
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

fn select_image(image: Option<PathBuf>) -> io::Result<PathBuf> {
    Ok(match image {
        Some(path) => path,
        None => {
            let images = images()?;
            images
                .iter()
                .find(|item| item.path.ends_with("default"))
                .or_else(|| images.first())
                .ok_or_else(|| io::Error::other("Build or select an image first."))?
                .path
                .clone()
        }
    })
}

fn boot(
    image: Option<PathBuf>,
    graphics: bool,
    share: Option<PathBuf>,
    read_only: bool,
) -> io::Result<()> {
    let image = select_image(image)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(image.join(".ornn-session"))?;
    lock.try_lock()
        .map_err(|_| io::Error::other("This image is already running."))?;
    let stop = image.join(".ornn-shutdown");
    if stop.exists() {
        fs::remove_file(&stop)?;
    }
    let (name, format) = disk(&image)?;
    let mut cmd = qemu("qemu-system-x86_64")?;
    cmd.current_dir(&image).args([
        "-name",
        "Tizen Ornn",
        "-machine",
        "pc",
        "-accel",
        "tcg,thread=multi",
        "-cpu",
        "max",
        "-m",
        "3072",
        "-smp",
        "2",
        "-nodefaults",
        "-no-reboot",
        "-monitor",
        "none",
        "-kernel",
        "bzImage",
        "-drive",
        &format!("file={name},format={format},if=virtio"),
        "-netdev",
        "user,id=net",
        "-device",
        "virtio-net-pci,netdev=net,romfile=",
        "-device",
        "virtio-rng-pci",
    ]);
    let selected = match share {
        Some(path) => Some(path),
        None => tizen_ornn::workdir::shared_directory()?,
    };
    let folder = match selected {
        Some(path) => std::path::absolute(path)?,
        None => {
            let path = tizen_ornn::workdir::default_share()?;
            fs::create_dir_all(&path)?;
            path
        }
    };
    if !folder.is_dir() {
        return Err(io::Error::other(format!(
            "Missing shared folder: {}",
            folder.display()
        )));
    }
    // QEMU needs a seekable file for fw_cfg. Keep it alive through guest shutdown.
    let temp = tempfile::tempdir()?;
    let script = temp.path().join("share.sh");
    let status = temp.path().join("boot-status");
    fs::write(&script, include_bytes!("../guest/tizen-ornn-share"))?;
    let escaped = |path: &Path| path.display().to_string().replace(',', ",,");
    let readonly = if read_only { "on" } else { "off" };
    cmd.args([
        "-fsdev",
        &format!(
            "local,id=hostshare,path={},security_model=mapped-xattr,readonly={readonly}",
            escaped(&folder)
        ),
        "-device",
        "virtio-9p-pci,fsdev=hostshare,mount_tag=tizen-share",
        "-fw_cfg",
        &format!("name=opt/tizen-ornn/share-script,file={}", escaped(&script)),
    ]);
    cmd.args(["-device", "virtio-serial-pci,id=guestbus"]);
    let console = if image.join(".tizen-virtio-console").is_file() {
        cmd.args([
            "-serial",
            "null",
            "-device",
            "virtconsole,bus=guestbus.0,nr=0,chardev=console",
        ]);
        "hvc0"
    } else {
        cmd.args(["-serial", "chardev:console"]);
        "ttyS0,115200"
    };
    cmd.args([
        "-chardev",
        &format!("file,id=bootstatus,path={}", escaped(&status)),
        "-serial",
        "chardev:bootstatus",
    ]);
    let boot_script = temp.path().join("boot.sh");
    fs::write(&boot_script, include_bytes!("../guest/tizen-ornn-boot"))?;
    cmd.args([
        "-fw_cfg",
        &format!("name=opt/tizen-ornn/boot,file={}", escaped(&boot_script)),
    ]);
    if let Some(pipe) = env::var_os("TIZEN_QMP") {
        let pipe = pipe
            .to_str()
            .filter(|s| {
                !s.is_empty()
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            })
            .ok_or_else(|| io::Error::other("TIZEN_QMP must be a local Windows pipe name."))?;
        cmd.args(["-qmp", &format!("pipe:{pipe}")]);
    }
    let target = if graphics {
        let runtime = Path::new(cmd.get_program()).parent().unwrap();
        for library in ["libEGL.dll", "libGLESv2.dll", "libgallium_wgl.dll"] {
            if !runtime.join(library).is_file() {
                return Err(io::Error::other(format!(
                    "Missing {library} in {}. Use the Windows portable bundle or install the packaged Mesa runtime beside QEMU for graphical boot.",
                    runtime.display()
                )));
            }
        }
        cmd.env("GALLIUM_DRIVER", "llvmpipe");
        let graphics_script = temp.path().join("graphics.sh");
        fs::write(
            &graphics_script,
            include_bytes!("../guest/tizen-ornn-graphics"),
        )?;
        cmd.args([
            "-fw_cfg",
            &format!(
                "name=opt/tizen-ornn/graphics,file={}",
                escaped(&graphics_script)
            ),
            "-fw_cfg",
            "name=opt/tizen-ornn/skia,string=1",
        ]);
        cmd.args([
            "-display",
            "sdl,gl=on,show-cursor=on",
            "-device",
            "virtio-gpu-gl-pci,xres=1024,yres=768",
            "-device",
            "virtio-keyboard-pci",
            "-device",
            "virtio-tablet-pci",
        ]);
        "graphical.target"
    } else {
        cmd.args(["-display", "none"]);
        "tizen-dev.target"
    };
    let mode = if read_only { "ro" } else { "rw" };
    cmd.args([
        "-fw_cfg",
        &format!("name=opt/tizen-ornn/share,string={mode}"),
    ]);
    let setup = "mount -t sysfs sysfs /sys && /bin/sh /sys/firmware/qemu_fw_cfg/by_name/opt/tizen-ornn/boot/raw && exec /usr/lib/systemd/systemd";
    let (rows, cols) = env::var("TIZEN_ORNN_INITIAL_SIZE")
        .ok()
        .and_then(|value| {
            let (r, c) = value.split_once('x')?;
            Some((r.parse::<u16>().ok()?, c.parse::<u16>().ok()?))
        })
        .filter(|(r, c)| *r > 0 && *c > 0)
        .unwrap_or((24, 80));
    cmd.args(["-append", &format!("root=/dev/vda rootfstype=ext4 rootwait rw console={console} init=/bin/sh systemd.unit={target} net.ifnames=0 tizen.console_size={rows}x{cols} -- -c \"{setup}; echo tizen-ornn: shared folder setup failed >&2; printf ! >/dev/ttyS1; sync; exec /usr/bin/systemctl --force --force poweroff\"")]);
    console::run(&mut cmd, &stop, &image)?;
    if fs::read(status)?.contains(&b'!') {
        return Err(io::Error::other(
            "Guest shared folder setup failed. See the guest console.",
        ));
    }
    Ok(())
}

fn launch(
    image: Option<PathBuf>,
    graphics: bool,
    share: Option<PathBuf>,
    read_only: bool,
) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    let image = select_image(image)?;
    disk(&image)?;
    let mut command = Command::new(env::current_exe()?);
    command.args(["run", "--worker", "--image-dir"]).arg(&image);
    if let Some((rows, cols)) = console::size() {
        command.env("TIZEN_ORNN_INITIAL_SIZE", format!("{rows}x{cols}"));
    }
    if graphics {
        command.arg("--gui");
    }
    if let Some(share) = share {
        command
            .arg(if read_only { "--share-ro" } else { "--share" })
            .arg(share);
    }
    // The worker has no console attachment. Closing a viewer leaves QEMU alive.
    let mut child = command
        .creation_flags(0x08000000)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()?;
    session::attach(&image, Some(&mut child))?;
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("VM exited with {status}")))
    }
}

pub fn execute(cli: Cli) -> io::Result<()> {
    match cli.command {
        Some(Action::Shutdown { image_dir }) => {
            let image = select_image(image_dir)?;
            let session = fs::OpenOptions::new()
                .write(true)
                .open(image.join(".ornn-session"))?;
            match session.try_lock() {
                Ok(()) => Err(io::Error::other("This image is stopped.")),
                Err(fs::TryLockError::Error(error)) => Err(error),
                Err(fs::TryLockError::WouldBlock) => {
                    fs::write(image.join(".ornn-shutdown"), [])?;
                    let start = std::time::Instant::now();
                    loop {
                        match session.try_lock() {
                            Ok(()) => return Ok(()),
                            Err(fs::TryLockError::WouldBlock) => (),
                            Err(fs::TryLockError::Error(error)) => return Err(error),
                        }
                        if start.elapsed().as_secs() >= 90 {
                            return Err(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "Guest has not finished shutting down.",
                            ));
                        }
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                }
            }
        }
        Some(Action::Images) => {
            println!("{}", serde_json::to_string(&images()?)?);
            Ok(())
        }
        Some(Action::Build { name, repository }) => {
            if let Some(name) = &name {
                crate::build_source::validate_name(name)?;
            }
            let source = repository
                .as_deref()
                .map(crate::build_source::BuildSource::resolve)
                .transpose()?;
            let name = name
                .or_else(|| source.as_ref().map(|s| s.release.replace('.', "-")))
                .unwrap_or_else(|| "default".into());
            build::create(&home()?, &name, source.as_ref())
        }
        Some(Action::ShareDir { path, .. }) => {
            tizen_ornn::workdir::save_share(path.as_deref().unwrap_or(""))
        }
        Some(Action::Console { image_dir }) => session::attach(&image_dir, None),
        None => launch(None, cli.gui, None, false),
        Some(Action::Run {
            worker,
            image_dir,
            share,
            share_ro,
        }) => {
            let read_only = share_ro.is_some();
            if worker {
                boot(image_dir, cli.gui, share.or(share_ro), read_only)
            } else {
                launch(image_dir, cli.gui, share.or(share_ro), read_only)
            }
        }
    }
}
