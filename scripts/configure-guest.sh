#!/usr/bin/env bash
set -euo pipefail
project=$(cd "$(dirname "$0")/.." && pwd)
root=${1:?usage: configure-guest.sh MOUNTED_GUEST}
[[ $EUID == 0 && "$(realpath "$root")" != / && -x "$root/usr/bin/rpm" ]]
mountpoint -q "$root"
install -d "$root/etc/systemd/system" "$root/usr/local/libexec" "$root/usr/local/bin"
for unit in tizen-dev.target tizen-dev-setup.service tizen-dev-console.service; do
    install -m644 "$project/guest/$unit" "$root/etc/systemd/system/$unit"
done
install -m755 "$project/guest/tizen-dev-setup" "$root/usr/local/libexec/"
install -m755 "$project/tests/smack.py" "$root/usr/local/libexec/tizen-dev-smack-check"
install -m755 "$project/guest/dev-check" "$root/usr/local/bin/tizen-dev-check"
install -m644 "$project/guest/tizen-dev-path.sh" "$root/etc/profile.d/zz-tizen-dev-path.sh"
install -Dm644 "$project/packaging/ui-demo/tizen-ui-counter.spec" \
    "$root/usr/share/tizen-dev/examples/efl-counter/tizen-ui-counter.spec"
install -m755 "$project/packaging/ui-demo/build-rpm.sh" \
    "$root/usr/share/tizen-dev/examples/efl-counter/"
if ! chroot "$root" /usr/bin/id dev >/dev/null 2>&1; then
    chroot "$root" /usr/sbin/useradd -m -d /home/dev -s /bin/bash dev
fi
# New paths in the offline root need explicit Tizen security attributes.
setfattr -n security.SMACK64 -v 'User::Shell' "$root/home/dev"
setfattr -n security.SMACK64TRANSMUTE -v TRUE "$root/home/dev"
printf 'nameserver 10.0.2.3\n' > "$root/etc/resolv.conf"
"$project/scripts/configure-platform.sh" "$root"
