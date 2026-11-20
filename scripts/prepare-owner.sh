#!/usr/bin/env bash
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
root=$(realpath "${1:?usage: prepare-owner.sh MOUNTED_GUEST}")
[[ $EUID == 0 && $root != / && -x $root/usr/bin/rpm ]]
mountpoint -q "$root"

IFS=: read -r _ _ uid gid _ home _ < <(chroot "$root" getent passwd owner)
[[ $home == /opt/usr/home/owner && -d $root$home ]]
# Keep Tizen's User::Home and application data intact. Only developer-owned
# directories inherit the SMACK label of the local developer shell.
for dir in workspace .cargo rpmbuild .local .tizen; do
    path=$root$home/$dir
    [[ ! -L $path ]]
    install -d -m 700 -o "$uid" -g "$gid" "$path"
    setfattr -n security.SMACK64 -v 'User::Shell' "$path"
    setfattr -n security.SMACK64TRANSMUTE -v TRUE "$path"
done
install -m 644 "$project/guest/tizen-owner-profile.sh" \
    "$root/etc/profile.d/zz-tizen-owner.sh"
