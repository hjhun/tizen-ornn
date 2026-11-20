#!/usr/bin/env bash
# Native build dependencies of aul-1 in the pinned Tizen 10.1 emulator snapshot.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd -P)
root=$(realpath "${1:?usage: install-aul-devel.sh MOUNTED_GUEST}")
[[ $EUID == 0 && $root != / && -x "$root/usr/bin/zypper" ]]
mountpoint -q "$root"

unshare --mount --propagation private -- bash -se "$root" <<'INSTALL'
set -euo pipefail
root=$1
mount -t proc proc "$root/proc"
mount --bind /etc/resolv.conf "$root/etc/resolv.conf"
deps=(cmake libattr-devel xdgmime-devel sqlite)
for pc in bundle capi-system-info capi-system-resource dlog gio-2.0 glib-2.0 \
    gmock cynara-uid libsmack libtzplatform-config libxml-2.0 parcel pkgmgr-info \
    sqlite3 ttrace uuid vconf xdgmime libunwind libunwind-ptrace; do
    deps+=("pkgconfig($pc)")
done
chroot "$root" /usr/bin/zypper --non-interactive --no-refresh install --no-recommends "${deps[@]}"
INSTALL
install -Dm755 "$project/guest/tizen-build-aul" "$root/usr/local/bin/tizen-build-aul"
setfattr -n security.SMACK64 -v _ "$root/usr/local/bin/tizen-build-aul"
