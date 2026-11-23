#!/usr/bin/env bash
# A writable, sparse raw filesystem plus a direct-boot x86 kernel.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
export TIZEN_OUT=${TIZEN_OUT:-$project/out}
[[ $EUID == 0 ]] || { echo 'Run with sudo on the Ubuntu build host.' >&2; exit 1; }
mkdir -p "$TIZEN_OUT"
TIZEN_OUT=$(cd "$TIZEN_OUT" && pwd)
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
if [[ -f $TIZEN_OUT/snapshot.lock.json ]]; then
    export TIZEN_SNAPSHOT_LOCK="$TIZEN_OUT/snapshot.lock.json"
elif [[ -z ${TIZEN_SNAPSHOT_LOCK:-} ]]; then
    # Direct callers can have a populated download cache. Pin inputs before
    # any build stage reads them, just as the tizen-image entry point does.
    lock_tmp=$(mktemp "$TIZEN_OUT/.snapshot.XXXXXXXX")
    trap 'rm -f -- "$lock_tmp"' EXIT
    "$project/scripts/tizen-image" --print-lock > "$lock_tmp"
    if [[ -e $TIZEN_OUT/.root-work/extracted ]]; then
        # A build started before locks existed always used this source image.
        python3 -c 'import json, sys; sys.exit(json.load(open(sys.argv[1]))["archive"]["sha256"] !=
            "7ce64dbf088fbaa800d601e73209a687944dd07b56da874c30366008d0428d0f")' "$lock_tmp" ||
            { echo "Different source image; select a new output directory." >&2; exit 1; }
    fi
    chmod 644 "$lock_tmp"
    mv "$lock_tmp" "$TIZEN_OUT/snapshot.lock.json"
    export TIZEN_SNAPSHOT_LOCK="$TIZEN_OUT/snapshot.lock.json"
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
scratch=$(mktemp -d "$TIZEN_OUT/.image-build.XXXXXXXX")
trap 'detach; rm -rf -- "$scratch"' EXIT
TIZEN_OUT="$scratch" "$project/scripts/build-kernel.sh"
for artifact in bzImage kernel.config kernel.sha256; do
    if [[ -f $scratch/$artifact ]]; then mv "$scratch/$artifact" "$TIZEN_OUT/$artifact"; fi
done
export TIZEN_BUILD_CACHE=$scratch/cache
"$project/scripts/build-root.sh"
TIZEN_BUILDROOT="$root" TIZEN_OUT="$scratch" "$project/scripts/build-git.sh"
"$project/scripts/install-git.sh" "$scratch/git-repo"
CACHE_DIR="$scratch/cache/sdk" "$project/scripts/install-sdk.sh" "$root"
TIZEN_OUT="$scratch" "$project/scripts/install-cross.sh" "$root"
TIZEN_OUT="$scratch" "$project/scripts/install-rust-cross.sh" "$root"
"$project/scripts/prepare-headed.sh" "$root"
"$project/scripts/configure-guest.sh" "$root"
"$project/scripts/prepare-owner.sh" "$root"
"$project/scripts/install-aul-devel.sh" "$root"
"$project/scripts/install-spec-builder.sh" "$root"
"$project/scripts/install-linux-tools.sh" "$root"
TIZEN_OUT="$scratch" "$project/scripts/install-ai-prereqs.sh" "$root"
"$project/scripts/install-guest-control.sh" "$root"
python3 "$project/scripts/build_ca.py" --root "$root"
sync -f "$root"
detach
# Linux releases autoclear loop devices asynchronously after umount.
for ((attempt=0; attempt<100; attempt++)); do
    [[ -n $(losetup -j "$disk") ]] || break
    sleep 0.1
done
[[ -z $(losetup -j "$disk") ]] || { echo "Loop device still attached after unmount: $disk" >&2; exit 1; }
chown "${SUDO_UID:-$(stat -c %u "$project")}:${SUDO_GID:-$(stat -c %g "$project")}" "$disk"
install -m 0644 "$project/config/image-revision" "$TIZEN_OUT/.tizen-ornn-image-revision"
touch "$TIZEN_OUT/.tizen-virtio-console" "$TIZEN_OUT/.image-ready"
# Preserve resumable progress until the disk has been unmounted successfully.
# Only remove the intermediate partition and markers created by build-root.
state=$TIZEN_OUT/.root-work
rm -f -- "$state"/{emulator-sysdata.img,extracted,opt-merged,original-packages-verified,packages.txt,packages-installed,repositories-ready,repositories-tested,build.lock}
for dir in "$state/sysdata" "$state" "$root"; do
    if [[ -d $dir ]]; then rmdir --ignore-fail-on-non-empty -- "$dir"; fi
done
