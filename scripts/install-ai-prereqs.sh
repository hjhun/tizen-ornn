#!/usr/bin/env bash
# Install the public x86_64 runtimes. AI clients are installed by each developer.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd -P)
root=$(realpath "${1:?usage: install-ai-prereqs.sh MOUNTED_GUEST}")
[[ $EUID == 0 && $root != / && -x $root/usr/bin/zypper ]]
mountpoint -q "$root"
cache=${TIZEN_OUT:-$project/out}/cache/dev-runtime
mkdir -p "$cache"

fetch() {
    local url=$1 sha=$2 dest="$cache/${1##*/}" tmp
    if [[ -f $dest ]] && printf '%s  %s\n' "$sha" "$dest" | sha256sum -c --status; then return; fi
    tmp=$(mktemp "$dest.XXXXXXXX")
    if curl --fail --location --retry 3 --proto '=https' --proto-redir '=https' -o "$tmp" "$url" &&
        printf '%s  %s\n' "$sha" "$tmp" | sha256sum -c --status; then
        chmod 644 "$tmp"
        mv -f "$tmp" "$dest"
    else
        rm -f "$tmp"
        echo "Could not verify ${url##*/}" >&2
        return 1
    fi
}

node=node-v24.22.3-linux-x64
rg=ripgrep-15.2.0-x86_64-unknown-linux-musl
fetch "https://nodejs.org/dist/v24.22.3/$node.tar.xz" \
    4f2b175a6d53a5c617a80dba2676238a0688cef28c6da4eba35c7d844e66fcfb
fetch "https://github.com/BurntSushi/ripgrep/releases/download/15.2.0/$rg.tar.gz" \
    33e15bcf1624b25cdd2a55813a47a2f95dbe126268203e76aa6a585d1e7b149c
# Tizen's bubblewrap RPM provides its platform-specific isu-sandbox command.
# Build the standard bwrap and socat with the native guest compiler and libc.
bubblewrap=bubblewrap-0.13.1
socat=socat-1.8.1.3
fetch "https://github.com/containers/bubblewrap/releases/download/v0.13.1/$bubblewrap.tar.xz" \
    0e85c8702248b8dd64144f594147553650cdd0e4c951690345faace79c217c4d
fetch "https://deb.debian.org/debian/pool/main/s/socat/socat_1.8.1.3.orig.tar.bz2" \
    25bc6476292b2e614220989c77b0b6fca87bb2525d9747b31a6639b1fb602418
install -d "$root/opt/tizen-dev" "$root/usr/local/bin"
tar -xJf "$cache/$node.tar.xz" -C "$root/opt/tizen-dev" --no-same-owner
tar -xzf "$cache/$rg.tar.gz" -C "$root/opt/tizen-dev" --no-same-owner \
    "$rg/rg" "$rg/COPYING" "$rg/LICENSE-MIT" "$rg/UNLICENSE"
for bin in node npm npx; do
    ln -sfn "/opt/tizen-dev/$node/bin/$bin" "$root/usr/local/bin/$bin"
done
ln -sfn "/opt/tizen-dev/$rg/rg" "$root/usr/local/bin/rg"

build=$(mktemp -d "$root/tmp/ai-tools.XXXXXXXX")
trap 'rm -rf -- "$build"' EXIT
tar -xJf "$cache/$bubblewrap.tar.xz" -C "$build" --no-same-owner
tar -xjf "$cache/socat_1.8.1.3.orig.tar.bz2" -C "$build" --no-same-owner
unshare --mount --propagation private -- bash -se "$root" "${build#"$root"}" "$bubblewrap" "$socat" <<'INSTALL'
set -eu
root=$1
mount -t proc proc "$root/proc"
mount --bind /etc/resolv.conf "$root/etc/resolv.conf"
chroot "$root" /usr/bin/zypper --non-interactive --no-refresh install --no-recommends libcap-devel
chroot "$root" /bin/bash -se "$2" "$3" "$4" <<'BUILD'
set -eu
cd "$1/$2"
meson setup build --buildtype=release -Dtests=false -Dman=disabled \
    -Dbash_completion=disabled -Dzsh_completion=disabled
ninja -C build bwrap
install -m755 build/bwrap /usr/local/bin/
install -d "/usr/local/share/licenses/$2"
install -m644 COPYING COPYING.LIB LICENSE "/usr/local/share/licenses/$2/"
cd "$1/$3"
./configure
make -j4 socat
install -m755 socat /usr/local/bin/
install -d "/usr/local/share/licenses/$3"
install -m644 COPYING COPYING.OpenSSL "/usr/local/share/licenses/$3/"
BUILD
INSTALL

install -Dm755 "$project/guest/tizen-ai-shell" "$root/usr/local/bin/tizen-ai-shell"
install -Dm755 "$project/guest/tizen-ai-install" "$root/usr/local/bin/tizen-ai-install"
install -Dm755 "$project/tests/ai-tools" "$root/usr/local/bin/tizen-ai-check"
for file in "$root/opt/tizen-dev/$node/bin/node" "$root/opt/tizen-dev/$rg/rg" \
            "$root/usr/local/bin/bwrap" "$root/usr/local/bin/socat" \
            "$root/usr/local/bin/tizen-ai-shell" "$root/usr/local/bin/tizen-ai-install" \
            "$root/usr/local/bin/tizen-ai-check"; do
    setfattr -n security.SMACK64 -v _ "$file"
done
