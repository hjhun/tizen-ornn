#!/usr/bin/env bash
# Build the Ubuntu host package without including an image, SDK, or local state.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
dest=${1:-"$project/dist"}
mkdir -p "$dest"
dest=$(realpath "$dest")
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
kernel_dir=${TIZEN_ORNN_KERNEL_DIR:-$project/kernel}
if [[ ! -f $kernel_dir/kernel.sha256 ]]; then
    kernel_dir=$project/out/share-kernel
    TIZEN_OUT="$kernel_dir" TIZEN_PREBUILT_KERNEL=0 "$project/scripts/build-kernel.sh" --reuse
fi
cargo build --locked --release --manifest-path "$project/Cargo.toml" --bin tizen-ornn
binary_dir=${CARGO_TARGET_DIR:-$project/target}/release
root=$stage/package
app=$root/usr/lib/tizen-ornn
mkdir -p "$root"/{DEBIAN,usr/bin,etc/tizen-ornn} "$app"
(cd "$project" && tar --exclude=__pycache__ --exclude='*.pyc' -cf - \
    LICENSE README.md config cross docs guest kernel packaging scripts sdk tests) |
    tar -xf - -C "$app"
install -m755 "$binary_dir/tizen-ornn" "$app/tizen-ornn"
install -m644 "$kernel_dir"/{bzImage,kernel.config,kernel.sha256} "$app/kernel/"
(cd "$app/kernel" && sha256sum --check kernel.sha256)
install -Dm644 "$project/kernel/copyright" "$root/usr/share/doc/tizen-ornn/copyright"
ln -s ../lib/tizen-ornn/tizen-ornn "$root/usr/bin/tizen-ornn"
install -m644 "$project/config/image.toml" "$root/etc/tizen-ornn/image.toml"
echo /etc/tizen-ornn/image.toml > "$root/DEBIAN/conffiles"
depends=$(awk '{for (i=1;i<=NF;i++) {printf "%s%s", sep, ($i=="python3" ? "python3 (>= 3.11)" : $i); sep=", "}}' "$project/config/host-packages")
version=1:$(cat "$project/config/version")
cat > "$root/DEBIAN/control" <<CONTROL
Package: tizen-ornn
Version: $version
Section: devel
Priority: optional
Architecture: amd64
Maintainer: tizen-ornn maintainers <36876573+hjhun@users.noreply.github.com>
Homepage: https://github.com/hjhun/tizen-ornn
Depends: $depends
Conflicts: tizen-vm
Replaces: tizen-vm
Description: Tizen Ornn development VM for Ubuntu and WSL2
 Build and run a writable x86_64 system with SMACK, systemd and a native
 development toolchain from dated Tizen snapshots.
CONTROL
dpkg-deb --root-owner-group --build "$root" "$stage/tizen-ornn_amd64.deb"
install -m644 "$stage/tizen-ornn_amd64.deb" "$dest/tizen-ornn_amd64.deb"

if [[ ${2:-} == --desktop ]]; then
    cargo build --locked --release --manifest-path "$project/Cargo.toml" --features desktop --bin tizen-ornn-gui
    gui=$stage/desktop
    mkdir -p "$gui/DEBIAN"
    install -Dm755 "$binary_dir/tizen-ornn-gui" "$gui/usr/lib/tizen-ornn/tizen-ornn-gui"
    install -Dm644 "$project/LICENSE" "$gui/usr/share/doc/tizen-ornn-gui/LICENSE"
    install -Dm644 "$project/packaging/art/fonts/OFL.txt" "$gui/usr/share/doc/tizen-ornn-gui/Barlow-OFL.txt"
    mkdir -p "$gui/usr/bin"
    ln -s ../lib/tizen-ornn/tizen-ornn-gui "$gui/usr/bin/tizen-ornn-gui"
    install -Dm644 "$project/packaging/art/tizen-ornn.png" \
        "$gui/usr/share/icons/hicolor/256x256/apps/tizen-ornn.png"
    install -Dm644 "$project/packaging/tizen-ornn.desktop" \
        "$gui/usr/share/applications/tizen-ornn.desktop"
    cat > "$gui/DEBIAN/control" <<CONTROL
Package: tizen-ornn-gui
Version: $version
Architecture: amd64
Section: devel
Priority: optional
Maintainer: Tizen Ornn maintainers <36876573+hjhun@users.noreply.github.com>
Depends: tizen-ornn (= $version), x-terminal-emulator, qemu-system-gui, python3-gi, gir1.2-ayatanaappindicator3-0.1, gir1.2-gtk-3.0, libgl1, libegl1, libxkbcommon0, libx11-6
Description: Graphical image launcher for Tizen Ornn
 Select a development image and open its guest console.
CONTROL
    dpkg-deb --root-owner-group --build "$gui" "$dest/tizen-ornn-gui_amd64.deb"
fi
