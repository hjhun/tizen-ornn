#!/usr/bin/env bash
# Adapt the stock emulator's boot dependencies to the single-disk QEMU VM.
set -euo pipefail
root=$(cd "${1:?usage: configure-platform.sh MOUNTED_GUEST}" && pwd -P)
[[ $EUID == 0 && "$root" != / && -d "$root/usr/lib/systemd/system" ]]
mountpoint -q "$root"
units="$root/etc/systemd/system"
# This VM has one ext4 volume and uses the development console on ttyS0.
ln -sfn tizen-dev.target "$units/default.target"
ln -sfn /dev/null "$units/"'dev-disk-by\x2dlabel-emulator\x2dswap.swap'
ln -sfn /dev/null "$units/serial-getty@ttyS0.service"

# Start the matching stock service if a matching peripheral is present.
for entry in 'emul-setup-audio-volume.service /dev/snd/controlC*' \
             'vmodem-x86.service /dev/vmodem0' \
             'wifi-tethering-manager.service /sys/class/ieee80211/phy*'; do
    unit=${entry%% *}
    device=${entry#* }
    install -d "$units/$unit.d"
    printf '[Unit]\nConditionPathExistsGlob=%s\n' "$device" > "$units/$unit.d/tizen-dev.conf"
done

# The snapshot's stability monitor uses the Tizen-specific proc_tsm provider.
# Its own startup loads the module when it is available for the running kernel.
install -d "$units/stability-monitor.service.d"
cat > "$units/stability-monitor.service.d/tizen-dev.conf" <<'UNIT'
[Service]
ExecCondition=/bin/sh -c 'test -e /proc/tsm || /usr/sbin/modinfo proc_tsm >/dev/null 2>&1'
UNIT

# The standalone VM accepts connections through the snapshot's TCP socket.
# The Studio eager start expects a host peer during emulator initialization.
studio_link="$root/usr/lib/systemd/system/emulator_preinit.target.wants/sdbd.service"
if [[ -L $studio_link ]]; then
    unlink "$studio_link"
fi
install -d "$units/sdbd.service.d"
cat > "$units/sdbd.service.d/tizen-dev.conf" <<'UNIT'
[Service]
ExecStart=
ExecStart=/usr/sbin/sdbd
UNIT

install -d "$units/hal-compatibility-checker.service.d"
cat > "$units/hal-compatibility-checker.service.d/tizen-dev.conf" <<'UNIT'
[Unit]
Requires=dlconf.service
After=dlconf.service
UNIT
