#!/usr/bin/env bash
# Update a checkout of the apt branch with the matching GitHub release DEBs.
set -euo pipefail
assets=$(realpath "${1:?release assets directory required}")
repo=$(realpath "${2:?APT repository directory required}")
project=$(cd "$(dirname "$0")/../.." && pwd)
version=$(cat "$project/config/version")
fingerprint=8F8759D75EA90E3F0E22312BD732D10D8D43FD25
: "${GNUPGHOME:?set GNUPGHOME to the archive signing keyring}"
cd "$repo"
pool=pool/main/t/tizen-ornn
suite=dists/stable
index=$suite/main/binary-amd64
mkdir -p "$pool" "$index/by-hash/SHA256"
for name in tizen-ornn tizen-ornn-gui; do
    deb="$assets/${name}_amd64.deb"
    [[ $(dpkg-deb -f "$deb" Package) == "$name" &&
       $(dpkg-deb -f "$deb" Version) == "$version" &&
       $(dpkg-deb -f "$deb" Architecture) == amd64 ]]
    dest="$pool/${name}_${version}_amd64.deb"
    if [[ -e $dest ]]; then cmp "$deb" "$dest"; else install -m644 "$deb" "$dest"; fi
done
apt-ftparchive packages pool > "$index/Packages"
gzip -n -c "$index/Packages" > "$index/Packages.gz"
for file in "$index/Packages" "$index/Packages.gz"; do
    digest=$(sha256sum "$file" | cut -d' ' -f1)
    cp "$file" "$index/by-hash/SHA256/$digest"
done
rm -f -- "$suite/Release"
apt-ftparchive --no-md5 --no-sha1 --no-sha512 \
    -o APT::FTPArchive::Release::Origin='Tizen Ornn' \
    -o APT::FTPArchive::Release::Label='Tizen Ornn' \
    -o APT::FTPArchive::Release::Suite=stable \
    -o APT::FTPArchive::Release::Codename=stable \
    -o APT::FTPArchive::Release::Architectures=amd64 \
    -o APT::FTPArchive::Release::Components=main \
    -o APT::FTPArchive::Release::Acquire-By-Hash=yes \
    release "$suite" > "$repo/Release.new"
mv "$repo/Release.new" "$suite/Release"
gpg --batch --yes --local-user "$fingerprint" --clearsign -o "$suite/InRelease" "$suite/Release"
gpg --batch --yes --local-user "$fingerprint" --armor --detach-sign -o "$suite/Release.gpg" "$suite/Release"
