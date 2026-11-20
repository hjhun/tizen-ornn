Name:           tizen-build
Version:        1.0
Release:        1
Summary:        Incremental CMake builds from Tizen RPM specs
License:        NOASSERTION
BuildArch:      noarch
Source0:        tizen-build
Requires:       python3-base
Requires:       python3-rpm
Requires:       rpm-build
Requires:       cmake
Requires:       ninja
Requires:       bash

%description
Configure and build a working tree using the CMake options from its Tizen
package spec. Run the project's CTest tests against the build tree.

%install
install -D -m 0755 %{SOURCE0} %{buildroot}%{_bindir}/tizen-build

%files
%{_bindir}/tizen-build
