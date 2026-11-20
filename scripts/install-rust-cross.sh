#!/usr/bin/env bash
# Install only the target rlibs from the matching ARM Tizen compiler build.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
root=${1:?usage: install-rust-cross.sh MOUNTED_ROOT}
mountpoint -q "$root"
[[ $EUID == 0 && "$(realpath "$root")" != / ]]
target=arm-unknown-linux-gnueabi
cache=${TIZEN_OUT:-$project/out}/cache/rust
fields=$(python3 "$project/scripts/snapshot_lock.py" rust)
mapfile -t fields <<< "$fields"
sha=${fields[0]}
url=${fields[1]}
version=${fields[2]}
file=${url##*/}
mkdir -p "$cache"
cache=$(realpath "$cache")
work=$(mktemp -d "$cache/.rust-XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT
if [[ ! -e "$cache/$file" ]]; then
    curl -fLSs --retry 3 "$url" -o "$work/$file"
    echo "$sha  $work/$file" | sha256sum -c -
    mv "$work/$file" "$cache/$file"
fi
echo "$sha  $cache/$file" | sha256sum -c -
if [[ $(chroot "$root" /bin/rpm -q --qf '%{VERSION}' rust) != "$version" ]]; then
    echo "ARM rust-std-static requires native rust $version; check the selected snapshot." >&2
    exit 1
fi
libdir=$(chroot "$root" /usr/bin/rustc --print target-libdir --target "$target")
[[ $libdir == /usr/lib*/rustlib/"$target"/lib ]]
(
    cd "$work"
    rpm2cpio "$cache/$file" | cpio --quiet -idm "./usr/lib/rustlib/$target/lib/*" './usr/share/licenses/*/*'
)
install -d "$root$libdir" "$root/usr/share/licenses/tizen-arm-rust-std"
cp -a "$work/usr/lib/rustlib/$target/lib/." "$root$libdir/"
cp -a "$work/usr/share/licenses/rust-std-static/." "$root/usr/share/licenses/tizen-arm-rust-std/"
install -m 0644 "$project/guest/tizen-cargo-arm.sh" "$root/etc/profile.d/tizen-cargo-arm.sh"
