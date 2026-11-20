Name:           tizen-ui-counter
Version:        1.0
Release:        1
Summary:        Tizen Elementary counter example
License:        MIT
Source0:        main.c
BuildRequires:  gcc
BuildRequires:  pkgconfig(elementary)

%description
An interactive Wayland application that increments a counter on button clicks.

%prep
%setup -q -c -T
cp %{SOURCE0} .

%build
%{__cc} %{optflags} -std=gnu11 main.c -o tizen-ui \
    $(pkg-config --cflags --libs elementary)

%install
install -D -m 0755 tizen-ui %{buildroot}%{_bindir}/tizen-ui

%files
%{_bindir}/tizen-ui
