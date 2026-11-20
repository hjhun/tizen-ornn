# tizen-wsl

WSL2 또는 Ubuntu에서 Tizen 10.1 플랫폼과 앱을 개발하는 x86_64 VM입니다. QEMU가 RPi4용 Tizen 소스에서 빌드한 **Linux 6.12.41**을 부팅하므로, 게스트 안에서 SMACK과 systemd가 동작합니다. Windows에서는 Ubuntu WSL2 셸에서 실행합니다.

게스트에는 Git, Vim, GCC/G++, glibc·libstdc++·GLib 개발 헤더, CMake, Ninja, Make, Autotools, Meson, GDB, strace, Cargo와 zypper가 있습니다. C/C++·Rust의 x86_64 및 ARMv7 빌드, RPM 생성, Tizen CLI의 TPK 패키징, SDB를 이용한 장치 연결을 지원합니다.

## 처음 실행하기

x86_64 Ubuntu 24.04 기준입니다. WSL2에서는 홈 디렉터리의 리눅스 파일시스템에서 실행합니다. 호스트에 루프 장치와 ext4 확장 속성 지원이 필요합니다. 커널을 직접 컴파일할 때는 커널 소스와 빌드 디렉터리 공간도 확보하십시오.

```sh
sudo apt update
sudo apt install git build-essential bc bison flex libelf-dev libssl-dev bubblewrap \
  rpm rpm2cpio createrepo-c gnupg cpio curl ca-certificates rsync attr \
  e2fsprogs util-linux zstd unzip xz-utils patch pkg-config dpkg-dev python3 \
  qemu-system-x86 qemu-system-gui qemu-utils seabios

git clone https://github.com/hjhun/tizen-wsl.git
cd tizen-wsl
sudo env TIZEN_PREBUILT_KERNEL=1 ./scripts/tizen-image --out "$PWD/out/dev"
TIZEN_OUT="$PWD/out/dev" ./scripts/run-vm.sh
```

이 명령은 Release의 커널 해시를 확인하고, 공식 스냅샷과 RPM에서 쓰기 가능한 16 GiB 게스트 디스크를 만듭니다. TPK 패키징과 SDB에 필요한 Tizen Studio 구성품은 이미지 생성 시 공식 서버에서 내려받아 로컬에 설치합니다. 전체 SDK가 들어간 게스트 디스크는 로컬에서 생성합니다. SDK 이용에는 공급자의 사용 조건이 적용됩니다. 이미지 생성에만 `sudo`가 필요합니다. [R4 Release](https://github.com/hjhun/tizen-wsl/releases/tag/10.1-r4) · [Tizen Studio 공식 설치 및 사용권 안내](https://developer.samsung.com/tizen/get-started/installation.html)

화면이 필요한 앱은 호스트에서 마지막 명령에 `--gui`를 붙여 실행합니다. WSLg 또는 Ubuntu 데스크톱에서 Tizen Wayland 창이 열립니다. 직렬 콘솔의 게스트 root 셸에서 `systemctl poweroff`로 종료합니다. 다음 실행에도 작업 결과와 설치한 패키지가 유지됩니다.

## 게스트에서 첫 빌드

부팅하면 Tizen 데몬과 `owner` 사용자 세션이 시작됩니다. root는 zypper로 RPM과 의존성을 설치하고, 개발자는 `owner`로 편집·빌드합니다.

```sh
# 게스트 root
systemctl --failed
zypper search glib2-devel
su - owner

# 게스트 owner
cd /home/owner/workspace
cargo new hello
cd hello
cargo run --offline
```

Tizen 플랫폼의 CMake 프로젝트는 소스에서 **`tizen-build test`** 한 줄로 빌드하고 테스트합니다. 이 도구는 프로젝트의 RPM spec에 적힌 CMake 옵션과 매크로를 읽습니다. 같은 명령을 반복하면 바뀐 파일이 증분 빌드됩니다. `aul-1`은 Gerrit 또는 공개 소스 RPM에서 가져올 수 있습니다. [aul-1 소스 받기와 빌드](docs/development.md#owner로-aul-1-빌드)

| 하고 싶은 일 | 가이드 |
|---|---|
| 새 날짜의 스냅샷이나 로컬 원본 이미지로 VM 만들기 | [이미지 생성기와 스냅샷 잠금](docs/snapshot-build.md) |
| 디스크 크기, GUI, KVM, 부팅·종료, 로컬 아카이브 | [VM 실행과 저장공간](docs/running.md) |
| Tizen 데몬, 패키지 설치, 개발 도구, aul-1 증분 빌드와 RPM | [게스트 개발 안내](docs/development.md) · [RPM 저장소](docs/snapshot-packages.md) |
| EFL 앱 화면 띄우기, ARMv7 ELF·RPM, Cargo TPK 만들기, SDB 연결 | [앱과 크로스 빌드 예제](docs/development.md#화면에서-ui-앱-개발) |
| 실제로 부팅·빌드·실행한 항목과 기기 검증 범위 | [검증 환경과 결과](docs/verification.md) |

## 빌드 기준

기본 입력은 Tizen 10.1 Unified `20261119.053314`의 `tizen-headed-emulator64-wayland` 이미지와 Base `20261118.084133`입니다. 공식 에뮬레이터 커널 패키지는 4.4.35이며, 이 프로젝트는 RPi4 커널 `platform/kernel/linux-rpi`의 커밋 `b6322b453844b6111e4619615fc65bd29b86c83e`에서 x86_64 가상 PC용 커널을 빌드합니다. `TIZEN_PREBUILT_KERNEL=1`을 생략하면 같은 소스 RPM으로 직접 컴파일합니다. [공식 Unified 스냅샷](https://download.tizen.org/snapshots/TIZEN/Tizen-10.1/Tizen-10.1-Unified/tizen-10.1-unified_20261119.053314/) · [공식 Base 스냅샷](https://download.tizen.org/snapshots/TIZEN/Tizen-10.1/Tizen-10.1-Base/tizen-10.1-base_20261118.084133/)
