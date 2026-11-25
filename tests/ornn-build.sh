#!/usr/bin/env bash
# Run inside a Tizen guest with the ornn under test on PATH.
set -euo pipefail
fixture=$(mktemp -d /var/tmp/ornn-test.XXXXXXXX)
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/repo/packaging"
export XDG_DATA_HOME=$fixture/state
export ORNN_BUILD_ROOT="$fixture/workspaces"
for package in ornn-fixture ornn-fixture-devel; do
    if rpm -q "$package" >/dev/null 2>&1; then
        echo "Fixture package already installed: $package" >&2
        exit 1
    fi
done
cd "$fixture/repo"
git init -q
cat > packaging/example.spec <<'SPEC'
Name: ornn-fixture
Version: 1
Release: 1
Summary: Verify Ornn source preparation
License: MIT
Source0: %{name}-%{version}.tar.gz
Source1: expected
%description
Ornn guest build fixture.
%package devel
Summary: Verify related RPM installation
Requires: %{name} = %{version}-%{release}
%description devel
Development files for the Ornn fixture.
%prep
%setup -q
%check
test "$(./read-value)" = "$(cat %{SOURCE1})"
test "$(readlink read-value)" = script
test ! -e removed
test ! -e untracked
%install
mkdir -p %{buildroot}/usr/share/ornn-fixture
cp value %{buildroot}/usr/share/ornn-fixture/
cp value %{buildroot}/usr/share/ornn-fixture/header
%files
/usr/share/ornn-fixture/value
%files devel
/usr/share/ornn-fixture/header
SPEC
printf '#!/bin/sh\ncat value\n' > script
chmod +x script
ln -s script read-value
echo staged > value
touch removed
git add packaging/example.spec value script read-value removed
rm removed
touch untracked
echo edited > value
echo edited > packaging/expected
git status --porcelain --untracked-files=all > "$fixture/before"
ornn build --no-install
workspace=$ORNN_BUILD_ROOT/repo/x86_64
run=$(cat "$workspace/.last-success")
rpm -qp --qf '%{NAME}\n' "$workspace/$run"/RPMS/*/ornn-fixture-1-*.rpm | grep -Fx ornn-fixture
test -s "$workspace/$run/build.log"
test -e "$workspace/$run/BUILD/ornn-fixture-1/value"
test ! -e Build
(cd "$fixture" && ornn install ornn-fixture-devel)
test "$(cat /usr/share/ornn-fixture/value)" = edited
test "$(cat /usr/share/ornn-fixture/header)" = edited
if ornn uninstall ornn-fixture; then exit 1; fi
(
    exec 8< .
    flock -s 8
    if ornn clean; then exit 1; fi
)
mkdir Build
touch Build/unrelated
ornn clean
test -e Build/unrelated
test ! -e "$workspace"
rm Build/unrelated
rmdir Build
ornn clean
ln -s "$fixture" Build
ornn clean
test -L Build
rm Build
git status --porcelain --untracked-files=all | cmp - "$fixture/before"
test ! -e packaging/ornn-fixture-1.tar.gz
mkdir "$fixture/ornn-fixture-1"
cp -P script read-value "$fixture/ornn-fixture-1/"
echo supplied > "$fixture/ornn-fixture-1/value"
tar -C "$fixture" -czf packaging/ornn-fixture-1.tar.gz ornn-fixture-1
echo supplied > packaging/expected
ornn build --no-install --out "$fixture/second"
rpm -qp --qf '%{NAME}\n' "$fixture/second"/*/ornn-fixture-1-*.rpm | grep -Fx ornn-fixture

last_run=$(cat "$workspace/.last-success")
# A failing %check must stop publication and installation.
sed -i '/^%check$/a printf "add_test(expected-ornn-check-failure /bin/false)\\n" > CTestTestfile.cmake\nctest --output-on-failure' packaging/example.spec
if ornn build --out "$fixture/failed" > "$fixture/failed.log" 2>&1; then
    echo 'Unexpected build success with a failing %check' >&2
    exit 1
fi
grep -Fq '0% tests passed, 1 tests failed out of 1' "$fixture/failed.log"
grep -Fq 'expected-ornn-check-failure (Failed)' "$fixture/failed.log"
test ! -e "$fixture/failed"
test "$(cat "$workspace/.last-success")" = "$last_run"
grep -lF 'expected-ornn-check-failure (Failed)' "$workspace"/run.*/build.log
(cd "$fixture" && ornn install ornn-fixture-devel)
rpm -q ornn-fixture
test "$(cat /usr/share/ornn-fixture/value)" = supplied
test "$(cat /usr/share/ornn-fixture/header)" = supplied
ornn install ornn-fixture-devel | grep -Fx 'Already installed.'
ornn uninstall ornn-fixture-devel ornn-fixture
! rpm -q ornn-fixture
test -s "$workspace/$last_run"/RPMS/*/ornn-fixture-1-*.rpm

# Model an interrupted receipt commit using the guest's actual RPM database.
key=$(pwd -P | sha256sum)
state=$XDG_DATA_HOME/ornn/projects/${key%% *}
rpm -q --qf '%{NAME}.%{ARCH} %{SIGMD5}\n' filesystem > "$state/.install-pending"
cp "$state/.install-pending" "$state/.last-build"
ornn status > "$fixture/status"
test ! -e "$state/.install-pending"
cmp "$state/.last-build" "$state/.installed-rpms"
awk '$2=="local" && $3==$4 {matches++} END {exit(matches!=1)}' "$fixture/status"
printf '%s\n' 'ornn-absent-probe.x86_64 00000000000000000000000000000000' >> "$state/.last-build"
cp "$state/.last-build" "$state/.install-pending"
ornn status > "$fixture/status" 2> "$fixture/warning"
test -e "$state/.install-pending"
grep -Fq 'Unfinished install' "$fixture/warning"
awk '$2=="missing" {missing++} END {exit(missing!=1)}' "$fixture/status"
