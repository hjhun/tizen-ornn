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

## Daily use

| From | Command | Result |
|---|---|---|
| Host | `tizen-vm run` or `tizen-vm` | Start the selected VM or reconnect to its console |
| Host | `tizen-vm run --gui` | Start with a window for UI apps on WSLg or a Linux desktop |
| Guest console | Ctrl+C | Interrupt the foreground command, including a running dlog viewer |
| Top-level guest shell | `exit` | Return to the host while keeping the VM, shell and jobs running |
| Guest root shell | `systemctl poweroff` | Shut down the VM |
| Another host terminal | `tizen-vm install ed` | Install a Tizen RPM and its dependencies in the running VM |
| Another host terminal | `tizen-vm install codex` | Install the optional coding client for the guest developer |

Connections share the same serial console. Closing a terminal leaves the VM running while the Linux host stays up. An `exit` inside `su - owner` returns to the parent guest shell. A single Ctrl+D at an empty top-level login prompt leaves it open. Shut down the guest before changing its display mode. [Console and graphical sessions](docs/running.md)

## Develop inside Tizen

Use the ordinary Tizen account for development. The guest supplies Git, Vim, GCC/G++, glibc, libstdc++, GLib and headers, CMake, Ninja, Make, Meson, Autotools, Cargo, GDB and strace.

```sh
su - owner
cd /home/owner/workspace
cargo new hello
cd hello
cargo run --offline
```

In a Tizen CMake project such as `aul-1`, run `tizen-build test` in the source directory. It reads CMake options from the RPM spec, builds incrementally and runs CTest. The image also supports x86_64 and ARMv7 ELF/RPM builds, TPK packaging and SDB connections. Guest root can use `zypper` directly. [Native, cross and app build examples (Korean)](docs/development.md)

For a coding assistant, use `tizen-vm install agy`, `gemini`, `claude` or `codex` on the host. Then run `tizen-ai-shell` inside the VM and launch the installed CLI from your project. The image includes Node.js, npm, ripgrep, bubblewrap and socat. Sign in with your own provider account. [AI tool setup and verification](docs/ai-tools.md)

## Update

| Host command | When to use it |
|---|---|
| `tizen-vm update --packages` | Update installed RPMs in the running guest from the latest configured snapshot |
| `tizen-vm update --packages ed` | Limit the RPM transaction to a named package and its required dependencies |
| `tizen-vm update --image` or `tizen-vm update` | Build a replacement system and migrate the previous disk's persistent files after guest shutdown |
| `tizen-vm rollback` | Switch back to the previous disk after guest shutdown |

An image update merges the previous disk's `/opt/usr`, `/home` and `/root` into the new image, including owner projects and the AI tool home. Existing file contents, ownership, links and SMACK attributes are carried across. The new image supplies the system RPMs, package database and directories such as `/etc` and `/usr`. Reinstall any additional system RPMs you need. The previous disk remains available with its original system and data. Edits made after switching belong to the disk on which they were made.

Allow space for both images and build downloads. Migrating a QCOW2 disk or recovering an unclean ext4 filesystem also needs space for a temporary sparse raw copy. To select a dated release for either update mode, append `--snapshot tizen-10.1-unified_20261119.053314`. To update the **host CLI**, repeat the first two installation commands. A VM built with an older CLI needs one `update --image` to gain live package management.

## Choose the snapshot source

`tizen-vm config` shows the effective source and initial snapshot. The default source is **download.tizen.org**. Choose a different initial snapshot or a compatible HTTPS mirror before building:

```sh
tizen-vm config --snapshot tizen-10.1-unified_20261119.053314
tizen-vm config --repository https://mirror.example.org/snapshots/TIZEN/Tizen-10.1/
```

The repository must provide the same `Tizen-10.1-Unified/` and `Tizen-10.1-Base/` layout as the official server. Per-user settings live in `~/.config/tizen-vm/image.toml`. The system default is `/etc/tizen-vm/image.toml`. Set `TIZEN_IMAGE_CONFIG=/absolute/path/to/image.toml` for a one-off configuration. Tizen Studio SDK packages and the prebuilt project kernel have separately pinned sources. [Snapshot builder (Korean)](docs/snapshot-build.md) · [RPM repository details](docs/snapshot-packages.md) · [Guest verification results (Korean)](docs/verification.md)
