#!/usr/bin/env bash
# Build the Ubuntu host package without including an image, SDK, or local state.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
dest=${1:-"$project/dist"}
mkdir -p "$dest"
dest=$(realpath "$dest")
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
kernel_dir=$project/kernel
if [[ ! -f $kernel_dir/kernel.sha256 ]]; then
    kernel_dir=$project/out/share-kernel
    TIZEN_OUT="$kernel_dir" TIZEN_PREBUILT_KERNEL=0 "$project/scripts/build-kernel.sh" --reuse
fi
root=$stage/package
app=$root/usr/lib/tizen-vm
mkdir -p "$root"/{DEBIAN,usr/bin,etc/tizen-vm} "$app"
(cd "$project" && tar --exclude=__pycache__ --exclude='*.pyc' -cf - \
    README.md tizen-vm config cross docs guest kernel packaging scripts sdk tests) |
    tar -xf - -C "$app"
install -m644 "$kernel_dir"/{bzImage,kernel.config,kernel.sha256} "$app/kernel/"
(cd "$app/kernel" && sha256sum --check kernel.sha256)
install -Dm644 "$project/kernel/copyright" "$root/usr/share/doc/tizen-vm/copyright"
ln -s ../lib/tizen-vm/tizen-vm "$root/usr/bin/tizen-vm"
install -m644 "$project/config/image.toml" "$root/etc/tizen-vm/image.toml"
echo /etc/tizen-vm/image.toml > "$root/DEBIAN/conffiles"
depends=$(awk '{for (i=1;i<=NF;i++) {printf "%s%s", sep, ($i=="python3" ? "python3 (>= 3.11)" : $i); sep=", "}}' "$project/config/host-packages")
version=$(cat "$project/config/version")
cat > "$root/DEBIAN/control" <<CONTROL
Package: tizen-vm
Version: $version
Section: devel
Priority: optional
Architecture: amd64
Maintainer: tizen-vm maintainers <36876573+hjhun@users.noreply.github.com>
Homepage: https://github.com/hjhun/tizen-vm
Depends: $depends
Description: Tizen development VM for Ubuntu and WSL2
 Build and run a writable x86_64 system with SMACK, systemd and a native
 development toolchain from dated Tizen snapshots.
CONTROL
dpkg-deb --root-owner-group --build "$root" "$stage/tizen-vm_amd64.deb"
install -m644 "$stage/tizen-vm_amd64.deb" "$dest/tizen-vm_amd64.deb"
