#!/usr/bin/env bash
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
usage() { echo "Usage: $0 [--gui] [--foreground]"; }
gui=0
managed=1
for option in "$@"; do
    case "$option" in
        --gui) gui=1 ;;
        --foreground) managed=0 ;;
        --help|-h) usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
done
out=${TIZEN_OUT:-$project/out}
if [[ -z ${TIZEN_OUT:-} ]]; then
    if [[ -L $out/.tizen-current || ( -d $out/current && ! -L $out/current ) ]]; then
        selection=$out/.tizen-current
    else
        selection=$out/current
    fi
    if [[ -z ${TIZEN_DISK:-} ]]; then export TIZEN_SELECTION_LINK=$selection; fi
    if [[ -d $selection ]]; then out=$selection
    elif [[ -d $out/current ]]; then out=$out/current
    fi
fi
out=$(realpath -- "$out")
kernel=${TIZEN_KERNEL:-$out/bzImage}
default_disk=$out/tizen-rootfs.ext4
if [[ ! -e $default_disk && -f $out/tizen-rootfs.qcow2 ]]; then
    default_disk=$out/tizen-rootfs.qcow2
fi
disk=${TIZEN_DISK:-$default_disk}
format=${TIZEN_DISK_FORMAT:-raw}
if [[ -z ${TIZEN_DISK_FORMAT:-} && $disk == *.qcow2 ]]; then format=qcow2; fi
[[ -r "$kernel" && -w "$disk" ]] || { echo 'Missing kernel or writable Tizen disk' >&2; exit 1; }
if (( managed )) && [[ -t 0 && -t 1 ]]; then
    if [[ ! -f $out/.tizen-vm-image-revision ]]; then
        echo "Legacy image: upgrade with tizen-vm update --image to detach by typing exit." >&2
    fi
    # The pane owns the disk lock. A client can leave without releasing it.
    exec "$project/scripts/console-session" "$disk" env TIZEN_CONSOLE_MANAGED=1 \
        TIZEN_OUT="$out" TIZEN_DISK="$(realpath -- "$disk")" TIZEN_DISK_FORMAT="$format" \
        TIZEN_KERNEL="$(realpath -- "$kernel")" TIZEN_SELECTION_LINK="${TIZEN_SELECTION_LINK:-}" \
        "$project/scripts/run-vm.sh" --foreground "$@"
fi
# Hold the same lock as the packager for this guest's lifetime.
exec 9<"$disk"
flock -n -x 9 || { echo "Tizen disk is already in use: $disk" >&2; exit 1; }
if [[ -n ${TIZEN_SELECTION_LINK:-} && -L $TIZEN_SELECTION_LINK && ! $TIZEN_SELECTION_LINK -ef $out ]]; then
    echo 'Active image changed during startup. Run tizen-vm again.' >&2
    exit 1
fi
if [[ -n "$(losetup -j "$disk")" ]]; then
    echo "Disk still attached to a host loop device: $disk" >&2
    exit 1
fi
target=tizen-dev.target
display=(-display none)
if (( gui )); then
    target=tizen-dev-gui.target
    display=(-display sdl,gl=off -name Tizen-Dev
        -device virtio-gpu-pci,xres=1024,yres=768
        -device virtio-keyboard-pci -device virtio-tablet-pci)
fi
prefix=$("$project/scripts/vm-runtime" "$disk")
args=(-device virtio-serial-pci,id=guestbus
    -chardev "socket,id=packages,path=$prefix.packages,server=on,wait=off"
    -device virtserialport,bus=guestbus.0,chardev=packages,name=org.tizen.vm.packages)
if [[ -n ${TIZEN_QMP:-} ]]; then
    args+=(-qmp "unix:${TIZEN_QMP//,/,,},server=on,wait=off")
fi
if command -v qemu-system-x86_64 >/dev/null; then
    qemu=$(command -v qemu-system-x86_64)
else
    host=$project/out/host-tools/usr
    qemu=$host/bin/qemu-system-x86_64
    [[ -x "$qemu" ]] || { echo 'Install qemu-system-x86 and seabios on the Linux host' >&2; exit 1; }
    export LD_LIBRARY_PATH="$host/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export QEMU_MODULE_DIR="$host/lib/x86_64-linux-gnu/qemu"
    args+=(-L "$host/share/qemu" -bios "$host/share/seabios/bios-256k.bin")
fi
if [[ -r /dev/kvm && -w /dev/kvm ]]; then
    args+=(-accel kvm -cpu host)
else
    args+=(-accel tcg -cpu max)
fi
if [[ -n ${TIZEN_USB:-} ]]; then
    if [[ ! $TIZEN_USB =~ ^([0-9]{1,3}):([0-9]{1,3})$ ]]; then
        echo 'Set TIZEN_USB to the host bus:device from lsusb (e.g. 001:004)' >&2
        exit 2
    fi
    args+=(-device qemu-xhci,id=xhci -device "usb-host,bus=xhci.0,hostbus=$((10#${BASH_REMATCH[1]})),hostaddr=$((10#${BASH_REMATCH[2]}))")
fi
console_cmdline=
if [[ ${TIZEN_CONSOLE_MANAGED:-0} == 1 ]]; then console_cmdline=' tizen.console=managed'; fi
exec "$qemu" "${args[@]}" -machine pc -smp "${TIZEN_CPUS:-2}" -m "${TIZEN_RAM:-3072}" \
    -nodefaults "${display[@]}" -no-reboot -monitor none \
    -chardev stdio,id=console,signal=off -serial chardev:console \
    -kernel "$kernel" \
    -append "root=/dev/vda rootfstype=ext4 rootwait rw console=ttyS0,115200 init=/usr/lib/systemd/systemd systemd.unit=$target net.ifnames=0$console_cmdline" \
    -blockdev "driver=file,node-name=file,filename=${disk//,/,,}" \
    -blockdev "driver=$format,node-name=root,file=file" \
    -device virtio-blk-pci,drive=root \
    -netdev user,id=net -device virtio-net-pci,netdev=net,romfile= \
    -device virtio-rng-pci
