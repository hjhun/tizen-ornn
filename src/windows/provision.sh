#!/bin/bash
# Run inside the disposable Ubuntu builder VM. Only the output disk survives.
set -euo pipefail
trap 'systemctl --no-block poweroff' EXIT
disk=/dev/disk/by-id/virtio-ornn-output
status=/dev/disk/by-id/virtio-ornn-status
test -b "$status"
work=/var/tmp/ornn
mkdir -p /mnt/ornn-seed "$work"
mount -o ro /dev/disk/by-label/CIDATA /mnt/ornn-seed
cp /mnt/ornn-seed/ornn.deb /var/tmp/tizen-ornn.deb
for key in http_proxy https_proxy all_proxy no_proxy; do
    if [[ -f /mnt/ornn-seed/$key ]]; then
        value=$(cat "/mnt/ornn-seed/$key")
        export "$key=$value" "${key^^}=$value"
    fi
done
export DEBIAN_FRONTEND=noninteractive TIZEN_PREBUILT_KERNEL=1 JOBS=4
if [[ -f /mnt/ornn-seed/build-ca.crt ]]; then
    export TIZEN_BUILD_CA_CERT=/mnt/ornn-seed/build-ca.crt
    python3 /mnt/ornn-seed/build_ca.py --export /usr/local/share/ca-certificates/ornn-build.crt
    update-ca-certificates
fi
apt-get -o Acquire::Retries=3 update
apt-get -o Acquire::Retries=3 install -y /var/tmp/tizen-ornn.deb
if [[ -f /mnt/ornn-seed/image.toml ]]; then export TIZEN_IMAGE_CONFIG=/mnt/ornn-seed/image.toml; fi
arch=$(cat /mnt/ornn-seed/guestarch)
case $arch in
armv7l|aarch64)
    if [[ $arch == aarch64 ]]; then compiler=gcc-aarch64-linux-gnu; kernel=Image
    else compiler=gcc-arm-linux-gnueabihf; kernel=zImage; fi
    apt-get -o Acquire::Retries=3 install -y "$compiler" build-essential bc bison flex libelf-dev libssl-dev patch
    TIZEN_ARM_ARCH=$arch /usr/lib/tizen-ornn/scripts/armv7l/build-image "$work"
    mkdir -p /mnt/ornn-output
    mount -t 9p -o trans=virtio,version=9p2000.L ornn-arm-output /mnt/ornn-output
    mkdir /mnt/ornn-output/disk
    cp --sparse=always "$work"/disk/*.img /mnt/ornn-output/disk/
    cp "$work/$kernel" /mnt/ornn-output/
    umount /mnt/ornn-output
    ;;
x86_64)
    test -b "$disk"
    /usr/lib/tizen-ornn/scripts/tizen-image --out "$work"
    test -f "$work/.image-ready" && test -f "$work/.tizen-virtio-console"
    test "$(stat -c %s "$work/tizen-rootfs.ext4")" = "$(blockdev --getsize64 "$disk")"
    dd if="$work/tizen-rootfs.ext4" of="$disk" bs=4M conv=sparse,fsync status=progress
    blockdev --flushbufs "$disk"
    ;;
*) exit 2 ;;
esac
printf 'TIZEN_ORNN_IMAGE_READY\n' | dd of="$status" conv=fsync,notrunc status=none
