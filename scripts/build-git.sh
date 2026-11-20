#!/usr/bin/env bash
set -euo pipefail

# TIZEN_BUILDROOT must contain native rpmbuild and Git's BuildRequires.
# The builder is mounted read-only. All scratch and output are outside it.
project=$(cd "$(dirname "$0")/.." && pwd)
builder=${TIZEN_BUILDROOT:?set TIZEN_BUILDROOT to the Tizen x86_64 buildroot}
out=${TIZEN_OUT:-"$project/out"}
jobs=${JOBS:-4}
[[ $jobs =~ ^[1-9][0-9]*$ ]]
builder=$(realpath "$builder")
mkdir -p "$out/work/git-sources" "$out/git-repo/x86_64"
out=$(realpath "$out")
job=$(mktemp -d "$out/work/git-rpmbuild.XXXXXX")
mkdir -p "$job"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
file=git-2.56.1.tar.xz
digest=f0b0bf89264b9bd2e1c640363e7529643989da76281600473c50745cd34a607f
archive="$out/work/git-sources/$file"
if [[ ! -f $archive ]]; then
    curl --fail --location --retry 2 \
        "https://www.kernel.org/pub/software/scm/git/$file" \
        -o "$job/$file"
    echo "$digest  $job/$file" | sha256sum --check
    mv "$job/$file" "$archive"
fi
echo "$digest  $archive" | sha256sum --check
install -m 644 "$archive" "$job/SOURCES/$file"
install -m 644 "$project/packaging/git/git.spec" "$job/SPECS/"

# A root caller already has mount privileges and can traverse the host
# project path. Map an ordinary caller to root inside its own user namespace.
namespace=()
if (( EUID != 0 )); then
    namespace=(--unshare-user --uid 0 --gid 0)
fi
bwrap "${namespace[@]}" --die-with-parent \
    --ro-bind "$builder" / --dev /dev --proc /proc --tmpfs /tmp \
    --tmpfs /var/tmp --bind "$job" /tmp/build-git --chdir /tmp/build-git \
    --clearenv --setenv PATH /usr/bin:/bin --setenv HOME /tmp \
    /usr/bin/rpmbuild -ba --define '_topdir /tmp/build-git' \
    --define "_smp_mflags -j$jobs" /tmp/build-git/SPECS/git.spec \
    2>&1 | tee "$job/build.log"
install -m 644 "$job/RPMS/x86_64/git-2.56.1-1.x86_64.rpm" \
    "$out/git-repo/x86_64/"
"$project/scripts/sign-git-repo.sh" "$out/git-repo"
