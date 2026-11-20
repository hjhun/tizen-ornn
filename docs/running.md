# VM 실행과 저장공간

이미지 생성 시 지정한 디렉터리에는 커널과 쓰기 가능한 게스트 파일시스템이 있습니다. 아래는 저장소에서 실행하는 호스트 명령입니다.

```sh
export TIZEN_OUT="$PWD/out/dev"
./scripts/run-vm.sh
# 다음 부팅에 GUI와 다른 자원을 사용하는 예
TIZEN_CPUS=4 TIZEN_RAM=4096 ./scripts/run-vm.sh --gui
```

기본 CPU는 2개, 메모리는 3072 MiB입니다. 사용자가 호스트의 `/dev/kvm`에 접근할 수 있으면 KVM을 사용하고, 그 외에는 QEMU 소프트웨어 실행을 사용합니다. 네트워크는 기본적으로 사용자 모드 NAT입니다. GUI에는 WSLg 또는 Ubuntu 그래픽 세션이 필요합니다. 데스크톱 앱 창과 별도로 터미널에는 게스트의 root 셸이 열립니다. 종료는 게스트에서 `systemctl poweroff`로 요청합니다.

## 디스크

기본 디스크 `tizen-rootfs.ext4`는 논리 크기 16 GiB의 sparse 파일입니다. 설치한 패키지와 소스, 빌드 결과는 게스트를 껐다 켜도 남습니다. 호스트에서는 실제 기록한 블록만큼 공간을 사용합니다. 게스트 안에서 `df -h /`, 호스트에서 `du -h "$TIZEN_OUT/tizen-rootfs.ext4"`로 각각 확인합니다. 홈의 개발 공간은 `/home/owner/workspace`입니다.

처음부터 크게 만들려면 `sudo env TIZEN_FS_SIZE=32G ./scripts/tizen-image --out "$PWD/out/larger"`로 생성합니다. 기존 raw 디스크는 **VM을 종료하고 호스트 마운트도 해제한 후** 백업하고 확장합니다.

```sh
disk="$TIZEN_OUT/tizen-rootfs.ext4"
truncate -s +8G "$disk"
e2fsck -f "$disk"
resize2fs "$disk"
```

## 로컬 아카이브

정지된 디스크와 커널, 실행 스크립트를 함께 묶을 수 있습니다. SDK와 개발자가 추가한 소스 및 데이터도 디스크에 들어갑니다.

```sh
./scripts/package-image.sh "$TIZEN_OUT" "$PWD/dist"
cd dist
sha256sum -c tizen-dev-10.1-x86_64-20261119.053314-r4.tar.zst.sha256
tar --zstd -xf tizen-dev-10.1-x86_64-20261119.053314-r4.tar.zst
cd tizen-dev-10.1-x86_64-20261119.053314-r4
sha256sum -c SHA256SUMS
unset TIZEN_OUT
./scripts/run-vm.sh --gui
```

파일 이름의 날짜와 시각은 선택한 스냅샷에 따라 달라집니다. 압축을 푼 뒤에는 호스트에 `qemu-system-x86`, `qemu-system-gui`, `seabios`, `util-linux`가 있으면 실행할 수 있습니다.
