<img src="packaging/art/tizen-ornn.png" width="64" height="64" alt="Tizen Ornn logo">

# Tizen Ornn

Tizen Ornn is a Rust CLI and optional desktop launcher for writable Tizen development VMs. Windows runs native QEMU. Both hosts can share a folder with x86_64 guests. The supported guest architecture is x86_64. Inside the guest, the Ornn CLI can cross-compile ARMv7 and AArch64 RPMs. Both hosts support x86_64 image updates. Ubuntu also supports guest package management.

Install Ornn, build a Tizen image, and run your first app inside the guest. [한국어 가이드](docs/user-guide.md) · [English guide](docs/user-guide_en.md)

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

Add the signed Ornn APT repository once for installation and subsequent updates. For CLI-only use, omit `tizen-ornn-gui`.

```sh
sudo curl -fsSL https://raw.githubusercontent.com/hjhun/tizen-ornn/main/packaging/apt/tizen-ornn.sources -o /etc/apt/sources.list.d/tizen-ornn.sources
sudo apt update
sudo apt install tizen-ornn tizen-ornn-gui
```

After registration, run `sudo apt update && sudo apt upgrade` to receive new Ornn releases with your other Ubuntu packages. This also updates Ornn packages previously installed from a downloaded `.deb`.

To install the CLI directly from a release file:

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

Use **Add image…** to connect an existing image directory. The launcher selects it immediately and remembers it in the current work directory’s image library. Use **Disconnect image…** to remove an external folder from the library while keeping its files. For an image built in the managed store, stop the guest and choose **Delete image…** to review its path and delete its guest disk.

Select an image and click **Start with display** to open its Tizen graphical session and console. Close the terminal window at any time. Click **Open terminal** on the running image to reconnect to the same shell and its running jobs. From the CLI, use `tizen-ornn console --image-dir PATH`. **Shut down** asks that guest to power off cleanly. Choose **Start terminal only** for an x86_64 console-only boot. **Build image…** opens a terminal for build progress and Ubuntu sudo prompts.

Run the CLI as your normal user. The first build downloads the dated snapshot and SDK components, verifies their hashes and creates a 16 GiB sparse disk. Set `TIZEN_ORNN_HOME` to select another image store on a Linux filesystem. [SDK installation and terms](https://developer.samsung.com/tizen/get-started/installation.html)

### Portable GUI on Windows and Ubuntu

Open **Settings**, enter the work directory and click **Save work directory**. Images, downloads and intermediate build files use that directory. Saving a blank value restores the default, `%LOCALAPPDATA%\Tizen Ornn` on Windows or `~/.local/tizen-ornn` on Ubuntu. Changing the setting switches the image list and future builds. Existing files stay in their original directory.

While a guest is running, the GUI adds a notification icon for that image. You can close the launcher and use the icon menu to open a terminal, open settings, or shut down that guest. The icon goes away when QEMU exits. On Windows, check the taskbar’s hidden icons. On Ubuntu, use a desktop panel with AppIndicator support. The GUI Debian package installs the indicator bindings. For the Ubuntu portable GUI, install `python3-gi gir1.2-gtk-3.0 gir1.2-ayatanaappindicator3-0.1` on the host.

Set **Shared folder (x86_64)** to an existing absolute host path, or leave it blank to use the work directory’s `share` folder. The CLI uses the same setting through `tizen-ornn share-dir PATH` and `tizen-ornn share-dir --reset`. Inside the guest, open `/home/tizen/share` or `/mnt/host` to access the same files. [First-boot preservation and one-time overrides](docs/running.md#share-a-host-directory)

On Ubuntu, run `./tizen-ornn-ubuntu-amd64-portable.run --install-shortcut` to add the portable launcher and its icon to the application menu.

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

The Windows builder fetches a SHA-256-pinned Ubuntu cloud image and runs the project's snapshot and RPM build recipe inside a temporary QEMU VM. Windows requires neither a WSL distribution nor a pre-existing Tizen disk. The build requires network access, 4 GiB of guest RAM and disk space for a sparse 64 GiB scratch disk and a 16 GiB output disk. Allow extra time for QEMU software emulation. The completed guest disk is independent of the builder. Images live under `%LOCALAPPDATA%\Tizen Ornn\images`; set `TIZEN_ORNN_HOME` to place the images and temporary build workspace on another drive. Each new build removes its builder downloads, VM overlay and staging files when it finishes. Ubuntu builds also remove temporary kernel objects, downloaded archives and RPM staging files. A local source archive supplied by the user is preserved. An interrupted Ubuntu x86 build retains the disk and extraction state needed to resume.

Closing a Windows terminal disconnects that viewer. Use **Shut down** or `tizen-ornn shutdown --image-dir PATH` to power off the guest. The native console follows the visible Windows terminal size at startup and on resize. The native Windows backend supports building and running x86_64 images; the Ubuntu-specific management commands below apply to the Ubuntu CLI.

### Guest development account

The x86_64 development console logs in as `tizen`. New images give this account
`root` (GID 0) as a supplementary group. Run `id` in the guest terminal to
check the active login. For operations requiring UID 0, use `su -`. To apply
the new account setup to an existing managed Ubuntu image, shut down its guest
and run `tizen-ornn update --image`.

The host tools follow the [Ornn version and release policy](docs/versioning.md).

### Build the host tools with Cargo

Use a current stable Rust toolchain. Run the following in the source checkout on Ubuntu or Windows:

```sh
cargo build --locked --release --bin tizen-ornn
cargo build --locked --release --features desktop --bin tizen-ornn-gui
```

Cargo writes the executables to `target/release`. For Windows image creation the release ZIP supplies the matching Ubuntu recipe package and boot kernel in `builder/`. On Ubuntu, `scripts/package-cli.sh dist --desktop` builds both Debian packages. The installed Ubuntu CLI uses the project's Python and shell helpers for snapshot, disk and guest operations.

### Upgrade from tizen-vm

Shut down any running guest with the old CLI before installing the new package.
APT replaces the old host package. To keep using disks under
`~/.local/share/tizen-vm`, choose that directory in **Settings**. For a CLI
override, change `TIZEN_VM_HOME` to `TIZEN_ORNN_HOME` and keep its value.
Copy any customized `image.toml` from the old `tizen-vm` configuration
directory to `~/.config/tizen-ornn/` (per user) or `/etc/tizen-ornn/`
(system-wide). For an existing disk with a `.tizen-vm-image-revision` file,
copy that file to `.tizen-ornn-image-revision` alongside the disk to retain
its build revision.
The guest's RPM builder is still called `ornn`.

### Find the VM disk

On Linux, the GUI and CLI use the same work directory. The default is
`~/.local/tizen-ornn`. Managed x86_64 disks are under `images/<name>/`. A work directory
saved in **Settings** takes effect for both tools. Portable installations keep
their settings beside the launcher. The CLI also honors explicit
`TIZEN_ORNN_HOME` and legacy `TIZEN_OUT` overrides.

Run `tizen-ornn images` to see the actual image paths. With no saved work
directory and no images in the default store, both tools can reuse an older
XDG image store. To use a previous checkout's `out/` directory, select it in
**Settings** or set `TIZEN_OUT` for the CLI.

In an Ubuntu or WSL terminal, run `tizen-ornn delete` to pick a managed disk. At the selection menu, Enter picks the first entry and `q` cancels. At the deletion prompt, type `y`, `yes`, or the exact image name to delete it. Press Enter at this prompt to cancel. With redirected input or output, this command lists the disks. For automation, use `tizen-ornn delete IMAGE_DIRECTORY_NAME --yes`. The CLI can also delete legacy ARM disks on explicit confirmation. This permanently removes the guest disk and its contents. The host cache, image metadata, and other disks are retained. When deleting the selected image, the recorded previous image becomes active if available.

In WSL, `wslpath -w /path/from/the/image/list` prints the Windows Explorer path.

If HTTPS fails while downloading the image or installing guest tools, check the CA store of the system making the connection. [HTTPS and private CA setup](docs/https-certificates.md)

## Daily use on Ubuntu

| From | Command | Result |
|---|---|---|
| Host | `tizen-ornn` | In a terminal, choose an installed image by number when several are ready. Enter accepts the first image and q cancels. A single image opens directly. |
| Host | `tizen-ornn run` | Start the currently selected x86_64 VM or reconnect to its console |
| Host | `tizen-ornn run --share "$PWD"` | Open the host project at `/mnt/host` inside Tizen |
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

The console opens as `tizen` with SMACK label `System::Privileged`. Type `su -` for a passwordless root shell. Run `~/dev-setup/install.sh` to install Gemini CLI, Claude Code, Codex, Herdr and the editor setup. Add `--with-ssh-key` to generate a Gerrit key interactively. Set your Gerrit username in `~/.ssh/config` and your Git author name and email before cloning with `gerrit-clone PROJECT`. The guest supplies Git, Vim, GCC/G++, glibc, libstdc++, GLib and headers, CMake, Ninja, Make, Meson, Autotools, Cargo, GDB and strace.

```sh
cd ~/workspace
cargo new hello
cd hello
cargo run --offline
```

In a Tizen CMake project such as `aul-1`, run `tizen-build test` in the source directory. It reads CMake options from the RPM spec, builds incrementally and runs CTest. The image also supports x86_64, ARMv7 and AArch64 C/C++ ELF/RPM builds, TPK packaging and SDB connections. Guest root can use `zypper` directly. [Native, cross and app build examples (Korean)](docs/development.md)
For a standalone ARMv7 RPM builder on Ubuntu or Windows with WSL2, see [Build with ornn in an ARMv7 container (Korean)](docs/armv7l-container.md).

For a coding assistant, use `tizen-ornn install agy`, `gemini`, `claude` or `codex` on the host. Then run `tizen-ai-shell` inside the VM and launch the installed CLI from your project. The image includes Node.js, npm, ripgrep, bubblewrap and socat. Sign in with your own provider account. [AI tool setup and verification](docs/ai-tools.md)

## Update

| Host command | When to use it |
|---|---|
| `tizen-ornn update --packages` | Update installed RPMs in the running guest from the latest configured snapshot |
| `tizen-ornn update --packages ed` | Limit the RPM transaction to a named package and its required dependencies |
| `tizen-ornn update --image` or `tizen-ornn update` | Build a replacement system and migrate the previous disk's persistent files after guest shutdown |
| `tizen-ornn rollback` | Switch back to the previous disk after guest shutdown |

In a terminal, both update modes offer a numbered choice when multiple updatable x86_64 images are installed. Image updates use the chosen stopped disk as the migration source. Package updates require the chosen guest to be running. Noninteractive updates use the active image. ARM guest updates are unsupported.

An image update merges the previous disk's `/opt/usr`, `/home` and `/root` into the new image, including owner projects and the AI tool home. Existing file contents, ownership, links and SMACK attributes are carried across. The new image supplies the system RPMs, package database and directories such as `/etc` and `/usr`. Reinstall any additional system RPMs you need. The previous disk remains available with its original system and data. Files in the host-backed `/home/tizen/share` remain available across updates and rollbacks.

In the GUI, select a stopped x86_64 image and click **Update image**. The CLI equivalent is `tizen-ornn update --image --image-dir PATH`. After the update, select the new dated image in the library. Windows builds the replacement beside the old disk and keeps using the same shared folder; migrate any disk-only files you need before switching. On Ubuntu the previous disk is also available via `tizen-ornn rollback`.

Allow space for both images and build downloads. Migrating a QCOW2 disk or recovering an unclean ext4 filesystem also needs space for a temporary sparse raw copy. To select a dated release for either update mode, append `--snapshot tizen-10.1-unified_20261119.053314`. To update the **host CLI**, repeat the first two installation commands. Existing R5 guests gain host sharing and proxy import after the host package upgrade and guest shutdown. Guests built before R5 need one `update --image` to gain live package management.

Fresh x86_64 images include the terminfo-aware `clear` command. For an existing running Ubuntu-managed guest, use `tizen-ornn install ncurses-utils`. Inside a guest with the pinned repositories, root can also run `zypper install ncurses-utils`.

## Build RPMs inside the x86_64 guest

Run `ornn build` for an x86_64 RPM, `ornn build --target armv7l` for an ARMv7 RPM, or `ornn build --target aarch64` for an AArch64 RPM, in a checkout containing the spec and its sources. Cross builds use the selected architecture’s C/C++ compiler and Tizen sysroot.

For a tracked spec in a Git checkout, run from the checkout root. If the local `Source0: %{name}-%{version}.tar.gz` is missing, Ornn creates it from the current contents of Git-tracked files. Use `git add` for new source files you want to include. Existing archives and other declared sources are used as supplied.

Ornn keeps every build under `/home/tizen/share/build/<project>/<target>/run.*/`. Open `build.log`, `BUILD/` (including CMakeCache.txt and CTest logs), `BUILDROOT/` or `RPMS/` from the host. A failed attempt keeps its files. If the share cannot run build tools, Ornn builds on the guest disk and copies the tree back. Files a share cannot represent are also preserved in `work.tar`. Run `ornn clean` in the original checkout to remove its attempts and RPMs.

Run `ornn install NAME` in any directory to install a compatible RPM from the last successful build of its project. It also selects matching local subpackages required by NAME. A different RPM with the same version can replace an installed one. When no local candidate exists, Ornn uses Zypper's guest repositories. An already installed package satisfies the request. `ornn uninstall NAME` removes only the installed package and reports any dependencies preventing removal.

To select the remote RPM-MD repositories for build dependencies, install and restore, put their base URLs in `~/.ornn.conf`. Earlier entries have higher priority. Use the URLs shown by `zypper repos --uri` for the image's pinned snapshot, or a signed repository trusted by the guest.

```ini
[repos]
url = https://repository.example/team/
url = https://repository.example/platform/
```

A successful native build installs its RPMs in the guest so dependent projects see the new headers and libraries immediately. Use `ornn build --no-install` to produce RPMs only. From the project root, run `ornn restore` to reinstall its last installed package set from the configured HTTP(S) repositories, or name packages explicitly, for example `ornn restore bundle bundle-devel`. The per-project list in `${XDG_DATA_HOME:-~/.local/share}/ornn/projects/<key>/.installed-rpms` survives `ornn clean`. RPM checks installed build dependencies against the guest package database.

Run `ornn status` from the project root to compare the last build with the installed RPM identities. A different ID indicates a different build. After an interrupted install, retry the same package set or run `ornn restore`.

## Choose the snapshot source

On Windows or Ubuntu, open **Build image…** and paste the HTTPS snapshot repository, Unified snapshot directory, its `build.xml`, an emulator profile directory, or its dated `.ks`, `.packages`, or archive URL into **Snapshot repository URL**. A repository root resolves the latest dated snapshot. An empty **Image name** uses the ID from the repository metadata including its dots (for example, `tizen-10.1-unified_20261119.053314`). Enter your own name to keep multiple builds of one release. A URL build refuses an existing destination name.

```sh
tizen-ornn build --repository https://download.tizen.org/snapshots/TIZEN/Tizen-10.1/
```

The builder starts with the published x86_64 emulator archive. Before installing development tools it compares the kickstart partition labels, sizes and package selections with the image XML and package list. It checks the original filesystem’s RPM names, architectures and versions after accounting for literal package erasures in the kickstart post-install step. The input hashes and resulting inventory are saved in `snapshot.lock.json`. Profiles must provide the supported ext4 root and sysdata layout. The builder reads the kickstart as data and does not execute its scripts.

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
