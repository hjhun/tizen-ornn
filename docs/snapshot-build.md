# 새 Tizen 스냅샷에서 개발 이미지 만들기

Ubuntu 24.04 x86_64 또는 WSL2의 Ubuntu 셸에서 배포본이나 저장소의 최상위 디렉터리로 이동합니다. 호스트 의존성은 README의 ‘처음 실행하기’에서 설치합니다. [호스트 설치 명령](../README.md#처음-실행하기) 이 도구는 공식 **Tizen 10.1 Unified의 날짜가 붙은 headed emulator64 Wayland** 원본에서 x86_64용 SMACK 커널과 쓰기 가능한 개발 디스크를 만듭니다. 작업 디렉터리는 루프 마운트와 ext4 보안 확장 속성을 지원해야 합니다.

```sh
release=tizen-10.1-unified_20261119.053314
output="$PWD/out/$release-dev"
sudo env JOBS=4 ./scripts/tizen-image --snapshot "$release" --out "$output"
TIZEN_OUT="$output" ./scripts/run-vm.sh
```

`--snapshot`에는 다른 날짜의 Unified ID 또는 해당 디렉터리의 공식 `*_tizen-headed-emulator64-wayland.tar.gz` HTTPS URL을 넣을 수 있습니다. 날짜를 바꿀 때마다 새 출력 디렉터리를 사용합니다. 커널은 `$output/bzImage`, sparse 디스크는 `$output/tizen-rootfs.ext4`입니다. GUI를 시작하려면 마지막 명령 뒤에 `--gui`를 붙입니다. 디스크의 기본 논리 크기는 16 GiB이며 새 빌드에서는 `sudo env JOBS=4 TIZEN_FS_SIZE=32G ...`처럼 정할 수 있습니다.

원본을 이미 내려받은 경우에는 로컬 파일을 지정할 수 있습니다. 이름을 바꾸어 둔 파일도 공식 SHA-256과 바이트가 일치하면 사용합니다. 이 경우에도 저장소 메타데이터와 필요한 패키지를 받으려면 네트워크가 필요합니다.

```sh
sudo env JOBS=4 ./scripts/tizen-image \
  --snapshot "$release" --out "$output" \
  --archive "$HOME/Downloads/${release}_tizen-headed-emulator64-wayland.tar.gz"
```

먼저 다운로드 입력을 점검하려면 같은 명령에 `--lock-only`를 붙입니다. 이 단계는 일반 사용자로 실행할 수 있고 커널 빌드나 루프 마운트를 수행하지 않습니다. 외부에서 확인한 원본 SHA-256도 대조하려면 `--sha256 HEX`를 추가합니다. URL만 지정한 `--lock-only`는 원본의 공개 체크섬과 메타데이터를 기록합니다. `--archive`도 지정하면 그 파일의 전체 바이트를 읽어 대조합니다.

`snapshot.lock.json`에는 원본 이미지 URL과 SHA-256, 이미지 파티션 XML, Unified의 `build.xml`, 정해진 Base·Unified RPM 저장소의 메타데이터 해시, ARM sysroot와 Rust 표준 라이브러리의 RPM별 URL·SHA-256이 들어갑니다. 게스트에도 `/usr/share/tizen-snapshot/snapshot.lock.json`을 설치합니다. 같은 명령을 다시 실행하면 기존 입력과 일치하는지 확인합니다. 중단된 빌드는 같은 입력으로 재개할 수 있고, 완성한 개발 디스크는 별도 출력 경로에 새로 생성합니다.

## 스냅샷이 결정하는 범위

- 이미지별 XML에서 x86_64 아키텍처와 `emulator-rootfs.img`, `emulator-sysdata.img`의 ext4 구성을 확인합니다. 원본 아카이브와 파티션 XML은 공식 디렉터리의 `SHA256SUMS`와 대조합니다.
- Unified `build.xml`의 `base_id`로 Base 날짜를 찾습니다. 네이티브 RPM에는 해당 Unified emulator와 Base standard, ARM에는 Unified standard와 Base standard를 씁니다. 각 저장소의 `repomd.xml` SHA-256을 고정하고 primary와 개별 RPM 체크섬을 검증합니다. ARM 패키지 선정은 배포 레시피에 선언한 패키지 이름을 따릅니다.
- 커널은 RPi4 소스 RPM `rpi4-linux-kernel-6.12.41-0.src.rpm`과 Gerrit 리비전 `b6322b453844b6111e4619615fc65bd29b86c83e`에서 x86_64용으로 다시 빌드합니다. SDK는 `sdk/packages.sha256`에 고정한 CLI ZIP 패키지 2.5.107과 SDB 4.2.36 등을 사용합니다. 이 CLI가 게스트에서 표시하는 버전은 2.5.25입니다. Git은 레시피의 2.56.1을 사용합니다. 이 입력들의 버전을 변경하려면 해당 레시피와 런타임 호환성을 별도로 검증해야 합니다.
- 날짜가 달라지면 제공 RPM, Rust 타깃, 라이브러리 ABI, 서비스 그래프가 달라질 수 있습니다. 도구가 입력을 확인한 뒤 실제 빌드에서 RPM 트랜잭션·필수 파일을 검사합니다. 새 디스크를 부팅한 뒤 `tizen-linux-tools-check`와 실제 대상 프로젝트의 `tizen-build test`로 개발 동작을 확인할 수 있습니다.

[공식 Tizen 10.1 Unified 스냅샷 목록](https://download.tizen.org/snapshots/TIZEN/Tizen-10.1/Tizen-10.1-Unified/) · [예시 스냅샷의 Base·저장소 정보](https://download.tizen.org/snapshots/TIZEN/Tizen-10.1/Tizen-10.1-Unified/tizen-10.1-unified_20261119.053314/build.xml)
