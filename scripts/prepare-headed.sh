#!/usr/bin/env bash
# Install the Tizen Wayland developer session in a mounted emulator filesystem.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
root=$(realpath "${1:?usage: prepare-headed.sh MOUNTED_ROOT}")
[[ $EUID == 0 && "$root" != / && -x "$root/usr/bin/zypper" ]]
mountpoint -q "$root"
unshare --mount --propagation private -- bash -se "$root" <<'INSTALL'
set -eu
root=$1
mount -t proc proc "$root/proc"
mount --bind /etc/resolv.conf "$root/etc/resolv.conf"
chroot "$root" /usr/bin/zypper --non-interactive --no-refresh install --no-recommends \
    elementary-devel efl-devel wayland-devel patchelf
chroot "$root" /usr/bin/pkg-config --modversion elementary
INSTALL
units="$root/etc/systemd/system"
for unit in tizen-dev-gui.target tizen-headed-runtime.service tizen-headed-bus.service tizen-ui-demo.service; do
    install -D -m 0644 "$project/guest/$unit" "$units/$unit"
done
install -D -m 0644 "$project/guest/tizen-headed-display.conf" \
    "$units/display-manager.service.d/tizen-dev.conf"
install -D -m 0755 "$project/guest/tizen-headed-runtime" "$root/usr/local/libexec/tizen-headed-runtime"
install -D -m 0755 "$project/guest/tizen-ui-run" "$root/usr/local/bin/tizen-ui-run"
sample=/usr/share/tizen-dev/examples/efl-counter
install -d "$root$sample"
for file in main.c .project project_def.prop tizen-manifest.xml; do
    install -m 0644 "$project/guest/efl-example/$file" "$root$sample/$file"
done
install -m 0755 "$project/guest/efl-example/build.sh" "$root$sample/build.sh"
chroot "$root" /bin/sh -ec \
    'cc -std=gnu11 "$1/main.c" -o /usr/local/libexec/tizen-ui-counter $(pkg-config --cflags --libs elementary)' \
    sh "$sample"
