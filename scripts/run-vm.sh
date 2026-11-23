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
# Plain reattach keeps the running session's share, even after a one-boot override.
implicit_share=0
if [[ -n ${TIZEN_ORNN_BOOT_SHARE:-} ]]; then
    set -- "$@" --share "$TIZEN_ORNN_BOOT_SHARE"
    unset TIZEN_ORNN_BOOT_SHARE
    implicit_share=1
fi
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
# Use the repaired packaged kernel for disks created with the public R6 kernel.
if [[ -z ${TIZEN_KERNEL:-} && -f $project/kernel/bzImage && -r $kernel ]] &&
    [[ $(sha256sum < "$kernel" | cut -d' ' -f1) == $(awk '$2 == "bzImage"{print $1}' "$project/kernel/r6.sha256") ]]; then
    kernel=$project/kernel/bzImage
fi
default_disk=$out/tizen-rootfs.ext4
if [[ ! -e $default_disk && -f $out/tizen-rootfs.qcow2 ]]; then
    default_disk=$out/tizen-rootfs.qcow2
fi
disk=${TIZEN_DISK:-$default_disk}
format=${TIZEN_DISK_FORMAT:-raw}
if [[ -z ${TIZEN_DISK_FORMAT:-} && $disk == *.qcow2 ]]; then format=qcow2; fi
[[ -r "$kernel" && -w "$disk" ]] || { echo 'Missing kernel or writable Tizen disk' >&2; exit 1; }
# QEMU's emulated UART forwards bytes but not the host's terminal size.
console_size=${TIZEN_CONSOLE_SIZE:-$(stty size 2>/dev/null || :)}
[[ $console_size =~ ^[1-9][0-9]{0,3}\ [1-9][0-9]{0,3}$ ]] || console_size='24 80'
if (( managed )) && [[ -t 0 && -t 1 ]]; then
    export TIZEN_CONSOLE_SIZE="$console_size"
    if [[ ! -f $out/.tizen-ornn-image-revision ]]; then
        echo "Legacy image: upgrade with tizen-ornn update --image to detach by typing exit." >&2
    fi
    # The pane owns the disk lock. A client can leave without releasing it.
    share_id=
    if [[ -n $share_mode ]]; then share_id="$share_mode:$(stat -Lc '%d-%i' -- "$share_path")"; fi
    if (( implicit_share )); then share_id="tizen:$share_id"; fi
    export TIZEN_ORNN_IMPLICIT_SHARE=$implicit_share
    exec "$project/scripts/console-session" "$disk" "$share_id" "$host_network" "$host_browser" env TIZEN_CONSOLE_MANAGED=1 \
        TIZEN_OUT="$out" TIZEN_DISK="$(realpath -- "$disk")" TIZEN_DISK_FORMAT="$format" \
        TIZEN_KERNEL="${TIZEN_KERNEL:+$(realpath -- "$kernel")}" TIZEN_SELECTION_LINK="${TIZEN_SELECTION_LINK:-}" \
        "$project/scripts/run-vm.sh" --foreground "${launch_options[@]}"
fi
# Hold the same lock as the packager for this guest's lifetime.
exec 9<"$disk"
flock -n -x 9 || { echo "Tizen disk is already in use: $disk" >&2; exit 1; }
if [[ -n ${TIZEN_SELECTION_LINK:-} && -L $TIZEN_SELECTION_LINK && ! $TIZEN_SELECTION_LINK -ef $out ]]; then
    echo 'Active image changed during startup. Run tizen-ornn again.' >&2
    exit 1
fi
if [[ -n "$(losetup -j "$disk")" ]]; then
    echo "Disk still attached to a host loop device: $disk" >&2
    exit 1
fi
target=tizen-dev.target
display=(-display none)
if (( gui )); then
    target=graphical.target
    case ${TIZEN_GUI_GL:-1} in
        1) gpu=virtio-gpu-gl-pci; gl=on ;;
        0) gpu=virtio-gpu-pci; gl=off ;;
        *) echo 'TIZEN_GUI_GL must be 1 or 0' >&2; exit 2 ;;
    esac
    display=(-display "sdl,gl=$gl,show-cursor=on" -name 'Tizen Ornn'
        -device "$gpu,xres=1024,yres=768"
        -device virtio-keyboard-pci -device virtio-tablet-pci)
fi
prefix=$("$project/scripts/vm-runtime" "$disk")
args=(-device virtio-serial-pci,id=guestbus
    -chardev "socket,id=packages,path=$prefix.packages,server=on,wait=off"
    -device virtserialport,bus=guestbus.0,chardev=packages,name=org.tizen.vm.packages)
if (( gui )); then
    if [[ $gl == off ]]; then
        args+=(-fw_cfg name=opt/tizen-ornn/software-renderer,string=1)
    fi
    args+=(-fw_cfg "name=opt/tizen-ornn/graphics,file=${project//,/,,}/guest/tizen-ornn-graphics")
fi
network_data=
if (( host_network )); then network_data=$("$project/scripts/host-network"); fi
if [[ -n $network_data ]]; then
    # QEMU needs a seekable file. Keep it private and anonymous across exec.
    network_file=$(mktemp "$prefix.network.XXXXXXXX")
    exec {network_fd}<>"$network_file"
    unlink "$network_file"
    printf '%s' "$network_data" >&"$network_fd"
    args+=(-fw_cfg "name=opt/tizen-ornn/network-data,file=/proc/self/fd/$network_fd"
           -fw_cfg "name=opt/tizen-ornn/network,file=${project//,/,,}/guest/tizen-ornn-network")
fi
runner=()
if [[ -n $share_mode ]]; then
    readonly_flag=off
    if [[ $share_mode == ro ]]; then readonly_flag=on; fi
    args+=(-fsdev "local,id=hostshare,path=${share_path//,/,,},security_model=none,readonly=$readonly_flag"
           -device virtio-9p-pci,fsdev=hostshare,mount_tag=tizen-share
           -fw_cfg "name=opt/tizen-ornn/share,string=$share_mode"
           -fw_cfg "name=opt/tizen-ornn/share-script,file=${project//,/,,}/guest/tizen-ornn-share")
    mapped_user=10000
    mapped_group=10000
    if (( ! implicit_share )) && [[ ${TIZEN_ORNN_IMPLICIT_SHARE:-0} != 1 ]]; then
        mapped_user=5001
        mapped_group=100
        args+=(-fw_cfg name=opt/tizen-ornn/owner-share,string=1)
    fi
    # Map the developer's existing host identity to the development account for 9P stat
    # and access checks, including private source trees.
    runner=(unshare --user --map-user="$mapped_user" --map-group="$mapped_group" --)
fi
init=/usr/lib/systemd/systemd
init_args=
if (( gui )) || [[ -n $share_mode || -n $network_data ]]; then
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
        echo 'No compatible kernel was found for the requested guest boot options. Install the updated host package or select a compatible TIZEN_KERNEL.' >&2
        exit 1
    fi
    args+=(-fw_cfg "name=opt/tizen-ornn/boot,file=${project//,/,,}/guest/tizen-ornn-boot")
    init=/bin/sh
    # A failed export must not open a shell over an ordinary guest directory.
    init_args=' -- -c "mount -t sysfs -o nosuid,nodev,noexec sysfs /sys && /bin/sh /sys/firmware/qemu_fw_cfg/by_name/opt/tizen-ornn/boot/raw && exec /usr/lib/systemd/systemd; echo tizen-ornn: guest setup failed, powering off >&2; printf ! >/dev/ttyS1; sync; exec /usr/bin/systemctl --force --force poweroff"'
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
# Keep UART 0 reserved so the private boot-status channel remains ttyS1.
console=ttyS0,115200
serial=(-serial chardev:console)
if [[ -f $out/.tizen-virtio-console ]] &&
    grep -qx CONFIG_VIRTIO_CONSOLE=y "$(dirname -- "$kernel")/kernel.config" 2>/dev/null; then
    console=hvc0
    serial=(-serial null -device virtconsole,bus=guestbus.0,nr=0,chardev=console)
fi
console_cmdline=" tizen.console_size=${console_size/ /x}"
if [[ ${TIZEN_CONSOLE_MANAGED:-0} == 1 ]]; then console_cmdline+=' tizen.console=managed'; fi
exec "${runner[@]}" "$qemu" "${args[@]}" -machine pc -smp "${TIZEN_CPUS:-2}" -m "${TIZEN_RAM:-3072}" \
    -nodefaults "${display[@]}" -monitor none \
    -chardev stdio,id=console,signal=off "${serial[@]}" \
    -kernel "$kernel" \
    -append "root=/dev/vda rootfstype=ext4 rootwait rw console=$console init=$init systemd.unit=$target net.ifnames=0$console_cmdline$init_args" \
    -blockdev "driver=file,node-name=file,filename=${disk//,/,,}" \
    -blockdev "driver=$format,node-name=root,file=file" \
    -device virtio-blk-pci,drive=root \
    -netdev user,id=net -device virtio-net-pci,netdev=net,romfile= \
    -device virtio-rng-pci
