#!/usr/bin/env bash
# Build the native Tizen development filesystem on a Linux ext4 volume.
set -euo pipefail
if (( EUID != 0 )); then
    echo "Run as root. The build mounts loop devices and preserves security xattrs." >&2
    exit 1
fi
project=$(cd -- "$(dirname "$0")/.." && pwd -P)
out=${TIZEN_OUT:-"$project/out"}
cache="$out/cache"
root="$out/root"
disk="$out/tizen-rootfs.ext4"
work="$out/.root-work"
fields=$(python3 "$project/scripts/snapshot_lock.py" archive)
mapfile -t fields <<< "$fields"
archive_sha=${fields[0]}
url=${fields[1]}
archive=${TIZEN_SOURCE_ARCHIVE:-"$cache/${url##*/}"}
mkdir -p "$out" "$cache" "$work" "$root"
exec 9>"$work/build.lock"
flock -n 9 || { echo 'Another rootfs build holds the lock.' >&2; exit 1; }
if [[ ! -f "$archive" ]]; then
    [[ -z ${TIZEN_SOURCE_ARCHIVE:-} ]] || { echo "Missing source archive $archive" >&2; exit 1; }
    curl -fLSs --retry 3 "$url" -o "$archive.partial"
    echo "$archive_sha  $archive.partial" | sha256sum -c -
    mv "$archive.partial" "$archive"
fi
if [[ ! -e "$work/extracted" ]]; then
    echo "$archive_sha  $archive" | sha256sum -c -
    if [[ -e "$disk" ]]; then
        echo "An untracked filesystem already exists at $disk." >&2
        exit 1
    fi
    tar -xzf "$archive" -C "$work" emulator-rootfs.img emulator-sysdata.img
    # Grow the filesystem before mounting. truncate leaves unallocated space sparse.
    truncate -s "${TIZEN_FS_SIZE:-16G}" "$work/emulator-rootfs.img"
    fsck_status=0
    e2fsck -fp "$work/emulator-rootfs.img" || fsck_status=$?
    (( fsck_status <= 1 )) || exit "$fsck_status"
    resize2fs "$work/emulator-rootfs.img"
    mv "$work/emulator-rootfs.img" "$disk"
    touch "$work/extracted"
fi
if ! mountpoint -q "$root"; then
    mount -o loop,rw "$disk" "$root"
else
    dev=$(findmnt -no SOURCE --mountpoint "$root")
    test "$(readlink -f "$(losetup -nO BACK-FILE "$dev")")" = "$(readlink -f "$disk")"
fi
if [[ ! -e "$work/opt-merged" ]]; then
    sysdata="$work/sysdata"
    mkdir -p "$sysdata" "$root/opt"
    (mount -o loop,ro "$work/emulator-sysdata.img" "$sysdata"
     trap 'umount "$sysdata"' EXIT
     rsync -aHAXx --numeric-ids "$sysdata/" "$root/opt/")
    touch "$work/opt-merged"
fi
if [[ "${1:-}" = --extract-only ]]; then exit 0; fi
"$project/scripts/bootstrap-packages.sh" "$root" "$cache" "$work"
