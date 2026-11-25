#!/usr/bin/env bash
# Stage Tizen ARM link inputs separately from the guest's native RPM database.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd -P)
root=$(realpath "${1:?mounted Tizen root required}")
[[ $EUID == 0 && $root != / && -x "$root/usr/bin/zypper" ]]
mountpoint -q "$root"
cache=${CROSS_RPM_CACHE:-"${TIZEN_OUT:-$project/out}/cache/cross"}
mkdir -p "$cache" "$root/opt/tizen/sysroots"
stage_sysroot() (
arch=$1
lib=$2
loader=$3
section=$4
stage=$(mktemp -d "$root/opt/tizen/sysroots/.$arch.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT
python3 "$project/scripts/snapshot_lock.py" "$section" > "$stage/packages.sha256"
while read -r sha url; do
    name=${url##*/}
    rpmfile=$cache/$name
    if [[ ! -f "$rpmfile" ]]; then
        curl -fsSL --retry 3 "$url" -o "$rpmfile.part"
        echo "$sha  $rpmfile.part" | sha256sum -c -
        mv "$rpmfile.part" "$rpmfile"
    fi
    echo "$sha  $rpmfile" | sha256sum -c -
    # GCC's ARM runtime RPM also contains ARM-hosted compiler executables.
    # Select only start objects, compiler support headers and linker inputs.
    if [[ "$name" == gcc-[0-9]* ]]; then
        inputs=("./usr/$lib/gcc/*/*/crt*.o" "./usr/$lib/gcc/*/*/include/*"
                "./usr/$lib/libgcc_s.so" "./usr/$lib/libstdc++.so")
    else
        inputs=('./lib/*' './usr/include/*' './usr/lib/*' './usr/share/pkgconfig/*')
        if [[ $arch == aarch64 ]]; then inputs+=('./lib64/*' './usr/lib64/*'); fi
    fi
    (cd "$stage"; rpm2cpio "$rpmfile" | cpio --quiet -idmu --no-absolute-filenames "${inputs[@]}")
done < "$stage/packages.sha256"
# Absolute target links must resolve within this movable sysroot on the host.
python3 - "$stage" <<'PY'
import os
import sys
from pathlib import Path
root = Path(sys.argv[1])
for path in root.rglob('*'):
    if path.is_symlink():
        target = os.readlink(path)
        if target.startswith('/'):
            relative = os.path.relpath(root / target.lstrip('/'), path.parent)
            path.unlink()
            path.symlink_to(relative)
PY
test -s "$stage/$loader"
test -f "$stage/usr/include/glib-2.0/glib.h"
sysroot="$root/opt/tizen/sysroots/$arch"
chmod 755 "$stage"
if [[ -d "$sysroot" ]]; then
    test -f "$sysroot/packages.sha256" || {
        echo "Refusing to replace an unmanaged ARM sysroot: $sysroot" >&2
        exit 1
    }
    # Both trees are on the same ext4 filesystem. Exchange their names so an
    # interrupted update never exposes an incomplete sysroot.
    python3 - "$stage" "$sysroot" <<'PYTHON'
import ctypes
import os
import sys
libc = ctypes.CDLL(None, use_errno=True)
src, dest = map(os.fsencode, sys.argv[1:])
if libc.renameat2(-100, src, -100, dest, 2) != 0:
    err = ctypes.get_errno()
    raise OSError(err, os.strerror(err))
PYTHON
else
    mv "$stage" "$sysroot"
fi
)
stage_sysroot armv7l lib lib/ld-linux.so.3 cross
stage_sysroot aarch64 lib64 lib/ld-linux-aarch64.so.1 cross_aarch64
unshare --mount --propagation private -- bash -se "$root" <<'INSTALL'
set -eu
root=$1
mount -t proc proc "$root/proc"
mount --bind /etc/resolv.conf "$root/etc/resolv.conf"
chroot "$root" /usr/bin/zypper --non-interactive --no-refresh install \
    --no-recommends gcc-armv7l binutils-armv7l gcc-aarch64 binutils-aarch64 qemu-linux-user cpio
INSTALL
for arch in armv7l aarch64; do
    if [[ $arch == armv7l ]]; then triple=armv7l-tizen-linux-gnueabi
    else triple=aarch64-tizen-linux-gnu; fi
    install -m 755 "$project/cross/target-tool" "$root/usr/bin/tizen-$arch-gcc"
    for tool in g++ pkg-config; do
        ln -sfn "tizen-$arch-gcc" "$root/usr/bin/tizen-$arch-$tool"
    done
    bin="$root/opt/tizen/toolchains/$arch/bin"
    install -d "$bin"
    for tool in gcc g++ cc c++ pkg-config; do
        ln -sfn /usr/bin/tizen-$arch-gcc "$bin/$tool"
    done
    for tool in as ld ar nm ranlib strip objcopy objdump readelf; do
        ln -sfn "/usr/bin/$triple-$tool" "$bin/$tool"
    done
    install -Dm644 "$project/cross/$arch.cmake" "$root/usr/share/tizen-dev/cmake/$arch.cmake"
done
install -m 755 "$project/cross/tizen-rpmbuild" "$root/usr/bin/tizen-rpmbuild"
example="$root/usr/share/tizen-dev/examples/cross-rpm"
install -d "$example"
install -m 644 "$project/tests/cross-rpm/"* "$example/"
install -m 755 "$project/tests/cross-check" "$root/usr/local/bin/tizen-cross-check"
