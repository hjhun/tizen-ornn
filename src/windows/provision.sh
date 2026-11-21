#!/bin/bash
# Run inside the disposable Ubuntu builder VM. Only the output disk survives.
set -euo pipefail
trap 'systemctl --no-block poweroff' EXIT
disk=/dev/disk/by-id/virtio-ornn-output
status=/dev/disk/by-id/virtio-ornn-status
test -b "$disk" && test -b "$status"
work=/var/tmp/ornn
mkdir -p /mnt/ornn-seed "$work"
mount -o ro /dev/disk/by-label/CIDATA /mnt/ornn-seed
cp /mnt/ornn-seed/ornn.deb /var/tmp/tizen-ornn.deb
export DEBIAN_FRONTEND=noninteractive TIZEN_PREBUILT_KERNEL=1 JOBS=4
apt-get -o Acquire::Retries=3 update
apt-get -o Acquire::Retries=3 install -y /var/tmp/tizen-ornn.deb
/usr/lib/tizen-ornn/scripts/tizen-image --out "$work"
test -f "$work/.image-ready" && test -f "$work/.tizen-virtio-console"
test "$(stat -c %s "$work/tizen-rootfs.ext4")" = "$(blockdev --getsize64 "$disk")"
dd if="$work/tizen-rootfs.ext4" of="$disk" bs=4M conv=sparse,fsync status=progress
blockdev --flushbufs "$disk"
printf 'TIZEN_ORNN_IMAGE_READY\n' | dd of="$status" conv=fsync,notrunc status=none
