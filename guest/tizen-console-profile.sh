# This file is sourced by login shells from /etc/profile.d.
case $- in *i*) ;; *) return ;; esac
[ -n "${BASH_VERSION:-}" ] && [ -t 0 ] || return
case " $(cat /proc/cmdline) " in *" tizen.console=managed "*) ;; *) return ;; esac
[ "$(tty)" = /dev/ttyS0 ] || return

IGNOREEOF=10
# systemd starts the top-level Bash directly. su and nested shells keep
# their ordinary exit semantics. A detached console keeps its working jobs.
[ "$PPID" = 1 ] || return
_tizen_console_pid=$BASHPID
_tizen_console_exit() {
    if [ "$BASHPID" != "$_tizen_console_pid" ]; then
        builtin exit "$@"
    fi
    printf '\033ktizen-console-detach\033\\' > /dev/tty
}
alias exit='_tizen_console_exit'
