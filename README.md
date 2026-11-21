# Tizen Ornn

Tizen Ornn is a Rust CLI and optional desktop launcher for writable Tizen development VMs. Windows runs native QEMU. Both hosts can share a folder with x86_64 guests. Ubuntu also supports ARMv7 guests, image updates and package management.

## Downloads

| Host | Release files | First step |
| --- | --- | --- |
| Windows x64 desktop | [Portable EXE](https://github.com/hjhun/tizen-ornn/releases/latest/download/tizen-ornn-windows-x64-portable.exe) | Double-click in a writable folder. QEMU is included. |
| Ubuntu 24.04 amd64 desktop | [Portable GUI](https://github.com/hjhun/tizen-ornn/releases/latest/download/tizen-ornn-ubuntu-amd64-portable.run) | In a writable folder, run `chmod +x tizen-ornn-ubuntu-amd64-portable.run`, then `./tizen-ornn-ubuntu-amd64-portable.run`. |
| Ubuntu 24.04 amd64, terminal | [CLI package](https://github.com/hjhun/tizen-ornn/releases/latest/download/tizen-ornn_amd64.deb) | Install the package, then run `tizen-ornn build`. |
| Ubuntu 24.04 amd64, desktop | [CLI package](https://github.com/hjhun/tizen-ornn/releases/latest/download/tizen-ornn_amd64.deb) and [GUI package](https://github.com/hjhun/tizen-ornn/releases/latest/download/tizen-ornn-gui_amd64.deb) | Install both packages from the same release and open `tizen-ornn-gui`. |

Each [release](https://github.com/hjhun/tizen-ornn/releases) includes a `SHA256SUMS` file. Compare the checksum of your download using `sha256sum <filename>` on Ubuntu or `Get-FileHash <filename> -Algorithm SHA256` in PowerShell.

## Install and start

### Ubuntu 24.04 x86_64

Install the CLI for terminal use:

```sh
curl -fLO https://github.com/hjhun/tizen-ornn/releases/latest/download/tizen-ornn_amd64.deb
sudo apt install --no-install-recommends ./tizen-ornn_amd64.deb
tizen-ornn build
tizen-ornn run --headless
```

For the **desktop launcher**, install the GUI package alongside the matching CLI:

```sh
curl -fLO https://github.com/hjhun/tizen-ornn/releases/latest/download/tizen-ornn-gui_amd64.deb
sudo apt install ./tizen-ornn_amd64.deb ./tizen-ornn-gui_amd64.deb
tizen-ornn-gui
```

Use **Open image folder…** to connect an existing image directory. The launcher selects it immediately and remembers it in the current work directory’s image library. The disk stays in its original folder.

Select an image and click **Start with display** to open its Tizen graphical session and console. The selected image shows its current state. **Shut down** asks that guest to power off cleanly. Choose **Start terminal only** for an x86_64 console-only boot. **Build image…** opens a terminal for build progress and Ubuntu sudo prompts.

Run the CLI as your normal user. The first build downloads the dated snapshot and SDK components, verifies their hashes and creates a 16 GiB sparse disk. Set `TIZEN_ORNN_HOME` to select another image store on a Linux filesystem. [SDK installation and terms](https://developer.samsung.com/tizen/get-started/installation.html)

### Portable GUI on Windows and Ubuntu

Open **Settings**, enter the work directory and click **Save work directory**. Images, downloads and intermediate build files use that directory. Saving a blank value restores the default, `%LOCALAPPDATA%\Tizen Ornn` on Windows or `~/.local/tizen-ornn` on Ubuntu. Changing the setting switches the image list and future builds. Existing files stay in their original directory.

Set **Shared folder (x86_64)** to an existing absolute host path, or leave it blank to use the work directory’s `share` folder. The CLI uses the same setting through `tizen-ornn share-dir PATH` and `tizen-ornn share-dir --reset`. Inside the guest, open `/home/tizen/share`. [First-boot preservation and one-time overrides](docs/running.md#share-a-host-directory)

Both launchers unpack their runtime and keep the setting in `Tizen Ornn Portable` beside the downloaded file. Relative work directories resolve inside that folder. Move it along with the launcher to preserve settings. Ubuntu guest operations use the host terminal and the project's Ubuntu system dependencies.

### Windows x64 (native)

1. Download [tizen-ornn-windows-x64.exe](https://github.com/hjhun/tizen-ornn/releases/latest/download/tizen-ornn-windows-x64.exe) and double-click it. The launcher opens the GUI and keeps its CLI and image recipe under `%LOCALAPPDATA%\Tizen Ornn\tools` for subsequent runs. This step requires only the one file and runs as the current user.
2. To create or start a VM, install [QCW UCRT64 QEMU for Windows](https://github.com/arixmkii/qcw/releases/tag/v0.1.28) (the UCRT64 build includes both virtio-9p and user networking) in its default directory or set `TIZEN_ORNN_QEMU_DIR` to the directory containing `qemu-system-x86_64.exe` and `qemu-img.exe`.
3. Choose **Build image…**, enter an optional snapshot URL and image name, and click **Build**. After the build finishes, select the image and click **Start with display**. Click **Shut down** to power off that guest.

For a portable CLI directory, extract the complete [Windows ZIP](https://github.com/hjhun/tizen-ornn/releases/latest/download/tizen-ornn-windows-x64.zip) and open PowerShell in the extracted folder.

The same operations work from PowerShell:

```powershell
.\tizen-ornn.exe build --name default
.\tizen-ornn.exe images
.\tizen-ornn.exe run --headless
.\tizen-ornn.exe run --image-dir 'D:\Tizen Images\my-image'
```

The Windows builder fetches a SHA-256-pinned Ubuntu cloud image and runs the project's snapshot and RPM build recipe inside a temporary QEMU VM. Windows requires neither a WSL distribution nor a pre-existing Tizen disk. The build requires network access, 4 GiB of guest RAM and disk space for a sparse 64 GiB scratch disk and a 16 GiB output disk. Allow extra time for QEMU software emulation. The completed guest disk is independent of the builder. Images live under `%LOCALAPPDATA%\Tizen Ornn\images`; set `TIZEN_ORNN_HOME` to place images and the download cache on another drive.

A Windows guest terminal stays attached to its QEMU process. Use **Shut down** or `tizen-ornn shutdown --image-dir PATH` before closing it. The native console follows the visible Windows terminal size at startup and on resize. The native Windows backend supports building and running x86_64 images; the Ubuntu-specific management commands below apply to the Ubuntu CLI.

### Guest development account

The x86_64 development console logs in as `tizen`. New images give this account
`root` (GID 0) as a supplementary group. Run `id` in the guest terminal to
check the active login. For operations requiring UID 0, use `su -`. To apply
the new account setup to an existing managed Ubuntu image, shut down its guest
and run `tizen-ornn update --image`.

### Build the host tools with Cargo

Use a current stable Rust toolchain. Run the following in the source checkout on Ubuntu or Windows:

```sh
cargo build --locked --release --bin tizen-ornn
cargo build --locked --release --features desktop --bin tizen-ornn-gui
```

Cargo writes the executables to `target/release`. For Windows image creation the release ZIP supplies the matching Ubuntu recipe package and boot kernel in `builder/`. On Ubuntu, `scripts/package-cli.sh dist --desktop` builds both Debian packages. The installed Ubuntu CLI uses the project's Python and shell helpers for snapshot, disk and guest operations.

### Upgrade from tizen-vm

Shut down any running guest with the old CLI before installing the new package.
APT replaces the old host package. Tizen Ornn reuses the existing image store
in place when the new store does not exist. An existing `TIZEN_VM_HOME`
override still works; `TIZEN_ORNN_HOME` takes precedence. Existing per-user
snapshot settings are read until you save a new configuration. If you customized
`/etc/tizen-vm/image.toml`, copy those settings to `/etc/tizen-ornn/image.toml`.
The guest's native RPM builder is still called `ornn`.

### Find the VM disk

For a standard package installation, `current` points to the selected image. Run these commands in Ubuntu to see its directory and disk file. Fresh builds use `tizen-rootfs.ext4`. A compressed image uses `tizen-rootfs.qcow2`.

```sh
data=${XDG_DATA_HOME:-$HOME/.local/share}
store=${TIZEN_ORNN_HOME:-${TIZEN_VM_HOME:-}}
if [ -z "$store" ]; then
    store=$data/tizen-ornn
    if [ ! -e "$store" ] && [ -d "$data/tizen-vm" ]; then store=$data/tizen-vm; fi
fi
realpath "$store/current"
ls -lh "$store/current"/tizen-rootfs.*
```

In a terminal, `tizen-ornn delete` lets you pick a managed disk and asks you to type its name before deletion. Enter picks the first menu entry and q cancels. With redirected input or output, this command lists the disks. For automation, use `tizen-ornn delete IMAGE_DIRECTORY_NAME --yes`. The ARMv7 headed disk entry covers its four guest partitions. This permanently removes the guest disk and its contents. The host cache, image metadata, and other disks are retained. When deleting the selected image, the recorded previous image becomes active if available.

For Windows File Explorer, resolve the Linux symlinks in WSL2 first and paste the printed path into the address bar.

```sh
wslpath -w "$(realpath "$store/current")"
```

A source checkout normally keeps images under `out/`. When the installed CLI shares that store, `realpath` follows the link to the same disk.

If HTTPS fails while downloading the image or installing guest tools, check the CA store of the system making the connection. [HTTPS and private CA setup](docs/https-certificates.md)

## Daily use on Ubuntu

| From | Command | Result |
|---|---|---|
| Host | `tizen-ornn` | In a terminal, choose an installed image by number when several are ready. Enter accepts the first image and q cancels. A single image opens directly. |
| Host | `tizen-ornn run` | Start the currently selected x86_64 VM or reconnect to its console |
| Host | `tizen-ornn run --share "$PWD"` | Open the host project at `/home/tizen/share` inside Tizen |
| Host | `tizen-ornn run --host-browser` | Let guest tools such as `agy` open HTTPS links in the host browser |
| Host | `tizen-ornn run --share-ro "$PWD"` | Open the same directory read-only |
| Host | `tizen-ornn --headless` | Start with a console only |
| Guest console | Ctrl+C | Interrupt the foreground command, including a running dlog viewer |
| Top-level guest shell | `exit` | Return to the host while keeping the VM, shell and jobs running |
| Host | `tizen-ornn shutdown` | Shut down the selected VM and wait for its disk to be released |
| Guest root shell | `systemctl poweroff` | Shut down the VM |
| Another host terminal | `tizen-ornn install ed` | Install a Tizen RPM and its dependencies in the running VM |
| Another host terminal | `tizen-ornn install codex` | Install the optional coding client for the guest developer |

Connections share the same serial console. Closing a terminal leaves the VM running while the Linux host stays up. An `exit` inside `su - owner` returns to the parent guest shell. A single Ctrl+D at an empty top-level login prompt leaves it open. Shut down the guest before changing its display mode. [Console and graphical sessions](docs/running.md)

### Use the host network and proxy

The guest uses the Ubuntu host's outbound network and DNS through QEMU NAT. At boot, the CLI imports the host's DNS search domains and `http_proxy`, `https_proxy`, `all_proxy` and `no_proxy` settings. In WSL2 it also checks Windows manual proxy settings when Ubuntu has no proxy endpoint configured. Set a proxy for one boot with `https_proxy=http://proxy.example.org:3128 tizen-ornn run`, or start with `tizen-ornn run --no-host-network` to skip import. Power off the VM before changing boot options. [Shared directories, DNS and proxy details](docs/running.md#share-a-host-directory)

## Develop inside Tizen

The console opens as `tizen` with SMACK label `System::Privileged`. Type `su -` for a passwordless root shell. Run `~/dev-setup/install.sh` to set up the CLI tools and editor. Add `--with-ssh-key` to generate a Gerrit key interactively. Set your Gerrit username in `~/.ssh/config` and your Git author name and email before cloning with `gerrit-clone PROJECT`. The guest supplies Git, Vim, GCC/G++, glibc, libstdc++, GLib and headers, CMake, Ninja, Make, Meson, Autotools, Cargo, GDB and strace.

```sh
cd ~/workspace
cargo new hello
cd hello
cargo run --offline
```

In a Tizen CMake project such as `aul-1`, run `tizen-build test` in the source directory. It reads CMake options from the RPM spec, builds incrementally and runs CTest. The image also supports x86_64 and ARMv7 ELF/RPM builds, TPK packaging and SDB connections. Guest root can use `zypper` directly. [Native, cross and app build examples (Korean)](docs/development.md)
For a standalone ARMv7 RPM builder on Ubuntu or Windows with WSL2, see [Build with ornn in an ARMv7 container (Korean)](docs/armv7l-container.md).

For a coding assistant, use `tizen-ornn install agy`, `gemini`, `claude` or `codex` on the host. Then run `tizen-ai-shell` inside the VM and launch the installed CLI from your project. The image includes Node.js, npm, ripgrep, bubblewrap and socat. Sign in with your own provider account. [AI tool setup and verification](docs/ai-tools.md)

## Update

| Host command | When to use it |
|---|---|
| `tizen-ornn update --packages` | Update installed RPMs in the running guest from the latest configured snapshot |
| `tizen-ornn update --packages ed` | Limit the RPM transaction to a named package and its required dependencies |
| `tizen-ornn update --image` or `tizen-ornn update` | Build a replacement system and migrate the previous disk's persistent files after guest shutdown |
| `tizen-ornn rollback` | Switch back to the previous disk after guest shutdown |

In a terminal, both update modes offer a numbered choice when multiple updatable x86_64 images are installed. Image updates use the chosen stopped disk as the migration source. Package updates require the chosen guest to be running. Noninteractive updates use the active image. ARMv7 update support is pending.

An image update merges the previous disk's `/opt/usr`, `/home` and `/root` into the new image, including owner projects and the AI tool home. Existing file contents, ownership, links and SMACK attributes are carried across. The new image supplies the system RPMs, package database and directories such as `/etc` and `/usr`. Reinstall any additional system RPMs you need. The previous disk remains available with its original system and data. Edits made after switching belong to the disk on which they were made.

Allow space for both images and build downloads. Migrating a QCOW2 disk or recovering an unclean ext4 filesystem also needs space for a temporary sparse raw copy. To select a dated release for either update mode, append `--snapshot tizen-10.1-unified_20261119.053314`. To update the **host CLI**, repeat the first two installation commands. Existing R5 guests gain host sharing and proxy import after the host package upgrade and guest shutdown. Guests built before R5 need one `update --image` to gain live package management.

Fresh x86_64 images include the terminfo-aware `clear` command. For an existing running Ubuntu-managed guest, use `tizen-ornn install ncurses-utils`. Inside a guest with the pinned repositories, root can also run `zypper install ncurses-utils`.

## Choose the snapshot source

On Windows or Ubuntu, open **Build image…** and paste the HTTPS snapshot repository, Unified snapshot directory, or its `build.xml` into **Snapshot repository URL**. A repository root resolves the latest dated snapshot. An empty **Image name** uses the ID from the repository metadata with dots replaced by hyphens. Enter your own name to keep multiple builds of one release. A URL build refuses an existing destination name.

```sh
tizen-ornn build --repository https://download.tizen.org/snapshots/TIZEN/Tizen-10.1/
```

`tizen-ornn config` shows the effective source and initial snapshot. The default source is **download.tizen.org**. Choose a different initial snapshot or a compatible HTTPS mirror before building:

```sh
tizen-ornn config --snapshot tizen-10.1-unified_20261119.053314
tizen-ornn config --repository https://mirror.example.org/snapshots/TIZEN/Tizen-10.1/
```

The repository must provide the same `Tizen-10.1-Unified/` and `Tizen-10.1-Base/` layout as the official server. Per-user settings live in `~/.config/tizen-ornn/image.toml`. The system default is `/etc/tizen-ornn/image.toml`. Set `TIZEN_IMAGE_CONFIG=/absolute/path/to/image.toml` for a one-off configuration. Tizen Studio SDK packages and the prebuilt project kernel have separately pinned sources. [Snapshot builder (Korean)](docs/snapshot-build.md) · [RPM repository details](docs/snapshot-packages.md) · [Guest verification results (Korean)](docs/verification.md)

## License

Tizen Ornn is distributed under the [Apache License, Version 2.0](LICENSE).
Components with their own license notices are distributed under those terms.
These include the Linux kernel, QEMU, Rust dependencies and Tizen packages.
See the [kernel copyright and source information](kernel/copyright) for the bundled kernel.
