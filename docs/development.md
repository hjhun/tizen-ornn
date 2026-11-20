# 게스트에서 개발하기

## 부팅하는 서비스

콘솔과 GUI 모두 Tizen systemd의 `multi-user.target`과 `owner`(UID 5001)의 사용자 세션을 시작합니다. AMD(`ac`), `actd`, `esd`, `cynara`, `security-manager`, `deviced`, `resourced`, `dlog_logger`, `sessiond`, `pkgmgr-info`가 실행됩니다. `package-manager.socket`은 설치 요청을 기다리고, `sdbd_tcp.socket`은 TCP 26101 연결을 기다립니다. 이 둘은 요청에 따라 서비스 프로세스를 시작합니다.

```sh
# 게스트 콘솔
systemctl is-system-running
systemctl --failed
systemctl status ac.service user@5001.service package-manager.socket
su - owner
systemctl --user --no-pager list-units --type=service --state=running
```

이 VM에는 가상 모뎀·Wi-Fi PHY·ALSA 오디오 장치가 구성되어 있지 않아 해당 장치 초기화 서비스는 건너뜁니다. `stability-monitor`는 사용한 RPi 계열 커널에 `proc_tsm`이 없어 수집을 시작하지 않습니다.

## 개발과 패키지 설치

직렬 콘솔의 root는 패키지 관리에 사용할 수 있습니다. 플랫폼 사용자로 개발하려면 `su - owner` 후 `cd /home/owner/workspace`를 실행합니다. `/home`은 `/opt/usr/home`을 가리킵니다. 원래 홈은 Tizen의 `User::Home` SMACK 라벨을 유지하고, 소스와 산출물은 일반 셸에서 쓸 수 있는 개발 디렉터리에 저장합니다. GUI 예제에는 별도의 `dev` 계정도 사용할 수 있습니다. 기본 도구에는 Git, Vim, GCC/G++, glibc·libstdc++·GLib의 개발 헤더, pkg-config, CMake/CTest, Ninja, Make, GDB, strace, rustc, Cargo, rpmbuild가 포함되어 있습니다. Meson, Autoconf, Automake, Libtool, M4, SSH 클라이언트, rsync, jq와 iputils도 설치됩니다. `owner`의 작업 디렉터리에서 `tizen-linux-tools-check`로 기본 빌드·디버깅 도구를 검사할 수 있습니다.

```sh
# root
zypper repos --details
zypper search --details glib2-devel
zypper --non-interactive install ed
su - owner
cd /home/owner/workspace
cargo new hello
cd hello
cargo run --offline
```

이미지에는 날짜가 고정된 공식 Base·Unified RPM 인덱스와 이 이미지용으로 빌드한 Git RPM 저장소가 등록돼 있습니다. 필요한 의존성은 zypper가 함께 받습니다. 공식 인덱스의 고정 해시, 로컬 서명, 캐시 재생성 방법은 [패키지 저장소 설명](snapshot-packages.md)을 참조하십시오.

## owner로 aul-1 빌드

게스트의 직렬 콘솔에서 `su - owner`를 실행합니다. Gerrit 접근 권한이 있으면 아래의 `GERRIT_ID`를 자신의 사용자 이름으로 바꿔 clone합니다. R4와 같은 소스는 Unified 스냅샷의 `aul-0.83.29-1.src.rpm`에 기록된 커밋입니다.

```sh
cd /home/owner/workspace
git clone -b accepted/tizen_10.1_unified \
  ssh://GERRIT_ID@review.tizen.org:29418/platform/core/appfw/aul-1 aul-1
cd aul-1
git checkout --detach 30bcc3f359e33da4e9c1a34823c2a374a1a23d70
tizen-build test
```

Gerrit 계정이 없어도 R4 스냅샷의 **공개 소스 RPM**에서 같은 버전으로 시작할 수 있습니다. 아래는 새 작업 디렉터리에서 한 번만 실행하는 예입니다.

```sh
cd /home/owner/workspace
mkdir aul-snapshot && cd aul-snapshot
url=https://download.tizen.org/snapshots/TIZEN/Tizen-10.1/Tizen-10.1-Unified
snap=tizen-10.1-unified_20261119.053314
curl -fLO "$url/$snap/repos/emulator/source/aul-0.83.29-1.src.rpm"
echo '4e185878b5cf154ce0ac095b22979086c31997498deab8516bd0dafaa6e021cb  aul-0.83.29-1.src.rpm' | sha256sum -c - &&
  rpm2cpio aul-0.83.29-1.src.rpm > aul.cpio &&
  cpio -id aul-0.83.29.tar.gz < aul.cpio &&
  tar -xzf aul-0.83.29.tar.gz &&
  cd aul-0.83.29 &&
  tizen-build test
```

`tizen-build test`는 체크아웃에서 `packaging/*.spec` 또는 최상위 `*.spec`을 찾고, Tizen RPM이 확장한 매크로·조건·버전과 프로젝트의 CMake 옵션으로 구성, 빌드, CTest를 실행합니다. `tizen-build`만 실행하면 빌드까지만 진행합니다. `aul-1`의 개발 빌드 디렉터리는 소스 옆의 `/home/owner/workspace/aul-1-build`입니다. 소스를 편집한 뒤 같은 명령을 반복하면 Ninja가 변경된 대상을 다시 빌드합니다. 테스트는 이번 빌드에서 만든 공유 라이브러리를 먼저 로드합니다.

이 개발 도구는 신뢰하는 spec의 `%build`에서 체크아웃 루트에 대한 단일 `%cmake` 또는 `cmake` 구성 호출을 사용합니다. 여러 spec이 있으면 `tizen-build test --spec packaging/example.spec`처럼 선택합니다. 컴파일러와 개발 라이브러리는 프로젝트의 BuildRequires에 맞게 설치되어 있어야 합니다. 프로젝트 소스에 패치를 적용하는 `%prep`, CMake 외의 추가 빌드 명령, CTest 외의 `%check` 명령, 설치와 패키징은 RPM 빌드에서 수행합니다.

```sh
# Git 체크아웃에서 aul-1 RPM을 만들 때, 게스트 owner로 실행
tizen-build-aul .
ls ../aul-rpmbuild/RPMS/x86_64
```

이 예에서 생성한 RPM은 게스트의 네이티브 `rpmbuild` 결과입니다. 배포 품질 확인에 필요한 GBS 격리 빌드와 물리 기기 검증은 별도로 수행합니다.

기존 R3를 사용 중이면 보조 RPM만 설치해 같은 명령을 쓸 수 있습니다. 배포한 `tizen-build-1.0-1.noarch.rpm`을 게스트로 복사한 뒤, 게스트 root 셸에서 아래 명령을 실행합니다.

```sh
zypper --non-interactive install python3-rpm
rpm -Uvh ./tizen-build-1.0-1.noarch.rpm
```

## 화면에서 UI 앱 개발

`./scripts/run-vm.sh --gui`로 시작하면 Tizen의 Enlightenment/Wayland 창에 Elementary 카운터 예제가 나타납니다. **Add one** 버튼을 눌러 값을 올릴 수 있습니다. 자신의 코드로 바꿀 때에는 게스트의 직렬 콘솔에서 예제를 정지하고 개발 계정으로 다시 빌드합니다.

```sh
# 게스트 root
systemctl stop tizen-ui-demo.service
su dev
cd
cp -r /usr/share/tizen-dev/examples/efl-counter "$HOME/efl-counter"
cd "$HOME/efl-counter"
gcc -std=gnu11 main.c -o tizen-ui $(pkg-config --cflags --libs elementary)
tizen-ui-run "$PWD/tizen-ui"
```

이 경로는 Tizen의 Wayland 공유 메모리 버퍼로 그립니다. 스냅샷 기본 Flutter 홈 화면(`org.tizen.homescreen`)은 현재 EGL 디스플레이 초기화에 실패합니다. GUI 모드에서 Tizen의 `starter`는 이 홈 앱 실행을 재시도합니다. 아래 EFL 예제의 렌더링과 입력은 확인했습니다. `tizen-ui-run`은 일반 사용자에게 쓰기 권한이 있는 런타임 디렉터리와 게스트의 Wayland 소켓을 설정합니다.

같은 예제 디렉터리에서 `./build.sh "$HOME/ui"`를 실행하면 실행 파일과 공식 CLI가 서명한 `org.example.tizencounter-1.0.0-x86_64.tpk`가 함께 생성됩니다. TPK의 ELF와 원래 파일이 일치하는지 빌드 스크립트가 검사합니다. 플랫폼의 패키지 관리 경로를 사용하려면 root 콘솔에서 `pkgcmd -i -t tpk -p /home/dev/ui/org.example.tizencounter-1.0.0-x86_64.tpk`를 실행하고 `pkginfo --app org.example.tizencounter`로 등록을 조회합니다.

## x86_64와 ARMv7 빌드

아래 예시의 `$HOME/ui-*`, `$HOME/packages` 등 경로는 `dev` 계정 기준입니다. `owner`로 실행할 때는 출력 경로를 `$HOME/workspace/` 아래로 지정합니다. `gcc`, `g++`, `pkg-config`는 x86_64용입니다. ARMv7은 `tizen-armv7l-gcc`, `tizen-armv7l-g++`, `tizen-armv7l-pkg-config`를 사용합니다. ARM 헤더·라이브러리는 공식 `armv7l` RPM에서 가져와 `/opt/tizen/sysroots/armv7l`에 설치했습니다.

```sh
cd /usr/share/tizen-dev/examples/efl-counter
gcc -std=gnu11 main.c -o "$HOME/ui-x86_64" $(pkg-config --cflags --libs elementary)
tizen-armv7l-gcc -std=gnu11 main.c -o "$HOME/ui-armv7l" \
    $(tizen-armv7l-pkg-config --cflags --libs elementary)
./build-rpm.sh x86_64 "$HOME/packages"
./build-rpm.sh armv7l "$HOME/packages"
rpm -qp --qf '%{NAME} %{ARCH}\n' "$HOME"/packages/*.rpm
```

예제의 `tizen-ui-counter.spec` 및 `build-rpm.sh`를 다른 프로젝트에 맞게 사용할 수 있습니다. 일반 spec은 `tizen-rpmbuild armv7l -bb example.spec` 또는 `tizen-rpmbuild x86_64 -bb example.spec`로 빌드합니다. ARM용 spec은 크로스 컴파일러와 타깃 의존성을 사용할 수 있어야 합니다. 예제 RPM은 `/usr/bin/tizen-ui`를 제공합니다. 제품 앱의 앱 ID 등록·배포에는 앱 매니페스트와 해당 패키지 절차가 추가로 필요합니다.

CMake 프로젝트에는 동봉한 툴체인 파일을 지정할 수 있습니다. 다음 예제의 CTest는 ARM용 실행 파일을 Tizen의 `qemu-arm`으로 실행합니다.

```sh
cmake -S /usr/share/tizen-dev/examples/cross-rpm -B "$HOME/build-arm" -G Ninja \
    --toolchain /usr/share/tizen-dev/cmake/armv7l.cmake
cmake --build "$HOME/build-arm"
ctest --test-dir "$HOME/build-arm" --output-on-failure
```

Rust도 동일한 Tizen ARM 런타임에 링크합니다. Tizen RPM 아키텍처 이름은 `armv7l`, 이 스냅샷의 Rust 표준 라이브러리 타깃 이름은 `arm-unknown-linux-gnueabi`입니다. 일반 로그인 셸에는 해당 링커와 QEMU user-mode 실행기가 설정됩니다.

```sh
cd /home/owner/workspace/hello
. /etc/profile.d/tizen-cargo-arm.sh
cargo build --release --offline
cargo run --release --offline --target arm-unknown-linux-gnueabi
readelf -A target/arm-unknown-linux-gnueabi/release/hello
```

`--offline` 예제는 외부 crate가 없는 프로젝트 기준입니다. `tizen-cross-check "$HOME/cross-results"`는 빈 출력 경로에 C/GLib·C++ 샘플의 두 RPM을 만들고 각각 포장된 ELF의 아키텍처와 실행 결과를 검사합니다.

## Cargo 결과물을 TPK로 패키징

다음은 위에서 만든 `hello` 프로젝트에서 공식 Tizen Studio CLI로 네이티브 TPK를 생성하는 예입니다. 도우미는 ELF를 읽어 SDK의 `x86_64` 또는 `arm` 패키지 아키텍처를 선택합니다.

```sh
cargo build --release --offline
/usr/share/tizen-dev/examples/native-tpk/package.sh "$HOME/packages" target/release/hello
cargo build --release --offline --target arm-unknown-linux-gnueabi
/usr/share/tizen-dev/examples/native-tpk/package.sh "$HOME/packages" \
    target/arm-unknown-linux-gnueabi/release/hello
```

두 번째 인자를 생략하면 함께 제공된 C++ 예제를 컴파일하고 패키징합니다. 예제는 고정된 앱 ID와 매니페스트를 사용하고 결과물 안의 ELF와 두 서명 파일을 검사합니다. 제품 앱은 자체 매니페스트와 서명 프로필을 사용하십시오. 기본 서명은 SDK가 제공하는 에뮬레이터 테스트용입니다. 개인 인증서는 포함하지 않습니다. `tizen package --help`에서 공식 옵션을 볼 수 있습니다. 10.1 플랫폼 스냅샷과 별도로 Ubuntu x86_64용 공개 Tizen Studio CLI 패키지를 해시 고정해 공급합니다.

## SDB로 장치 연결

```sh
sdb version
sdb connect DEVICE_IP:26101
sdb devices
sdb -s DEVICE_IP:26101 shell uname -m
```

위 주소를 장치의 개발자 모드에서 허용한 실제 IP와 포트로 바꾸십시오. QEMU 호스트의 루프백 서비스는 게스트에서 기본적으로 `10.0.2.2`로 접근할 수 있습니다. 실제 장치가 개발자 PC의 IP를 허용 목록에 요구하면 해당 장치의 개발자 설정도 적용해야 합니다.

USB는 장치를 호스트에 연결하고, 호스트에서 `lsusb`로 확인한 **Bus:Device** 한 개를 명시해 VM에 전달합니다. QEMU 프로세스에는 해당 USB 노드를 열 권한이 필요합니다.

```sh
TIZEN_USB=001:004 ./scripts/run-vm.sh
```

WSL2에서는 먼저 Windows에서 usbipd-win으로 선택한 장치를 WSL에 연결한 후 WSL의 `lsusb` 번호를 사용합니다. Windows의 BUSID와 이 번호는 형식이 다릅니다. 게스트 커널에는 xHCI/PCI와 USB 지원이 내장돼 있습니다. TCP 검증은 별도 환경의 공식 Tizen sdbd를 대상으로 했으며, 물리 USB 장치 접속은 미검증입니다. [Microsoft USB 연결 안내](https://learn.microsoft.com/en-us/windows/wsl/connect-usb)
