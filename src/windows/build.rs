use super::{checked, disk, qemu};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

const CLOUD_RELEASE: &str = "https://cloud-images.ubuntu.com/releases/noble/release-20261017";
const CLOUD_SHA: &str = "04e8f2461a16a44e78a370284471f29616cdd9baf1893df25807b71f985b6c94";

fn verify(file: &Path, expected: &str) -> io::Result<()> {
    let mut hash = Sha256::new();
    io::copy(&mut File::open(file)?, &mut hash)?;
    if format!("{:x}", hash.finalize()) != expected {
        return Err(io::Error::other(format!(
            "SHA-256 mismatch: {}",
            file.display()
        )));
    }
    Ok(())
}

fn builder_file(cache: &Path, remote: &str, sha: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(cache)?;
    let path = cache.join(format!("ubuntu-{sha}"));
    if !path.is_file() {
        let temp = tempfile::NamedTempFile::new_in(cache)?.into_temp_path();
        checked(
            Command::new("curl.exe")
                .args([
                    "--fail",
                    "--location",
                    "--retry",
                    "2",
                    "--proto",
                    "=https",
                    "--proto-redir",
                    "=https",
                    "--output",
                ])
                .arg(&temp)
                .arg(format!("{CLOUD_RELEASE}/{remote}")),
            "Download Ubuntu builder",
        )?;
        verify(&temp, sha)?;
        if let Err(e) = temp.persist_noclobber(&path) {
            if e.error.kind() != io::ErrorKind::AlreadyExists {
                return Err(e.error);
            }
        } else {
            return Ok(path);
        }
    }
    verify(&path, sha)?;
    Ok(path)
}

fn make_seed(path: &Path, deb: &Path) -> io::Result<()> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)?;
    file.set_len(fs::metadata(deb)?.len().next_multiple_of(512) + 16 * 1024 * 1024)?;
    fatfs::format_volume(
        &file,
        fatfs::FormatVolumeOptions::new().volume_label(*b"CIDATA     "),
    )?;
    let volume = fatfs::FileSystem::new(file, fatfs::FsOptions::new())?;
    {
        let root = volume.root_dir();
        root.create_file("meta-data")?
            .write_all(b"instance-id: ornn-builder\nlocal-hostname: ornn-builder\n")?;
        // NoCloud shell user-data executes once on each fresh builder overlay.
        root.create_file("user-data")?
            .write_all(include_bytes!("provision.sh"))?;
        io::copy(&mut File::open(deb)?, &mut root.create_file("ornn.deb")?)?;
    }
    volume.unmount()
}

fn valid_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        && !matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.as_bytes()[3].is_ascii_digit())
}

pub(super) fn create(home: &Path, name: &str) -> io::Result<()> {
    if !valid_name(name) {
        return Err(io::Error::other(
            "Use 1–64 letters, digits, hyphens or underscores for the image name.",
        ));
    }
    let images = home.join("images");
    fs::create_dir_all(&images)?;
    let dest = images.join(name);
    if dest.exists() {
        disk(&dest)?;
        println!("Existing image ready: {}", dest.display());
        return Ok(());
    }
    let assets = env::current_exe()?.with_file_name("builder");
    let deb = assets.join("tizen-ornn_amd64.deb");
    let kernel = assets.join("bzImage");
    if !deb.is_file() {
        return Err(io::Error::other(
            "Extract the complete Windows release ZIP, including builder/.",
        ));
    }
    verify(
        &kernel,
        include_str!("../../kernel/r6.sha256")
            .split_whitespace()
            .next()
            .unwrap(),
    )?;
    let cache = home.join("cache");
    let base = builder_file(&cache, "ubuntu-24.04-server-cloudimg-amd64.img", CLOUD_SHA)?
        .canonicalize()?;
    let linux = builder_file(
        &cache,
        "unpacked/ubuntu-24.04-server-cloudimg-amd64-vmlinuz-generic",
        "41ba36d860aa0b0cd995f4ca8856d80249b244577e704929b55be13fbd1ebd2f",
    )?
    .canonicalize()?;
    let initrd = builder_file(
        &cache,
        "unpacked/ubuntu-24.04-server-cloudimg-amd64-initrd-generic",
        "bf276ffc12872256080890698b42db0dc9c5a469c896d8b59af6bc4e723cd186",
    )?
    .canonicalize()?;
    let work = tempfile::Builder::new()
        .prefix(".build-")
        .tempdir_in(&images)?;
    let output = work.path().join("output");
    fs::create_dir(&output)?;
    make_seed(&work.path().join("seed.img"), &deb)?;
    File::create(work.path().join("status.raw"))?.set_len(1024 * 1024)?;
    checked(
        qemu("qemu-img")?
            .current_dir(work.path())
            .args(["create", "-f", "qcow2", "-F", "qcow2", "-b"])
            .arg(&base)
            .args(["work.qcow2", "64G"]),
        "Create builder overlay",
    )?;
    checked(
        qemu("qemu-img")?.current_dir(work.path()).args([
            "create",
            "-f",
            "qcow2",
            "output/tizen-rootfs.qcow2",
            "16G",
        ]),
        "Create Tizen disk",
    )?;
    println!(
        "Building {name} from the configured Tizen snapshot. Keep this console open until the builder powers off."
    );
    checked(
        qemu("qemu-system-x86_64")?
            .current_dir(work.path())
            .arg("-kernel")
            .arg(linux)
            .arg("-initrd")
            .arg(initrd)
            .args([
                "-append",
                "root=LABEL=cloudimg-rootfs ro console=ttyS0",
                "-machine",
                "pc",
                "-accel",
                "tcg,thread=multi",
                "-cpu",
                "max",
                "-smp",
                "4",
                "-m",
                "4096",
                "-nodefaults",
                "-display",
                "none",
                "-serial",
                "stdio",
                "-monitor",
                "none",
                "-no-reboot",
                "-drive",
                "file=work.qcow2,format=qcow2,if=virtio",
                "-drive",
                "file=output/tizen-rootfs.qcow2,if=none,id=out,format=qcow2",
                "-device",
                "virtio-blk-pci,drive=out,serial=ornn-output",
                "-drive",
                "file=status.raw,if=none,id=status,format=raw",
                "-device",
                "virtio-blk-pci,drive=status,serial=ornn-status",
                "-drive",
                "file=seed.img,format=raw,if=virtio,readonly=on",
                "-netdev",
                "user,id=net",
                "-device",
                "virtio-net-pci,netdev=net,romfile=",
            ]),
        "Build Tizen image",
    )?;
    let mut marker = [0u8; 23];
    File::open(work.path().join("status.raw"))?.read_exact(&mut marker)?;
    if marker != *b"TIZEN_ORNN_IMAGE_READY\n" {
        return Err(io::Error::other(
            "The builder stopped before finishing the image. See the guest console output.",
        ));
    }
    fs::copy(kernel, output.join("bzImage"))?;
    File::create(output.join(".tizen-virtio-console"))?;
    fs::rename(output, &dest)?;
    println!("Image ready: {}", dest.display());
    Ok(())
}
