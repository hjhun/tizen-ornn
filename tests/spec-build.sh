#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C
qa=$(mktemp -d "$PWD/small-spec.XXXXXX")
trap 'rm -rf -- "$qa"' EXIT
cd "$qa"
mkdir -p example/packaging
cd example
git init -q
cat >CMakeLists.txt <<'EOF'
cmake_minimum_required(VERSION 3.16)
project(seed LANGUAGES C)
set(SPEC_VALUE "fallback" CACHE STRING "example option")
add_executable(seed main.c)
target_compile_definitions(seed PRIVATE "SPEC_VALUE=\"${SPEC_VALUE}\"")
enable_testing()
add_test(NAME basic COMMAND seed)
EOF
cat >main.c <<'EOF'
#include <stdio.h>
int main(void) { puts(SPEC_VALUE); return 0; }
EOF
cat >packaging/example.spec <<'EOF'
%define fixture_value initial
Name: example
Version: 1
Release: 1
License: MIT
Summary: exercise CMake cache changes
Source0: %{name}-%{version}.tar.gz
%description
exercise CMake cache changes
%prep
%setup -q
%build
%cmake -DSPEC_VALUE=%{fixture_value} .
%__make %{?_smp_mflags}
%check
ctest -V
%install
:
%files
EOF
tizen-build test
build=../example-build
test "$("$build/seed")" = initial
sed -i 's/define fixture_value initial/define fixture_value changed/' packaging/example.spec
tizen-build
test "$("$build/seed")" = changed
sed -i 's/ -DSPEC_VALUE=%{fixture_value}//' packaging/example.spec
tizen-build
test "$("$build/seed")" = fallback
echo 'spec-derived binary output: initial -> changed -> fallback'
cp packaging/example.spec packaging/second.spec
if tizen-build >../ambiguous.log 2>&1; then echo 'Unexpected ambiguous spec success'; exit 1; fi
cat ../ambiguous.log
tizen-build --spec packaging/example.spec
rm packaging/second.spec
sed -i 's/^%cmake \./echo omitted-cmake/' packaging/example.spec
if tizen-build >../unsupported.log 2>&1; then echo 'Unexpected unsupported spec success'; exit 1; fi
cat ../unsupported.log
