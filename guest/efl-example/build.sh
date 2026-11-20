#!/bin/sh
set -eu
dest=${1:-"$PWD"}
sample=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$dest"
dest=$(cd "$dest" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/project/Debug" "$work/output"
cp "$sample/.project" "$sample/project_def.prop" "$sample/tizen-manifest.xml" "$work/project/"
cd "$work/project"
cc -std=gnu11 "$sample/main.c" -o Debug/tizen-ui $(pkg-config --cflags --libs elementary)
cat >Debug/build.info <<INFO
project-path=$PWD
profile=tizen
profile-version=10.0
architecture=x86_64
toolchain=gcc
config=Debug
type=app
INFO
tizen package -t tpk -S off -o "$work/output" -- "$PWD/Debug"
tpk="$work/output/org.example.tizencounter-1.0.0-x86_64.tpk"
unzip -tq "$tpk"
unzip -p "$tpk" bin/tizen-ui | cmp - Debug/tizen-ui
for entry in author-signature.xml signature1.xml tizen-manifest.xml; do
    unzip -p "$tpk" "$entry" >"$work/entry"
    test -s "$work/entry"
done
install -m 0755 Debug/tizen-ui "$dest/tizen-ui"
install -m 0644 "$tpk" "$dest/"
printf '%s\n' "$dest/tizen-ui" "$dest/${tpk##*/}"
