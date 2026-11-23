#!/usr/bin/env bash
# Called by build-root.sh while the target filesystem is mounted.
set -euo pipefail
root=$1
cache=$2
work=$3
here=$(cd "$(dirname "$0")" && pwd -P)
if [[ ! -e "$work/packages-installed" ]]; then
    "$here/snapshot_rpms.py" "$root" "$cache" "$work/packages.txt"
    unshare --mount --propagation private -- bash -se "$root" "$cache" "$work/packages.txt" <<'INSTALL'
set -euo pipefail
root=$1
cache=$2
manifest=$3
mkdir -p "$root/mnt/bootstrap-rpms" "$root/proc"
mount -t proc proc "$root/proc"
mount --bind "$cache/rpms" "$root/mnt/bootstrap-rpms"
mount -o remount,bind,ro "$root/mnt/bootstrap-rpms"
rpms=()
while IFS= read -r filename; do
    rpms+=("/mnt/bootstrap-rpms/$filename")
done < "$manifest"
if (( ${#rpms[@]} )); then
    chroot "$root" /bin/rpm -U --test "${rpms[@]}"
    chroot "$root" /bin/rpm -Uvh "${rpms[@]}"
fi
INSTALL
    rmdir "$root/mnt/bootstrap-rpms"
    touch "$work/packages-installed"
fi
python3 "$here/build_ca.py" --root "$root"
if [[ ! -e "$work/repositories-ready" ]]; then
    PYTHONDONTWRITEBYTECODE=1 "$here/prepare-zypp.py" "$root"
    install -m 755 "$here/tizen-refresh-pinned" "$root/usr/bin/tizen-refresh-pinned"
    setfattr -n security.SMACK64 -v _ "$root/usr/bin/tizen-refresh-pinned"
    chroot "$root" /bin/rpm --import /usr/share/tizen-snapshot/snapshot-index.asc
    chroot "$root" /usr/bin/tizen-refresh-pinned
    touch "$work/repositories-ready"
fi
# Exercise one uncached package from each pinned HTTPS repository on a fresh build.
# The guest later receives its own /etc/resolv.conf from its runtime.
if [[ ! -e "$work/repositories-tested" ]]; then
    unshare --mount --propagation private -- bash -se "$root" <<'VERIFY'
set -euo pipefail
root=$1
mount -t proc proc "$root/proc"
mount --bind /etc/resolv.conf "$root/etc/resolv.conf"
chroot "$root" /usr/bin/zypper --non-interactive --no-refresh install --no-recommends bc ltrace
chroot "$root" /bin/rpm -q bc ltrace
test "$(chroot "$root" /bin/sh -c "echo '12 * 12' | bc")" = 144
VERIFY
    touch "$work/repositories-tested"
fi
