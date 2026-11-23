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

fn digest(file: &Path) -> io::Result<String> {
    let mut hash = Sha256::new();
    io::copy(&mut File::open(file)?, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn verify(file: &Path, expected: &str) -> io::Result<()> {
    if digest(file)? != expected {
        return Err(io::Error::other(format!(
            "SHA-256 mismatch: {}",
            file.display()
        )));
    }
    Ok(())
}

pub(super) fn kernel_for_boot(image: &Path) -> io::Result<PathBuf> {
    let local = image.join("bzImage");
    let bundled = env::current_exe()?
        .with_file_name("builder")
        .join("bzImage");
    let previous = include_str!("../../kernel/r6.sha256")
        .split_whitespace()
        .next()
        .unwrap();
    if bundled.is_file() && digest(&local)? == previous {
        verify(
            &bundled,
            include_str!("../../kernel/r26.sha256")
                .split_whitespace()
                .next()
                .unwrap(),
        )?;
        return Ok(bundled);
    }
    Ok(local)
}

fn builder_file(
    work: &Path,
    remote: &str,
    sha: &str,
    proxy: &tizen_ornn::proxy::BuildProxy,
) -> io::Result<PathBuf> {
    let path = work.join(format!("ubuntu-{sha}"));
    let mut command = Command::new("curl.exe");
    proxy.apply(&mut command);
    checked(
        command
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
            .arg(&path)
            .arg(format!("{CLOUD_RELEASE}/{remote}")),
        "Download Ubuntu builder",
    )?;
    verify(&path, sha)?;
    Ok(path)
}

fn make_seed(
    path: &Path,
    deb: &Path,
    arch: &str,
    source: Option<&crate::build_source::BuildSource>,
    ca: Option<&Path>,
    proxy: &tizen_ornn::proxy::BuildProxy,
) -> io::Result<()> {
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
        root.create_file("guestarch")?.write_all(arch.as_bytes())?;
        if let Some(source) = source {
            root.create_file("image.toml")?
                .write_all(source.config().as_bytes())?;
        }
        for (key, value) in proxy.builder_values()? {
            root.create_file(&key)?.write_all(value.as_bytes())?;
        }
        if let Some(ca) = ca {
            let mut file = File::open(ca).map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!("TIZEN_BUILD_CA_CERT {}: {err}", ca.display()),
                )
            })?;
            if !file.metadata()?.is_file() || file.metadata()?.len() > 1024 * 1024 {
                return Err(io::Error::other(
                    "Expected a public CA certificate file (up to 1 MiB)",
                ));
            }
            root.create_file("build_ca.py")?
                .write_all(include_bytes!("../../scripts/build_ca.py"))?;
            io::copy(&mut file, &mut root.create_file("build-ca.crt")?)?;
        }
        // NoCloud shell user-data executes once on each fresh builder overlay.
        root.create_file("user-data")?
            .write_all(include_bytes!("provision.sh"))?;
        io::copy(&mut File::open(deb)?, &mut root.create_file("ornn.deb")?)?;
    }
    volume.unmount()
}

pub(super) fn update(home: &Path, image: &Path, snapshot: Option<&str>) -> io::Result<()> {
    let config = match fs::read_to_string(image.join("image.toml")) {
        Ok(value) => value,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            include_str!("../../config/image.toml").into()
        }
        Err(e) => return Err(e),
    };
    let previous: crate::build_source::BuildSource =
        toml::from_str(&config).map_err(io::Error::other)?;
    let id = snapshot.unwrap_or("latest");
    if id != "latest" && !crate::build_source::dated(id) {
        return Err(io::Error::other(
            "Expected latest or a dated Unified snapshot.",
        ));
    }
    let proxy = tizen_ornn::proxy::BuildProxy::load()?;
    let mut url = format!(
        "{}/Tizen-10.1-Unified/{id}",
        previous.repository.trim_end_matches('/')
    );
    if let Some(profile) = previous.profile {
        url.push_str(&format!("/images/emulator/{profile}"));
    }
    let source = crate::build_source::BuildSource::resolve(&url, &proxy)?;
    let config = source.config();
    let revision = include_str!("../../config/image-revision").trim();
    let hash = format!("{:x}", Sha256::digest(config.as_bytes()));
    let name = format!("{}-v{revision}-{}", source.release, &hash[..12]);
    let dest = home.join("images").join(&name);
    if dest.exists() {
        disk(&dest)?;
        if fs::read_to_string(dest.join("image.toml"))? != config {
            return Err(io::Error::other("Existing image has a different source."));
        }
        println!("Already up to date: {}", dest.display());
    } else {
        create(home, &name, "x86_64", Some(&source), &proxy)?;
    }
    let mut selected = tempfile::NamedTempFile::new_in(home)?;
    serde_json::to_writer(&mut selected, &name)?;
    selected.persist(home.join("updated-image.json"))?;
    println!("Selected image: {}", dest.display());
    Ok(())
}

pub(super) fn create(
    home: &Path,
    name: &str,
    arch: &str,
    source: Option<&crate::build_source::BuildSource>,
    proxy: &tizen_ornn::proxy::BuildProxy,
) -> io::Result<()> {
    let images = home.join("images");
    fs::create_dir_all(&images)?;
    let dest = images.join(name);
    if dest.exists() {
        if source.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Image already exists: {}", dest.display()),
            ));
        }
        if arch == "x86_64" {
            disk(&dest)?;
        } else if tizen_ornn::arm_arch(&dest) != Some(arch) {
            return Err(io::Error::other(
                "Destination exists with a different image layout.",
            ));
        }
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
        include_str!("../../kernel/r26.sha256")
            .split_whitespace()
            .next()
            .unwrap(),
    )?;
    let work = tempfile::Builder::new()
        .prefix(".build-")
        .tempdir_in(&images)?;
    let ca = tizen_ornn::workdir::ca_setting()?.or_else(|| {
        env::var_os("TIZEN_BUILD_CA_CERT")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    });
    make_seed(
        &work.path().join("seed.img"),
        &deb,
        arch,
        source,
        ca.as_deref(),
        proxy,
    )?;
    let downloads = work.path().join("downloads");
    fs::create_dir(&downloads)?;
    let base = builder_file(
        &downloads,
        "ubuntu-24.04-server-cloudimg-amd64.img",
        CLOUD_SHA,
        proxy,
    )?;
    let linux = builder_file(
        &downloads,
        "unpacked/ubuntu-24.04-server-cloudimg-amd64-vmlinuz-generic",
        "41ba36d860aa0b0cd995f4ca8856d80249b244577e704929b55be13fbd1ebd2f",
        proxy,
    )?;
    let initrd = builder_file(
        &downloads,
        "unpacked/ubuntu-24.04-server-cloudimg-amd64-initrd-generic",
        "bf276ffc12872256080890698b42db0dc9c5a469c896d8b59af6bc4e723cd186",
        proxy,
    )?;
    let output = work.path().join("output");
    fs::create_dir(&output)?;
    File::create(work.path().join("status.raw"))?.set_len(1024 * 1024)?;
    checked(
        qemu("qemu-img")?
            .current_dir(work.path())
            .args(["create", "-f", "qcow2", "-F", "qcow2", "-b"])
            .arg(&base)
            .args(["work.qcow2", "64G"]),
        "Create builder overlay",
    )?;
    if arch == "x86_64" {
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
    }
    println!(
        "Building {name} from the configured Tizen snapshot. Keep this console open until the builder powers off."
    );
    let mut builder = qemu("qemu-system-x86_64")?;
    builder
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
            "file=status.raw,if=none,id=status,format=raw",
            "-device",
            "virtio-blk-pci,drive=status,serial=ornn-status",
            "-drive",
            "file=seed.img,format=raw,if=virtio,readonly=on",
            "-netdev",
            "user,id=net",
            "-device",
            "virtio-net-pci,netdev=net,romfile=",
        ]);
    if arch == "x86_64" {
        builder.args([
            "-drive",
            "file=output/tizen-rootfs.qcow2,if=none,id=out,format=qcow2",
            "-device",
            "virtio-blk-pci,drive=out,serial=ornn-output",
        ]);
    } else {
        builder.args([
            "-fsdev",
            "local,id=armout,path=output,security_model=mapped-xattr",
            "-device",
            "virtio-9p-pci,fsdev=armout,mount_tag=ornn-arm-output",
        ]);
    }
    checked(&mut builder, "Build Tizen image")?;
    let mut marker = [0u8; 23];
    File::open(work.path().join("status.raw"))?.read_exact(&mut marker)?;
    if marker != *b"TIZEN_ORNN_IMAGE_READY\n" {
        return Err(io::Error::other(
            "The builder stopped before finishing the image. See the guest console output.",
        ));
    }
    if arch != "x86_64" {
        if tizen_ornn::arm_arch(&output) != Some(arch) {
            return Err(io::Error::other("Incomplete ARM image output."));
        }
        fs::rename(output, &dest)?;
        work.close()?;
        println!("Image ready: {}", dest.display());
        return Ok(());
    }
    fs::copy(kernel, output.join("bzImage"))?;
    File::create(output.join(".tizen-virtio-console"))?;
    if let Some(source) = source {
        fs::write(output.join("image.toml"), source.config())?;
    }
    fs::rename(output, &dest)?;
    work.close()?;
    println!("Image ready: {}", dest.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_seed_carries_the_selected_repository_and_dated_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let deb = temp.path().join("recipe.deb");
        fs::write(&deb, b"recipe").unwrap();
        let seed = temp.path().join("seed.img");
        let source = crate::build_source::BuildSource {
            profile: None,
            repository: "https://mirror.example/Tizen-10.1/".into(),
            release: "tizen-10.1-unified_20261119.053314".into(),
        };
        let ca = temp.path().join("company root.crt");
        fs::write(&ca, b"public certificate").unwrap();
        make_seed(
            &seed,
            &deb,
            "x86_64",
            Some(&source),
            Some(&ca),
            &Default::default(),
        )
        .unwrap();
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(seed)
            .unwrap();
        let volume = fatfs::FileSystem::new(file, fatfs::FsOptions::new()).unwrap();
        let mut actual = String::new();
        volume
            .root_dir()
            .open_file("image.toml")
            .unwrap()
            .read_to_string(&mut actual)
            .unwrap();
        assert_eq!(actual, source.config());
        actual.clear();
        volume
            .root_dir()
            .open_file("build-ca.crt")
            .unwrap()
            .read_to_string(&mut actual)
            .unwrap();
        assert_eq!(actual, "public certificate");
    }
}
