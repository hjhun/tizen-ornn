#!/usr/bin/env bash
# Install the locally built Git into the mounted Tizen development root.
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd -P)
out=${TIZEN_OUT:-"$project/out"}
root="$out/root"
repo=${1:-"$out/git-repo"}

[[ $EUID == 0 ]] || { echo 'Run as root with the Tizen filesystem mounted.' >&2; exit 1; }
mountpoint -q "$root"
test -s "$repo/repodata/repomd.xml.key"
test -s "$repo/repodata/repomd.xml.asc"
test -f "$repo/x86_64/git-2.56.1-1.x86_64.rpm"
install -d "$root/opt/tizen-dev/repo" "$root/etc/zypp/repos.d"
rsync -a --delete "$repo/" "$root/opt/tizen-dev/repo/"
cat > "$root/etc/zypp/repos.d/tizen-dev-local.repo" <<'REPO'
[tizen-dev-local]
name=Tizen development image locally built packages
enabled=1
autorefresh=0
baseurl=dir:///opt/tizen-dev/repo
type=rpm-md
gpgcheck=1
repo_gpgcheck=1
pkg_gpgcheck=1
REPO
unshare --mount --propagation private -- bash -se "$root" <<'INSTALL'
set -euo pipefail
root=$1
mount -t proc proc "$root/proc"
chroot "$root" /usr/bin/rpm --import /opt/tizen-dev/repo/repodata/repomd.xml.key
chroot "$root" /usr/bin/zypper --non-interactive refresh --force tizen-dev-local
chroot "$root" /usr/bin/zypper --non-interactive --no-refresh install \
    --force --from tizen-dev-local git=2.56.1-1
chroot "$root" /usr/bin/rpm -q git
chroot "$root" /usr/bin/git --version
# The snapshot supplies more with util-linux. Git invokes it on a terminal.
test -x "$root/usr/bin/more"
chroot "$root" /usr/bin/git config --system core.pager more
INSTALL
