#!/usr/bin/env bash
# A writable, sparse raw filesystem plus a direct-boot x86 kernel.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
export TIZEN_OUT=${TIZEN_OUT:-$project/out}
[[ $EUID == 0 ]] || { echo 'Run with sudo on the Ubuntu build host.' >&2; exit 1; }
mkdir -p "$TIZEN_OUT"
TIZEN_OUT=$(cd "$TIZEN_OUT" && pwd)
if [[ -f $TIZEN_OUT/snapshot.lock.json ]]; then
    export TIZEN_SNAPSHOT_LOCK="$TIZEN_OUT/snapshot.lock.json"
fi
root=$TIZEN_OUT/root
disk=$TIZEN_OUT/tizen-rootfs.ext4
exec 8>"$TIZEN_OUT/.image-build.lock"
flock -n 8 || { echo 'Another image build is in progress' >&2; exit 1; }
if [[ -e $disk && ! -e "$TIZEN_OUT/.root-work/extracted" ]]; then
    echo 'Use a fresh output directory to build a new image.' >&2
    exit 1
fi
if [[ -e "$TIZEN_OUT/.image-ready" ]]; then
    echo 'Use a fresh TIZEN_OUT directory to preserve the installed development filesystem.' >&2
    exit 1
fi
# The VM owns its writable image after finalization. This trap is restricted to
# the loop device backed by the disk created by this build.
detach() {
    if mountpoint -q "$root"; then
        local device
        device=$(findmnt -no SOURCE --mountpoint "$root")
        if [[ $(losetup -nO BACK-FILE "$device") == "$disk" ]]; then
            umount "$root"
        fi
    fi
}
trap detach EXIT
"$project/scripts/build-kernel.sh"
"$project/scripts/build-root.sh"
TIZEN_BUILDROOT="$root" "$project/scripts/build-git.sh"
"$project/scripts/install-git.sh"
CACHE_DIR="$TIZEN_OUT/cache/sdk" "$project/scripts/install-sdk.sh" "$root"
"$project/scripts/install-cross.sh" "$root"
"$project/scripts/install-rust-cross.sh" "$root"
"$project/scripts/prepare-headed.sh" "$root"
"$project/scripts/configure-guest.sh" "$root"
"$project/scripts/prepare-owner.sh" "$root"
"$project/scripts/install-aul-devel.sh" "$root"
"$project/scripts/install-spec-builder.sh" "$root"
"$project/scripts/install-linux-tools.sh" "$root"
sync -f "$root"
detach
[[ -z $(losetup -j "$disk") ]]
chown "${SUDO_UID:-$(stat -c %u "$project")}:${SUDO_GID:-$(stat -c %g "$project")}" "$disk"
touch "$TIZEN_OUT/.image-ready"
