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
ln -sfn /dev/null "$root/etc/systemd/system/serial-getty@hvc0.service"
install -m755 "$project/guest/tizen-dev-setup" "$root/usr/local/libexec/"
install -m755 "$project/tests/smack.py" "$root/usr/local/libexec/tizen-dev-smack-check"
install -m755 "$project/guest/dev-check" "$root/usr/local/bin/tizen-dev-check"
install -m644 "$project/guest/tizen-dev-path.sh" "$root/etc/profile.d/zz-tizen-dev-path.sh"
install -m644 "$project/guest/tizen-prompt.sh" "$root/etc/profile.d/zz-tizen-prompt.sh"
install -m755 "$project/guest/tizen-su" "$root/usr/local/bin/su"
install -m644 "$project/guest/tizen-console-profile.sh" "$root/etc/profile.d/zz-tizen-console.sh"
install -Dm644 "$project/packaging/ui-demo/tizen-ui-counter.spec" \
    "$root/usr/share/tizen-dev/examples/efl-counter/tizen-ui-counter.spec"
install -m755 "$project/packaging/ui-demo/build-rpm.sh" \
    "$root/usr/share/tizen-dev/examples/efl-counter/"
if ! chroot "$root" /usr/bin/id tizen >/dev/null 2>&1; then
    chroot "$root" /usr/sbin/useradd -m -d /home/tizen -s /bin/bash tizen
fi
chroot "$root" /usr/sbin/usermod -a -G root tizen
install -d "$root/usr/share/tizen-dev/dev-setup"
cp -R "$project/guest/dev-setup/." "$root/usr/share/tizen-dev/dev-setup/"
install -m755 "$project/guest/tizen-init-home" "$root/usr/local/bin/"
for account in root tizen owner; do
    IFS=: read -r _ _ uid gid _ home _ < <(chroot "$root" getent passwd "$account")
    # Resolve /home inside the guest, even when the snapshot uses an absolute symlink.
    home=$(chroot "$root" readlink -f "$home")
    if [[ $account == tizen ]]; then
        dest="$root$home"
        install -d -o "$uid" -g "$gid" "$dest/workspace" "$dest/dev-setup"
        cp -R "$project/guest/dev-setup/." "$dest/dev-setup/"
        chown -R "$uid:$gid" "$dest/dev-setup"
        # Keep image defaults in the user's editable setup kit.
        for f in bashrc bash_profile vimrc; do
            if [[ ! -e "$dest/.$f" && ! -L "$dest/.$f" ]]; then
                ln -s "dev-setup/$f" "$dest/.$f"
                chown -h "$uid:$gid" "$dest/.$f"
            fi
        done
        find "$dest" -xdev ! -type l -exec setfattr -n security.SMACK64 -v 'User::Shell' {} +
        find "$dest" -xdev -type d -exec setfattr -n security.SMACK64TRANSMUTE -v TRUE {} +
        continue
    fi
    if [[ ! -e "$root$home/.bash_profile" && ! -e "$root$home/.bash_login" && ! -e "$root$home/.profile" ]]; then
        printf '%s\n' 'case $- in *i*) [ ! -r ~/.bashrc ] || . ~/.bashrc ;; esac' > "$root$home/.profile"
        chown "$uid:$gid" "$root$home/.profile"
    fi
    if [[ ! -e "$root$home/.bashrc" ]]; then
        install -o "$uid" -g "$gid" -m644 "$project/guest/tizen-bashrc" "$root$home/.bashrc"
        case $account in
            owner) setfattr -n security.SMACK64 -v 'User::Shell' "$root$home/.bashrc" ;;
        esac
    fi
done
# The snapshot's Dali login hook cleans the platform owner's private cache.
dali="$root/etc/profile.d/dali3-shader-conf.sh"
if [[ -f $dali ]] && ! grep -q '^# tizen-ornn owner cache' "$dali"; then
    sed -i '1i# tizen-ornn owner cache' "$dali"
    sed -i '2i[ "$(id -un)" != tizen ] || return 0' "$dali"
fi
# Local development VM: permit tizen to enter the root login shell.
if ! grep -qxF 'root:tizen:NOPASS' "$root/etc/suauth" 2>/dev/null; then
    (umask 077; echo 'root:tizen:NOPASS' >> "$root/etc/suauth")
fi
printf 'nameserver 10.0.2.3\n' > "$root/etc/resolv.conf"
"$project/scripts/configure-platform.sh" "$root"
