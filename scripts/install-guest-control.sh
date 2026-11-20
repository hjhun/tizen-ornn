#!/usr/bin/env bash
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
root=${1:?usage: install-guest-control.sh MOUNTED_GUEST}
[[ $EUID == 0 && "$(realpath -- "$root")" != / && -x "$root/usr/bin/zypper" ]]
mountpoint -q "$root"
install -Dm755 "$project/guest/tizen-guest-control" "$root/usr/local/libexec/tizen-guest-control"
install -Dm644 "$project/guest/tizen-guest-control.service" "$root/etc/systemd/system/tizen-guest-control.service"
install -Dm644 "$project/guest/90-tizen-vm-packages.rules" "$root/etc/udev/rules.d/90-tizen-vm-packages.rules"
install -Dm755 "$project/guest/tizen-update-rpms" "$root/usr/local/libexec/tizen-update-rpms"
for file in "$root/usr/local/libexec/tizen-guest-control" "$root/usr/local/libexec/tizen-update-rpms" \
    "$root/etc/systemd/system/tizen-guest-control.service" "$root/etc/udev/rules.d/90-tizen-vm-packages.rules"; do
    setfattr -n security.SMACK64 -v _ "$file"
done
