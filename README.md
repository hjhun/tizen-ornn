# tizen-vm

A writable Tizen 10.1 development VM for x86_64 Ubuntu and Ubuntu on WSL2. QEMU boots Tizen's systemd services with SMACK and Linux 6.12.41 from the Tizen RPi4 kernel tree. Use a terminal for platform development or open a window for EFL applications.

## Install and start

Run these commands in an **Ubuntu 24.04 x86_64** terminal. On Windows, open Ubuntu in WSL2.

```sh
curl -fLO https://github.com/hjhun/tizen-vm/releases/latest/download/tizen-vm_amd64.deb
sudo apt install ./tizen-vm_amd64.deb
tizen-vm build
tizen-vm run
```

APT installs the host CLI and its dependencies. Run `build` and `run` as your normal user. The first build requests sudo to mount the image, downloads the dated Tizen snapshot and SDK components, verifies their hashes and creates a 16 GiB sparse development disk. The VM uses host space as its disk fills, up to the guest filesystem's capacity. Keep the default image store, `~/.local/share/tizen-vm`, on a Linux filesystem. Set `TIZEN_VM_HOME` to choose another location. [Host package](https://github.com/hjhun/tizen-vm/releases/latest/download/tizen-vm_amd64.deb) · [Tizen SDK installation and terms](https://developer.samsung.com/tizen/get-started/installation.html)

### Find the VM disk

For a standard package installation, `current` points to the selected image. Run these commands in Ubuntu to see its directory and disk file. Fresh builds use `tizen-rootfs.ext4`. A compressed image uses `tizen-rootfs.qcow2`.

```sh
store=${TIZEN_VM_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/tizen-vm}
realpath "$store/current"
ls -lh "$store/current"/tizen-rootfs.*
```

In a terminal, `tizen-vm delete` lets you pick a managed disk and asks you to type its name before deletion. Enter picks the first menu entry and q cancels. With redirected input or output, this command lists the disks. For automation, use `tizen-vm delete IMAGE_DIRECTORY_NAME --yes`. The ARMv7 headed disk entry covers its four guest partitions. This permanently removes the guest disk and its contents. The host cache, image metadata, and other disks are retained. When deleting the selected image, the recorded previous image becomes active if available.

For Windows File Explorer, resolve the Linux symlinks in WSL2 first and paste the printed path into the address bar.

```sh
wslpath -w "$(realpath "$store/current")"
```

A source checkout normally keeps images under `out/`. When the installed CLI shares that store, `realpath` follows the link to the same disk.

If HTTPS fails while downloading the image or installing guest tools, check the CA store of the system making the connection. [HTTPS and private CA setup](docs/https-certificates.md)

## Daily use

| From | Command | Result |
|---|---|---|
| Host | `tizen-vm` | In a terminal, choose an installed image by number when several are ready. Enter accepts the first image and q cancels. A single image opens directly. |
| Host | `tizen-vm run` | Start the currently selected x86_64 VM or reconnect to its console |
| Host | `tizen-vm run --share "$PWD"` | Open the host project at `/home/owner/host` inside Tizen |
| Host | `tizen-vm run --host-browser` | Let guest tools such as `agy` open HTTPS links in the host browser |
| Host | `tizen-vm run --share-ro "$PWD"` | Open the same directory read-only |
| Host | `tizen-vm --headless` | Start with a console only |
| Guest console | Ctrl+C | Interrupt the foreground command, including a running dlog viewer |
| Top-level guest shell | `exit` | Return to the host while keeping the VM, shell and jobs running |
| Host | `tizen-vm shutdown` | Shut down the selected VM and wait for its disk to be released |
| Guest root shell | `systemctl poweroff` | Shut down the VM |
| Another host terminal | `tizen-vm install ed` | Install a Tizen RPM and its dependencies in the running VM |
| Another host terminal | `tizen-vm install codex` | Install the optional coding client for the guest developer |

Connections share the same serial console. Closing a terminal leaves the VM running while the Linux host stays up. An `exit` inside `su - owner` returns to the parent guest shell. A single Ctrl+D at an empty top-level login prompt leaves it open. Shut down the guest before changing its display mode. [Console and graphical sessions](docs/running.md)

### Use the host network and proxy

The guest uses the Ubuntu host's outbound network and DNS through QEMU NAT. At boot, the CLI imports the host's DNS search domains and `http_proxy`, `https_proxy`, `all_proxy` and `no_proxy` settings. In WSL2 it also checks Windows manual proxy settings when Ubuntu has no proxy endpoint configured. Set a proxy for one boot with `https_proxy=http://proxy.example.org:3128 tizen-vm run`, or start with `tizen-vm run --no-host-network` to skip import. Power off the VM before changing boot options. [Shared directories, DNS and proxy details](docs/running.md#share-a-host-directory)

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

For a coding assistant, use `tizen-vm install agy`, `gemini`, `claude` or `codex` on the host. Then run `tizen-ai-shell` inside the VM and launch the installed CLI from your project. The image includes Node.js, npm, ripgrep, bubblewrap and socat. Sign in with your own provider account. [AI tool setup and verification](docs/ai-tools.md)

## Update

| Host command | When to use it |
|---|---|
| `tizen-vm update --packages` | Update installed RPMs in the running guest from the latest configured snapshot |
| `tizen-vm update --packages ed` | Limit the RPM transaction to a named package and its required dependencies |
| `tizen-vm update --image` or `tizen-vm update` | Build a replacement system and migrate the previous disk's persistent files after guest shutdown |
| `tizen-vm rollback` | Switch back to the previous disk after guest shutdown |

In a terminal, both update modes offer a numbered choice when multiple updatable x86_64 images are installed. Image updates use the chosen stopped disk as the migration source. Package updates require the chosen guest to be running. Noninteractive updates use the active image. ARMv7 update support is pending.

An image update merges the previous disk's `/opt/usr`, `/home` and `/root` into the new image, including owner projects and the AI tool home. Existing file contents, ownership, links and SMACK attributes are carried across. The new image supplies the system RPMs, package database and directories such as `/etc` and `/usr`. Reinstall any additional system RPMs you need. The previous disk remains available with its original system and data. Edits made after switching belong to the disk on which they were made.

Allow space for both images and build downloads. Migrating a QCOW2 disk or recovering an unclean ext4 filesystem also needs space for a temporary sparse raw copy. To select a dated release for either update mode, append `--snapshot tizen-10.1-unified_20261119.053314`. To update the **host CLI**, repeat the first two installation commands. Existing R5 guests gain host sharing and proxy import after the host package upgrade and guest shutdown. Guests built before R5 need one `update --image` to gain live package management.

## Choose the snapshot source

`tizen-vm config` shows the effective source and initial snapshot. The default source is **download.tizen.org**. Choose a different initial snapshot or a compatible HTTPS mirror before building:

```sh
tizen-vm config --snapshot tizen-10.1-unified_20261119.053314
tizen-vm config --repository https://mirror.example.org/snapshots/TIZEN/Tizen-10.1/
```

The repository must provide the same `Tizen-10.1-Unified/` and `Tizen-10.1-Base/` layout as the official server. Per-user settings live in `~/.config/tizen-vm/image.toml`. The system default is `/etc/tizen-vm/image.toml`. Set `TIZEN_IMAGE_CONFIG=/absolute/path/to/image.toml` for a one-off configuration. Tizen Studio SDK packages and the prebuilt project kernel have separately pinned sources. [Snapshot builder (Korean)](docs/snapshot-build.md) · [RPM repository details](docs/snapshot-packages.md) · [Guest verification results (Korean)](docs/verification.md)
