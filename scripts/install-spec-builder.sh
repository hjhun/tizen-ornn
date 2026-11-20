#!/usr/bin/env bash
# Build with the guest's RPM macros and install the same package we distribute.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd -P)
root=$(realpath "${1:?usage: install-spec-builder.sh MOUNTED_GUEST [RPM_OUTPUT_DIR]}")
[[ $EUID == 0 && $root != / && -x $root/usr/bin/rpmbuild ]]
mountpoint -q "$root"
work=$(mktemp -d "$root/tmp/tizen-build-rpm.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT
top=${work#"$root"}
install -Dm644 "$project/guest/tizen-build" "$work/SOURCES/tizen-build"
install -Dm644 "$project/packaging/tizen-build/tizen-build.spec" "$work/SPECS/tizen-build.spec"
unshare --mount --propagation private -- bash -se "$root" "$top" <<'BUILD'
set -eu
root=$1
top=$2
mount -t proc proc "$root/proc"
mount --bind /etc/resolv.conf "$root/etc/resolv.conf"
chroot "$root" /usr/bin/zypper --non-interactive --no-refresh install --no-recommends python3-rpm
chroot "$root" /usr/bin/rpmbuild -bb --define "_topdir $top" "$top/SPECS/tizen-build.spec"
BUILD
rpms=("$work"/RPMS/noarch/*.rpm)
[[ ${#rpms[@]} == 1 && -s ${rpms[0]} ]]
chroot "$root" /bin/rpm -U --replacepkgs "${rpms[0]#"$root"}"
setfattr -n security.SMACK64 -v _ "$root/usr/bin/tizen-build"
if [[ $# -gt 1 ]]; then
    install -Dm644 "${rpms[0]}" "$2/${rpms[0]##*/}"
fi
