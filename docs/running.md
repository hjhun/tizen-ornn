# Running the VM and managing storage

From the host terminal, run `tizen-ornn` to start the selected guest or return to its existing console. A new VM opens a graphical window on WSLg or a Linux desktop by default. Use `tizen-ornn --headless` for a console-only boot. Both options also work with `run` and `start`. To change the display mode of a running VM, use `tizen-ornn shutdown` first. On Windows, x86_64 images use a GTK window and ARM images use SDL. Both use the bundled Mesa runtime and support a visible pointer and a virtio tablet. Close the terminal window to disconnect. **Open terminal** in the launcher or `tizen-ornn console --image-dir PATH` reconnects to the same shell on both hosts. CPU, memory and display settings take effect when the VM boots.

Ubuntu GUI boots use QEMU's virtio GPU with virgl and the guest's Mesa driver.
This provides the multisampled EGL surface used by the Flutter home screen.
Graphical boots start the snapshot’s graphical target and home app. The sample counter is available through `systemctl start tizen-ui-demo.service`. Ubuntu needs an OpenGL-capable SDL display. On Ubuntu hosts without one, start
with `TIZEN_GUI_GL=0 tizen-ornn --gui` to use guest software rendering. On Tizen 10.1, software boots select Skia for the stock Flutter home app. A subsequent virgl boot restores the app’s declared Impeller setting.

R26 uses its bundled x86_64 kernel when an existing image contains the previous official kernel. The launcher identifies that kernel by SHA-256 and leaves the image directory intact. The updated kernel limits the number of virgl uniform buffers advertised to the Tizen Mesa driver.

On an existing guest using the emulator audio HAL, run
`tizen-ornn install patchelf` on the host before restarting with `--gui`.
Fresh images include this dependency. PulseAudio uses a boot-local copy of
the emulator HAL linked to libm so the Volume app can query its volume range.

| Action | Result |
| --- | --- |
| Press Ctrl+C during `dlogutil` or a build | The guest sends SIGINT to the foreground job. |
| Press Ctrl+D once at the empty top-level prompt | The shell stays open. |
| Type `exit` in a nested shell or after `su - owner` | Return to the parent shell. |
| Type `exit` at the top-level serial shell | Detach all viewers. The VM, shell and background jobs continue running. |
| Close the host terminal | Other viewers remain connected. Run `tizen-ornn` to reconnect. |
| Run `tizen-ornn shutdown` on the host | Shut down the selected guest, including a detached session. |
| Run `systemctl poweroff` as guest root | Shut down the VM and release its disk. |

Images built with R11 use a virtio console (`/dev/hvc0`) for interactive output, including full-screen applications such as Herdr and Codex. Older images and kernels without known built-in virtio console support use the UART (`/dev/ttyS0`). For an existing installation, install the updated host package, shut down the VM and run `tizen-ornn update --image` to rebuild and migrate it. Run `tty` in the guest login shell to see the active transport.

At startup the serial console takes the host terminal’s rows and columns, with a 24 by 80 fallback. The managed console follows the effective tmux pane size when you resize a terminal, adjust a Herdr split, or reattach. All viewers share that size. Updates resume automatically when the guest control channel becomes available.

Two host terminals running the launcher for the same disk share a serial console with the same input, output and shell. A private tmux server keeps it available while the host is running. An older image with the original console profile needs a guest shutdown and `tizen-ornn update --image` to support detaching via `exit`. A VM already started in foreground mode must also be shut down before the first managed launch. See the [installation and update guide](../README.md).

The serial console logs in as the ordinary Unix user `tizen` with the `System::Privileged` SMACK label. Run `su -` for a passwordless root login. From root, `su - owner` selects the platform owner's `User::Shell` label. Use `id` and `cat /proc/$$/attr/current` to inspect the login.

The tizen account has its development tools under `~/dev-setup`. Owner uses `~/workspace/.ai-home/.local/bin` for the same tools installed by `tizen-ornn install`. Each account can customize its interactive Bash shell in `~/.bashrc`. In a Git worktree the prompt shows the branch in green when clean, red with local changes and cyan when clean and ahead of its upstream.

The host shutdown command requires an image with the poweroff operation. On an older image, run `systemctl poweroff` at the guest console, then use the updated host CLI to run `tizen-ornn update --image`.

## Launch from a checkout

The low-level script also accepts an explicit image directory. Defaults are two CPUs, 3072 MiB of RAM and user-mode NAT. The VM uses KVM when the host user can access `/dev/kvm`, and QEMU software emulation otherwise.

```sh
export TIZEN_OUT="$PWD/out/dev"
TIZEN_CPUS=4 TIZEN_RAM=4096 ./scripts/run-vm.sh --gui
```

For serial automation such as Expect, use `./scripts/run-vm.sh --foreground`. This connects stdin and stdout directly to QEMU. Piped input or output also selects foreground mode. The interactive launcher takes an exclusive lock on the disk throughout the VM's lifetime, including while detached.

## Install packages while developing

In a second host terminal, run `tizen-ornn install ed` to install a Tizen RPM or `tizen-ornn update --packages ed` to update it from the configured snapshot. The commands stream guest output to that terminal and return the guest operation's exit status. The serial console keeps its foreground process. A private virtio port connects the host user to the guest service. The guest must be running. Older images gain this service through `tizen-ornn update --image` after shutdown.

## Share a host directory

Windows and Ubuntu mount a host folder at `/mnt/host` on every x86_64 development guest boot. The default is `share` under the launcher work directory (or `TIZEN_ORNN_HOME` when set). The launcher creates this default directory. The same folder is available at `/home/tizen/share` with the same read/write or read-only access. Changes through either path are visible on the host.

Use **Add image…** to connect an existing image directory. The launcher selects it immediately and remembers it in the current work directory’s image library. Use **Disconnect image…** to remove an external folder from the library while keeping its files. For an image built in the managed store, stop the guest and choose **Delete image…** to review its path and delete its guest disk.

Set **Settings → Shared folder (x86_64) → Save shared folder** in the desktop launcher, then select an image in **Image library** and choose **Start with display** or **Start terminal only**. The CLI accepts the same persistent setting. Supply an existing absolute directory. In a portable installation, run the CLI beside the GUI binary inside `Tizen Ornn Portable/runtime-…` to use that installation’s settings and bundled Windows QEMU.

```sh
tizen-ornn share-dir "$PWD"       # Ubuntu
tizen-ornn run --headless
tizen-ornn share-dir --reset      # restore the default for future boots
```

```powershell
.\tizen-ornn.exe share-dir 'D:\Projects'
.\tizen-ornn.exe run --gui --image-dir 'D:\Tizen Images\my-image'
```

Use `run --share PATH` or `run --share-ro PATH` for a single boot. A running VM keeps its existing mount until shutdown. On Ubuntu, these two legacy one-boot flags keep the original guest `owner` identity for host permissions and also expose the folder at `/home/owner/host` when that location is empty. Use `share-dir` or the GUI setting to work on private host files as the interactive `tizen` account. Start QEMU as your ordinary host user. WSL paths such as `/mnt/c/Users/me/My Project` work with the Ubuntu launcher. Native Windows uses a local NTFS folder and a 9P-capable QCW UCRT64 QEMU runtime.

Before mounting, the launcher’s guest startup script renames a populated guest `/mnt/host` directory to `/mnt/host.before-host` on the same guest disk. Hidden files and permissions are preserved. If that name already exists, the script selects `.1`, `.2`, and so on. Later boots leave those backups in place. The same preservation rules apply to `/home/tizen/share`. Existing files are kept beside the mount at `/home/tizen/share.before-host` or the next available numbered backup.

## Host networking

The VM keeps its own interface and private IP. QEMU routes outbound connections over Ubuntu's network and forwards guest DNS requests through `10.0.2.3`. On startup the CLI reads the host resolver's search domains so tools can resolve short hostnames. In WSL2, DNS and VPN reachability depend on the Windows-to-WSL network configuration. [Microsoft's WSL network guide](https://learn.microsoft.com/en-us/windows/wsl/networking)

The CLI takes proxy settings from the environment in the terminal that starts the VM. Lowercase names take precedence over uppercase names. In WSL2, if no proxy endpoint variable is set, the CLI reads the current Windows user's manual proxy settings through PowerShell interop. It supports one proxy for all HTTP traffic or separate HTTP, HTTPS and SOCKS endpoints. The guest receives settings at boot for login shells and systemd services, including the package service behind `tizen-ornn install`.

| Host setting | Guest behavior |
|---|---|
| `http_proxy`, `https_proxy`, `all_proxy` | Use the configured endpoint. A loopback proxy address is translated to `10.0.2.2`, the QEMU host gateway. The proxy must be reachable from Ubuntu or WSL. |
| `no_proxy` | Preserve the comma-separated bypass list. Windows manual bypass entries such as `*.example.org` become domain suffixes. |
| PAC, WPAD, or the Windows `<local>` bypass rule | Applications differ in how they evaluate these rules. Set explicit proxy endpoints and `no_proxy` in Ubuntu for the desired routes. |
| Proxy URL with credentials | Only the endpoint is forwarded. Configure any required authentication in the guest through your organization's approved mechanism. |

A terminal reconnect keeps the current VM's settings. To pick up a changed proxy or DNS search domain, shut down the guest and start it again. `tizen-ornn run --no-host-network` keeps ordinary NAT and DNS while skipping the host proxy and search-domain import. Private CA certificates are configured separately. [HTTPS trust setup](https-certificates.md)

## Disk size

The default raw disk `tizen-rootfs.ext4` is a 16 GiB sparse file. The guest retains its packages, source trees and build output across shutdowns. Use `df -h /` inside the guest to see free guest space and `du -h "$TIZEN_OUT/tizen-rootfs.ext4"` on the host to see allocated host space. Develop under `/home/owner/workspace`.

For a fresh store, `TIZEN_FS_SIZE=32G tizen-ornn build` creates a 32 GiB disk. Use `TIZEN_CPUS=4 TIZEN_RAM=4096 tizen-ornn run` to start with four virtual CPUs and 4096 MiB of guest RAM. To grow an existing raw disk, first shut down the guest, unmount the disk on the host and back it up. Then run these commands on the host.

```sh
disk="$TIZEN_OUT/tizen-rootfs.ext4"
truncate -s +8G "$disk"
e2fsck -f "$disk"
resize2fs "$disk"
```

## Local archive

With the VM shut down, you can bundle the disk, kernel and launcher for local use. The archive includes SDK components and any source, credentials or other data you have put into this guest.

```sh
./scripts/package-image.sh "$TIZEN_OUT" "$PWD/dist"
```

Use the archive name printed by the command. For example, the R6 bundle for the November 19 snapshot can be unpacked and started as follows.

```sh
cd dist
bundle=tizen-dev-10.1-x86_64-20261119.053314-r6
sha256sum -c "$bundle.tar.zst.sha256"
tar --zstd -xf "$bundle.tar.zst"
cd "$bundle"
sha256sum -c SHA256SUMS
unset TIZEN_OUT
./scripts/run-vm.sh --gui
```

An unpacked bundle requires `qemu-system-x86`, `seabios`, `util-linux` and `tmux` on the host. Install `qemu-system-gui` for the graphical window.
