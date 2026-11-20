# Running the VM and managing storage

From the host terminal, run `tizen-vm` to start the selected guest or return to its existing console. For a graphical guest, use `tizen-vm --gui` on the first start. The window needs WSLg or a Linux desktop. CPU, memory and display settings take effect when the VM boots.

| Action | Result |
| --- | --- |
| Press Ctrl+C during `dlogutil` or a build | The guest sends SIGINT to the foreground job. |
| Press Ctrl+D once at the empty top-level prompt | The shell stays open. |
| Type `exit` in a nested shell or after `su - owner` | Return to the parent shell. |
| Type `exit` at the top-level serial shell | Detach all viewers. The VM, shell and background jobs continue running. |
| Close the host terminal | Other viewers remain connected. Run `tizen-vm` to reconnect. |
| Run `tizen-vm shutdown` on the host | Shut down the selected guest, including a detached session. |
| Run `systemctl poweroff` as guest root | Shut down the VM and release its disk. |

At startup the serial console takes the host terminal’s rows and columns, with a 24 by 80 fallback. The managed console follows the effective tmux pane size when you resize a terminal, adjust a Herdr split, or reattach. All viewers share that size. Updates resume automatically when the guest control channel becomes available.

Two host terminals running the launcher for the same disk share a serial console with the same input, output and shell. A private tmux server keeps it available while the host is running. An older image with the original console profile needs a guest shutdown and `tizen-vm update --image` to support detaching via `exit`. A VM already started in foreground mode must also be shut down before the first managed launch. See the [installation and update guide](../README.md).

The serial console logs in as the ordinary Unix user `tizen` with the `System::Privileged` SMACK label. Run `su -` for a passwordless root login. From root, `su - owner` selects the platform owner's `User::Shell` label. Use `id` and `cat /proc/$$/attr/current` to inspect the login.

The tizen account has its development tools under `~/dev-setup`. Owner uses `~/workspace/.ai-home/.local/bin` for the same tools installed by `tizen-vm install`. Each account can customize its interactive Bash shell in `~/.bashrc`. In a Git worktree the prompt shows the branch in green when clean, red with local changes and cyan when clean and ahead of its upstream.

The host shutdown command requires an image with the poweroff operation. On an older image, run `systemctl poweroff` at the guest console, then use the updated host CLI to run `tizen-vm update --image`.

## Launch from a checkout

The low-level script also accepts an explicit image directory. Defaults are two CPUs, 3072 MiB of RAM and user-mode NAT. The VM uses KVM when the host user can access `/dev/kvm`, and QEMU software emulation otherwise.

```sh
export TIZEN_OUT="$PWD/out/dev"
TIZEN_CPUS=4 TIZEN_RAM=4096 ./scripts/run-vm.sh --gui
```

For serial automation such as Expect, use `./scripts/run-vm.sh --foreground`. This connects stdin and stdout directly to QEMU. Piped input or output also selects foreground mode. The interactive launcher takes an exclusive lock on the disk throughout the VM's lifetime, including while detached.

## Install packages while developing

In a second host terminal, run `tizen-vm install ed` to install a Tizen RPM or `tizen-vm update --packages ed` to update it from the configured snapshot. The commands stream guest output to that terminal and return the guest operation's exit status. The serial console keeps its foreground process. A private virtio port connects the host user to the guest service. The guest must be running. Older images gain this service through `tizen-vm update --image` after shutdown.

## Share a host directory

Run the launcher as your normal Ubuntu user. On an existing R5 disk, upgrade the host CLI to R6 or later to use the bundled kernel for sharing. The project stays on the host and its files are accessible under `/home/owner/host` for this boot.

```sh
tizen-vm run --share "$PWD"
# In the guest console
su -
su - owner
cd /home/owner/host
```

To share a Windows project from WSL2, use its WSL path, for example `tizen-vm run --share "/mnt/c/Users/me/My Project"`. Use `--share-ro` to enforce read-only access on the host side. With `--share`, guest edits and deletions affect the host files immediately. The export has the access rights of the Ubuntu account launching QEMU. Guest `owner` can work in private directories without changing their host ownership. File permissions, case sensitivity and link support on Windows paths depend on the underlying filesystem and WSL mount settings.

Choose one directory per boot. The guest mountpoint must be absent or empty. The launcher uses a Linux user namespace and a virtio-9p device. To check whether your Ubuntu host permits the required user mapping, run `unshare --user --map-user=5001 --map-group=100 -- true` as the user who starts the VM. The guest shuts down with a diagnostic if it cannot mount the directory. Run plain `tizen-vm` to reattach. After `systemctl poweroff`, the next launch may select a different directory or omit sharing. A full image update carries the files stored on the guest disk. Shared project files already live on the host.

## Host networking

The VM keeps its own interface and private IP. QEMU routes outbound connections over Ubuntu's network and forwards guest DNS requests through `10.0.2.3`. On startup the CLI reads the host resolver's search domains so tools can resolve short hostnames. In WSL2, DNS and VPN reachability depend on the Windows-to-WSL network configuration. [Microsoft's WSL network guide](https://learn.microsoft.com/en-us/windows/wsl/networking)

The CLI takes proxy settings from the environment in the terminal that starts the VM. Lowercase names take precedence over uppercase names. In WSL2, if no proxy endpoint variable is set, the CLI reads the current Windows user's manual proxy settings through PowerShell interop. It supports one proxy for all HTTP traffic or separate HTTP, HTTPS and SOCKS endpoints. The guest receives settings at boot for login shells and systemd services, including the package service behind `tizen-vm install`.

| Host setting | Guest behavior |
|---|---|
| `http_proxy`, `https_proxy`, `all_proxy` | Use the configured endpoint. A loopback proxy address is translated to `10.0.2.2`, the QEMU host gateway. The proxy must be reachable from Ubuntu or WSL. |
| `no_proxy` | Preserve the comma-separated bypass list. Windows manual bypass entries such as `*.example.org` become domain suffixes. |
| PAC, WPAD, or the Windows `<local>` bypass rule | Applications differ in how they evaluate these rules. Set explicit proxy endpoints and `no_proxy` in Ubuntu for the desired routes. |
| Proxy URL with credentials | Only the endpoint is forwarded. Configure any required authentication in the guest through your organization's approved mechanism. |

A terminal reconnect keeps the current VM's settings. To pick up a changed proxy or DNS search domain, shut down the guest and start it again. `tizen-vm run --no-host-network` keeps ordinary NAT and DNS while skipping the host proxy and search-domain import. Private CA certificates are configured separately. [HTTPS trust setup](https-certificates.md)

## Disk size

The default raw disk `tizen-rootfs.ext4` is a 16 GiB sparse file. The guest retains its packages, source trees and build output across shutdowns. Use `df -h /` inside the guest to see free guest space and `du -h "$TIZEN_OUT/tizen-rootfs.ext4"` on the host to see allocated host space. Develop under `/home/owner/workspace`.

For a fresh store, `TIZEN_FS_SIZE=32G tizen-vm build` creates a 32 GiB disk. Use `TIZEN_CPUS=4 TIZEN_RAM=4096 tizen-vm run` to start with four virtual CPUs and 4096 MiB of guest RAM. To grow an existing raw disk, first shut down the guest, unmount the disk on the host and back it up. Then run these commands on the host.

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
