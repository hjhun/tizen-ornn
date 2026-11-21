use super::{Action, Cli};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};
use tizen_ornn::Image;
mod build;
mod console;

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

fn boot(image: &Path, graphics: bool) -> io::Result<()> {
    let (name, format) = disk(image)?;
    let mut cmd = qemu("qemu-system-x86_64")?;
    cmd.current_dir(image).args([
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
    let console = if image.join(".tizen-virtio-console").is_file() {
        cmd.args([
            "-serial",
            "null",
            "-device",
            "virtio-serial-pci,id=guestbus",
            "-device",
            "virtconsole,bus=guestbus.0,nr=0,chardev=console",
        ]);
        "hvc0"
    } else {
        cmd.args(["-serial", "chardev:console"]);
        "ttyS0,115200"
    };
    let target = if graphics {
        cmd.args([
            "-display",
            "sdl,gl=off",
            "-device",
            "virtio-gpu-pci,xres=1024,yres=768",
            "-device",
            "virtio-keyboard-pci",
            "-device",
            "virtio-tablet-pci",
        ]);
        "tizen-dev-gui.target"
    } else {
        cmd.args(["-display", "none"]);
        "tizen-dev.target"
    };
    cmd.args(["-append",&format!("root=/dev/vda rootfstype=ext4 rootwait rw console={console} init=/usr/lib/systemd/systemd systemd.unit={target} net.ifnames=0 tizen.console_size=30x100")]);
    console::run(&mut cmd)
}

pub fn execute(cli: Cli) -> io::Result<()> {
    match cli.command {
        Some(Action::Images) => {
            println!("{}", serde_json::to_string(&images()?)?);
            Ok(())
        }
        Some(Action::Build { name }) => build::create(&home()?, &name),
        None | Some(Action::Run { image_dir: None }) => {
            let found = images()?;
            let image = found
                .iter()
                .find(|i| i.path.ends_with("default"))
                .or_else(|| found.first())
                .ok_or_else(|| {
                    io::Error::other("Run tizen-ornn build to create your first image.")
                })?;
            boot(&image.path, cli.gui)
        }
        Some(Action::Run {
            image_dir: Some(dir),
        }) => boot(&dir, cli.gui),
    }
}
