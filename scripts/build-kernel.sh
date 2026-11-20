#!/usr/bin/env bash
# Build the x86_64 guest kernel from the Tizen 10.1 RPi source RPM.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
out=${TIZEN_OUT:-"$root/out"}
if [[ ${TIZEN_PREBUILT_KERNEL:-0} == 1 ]]; then
    mkdir -p "$out"
    stage=$(mktemp -d "$out/.kernel.XXXXXXXX")
    trap 'rm -rf -- "$stage"' EXIT
    release=https://github.com/hjhun/tizen-vm/releases/download/10.1-r4
    for file in bzImage kernel.config; do
        curl -fLsS --retry 2 "$release/$file" -o "$stage/$file"
    done
    (cd "$stage" && sha256sum -c "$root/kernel/r4.sha256")
    mv -fT "$stage/kernel.config" "$out/kernel.config"
    mv -fT "$stage/bzImage" "$out/bzImage"
    exit 0
fi
mkdir -p "$out/work/kernel-6.12.41"
out=$(cd "$out" && pwd)
work=$out/work/kernel-6.12.41
fragment=$root/kernel/qemu-x86_64.config
revision=b6322b453844b6111e4619615fc65bd29b86c83e
sha256=f29de2d367d165832402332f73e955cc9a231cd2b0b534e3b8048aea67a4a384
snapshot=tizen-10.1-unified_20261119.053314
if [[ -n ${TIZEN_SNAPSHOT_LOCK:-} || -f $out/snapshot.lock.json ]]; then
    source_url=$(python3 "$root/scripts/snapshot_lock.py" source)
else
    source_url=$("$root/scripts/tizen-image" --print-source)
fi
url=${source_url}Tizen-10.1-Unified/$snapshot/repos/standard/source/rpi4-linux-kernel-6.12.41-0.src.rpm
jobs=${JOBS:-$(nproc)}

exec 9>"$work/build.lock"
flock 9
stage=$(mktemp -d "$work/staging.XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT
if [[ ! -f $work/source.rpm ]]; then
    curl --fail --location --retry 2 "$url" -o "$stage/source.rpm"
    printf '%s  %s\n' "$sha256" "$stage/source.rpm" | sha256sum -c -
    mv "$stage/source.rpm" "$work/source.rpm"
fi
printf '%s  %s\n' "$sha256" "$work/source.rpm" | sha256sum -c -
if [[ $(rpm -qp --qf '%{VCS}' "$work/source.rpm") != \
    "platform/kernel/linux-rpi#$revision" ]]; then
    echo 'Unexpected source RPM revision' >&2
    exit 1
fi

if [[ ! -f $work/src/.tizen-source-$revision ]]; then
    if [[ -e $work/src ]]; then
        echo "Unrecognized kernel source directory: $work/src" >&2
        exit 1
    fi
    mkdir "$stage/srpm" "$stage/src"
    (
        cd "$stage/srpm"
        rpm2cpio "$work/source.rpm" | cpio --quiet -id
        tar -xJf linux-kernel-6.12.41.tar.xz -C ../src --strip-components=1
        gzip -dc 8f5ff97-to-ea12eac.diff.gz | patch -s -d ../src -p1
        for patchfile in 0*.patch; do
            patch -s -d ../src -p1 <"$patchfile"
        done
    )
    touch "$stage/src/.tizen-source-$revision"
    mv "$stage/src" "$work/src"
fi

# On Ubuntu a private libelf-dev extraction suffices for the host objtool.
if ! printf '#include <gelf.h>\n' | ${HOSTCC:-cc} -E -x c - >/dev/null 2>&1; then
    multiarch=$(dpkg-architecture -qDEB_HOST_MULTIARCH)
    if [[ ! -f $work/host-dev/usr/include/gelf.h ]]; then
        mkdir "$stage/host-dev"
        (
            cd "$stage/host-dev"
            apt-get download libelf-dev
            dpkg-deb -x ./libelf-dev*.deb .
            ln -sf "/usr/lib/$multiarch/libelf.so.1" \
                "usr/lib/$multiarch/libelf.so"
        )
        mv "$stage/host-dev" "$work/host-dev"
    fi
    export C_INCLUDE_PATH="$work/host-dev/usr/include${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}"
    export LIBRARY_PATH="$work/host-dev/usr/lib/$multiarch${LIBRARY_PATH:+:$LIBRARY_PATH}"
fi

kmake=(make -C "$work/src" O="$work/build" ARCH=x86_64)
"${kmake[@]}" KCONFIG_ALLCONFIG="$fragment" allnoconfig
# allnoconfig drops values whose Kconfig dependencies are unmet.
missing=$(LC_ALL=C comm -23 <(grep '^CONFIG_' "$fragment" | LC_ALL=C sort) \
    <(grep '^CONFIG_' "$work/build/.config" | LC_ALL=C sort))
if [[ -n $missing ]]; then
    printf 'Unresolved kernel configuration:\n%s\n' "$missing" >&2
    exit 1
fi
"${kmake[@]}" -j"$jobs" bzImage
install -m 0644 "$work/build/arch/x86/boot/bzImage" "$stage/bzImage"
install -m 0644 "$work/build/.config" "$stage/kernel.config"
mv -fT "$stage/kernel.config" "$out/kernel.config"
mv -fT "$stage/bzImage" "$out/bzImage"
sha256sum "$out/bzImage"
