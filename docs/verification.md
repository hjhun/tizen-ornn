# 검증 환경과 결과

게스트 root 셸의 `tizen-dev-check`는 SMACK 파일 접근 허용·거부, GCC/G++와 GLib를 사용한 CMake/CTest, Cargo 오프라인 빌드·실행 및 기본 CLI들을 확인합니다. 호스트의 `tests/boot-smoke.expect`는 일회용 QCOW2 오버레이에 두 번 부팅해 일반 사용자 빌드, C++·Rust TPK, 디버거, HTTPS와 데이터 지속성까지 검사합니다. `expect`와 `qemu-utils`가 필요합니다.

```sh
qemu-img create -f qcow2 -F raw -b "$PWD/out/dev/tizen-rootfs.ext4" verification.qcow2
TIZEN_OUT="$PWD/out/dev" TIZEN_DISK="$PWD/verification.qcow2" TIZEN_DISK_FORMAT=qcow2 \
  ./tests/boot-smoke.expect "$PWD/verification.log"
```

## R5 호스트 CLI와 게스트 검증

Ubuntu 24.04.4 LTS x86_64를 실행하는 WSL2에서 Ubuntu 패키지를 APT로 설치했습니다. 공식 Unified `20261118.052410`과 `20261119.053314`을 사용했습니다. 별도 Ubuntu 물리 장비와 AI 서비스 계정 로그인은 이번 검증에 포함하지 않았습니다.

| 실행한 경로 | 실제 결과 |
|---|---|
| 전체 이미지 교체 | 18일 게스트의 owner로 Git 저장소와 파일을 만들고 `/root`, `/opt/usr/data`에도 파일을 기록했습니다. 19일 이미지로 교체한 뒤 새 게스트에서 내용, UID 5001, 심볼릭 링크와 하드링크를 확인했습니다. 이전 디스크 선택과 재전환도 성공했습니다. |
| 강제 종료된 ext4 | 테스트용 이전 디스크의 저널을 임시 사본에 재생한 뒤 마이그레이션했습니다. 소형 raw·QCOW2·unclean ext4 검증에서는 이전 디스크 SHA-256, SMACK 속성, 소유권과 파일 데이터를 대조했습니다. |
| 실행 중 RPM 설치 | 18일 기반 게스트에 현재 제어 서비스를 설치해 부팅했습니다. 직렬 콘솔에서 dlog를 읽는 동안 별도 호스트 터미널에서 `ed`를 설치했습니다. 없는 RPM은 게스트 오류 코드 104를 반환했습니다. |
| 실행 중 RPM 갱신 | 19일 저장소로 지정해 `procps-ng`가 `4.0.4-1.1 → 4.0.4-2.1`로 바뀌었습니다. 이어서 `account-common-0.1.14-1`을 요청하자 새 체크섬의 RPM만 재설치됐습니다. 두 이름으로 재실행하자 추가 재설치가 없었습니다. 최신 R5 게스트에서 패키지 이름을 생략한 전체 갱신도 변경 없이 종료됐습니다. 셸 PID와 부팅 ID, 작업 파일이 유지됐습니다. |
| 콘솔 수명 | 두 터미널로 같은 VM에 접속해 Ctrl+C로 dlog를 종료했습니다. 최상위 셸에서 `exit`를 입력해 양쪽을 분리한 뒤 다시 접속해 같은 셸 PID, 작업 디렉터리, 백그라운드 프로세스를 확인했습니다. |
| 최종 이미지 부팅 | 두 번 부팅해 `tests/boot-smoke.expect`의 18단계가 통과했습니다. systemd 플랫폼 서비스, SMACK, C/C++·Cargo, x86_64·ARMv7 빌드와 패키징, AI 런타임, GDB·strace, HTTPS, 재부팅 데이터 보존을 확인했습니다. SDB 피어 왕복은 이 실행에서 생략했습니다. |
| WSLg GUI와 게스트 설치 | 선택된 R5의 일회용 오버레이를 `tests/guest-packages.expect --gui`로 부팅했습니다. 호스트에서 RPM을 설치하는 동안 게스트 전경 프로세스 PID가 유지됐고, 실제 가상 태블릿으로 EFL 버튼을 클릭해 화면의 숫자가 0에서 1로 바뀌었습니다. GUI 검사에는 호스트의 Expect와 Python Pillow가 필요합니다. |
| 선택형 AI 도구 | 19일 게스트에 호스트 패키지의 `tizen-vm install codex`로 설치한 뒤 owner로 `codex sandbox`에서 프로세스를 실행했습니다. 별도의 10.1 게스트에서는 `agy`, `gemini`, `claude`, `codex` 모두 설치 후 버전 출력까지 확인했습니다. [AI 런타임과 검증 버전](ai-tools.md) |

## R4 추가 검증

WSL2에서 x86_64 Tizen VM을 부팅해 수행했습니다. 게스트의 일반 사용자 `owner`(UID 5001)가 소스와 빌드 디렉터리에 쓸 수 있는지 함께 확인했습니다.

| 작업 | 관찰 결과 |
|---|---|
| `aul-1`에서 `tizen-build test` | CTest 1/1, 내부 Google Test 295/295 통과. 무변경 빌드는 오브젝트 272개 모두 수정 시각 유지 |
| C++ 소스 1개 수정 후 `tizen-build test` | 오브젝트 1개 재컴파일, 새 `libaul.so`의 해시 변경. 테스트 프로세스에서 새 라이브러리의 표식을 관찰하고 295/295 재통과 |
| RPM 매크로 변경·삭제 | 간단한 CMake 프로젝트의 실행 결과가 spec에 따라 `initial → changed → fallback`으로 전환 |
| 다른 플랫폼 프로젝트 | `capi-base-common`(직접 cmake 호출)과 `libslp-db-util`(%cmake)에서도 x86-64 공유 라이브러리 생성 |
| `tizen-linux-tools-check` | Meson/Ninja 1/1, Autotools/Make 1/1. SSH 클라이언트, rsync, patch, jq, GDB, strace 실행 |

GitHub 계정이나 Gerrit 자격 증명 없이 게스트에서 공개 `aul-0.83.29-1.src.rpm`을 내려받아 체크섬을 대조했습니다. 풀어낸 소스에서 `tizen-build test`로 CTest 1/1, Google Test 295/295를 통과했습니다.

## GitHub Release에서 처음 시작하는 경로

R4 태그 `7054021`을 인증 없이 새로 clone한 후 빈 출력 디렉터리에서 README의 `TIZEN_PREBUILT_KERNEL=1 ./scripts/tizen-image` 경로를 실행했습니다. WSL2 Ubuntu 24.04.4, 사전에 설치한 호스트 빌드 도구, tmpfs 작업 디렉터리에서 이미지 생성에 **3분 50초**가 걸렸습니다(1회 측정, 호스트 패키지 설치·게스트 부팅 시간 제외). 스냅샷·SDK·RPM을 공식 서버에서 다시 받았고 커널은 Release에서 받아 SHA-256을 대조했습니다.

새 디스크의 일회용 오버레이에서 `tests/boot-smoke.expect`로 두 번 부팅했습니다. 플랫폼 서비스, SMACK 검사, 일반 사용자 C++·Rust TPK, x86_64/ARMv7 ELF·RPM, ARM CMake 테스트, GDB/strace, HTTPS Git 조회와 zypper 설치가 통과했습니다. 재부팅 후 첫 부팅에서 빌드한 프로그램과 TPK가 유지됐습니다. 이 실행에서는 별도 SDB 피어를 지정하지 않았습니다.

## 다른 날짜의 원본에서 생성한 VM

`scripts/tizen-image --snapshot tizen-10.1-unified_20261118.052410`으로 별도 개발 디스크를 만들었습니다. 생성기는 대응하는 Base `20261112.005213`을 선택했습니다. WSL2 Ubuntu 24.04.4에서 기존 커널 빌드 캐시와 해시가 일치하는 일부 패키지 다운로드를 재사용했습니다.

- 게스트 커널은 `6.12.41-tizen-qemu`였고 systemd가 보고한 실패 유닛은 0개였습니다. 보안 서비스, 앱 프레임워크 데몬, `user@5001`, SDB와 패키지 관리자 소켓이 시작됐습니다.
- 같은 UID와 파일 권한에서 SMACK 라벨을 바꾸었을 때 접근 허용 및 `EACCES`를 각각 확인했습니다.
- `owner`에서 `tizen-linux-tools-check`를 통과했고 `tizen-build`로 해당 날짜의 `capi-base-common`을 x86-64 공유 라이브러리로 빌드했습니다. 이 날짜의 GUI와 ARM 앱 실행은 별도로 검증하지 않았습니다.

## R3 기반 이미지에서 검증한 기능

WSL2의 Ubuntu 24.04.4에서 배포 커널과 디스크의 일회용 오버레이로 검사했습니다.

| 게스트에서 실행한 작업 | 결과 |
|---|---|
| Tizen 데몬·패키지 관리자 | `multi-user.target`, `user@5001`, AMD 및 보안 서비스 자동 시작. x86_64 TPK 설치 후 `pkginfo`와 owner의 `app_launcher -l`에서 앱 확인 |
| owner의 aul-1 빌드 | `aul-0.83.29` 원본 spec으로 x86_64 RPM 9개 생성. CTest 1/1 및 그 안의 Google Test 295/295 통과 |
| 같은 UID, 유효 capability 0에서 SMACK 라벨만 바꿔 파일 읽기 | 허용 라벨은 성공, 다른 라벨은 `EACCES`. 커널 감사 로그 확인 |
| GCC/G++·GLib, CMake·Ninja, Cargo | CTest 1/1 통과, Cargo 오프라인 빌드·실행, GDB·strace 실행 성공 |
| Tizen CLI 2.5.25 | 일반 사용자로 C++ 및 Rust ELF의 TPK 생성, 원본 ELF 해시와 두 테스트 서명 파일 확인 |
| EFL GUI | Tizen Enlightenment의 카운터 창에서 버튼 클릭 전후 `0 → 1` 전환 확인. [WSLg 창 캡처](gui-counter.png) |
| ARMv7 크로스 빌드 | ARMv7 EABI5 ELF·RPM과 Rust 표준 라이브러리 검증. CMake C/GLib·C++ 테스트와 Cargo 프로그램을 QEMU user-mode로 실행 |
| Git·zypper·데이터 보존 | HTTPS 원격 Git 조회, 스냅샷에서 `ed` RPM 설치, 재부팅 후 Rust 프로그램 실행과 TPK 확인 |
| SDB 4.2.36 | 일반 사용자 `dev`로 격리된 Tizen `sdbd-3.0.50`에 TCP 접속. 원격 `owner` 셸, 21바이트 push·pull과 SHA-256 일치, 원격 파일 삭제 및 연결 해제 확인 |

호스트 루프백에 폐기 가능한 Tizen `sdbd`가 있고 대상의 `/mnt`가 원격 사용자에게 쓰기 가능하다면 위 검사에 `TIZEN_SDB_TEST_PORT=<호스트 포트>`를 추가하여 SDB 왕복도 실행할 수 있습니다.
