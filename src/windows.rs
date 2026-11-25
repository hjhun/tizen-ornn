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
            "Expected an x86_64 guest image (missing bzImage) in {}",
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
            if item.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = item.path();
            if disk(&path).is_ok() {
                result.push(Image {
                    arch: "x86_64".into(),
                    path,
                });
            }
        }
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

fn delete_disk(name: &str) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;
    crate::build_source::validate_name(name)?;
    let root = home()?.join("images");
    let image = root.join(name);
    let partitions = image.join("disk");
    let arm = tizen_ornn::arm_arch(&image).is_some();
    let mut targets = Vec::new();
    if arm {
        for part in ["rootfs.img", "system-data.img", "user.img", "hal.img"] {
            targets.push(partitions.join(part));
        }
        if fs::read_dir(&partitions)?.count() != targets.len()
            || fs::symlink_metadata(&partitions)?.file_attributes() & 0x400 != 0
        {
            return Err(io::Error::other("Unexpected ARM partition directory."));
        }
    } else {
        let (filename, _) = disk(&image)?;
        if image.join("tizen-rootfs.ext4").exists() && image.join("tizen-rootfs.qcow2").exists() {
            return Err(io::Error::other("Expected exactly one guest disk."));
        }
        targets.push(image.join(filename));
    }
    for path in [&root, &image].into_iter().chain(targets.iter()) {
        if fs::symlink_metadata(path)?.file_attributes() & 0x400 != 0 {
            return Err(io::Error::other("Cannot delete through a reparse point."));
        }
    }
    let session = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(image.join(".ornn-session"))?;
    session.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => io::Error::other("This image is in use."),
        fs::TryLockError::Error(error) => error,
    })?;
    let canonical = targets
        .iter()
        .map(|p| p.canonicalize())
        .collect::<io::Result<Vec<_>>>()?;
    for item in fs::read_dir(&root)? {
        let overlay = item?.path().join("tizen-rootfs.qcow2");
        if !overlay.is_file() || canonical.contains(&overlay.canonicalize()?) {
            continue;
        }
        let info = qemu("qemu-img")?
            .args(["info", "-U", "--output=json", "--backing-chain"])
            .arg(&overlay)
            .output()?;
        if !info.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&info.stderr).into_owned(),
            ));
        }
        let layers: Vec<serde_json::Value> = serde_json::from_slice(&info.stdout)?;
        for layer in layers {
            if let Some(backing) = layer["full-backing-filename"].as_str()
                && canonical.contains(&Path::new(backing).canonicalize()?)
            {
                return Err(io::Error::other(format!(
                    "{} uses this disk as a backing image.",
                    overlay.display()
                )));
            }
        }
    }
    if arm {
        let trash = tempfile::tempdir_in(&image)?;
        fs::rename(partitions, trash.path().join("disk"))?;
        trash.close()
    } else {
        fs::remove_file(&targets[0])
    }
}

fn select_image(image: Option<PathBuf>) -> io::Result<PathBuf> {
    Ok(match image {
        Some(path) => path,
        None => {
            let images = images()?;
            let updated: Option<String> = match fs::read(home()?.join("updated-image.json")) {
                Ok(data) => Some(serde_json::from_slice(&data)?),
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(e),
            };
            images
                .iter()
                .find(|i| {
                    updated
                        .as_deref()
                        .is_some_and(|name| i.path.file_name() == Some(std::ffi::OsStr::new(name)))
                })
                .or_else(|| images.iter().find(|item| item.path.ends_with("default")))
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
    let (name, format) = disk(&image)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(image.join(".ornn-session"))?;
    lock.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => io::Error::other("This image is already running."),
        fs::TryLockError::Error(error) => error,
    })?;
    let stop = image.join(".ornn-shutdown");
    if stop.exists() {
        fs::remove_file(&stop)?;
    }
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
        "-monitor",
        "none",
    ]);
    cmd.arg("-kernel").arg(build::kernel_for_boot(&image)?);
    cmd.args([
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
    // Windows directory attributes can make a writable export appear as 0555.
    let mut permissions = fs::metadata(&folder)?.permissions();
    if !read_only && permissions.readonly() {
        // Verify the current account can write before clearing the folder flag.
        tempfile::NamedTempFile::new_in(&folder)?.close()?;
        permissions.set_readonly(false);
        fs::set_permissions(&folder, permissions)?;
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
        // Virgl emits gl_SampleID without the extension directive on Windows.
        // Let the bundled Mesa compiler use its supported GLSL extensions.
        cmd.env("force_glsl_extensions_warn", "true");
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
            // SDL can deadlock while the x86 vCPU updates the Windows title.
            "gtk,gl=on,show-cursor=on",
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
    let mode = if read_only { "ro:user" } else { "rw:user" };
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
    cmd.args(["-append", &format!("root=/dev/vda rootfstype=ext4 rootwait rw console={console} loglevel=0 init=/bin/sh systemd.unit={target} net.ifnames=0 tizen.console_size={rows}x{cols} -- -c \"{setup}; echo tizen-ornn: shared folder setup failed >&2; printf ! >/dev/ttyS1; sync; exec /usr/bin/systemctl --force --force poweroff\"")]);
    console::run(&mut cmd, &stop, &image, false)?;
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
        Some(Action::Delete {
            image: Some(name),
            yes: true,
        }) => delete_disk(&name),
        Some(Action::Delete { .. }) => Err(io::Error::other(
            "Specify delete IMAGE --yes to delete its disk.",
        )),
        Some(Action::Images) => {
            println!("{}", serde_json::to_string(&images()?)?);
            Ok(())
        }
        Some(Action::Build { name, repository }) => {
            let proxy = tizen_ornn::proxy::BuildProxy::load()?;
            if let Some(name) = &name {
                crate::build_source::validate_name(name)?;
            }
            let source = repository
                .as_deref()
                .map(|input| crate::build_source::BuildSource::resolve(input, &proxy))
                .transpose()?;
            let name = name
                .or_else(|| source.as_ref().map(|s| s.release.clone()))
                .unwrap_or_else(|| "default".into());
            build::create(&home()?, &name, source.as_ref(), &proxy)
        }
        Some(Action::Update {
            image_dir,
            snapshot,
            ..
        }) => {
            if cli.arch.as_deref().is_some_and(|arch| arch != "x86_64") {
                return Err(io::Error::other("Image updates require x86_64."));
            }
            let image = select_image(image_dir)?;
            disk(&image)?;
            let session = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(image.join(".ornn-session"))?;
            session.try_lock().map_err(io::Error::other)?;
            build::update(&home()?, &image, snapshot.as_deref())
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
            let graphics = cli.gui;
            let read_only = share_ro.is_some();
            if worker {
                boot(image_dir, graphics, share.or(share_ro), read_only)
            } else {
                launch(image_dir, graphics, share.or(share_ro), read_only)
            }
        }
    }
}
