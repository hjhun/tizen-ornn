# x86_64에서 Tizen ARMv7 RPM 빌드

Windows의 Ubuntu WSL2와 x86_64 Ubuntu에서 같은 Linux ARM/v7 OCI 이미지를 사용한다.
컨테이너 안의 Tizen GCC 14.2가 C/C++ 코드를 컴파일하고, `ornn build`가 spec을
읽어 RPM을 만든다. Ubuntu 커널이 시스템 호출을 처리하고 QEMU가 ARM 명령을 실행한다.
장치의 커널, SMACK 정책이나 GUI 동작을 확인하려면 별도의 장치 검증이 필요하다.

## 현재 PC에서 사용

저장소 루트에서 다음 명령을 실행한다. 이 PC에는 이미지가 이미 로드되어 있다.
다른 PC로 옮길 때는 OCI 아카이브와 SHA256SUMS를 같은 디렉터리에 놓고
`sha256sum -c SHA256SUMS` 후 `podman load -i tizen-ornn-armv7l.oci.tar`를 실행한다.

```bash
runner="$PWD/container/armv7l/run-isolated"
cd tests/cross-rpm
"$runner" /usr/local/bin/ornn build --target armv7l --no-install
ls Build/RPMS/armv7l/
```

현재 디렉터리는 컨테이너의 `/workspace`에 읽기·쓰기로 연결된다. 결과는
호스트 사용자의 `Build/RPMS/<arch>/`에 남는다. spec은 현재 디렉터리 또는
`packaging/`에서 하나를 찾으며, 여러 개면 `--spec FILE`로 선택한다.
`Source` 파일은 spec과 같은 디렉터리에 준비한다. 출력 위치는 `--out DIR`로
지정한다. 처음 실행할 때만 Ubuntu의 APT 저장소에서 QEMU와 PRoot 패키지를
받는다. 이 실행기는 Ubuntu 24.04 amd64용이며 시스템의 GBS/binfmt 설정을
변경하지 않는다. 빌드 컨테이너는 네트워크 없이 실행한다.

일반 Ubuntu에서 ARM binfmt가 정상 등록되어 있으면 저장소 루트에서 Podman으로
바로 검증할 수 있다. 아래 검사는 RPM을 빌드하고 체크섬과 ELF 아키텍처를
검사한 뒤, 패키지에서 꺼낸 두 실행 파일을 실행한다.

```bash
podman run --rm --platform linux/arm/v7 --network none \
  -v "$PWD/tests/cross-rpm:/src:ro" \
  -v "$PWD/container/armv7l/verify:/verify:ro" localhost/tizen-ornn:armv7l /verify
```

## 이미지 다시 만들기

Ubuntu 24.04 amd64에서 Podman, 정상 동작하는 ARM binfmt 및 qemu-user-static,
Python 3.11 이상, curl, tar, util-linux와 loop mount 권한이 필요하다.
빌드 스크립트가 공식 headless 스냅샷의 rootfs와 /opt 이미지를 읽기 전용으로
마운트한다. 기본 도구와 헤더는 고정된 Unified 및 Base 저장소에서 SHA-256 검증
후 **ARM 이미지 안에** 설치된다. 이미지 패키지 설치 시에는 컨테이너 호스트에서
사용할 수 없는 장치용 SMACK 플러그인을 끈다.

```bash
container/armv7l/build-image localhost/tizen-ornn:armv7l
podman save --format oci-archive -o tizen-ornn-armv7l.oci.tar localhost/tizen-ornn:armv7l
```

현재 지원 범위는 필요한 도구와 BuildRequires가 이미지에 준비된 spec 기반 RPM
빌드다. 프로젝트별 의존성 자동 설치, Git 소스 아카이브 생성, TPK/RPK 서명·패키징은
별도 구현이 필요하다.
