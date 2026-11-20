#!/usr/bin/env bash
# Install native build tools from the development image's pinned RPM repositories.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd -P)
root=$(realpath "${1:?usage: install-linux-tools.sh MOUNTED_GUEST}")
[[ $EUID == 0 && $root != / && -x $root/usr/bin/zypper ]]
mountpoint -q "$root"

unshare --mount --propagation private -- bash -se "$root" <<'INSTALL'
set -eu
root=$1
mount -t proc proc "$root/proc"
mount --bind /etc/resolv.conf "$root/etc/resolv.conf"
chroot "$root" /usr/bin/zypper --non-interactive --no-refresh install --no-recommends \
    meson autoconf automake libtool m4 openssh rsync jq iputils crash-worker-support-regdump
INSTALL

# Tizen ships its SSH client and socket-activated server in the same RPM.
# Keep the client available without opening a new login listener on boot.
unit=$root/etc/systemd/system/sshd.socket
if [[ ! -e $unit && ! -L $unit ]]; then
    ln -s /dev/null "$unit"
fi
install -Dm755 "$project/tests/linux-tools" "$root/usr/local/bin/tizen-linux-tools-check"
setfattr -n security.SMACK64 -v _ "$root/usr/local/bin/tizen-linux-tools-check"
