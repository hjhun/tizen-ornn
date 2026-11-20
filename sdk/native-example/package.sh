#!/bin/sh
# Run inside the development image. Only the verified final TPK is published.
set -eu
if [ "$#" -gt 2 ]; then
    echo "Usage: $0 [OUTPUT_DIR [TIZEN_EXECUTABLE]]" >&2
    exit 2
fi
sample=$(cd "$(dirname "$0")" && pwd)
dest=${1:-"$PWD"}
mkdir -p "$dest"
dest=$(cd "$dest" && pwd)
work=$(mktemp -d "$dest/.tizen-package.XXXXXXXX")
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/project/Debug" "$work/result"
cp "$sample/.project" "$sample/project_def.prop" "$sample/tizen-manifest.xml" \
    "$work/project/"
if [ "$#" -eq 2 ]; then
    # Copy before changing directory so callers can supply a relative Cargo path.
    install -m 0755 -- "$2" "$work/project/Debug/tizenprobe"
else
    c++ "$sample/tizenprobe.cc" -o "$work/project/Debug/tizenprobe"
    "$work/project/Debug/tizenprobe"
fi
cd "$work/project"
LC_ALL=C readelf -h Debug/tizenprobe >"$work/elf-header"
arch=$(LC_ALL=C awk '
    $1 == "Class:" { bits = $2 }
    $1 == "Machine:" && /X86-64/ { machine = "x86_64" }
    $1 == "Machine:" && $2 == "ARM" { machine = "arm" }
    $1 == "Flags:" && /Version5 EABI/ && /soft-float ABI/ { arm_eabi = 1 }
    $1 == "Type:" && ($2 == "EXEC" || $2 == "DYN") { executable = 1 }
    /Entry point address:/ && $NF ~ /^0x0*[1-9a-fA-F]/ { entry = 1 }
    END {
        if (executable && entry && bits == "ELF64" && machine == "x86_64") print machine
        if (executable && entry && bits == "ELF32" && machine == "arm" && arm_eabi) print machine
    }
' "$work/elf-header")
if [ -z "$arch" ]; then
    echo "Expected a Tizen x86_64 or ARM EABI5 soft-float executable" >&2
    exit 1
fi
if [ "$arch" = arm ]; then
    LC_ALL=C readelf -A Debug/tizenprobe | grep -Eq 'Tag_CPU_arch: v7$'
fi
cat >Debug/build.info <<INFO
project-path=$PWD
profile=tizen
profile-version=10.0
architecture=$arch
toolchain=gcc
config=Debug
type=app
INFO
# -S configures stripping. The SDK uses its emulator certificates by default.
tizen package -t tpk -S off -o "$work/result" -- "$PWD/Debug"
pkg=org.example.tizendev-1.0.0-$arch.tpk
artifact=$work/result/$pkg
unzip -tq "$artifact"
unzip -p "$artifact" bin/tizenprobe >"$work/packaged-elf"
test "$(sha256sum <Debug/tizenprobe)" = "$(sha256sum <"$work/packaged-elf")"
for entry in tizen-manifest.xml author-signature.xml signature1.xml; do
    unzip -p "$artifact" "$entry" >"$work/entry"
    test -s "$work/entry"
done
cp "$artifact" "$dest/$pkg"
printf '%s\n' "$dest/$pkg"
