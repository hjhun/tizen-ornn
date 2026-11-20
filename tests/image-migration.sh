#!/usr/bin/env bash
# Run as root. Use small, disposable ext4 images for real rsync/xattr checks.
set -euo pipefail
if [[ ${1:-} != --private ]]; then
    exec unshare --mount --propagation private -- "$0" --private
fi
project=$(cd "$(dirname "$0")/.." && pwd)
if ! command -v qemu-img >/dev/null && [[ -x $project/out/host-tools/usr/bin/qemu-img ]]; then
    host=$project/out/host-tools/usr
    export PATH="$host/bin:$PATH"
    export LD_LIBRARY_PATH="$host/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
temp=$(mktemp -d)
mnt=$temp/mnt
mkdir "$mnt"
cleanup() {
    if mountpoint -q "$mnt"; then umount "$mnt" || return; fi
    rm -rf -- "$temp"
}
trap cleanup EXIT

new_disk() {
    truncate -s 32M "$1"
    mkfs.ext4 -q -F "$1"
    mount -o loop "$1" "$mnt"
    mkdir -p "$mnt"/{opt/usr/{home,data},opt/dbspace,root,var/lib/rpm,usr/bin}
}

old=$temp/old.ext4
new_disk "$old"
ln -s opt/usr/home "$mnt/home"
p=$mnt/opt/usr/home/owner/workspace
mkdir -p "$p" "$mnt/opt/usr/home/dev"
echo project > "$p/.code"
ln "$p/.code" "$p/hardlink"
ln -s .code "$p/symlink"
chmod 0640 "$p/.code"
chown -R 5001:5001 "$p"
setfattr -n security.SMACK64 -v 'User::Shell' "$p"
setfattr -n user.migration -v present "$p/.code"
echo dev > "$mnt/opt/usr/home/dev/file"
echo userdata > "$mnt/opt/usr/data/service"
echo root > "$mnt/root/.profile-data"
for path in opt/dbspace/.pkgmgr_parser.db var/lib/rpm/Packages usr/bin/example; do
    echo previous > "$mnt/$path"
done
umount "$mnt"
qemu-img convert -f raw -O qcow2 "$old" "$temp/old.qcow2"

cp --sparse=always "$old" "$temp/unclean.ext4"
debugfs -w -R 'set_super_value state 0' "$temp/unclean.ext4"

for disk in "$old" "$temp/old.qcow2" "$temp/unclean.ext4"; do
    new=$temp/new.ext4
    new_disk "$new"
    if [[ $disk == *.qcow2 ]]; then
        mkdir "$mnt/home"
        home_path=/home
    else
        ln -s /opt/usr/home "$mnt/home"
        home_path=/opt/usr/home
    fi
    echo vendor > "$mnt/opt/usr/data/new-default"
    for path in opt/dbspace/.pkgmgr_parser.db var/lib/rpm/Packages usr/bin/example; do
        echo fresh > "$mnt/$path"
    done
    umount "$mnt"
    before=$(sha256sum "$disk")
    "$project/scripts/copy-workspace.sh" "$disk" "$new"
    [[ $before == "$(sha256sum "$disk")" ]]
    mount -o loop,ro,noload "$new" "$mnt"
    p=$mnt$home_path/owner/workspace
    [[ $(cat "$p/symlink") == project && "$p/hardlink" -ef "$p/.code" ]]
    [[ $(stat -c '%a %u %g' "$p/.code") == '640 5001 5001' ]]
    [[ $(getfattr --only-values -n user.migration "$p/.code" 2>/dev/null) == present ]]
    [[ $(getfattr --only-values -n security.SMACK64 "$p" 2>/dev/null) == 'User::Shell' ]]
    [[ $(cat "$mnt$home_path/dev/file") == dev ]]
    [[ $(cat "$mnt/opt/usr/data/service") == userdata ]]
    [[ $(cat "$mnt/opt/usr/data/new-default") == vendor ]]
    [[ $(cat "$mnt/root/.profile-data") == root ]]
    for path in opt/dbspace/.pkgmgr_parser.db var/lib/rpm/Packages usr/bin/example; do
        [[ $(cat "$mnt/$path") == fresh ]]
    done
    if [[ $home_path == /home ]]; then [[ ! -e $mnt/opt/usr/home/owner ]]; fi
    umount "$mnt"
    echo "$(basename "$disk") passed: user and root files, home layout, SMACK, links, pristine system and previous disk"
done
