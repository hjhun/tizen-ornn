#!/usr/bin/env bash
# Build the same UI application for either image-supported Tizen ABI.
set -euo pipefail
arch=${1:?usage: build-rpm.sh ARCH OUTPUT_DIR}
out=${2:?usage: build-rpm.sh ARCH OUTPUT_DIR}
case "$arch" in x86_64|armv7l) ;; *) exit 2 ;; esac
sample=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out"
out=$(cd "$out" && pwd -P)
top=$(mktemp -d "$out/.tizen-build.XXXXXXXX")
trap 'rm -rf -- "$top"' EXIT
mkdir -p "$top"/{BUILD,BUILDROOT,RPMS,SRPMS}
tizen-rpmbuild "$arch" --define "_topdir $top" --define "_sourcedir $sample" \
    -bb "$sample/tizen-ui-counter.spec"
rpmfile=$top/RPMS/$arch/tizen-ui-counter-1.0-1.$arch.rpm
test "$(rpm -qp --qf '%{ARCH}' "$rpmfile")" = "$arch"
mkdir "$top/payload"
(cd "$top/payload"; rpm2cpio "$rpmfile" | cpio --quiet -id)
elf=$top/payload/usr/bin/tizen-ui
case "$arch" in
    armv7l)
        readelf -h "$elf" | grep -E 'Machine:.*ARM'
        readelf -A "$elf" | grep -E 'Tag_CPU_arch: v7$'
        ;;
    x86_64) readelf -h "$elf" | grep -E 'Machine:.*X86-64' ;;
esac
readelf -d "$elf" | grep -F '[libelementary.so.1]'
install -m 644 "$rpmfile" "$out/"
sha256sum "$out/${rpmfile##*/}"
