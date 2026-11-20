#!/usr/bin/env bash
# Sign the local package and its RPM-MD index with one temporary image-build key.
set -euo pipefail
repo=$(realpath "${1:?local repository directory required}")
rpmfile="$repo/x86_64/git-2.56.1-1.x86_64.rpm"
test -s "$rpmfile"
keyring=$(mktemp -d)
trap 'gpgconf --homedir "$keyring" --kill all; rm -r -- "$keyring"' EXIT
gpg=(gpg --homedir "$keyring" --batch --pinentry-mode loopback --passphrase '')
"${gpg[@]}" --quick-generate-key 'Tizen developer image Git package' rsa3072 sign 0
fingerprint=$("${gpg[@]}" --with-colons --list-keys | awk -F: '$1=="fpr"{print $10;exit}')
GNUPGHOME="$keyring" rpmsign --define "_gpg_name $fingerprint" \
    --define "__gpg $(command -v gpg)" --addsign "$rpmfile"
createrepo_c --update "$repo"
index="$repo/repodata/repomd.xml"
"${gpg[@]}" --yes --armor --detach-sign "$index"
"${gpg[@]}" --armor --export "$fingerprint" > "$index.key"
mkdir "$keyring/rpmdb"
rpmkeys --dbpath "$keyring/rpmdb" --import "$index.key"
rpmkeys --dbpath "$keyring/rpmdb" --checksig "$rpmfile"
