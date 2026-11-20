#!/usr/bin/env bash
# Run in a private mount namespace with the caller holding the old disk lock.
set -euo pipefail
if (( EUID != 0 || $# != 2 )); then
    echo "Usage: unshare --mount --propagation private copy-workspace.sh OLD_DISK NEW_RAW_DISK (as root)" >&2
    exit 2
fi
old=$(realpath -e "$1")
new=$(realpath -e "$2")
[[ -f $old && -f $new && ! $old -ef $new ]]
if [[ -n $(losetup -j "$old") || -n $(losetup -j "$new") ]]; then
    echo 'Unmount the image before copying Tizen user data.' >&2
    exit 1
fi
stage=$(mktemp -d "$(dirname "$new")/.workspace.XXXXXXXX")
cleanup() {
    if mountpoint -q "$stage/new"; then umount "$stage/new" || return; fi
    if mountpoint -q "$stage/old"; then umount "$stage/old" || return; fi
    rm -rf -- "$stage"
}
trap cleanup EXIT
mkdir "$stage/old" "$stage/new"
if [[ $old == *.qcow2 ]]; then
    if ! command -v qemu-img >/dev/null; then
        project=$(cd "$(dirname "$0")/.." && pwd)
        host=$project/out/host-tools/usr
        export PATH="$host/bin:$PATH"
        export LD_LIBRARY_PATH="$host/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    fi
    qemu-img convert -f qcow2 -O raw "$old" "$stage/old.raw"
    old=$stage/old.raw
fi
metadata=$(LC_ALL=C tune2fs -l "$old")
if ! grep -Eq '^Filesystem state:[[:space:]]+clean$' <<< "$metadata" ||
        grep -Eq '^Filesystem features:.*\bneeds_recovery\b' <<< "$metadata"; then
    # A VM forcibly shut down may leave committed data in the ext4 journal.
    # Replay it on a private copy so rollback retains the exact original.
    if [[ $old != "$stage/old.raw" ]]; then
        cp --reflink=auto --sparse=always -- "$old" "$stage/old.raw"
        old=$stage/old.raw
    fi
    echo 'Recovering the previous ext4 filesystem on a temporary copy...'
    status=0
    e2fsck -p "$old" || status=$?
    if (( status > 1 )); then
        echo 'The temporary copy needs manual filesystem repair. The previous disk is still selected.' >&2
        exit 1
    fi
fi
mount -t ext4 -o loop,ro,noload,nodev,nosuid,noexec "$old" "$stage/old"
mount -t ext4 -o loop,rw,nodev,nosuid,noexec "$new" "$stage/new"

# Resolve the guest's /home at the guest root. Using host realpath directly
# on an absolute /home symlink would look up the target on the host.
guest_home() {
    local root=$1 target
    if [[ ! -L $root/home ]]; then
        echo /home
        return
    fi
    target=$(readlink "$root/home")
    case "$target" in
        opt/usr/home|/opt/usr/home|./opt/usr/home) echo /opt/usr/home ;;
        *) echo "Unsupported guest /home target: $target" >&2; return 1 ;;
    esac
}

merge() {
    local src=$stage/old$1 dst=$stage/new$2
    shift 2
    if [[ ! -e $src && ! -L $src ]]; then return; fi
    for path in "$src" "$dst"; do
        if [[ ! -d $path || $(realpath -e "$path") != "$path" ]]; then
            echo "Expected a real data directory inside the image: $path" >&2
            return 1
        fi
    done
    rsync -aHAXS --checksum --numeric-ids --one-file-system "$@" "$src/" "$dst/"
}

old_home=$(guest_home "$stage/old")
new_home=$(guest_home "$stage/new")
if [[ $old_home == /opt/usr/home && $old_home != "$new_home" ]]; then
    merge /opt/usr /opt/usr --exclude=/home
else
    merge /opt/usr /opt/usr
fi
if [[ $old_home != /opt/usr/home || $new_home != /opt/usr/home ]]; then
    merge "$old_home" "$new_home"
fi
# Restore user-installed tools, keeping the three image-supplied Node commands.
merge /root /root --exclude=/.local/bin/node --exclude=/.local/bin/npm --exclude=/.local/bin/npx
sync -f "$stage/new"
umount "$stage/new"
umount "$stage/old"
