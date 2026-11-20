#!/usr/bin/env bash
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
usage() { echo "Usage: $0 [--gui] [--foreground] [--share HOST_DIR | --share-ro HOST_DIR] [--no-host-network] [--host-browser]"; }
gui=0
managed=1
host_network=1
host_browser=0
share_mode=
share_path=
launch_options=()
while (( $# )); do
    case "$1" in
        --gui) gui=1; launch_options+=(--gui) ;;
        --foreground) managed=0 ;;
        --host-browser)
            (( EUID != 0 )) || { echo 'Start the host browser as your ordinary host user.' >&2; exit 2; }
            host_browser=1; launch_options+=(--host-browser) ;;
        --no-host-network) host_network=0; launch_options+=(--no-host-network) ;;
        --share|--share-ro)
            if [[ -n $share_mode || -z ${2:-} ]]; then usage >&2; exit 2; fi
            if (( EUID == 0 )); then
                echo 'Start the VM as your ordinary host user to share a directory.' >&2
                exit 2
            fi
            if ! IFS= read -r -d '' share_path < <(realpath -ze -- "$2") ||
                    [[ ! -d $share_path || ! -r $share_path || ! -x $share_path ]]; then
                printf 'Cannot share host directory: %s\n' "$2" >&2
                exit 2
            fi
            share_mode=rw
            if [[ $1 == --share-ro ]]; then share_mode=ro; fi
            if [[ $share_mode == rw && ! -w $share_path ]]; then
                echo 'Host directory is not writable. Use --share-ro for read-only access.' >&2
                exit 2
            fi
            launch_options+=("$1" "$share_path")
            shift ;;
        --help|-h) usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
    shift
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
    share_id=
    if [[ -n $share_mode ]]; then share_id="$share_mode:$(stat -Lc '%d-%i' -- "$share_path")"; fi
    exec "$project/scripts/console-session" "$disk" "$share_id" "$host_network" "$host_browser" env TIZEN_CONSOLE_MANAGED=1 \
        TIZEN_OUT="$out" TIZEN_DISK="$(realpath -- "$disk")" TIZEN_DISK_FORMAT="$format" \
        TIZEN_KERNEL="${TIZEN_KERNEL:+$(realpath -- "$kernel")}" TIZEN_SELECTION_LINK="${TIZEN_SELECTION_LINK:-}" \
        "$project/scripts/run-vm.sh" --foreground "${launch_options[@]}"
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
network_data=
if (( host_network )); then network_data=$("$project/scripts/host-network"); fi
if [[ -n $network_data ]]; then
    # QEMU needs a seekable file. Keep it private and anonymous across exec.
    network_file=$(mktemp "$prefix.network.XXXXXXXX")
    exec {network_fd}<>"$network_file"
    unlink "$network_file"
    printf '%s' "$network_data" >&"$network_fd"
    args+=(-fw_cfg "name=opt/tizen-vm/network-data,file=/proc/self/fd/$network_fd"
           -fw_cfg "name=opt/tizen-vm/network,file=${project//,/,,}/guest/tizen-vm-network")
fi
runner=()
if [[ -n $share_mode ]]; then
    readonly_flag=off
    if [[ $share_mode == ro ]]; then readonly_flag=on; fi
    args+=(-fsdev "local,id=hostshare,path=${share_path//,/,,},security_model=none,readonly=$readonly_flag"
           -device virtio-9p-pci,fsdev=hostshare,mount_tag=tizen-share
           -fw_cfg "name=opt/tizen-vm/share,string=$share_mode")
    # Map the developer's existing host identity to Tizen owner for 9P stat
    # and access checks, including private source trees.
    runner=(unshare --user --map-user=5001 --map-group=100 --)
fi
init=/usr/lib/systemd/systemd
init_args=
if [[ -n $share_mode || -n $network_data ]]; then
    candidates=("$kernel")
    if [[ -z ${TIZEN_KERNEL:-} ]]; then
        candidates+=("$project/kernel/bzImage" "$project/out/share-kernel/bzImage")
    fi
    kernel=
    for candidate in "${candidates[@]}"; do
        [[ -r $candidate ]] || continue
        config=$(dirname -- "$candidate")/kernel.config
        # An explicit custom kernel may ship without a companion config.
        if [[ -r $config || -z ${TIZEN_KERNEL:-} ]]; then
            grep -qx CONFIG_FW_CFG_SYSFS=y "$config" 2>/dev/null || continue
            if [[ -n $share_mode ]]; then
                grep -qx CONFIG_9P_FS=y "$config" && grep -qx CONFIG_NET_9P_VIRTIO=y "$config" || continue
            fi
        fi
        kernel=$candidate
        break
    done
    if [[ -z $kernel ]]; then
        echo 'This kernel lacks guest sharing or network configuration support. Install the updated host package or select a compatible TIZEN_KERNEL.' >&2
        exit 1
    fi
    args+=(-fw_cfg "name=opt/tizen-vm/boot,file=${project//,/,,}/guest/tizen-vm-boot")
    init=/bin/sh
    # A failed export must not open a shell over an ordinary guest directory.
    init_args=' -- -c "mount -t sysfs -o nosuid,nodev,noexec sysfs /sys && /bin/sh /sys/firmware/qemu_fw_cfg/by_name/opt/tizen-vm/boot/raw && exec /usr/lib/systemd/systemd; echo tizen-vm: guest setup failed, powering off >&2; printf ! >/dev/ttyS1; sync; exec /usr/bin/systemctl --force --force poweroff"'
fi
if (( host_browser )); then
    runner=("$project/scripts/qemu-boot-status" --host-browser "${runner[@]}")
elif [[ $init == /bin/sh ]]; then
    runner=("$project/scripts/qemu-boot-status" "${runner[@]}")
fi
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
exec "${runner[@]}" "$qemu" "${args[@]}" -machine pc -smp "${TIZEN_CPUS:-2}" -m "${TIZEN_RAM:-3072}" \
    -nodefaults "${display[@]}" -no-reboot -monitor none \
    -chardev stdio,id=console,signal=off -serial chardev:console \
    -kernel "$kernel" \
    -append "root=/dev/vda rootfstype=ext4 rootwait rw console=ttyS0,115200 init=$init systemd.unit=$target net.ifnames=0$console_cmdline$init_args" \
    -blockdev "driver=file,node-name=file,filename=${disk//,/,,}" \
    -blockdev "driver=$format,node-name=root,file=file" \
    -device virtio-blk-pci,drive=root \
    -netdev user,id=net -device virtio-net-pci,netdev=net,romfile= \
    -device virtio-rng-pci
