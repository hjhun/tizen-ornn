Name:           tizen-cross-probe
Version:        1.0
Release:        1
Summary:        Verify native and ARM development RPMs
License:        MIT
Source0:        probe.c
Source1:        probe.cc
BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  pkgconfig(glib-2.0)

%description
A C executable that calls GLib and a C++ executable using the standard library.

%prep
%setup -q -c -T
cp %{SOURCE0} .
cp %{SOURCE1} .

%build
%{__cc} %{optflags} probe.c $(pkg-config --cflags --libs glib-2.0) -o probe-c
%{__cxx} %{optflags} -std=c++17 probe.cc -o probe-cxx

%install
install -d %{buildroot}%{_bindir}
install -m 755 probe-c probe-cxx %{buildroot}%{_bindir}/

%files
%{_bindir}/probe-c
%{_bindir}/probe-cxx
