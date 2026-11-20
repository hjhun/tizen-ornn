Name:           git
Version:        2.56.1
Release:        1
Summary:        Distributed version control for the Tizen developer image
License:        GPL-2.0-only
URL:            https://git-scm.com/
Source0:        https://www.kernel.org/pub/software/scm/git/git-%{version}.tar.xz
BuildRequires:  gcc
BuildRequires:  diffutils
BuildRequires:  file
BuildRequires:  make
BuildRequires:  perl
BuildRequires:  zlib-devel
BuildRequires:  libopenssl3-devel
BuildRequires:  libexpat-devel
BuildRequires:  libcurl-devel

%description
Git command-line tools with local, SSH, and smart HTTP(S) transports.
The optional Perl, Python and graphical front ends are omitted.

%prep
%setup -q

%build
cat > config.mak <<'CONFIG'
prefix = /usr
libexecdir = /usr/libexec
NO_RUST = YesPlease
NO_GETTEXT = YesPlease
NO_TCLTK = YesPlease
NO_PERL = YesPlease
NO_PYTHON = YesPlease
INSTALL_SYMLINKS = YesPlease
CFLAGS = %{optflags}
CONFIG
make %{?_smp_mflags} all

%install
make DESTDIR=%{buildroot} install

%check
./git --version
test -x git-remote-http
GIT_CONFIG_NOSYSTEM=1 ./git init --bare --template=templates/blt test.git
./git --git-dir=test.git rev-parse --is-bare-repository | grep -x true

%files
%license COPYING
%{_bindir}/git*
%{_bindir}/scalar
%{_datadir}/bash-completion/completions/git
/usr/libexec/git-core
%{_datadir}/git-core
