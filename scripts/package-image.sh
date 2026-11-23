#!/usr/bin/env bash
# Bundle a stopped guest as a sparse archive suitable for either Linux host.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
out=$(realpath "${1:-$project/out}")
if [[ $# == 0 ]]; then
    if [[ -d $out/.tizen-current ]]; then out=$(realpath "$out/.tizen-current")
    elif [[ -d $out/current ]]; then out=$(realpath "$out/current")
    fi
fi
dest=${2:-$project/dist}
mkdir -p "$dest"
dest=$(realpath "$dest")
release=tizen-10.1-unified_20261119.053314
if [[ -f "$out/snapshot.lock.json" ]]; then
    release=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["release"])' "$out/snapshot.lock.json")
fi
[[ $release =~ ^tizen-10\.1-unified_([0-9]{8}\.[0-9]{6})$ ]] || { echo "Invalid snapshot ID" >&2; exit 1; }
snapshot=${BASH_REMATCH[1]}
version=$(<"$project/config/version")
number='(0|[1-9][0-9]*)'
identifier='[a-zA-Z0-9-]+'
version_pattern="^${number}([.]${number}){2}([+]${identifier}([.]${identifier})*)?$"
[[ $version =~ $version_pattern ]] || { echo "Invalid host package version: $version" >&2; exit 1; }
bundle=tizen-dev-10.1-x86_64-${snapshot}-${version}
archive="$dest/$bundle.tar.zst"
[[ -f "$out/bzImage" && -f "$out/kernel.config" ]]
disk="$out/tizen-rootfs.ext4"
if [[ ! -f "$disk" ]]; then disk="$out/tizen-rootfs.qcow2"; fi
[[ -f "$disk" ]] || { echo 'Missing Tizen disk' >&2; exit 1; }
exec 9<"$disk"
flock -n -x 9 || { echo 'Shut down the guest before packing.' >&2; exit 1; }
[[ -z $(losetup -j "$disk") ]] || { echo 'Unmount the disk before packing.' >&2; exit 1; }
[[ ! -e $archive ]] || { echo "Archive already exists: $archive" >&2; exit 1; }
stage=$(mktemp -d "$dest/.bundle.XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT
base=$stage/$bundle
mkdir -p "$base/out"
for item in LICENSE README.md config scripts kernel guest tests packaging sdk cross docs; do
    cp -a "$project/$item" "$base/"
done
find "$base" -type d -name __pycache__ -prune -exec rm -rf {} +
if [[ -f "$project/Cargo.toml" ]]; then
    cargo build --manifest-path "$project/Cargo.toml" --locked --release --bin tizen-ornn
    install -m755 "${CARGO_TARGET_DIR:-$project/target}/release/tizen-ornn" "$base/tizen-ornn"
else
    install -m755 "$project/tizen-ornn" "$base/tizen-ornn"
fi
# The launcher uses the release kernel for features missing from older disks.
# A host package already includes this pair. A checkout uses its kernel build.
if [[ ! -f $base/kernel/kernel.sha256 ]]; then
    kernel_dir=$project/out/share-kernel
    TIZEN_OUT="$kernel_dir" TIZEN_PREBUILT_KERNEL=0 "$project/scripts/build-kernel.sh" --reuse
    cp -- "$kernel_dir"/{bzImage,kernel.config,kernel.sha256} "$base/kernel/"
fi
(cd "$base/kernel" && sha256sum -c kernel.sha256)
cp "$out/bzImage" "$out/kernel.config" "$base/out/"
if [[ -f "$out/snapshot.lock.json" ]]; then cp "$out/snapshot.lock.json" "$base/out/"; fi
if [[ -f $out/.tizen-ornn-image-revision ]]; then
    cp "$out/.tizen-ornn-image-revision" "$base/out/"
fi
if [[ -f "$out/.tizen-virtio-console" ]]; then touch "$base/out/.tizen-virtio-console"; fi
# A qcow2 may have a backing file. Always export a standalone raw filesystem.
raw="$disk"
if [[ $disk == *.qcow2 ]]; then
    raw="$stage/tizen-rootfs.ext4"
    qemu_img=/usr/bin/qemu-img
    if [[ ! -x $qemu_img ]]; then qemu_img=qemu-img; fi
    "$qemu_img" convert -f qcow2 -O raw "$disk" "$raw"
fi
(cd "$base" && sha256sum out/bzImage out/kernel.config kernel/bzImage kernel/kernel.config > SHA256SUMS)
digest=$(sha256sum <"$raw")
printf '%s  out/tizen-rootfs.ext4\n' "${digest%% *}" >> "$base/SHA256SUMS"
# Read the raw disk in place, including when the archive is on another volume.
tar --sparse --owner=0 --group=0 -I 'zstd -T2 -4' -cf "$stage/image.tar.zst" \
    --transform="s|^tizen-rootfs\.ext4$|$bundle/out/tizen-rootfs.ext4|" \
    -C "$stage" "$bundle" -C "$(dirname "$raw")" tizen-rootfs.ext4
zstd -tq "$stage/image.tar.zst"
mv "$stage/image.tar.zst" "$archive"
(cd "$dest" && sha256sum "$bundle.tar.zst" > "$bundle.tar.zst.sha256")
echo "$archive"
