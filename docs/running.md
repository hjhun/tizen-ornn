# Running the VM and managing storage

From the host terminal, run `tizen-vm` to start the selected guest or return to its existing console. For a graphical guest, use `tizen-vm --gui` on the first start. The window needs WSLg or a Linux desktop. CPU, memory and display settings take effect when the VM boots.

| Action | Result |
| --- | --- |
| Press Ctrl+C during `dlogutil` or a build | The guest sends SIGINT to the foreground job. |
| Press Ctrl+D once at the empty top-level prompt | The shell stays open. |
| Type `exit` in a nested shell or after `su - owner` | Return to the parent shell. |
| Type `exit` at the top-level serial shell | Detach all viewers. The VM, shell and background jobs continue running. |
| Close the host terminal | Other viewers remain connected. Run `tizen-vm` to reconnect. |
| Run `systemctl poweroff` as guest root | Shut down the VM and release its disk. |

Two host terminals running the launcher for the same disk share a serial console with the same input, output and shell. A private tmux server keeps it available while the host is running. An older image with the original console profile needs a guest shutdown and `tizen-vm update --image` to support detaching via `exit`. A VM already started in foreground mode must also be shut down before the first managed launch. See the [installation and update guide](../README.md).

## Launch from a checkout

The low-level script also accepts an explicit image directory. Defaults are two CPUs, 3072 MiB of RAM and user-mode NAT. The VM uses KVM when the host user can access `/dev/kvm`, and QEMU software emulation otherwise.

```sh
export TIZEN_OUT="$PWD/out/dev"
TIZEN_CPUS=4 TIZEN_RAM=4096 ./scripts/run-vm.sh --gui
```

For serial automation such as Expect, use `./scripts/run-vm.sh --foreground`. This connects stdin and stdout directly to QEMU. Piped input or output also selects foreground mode. The interactive launcher takes an exclusive lock on the disk throughout the VM's lifetime, including while detached.

## Install packages while developing

In a second host terminal, run `tizen-vm install ed` to install a Tizen RPM or `tizen-vm update --packages ed` to update it from the configured snapshot. The commands stream guest output to that terminal and return the guest operation's exit status. The serial console keeps its foreground process. A private virtio port connects the host user to the guest service. The guest must be running. Older images gain this service through `tizen-vm update --image` after shutdown.

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

Use the archive name printed by the command. For example, the R5 bundle for the November 19 snapshot can be unpacked and started as follows.

```sh
cd dist
bundle=tizen-dev-10.1-x86_64-20261119.053314-r5
sha256sum -c "$bundle.tar.zst.sha256"
tar --zstd -xf "$bundle.tar.zst"
cd "$bundle"
sha256sum -c SHA256SUMS
unset TIZEN_OUT
./scripts/run-vm.sh --gui
```

An unpacked bundle requires `qemu-system-x86`, `seabios`, `util-linux` and `tmux` on the host. Install `qemu-system-gui` for the graphical window.
