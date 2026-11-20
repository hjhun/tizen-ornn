# This file is sourced by login shells from /etc/profile.d.
case $- in *i*) ;; *) return ;; esac
[ -n "${BASH_VERSION:-}" ] && [ -t 0 ] || return
[[ /proc/self/fd/0 -ef /run/tizen-dev-console ]] || return

_tizen_cmdline=$(cat /proc/cmdline)
if [[ $(stty size) =~ ^(0\ [0-9]+|[0-9]+\ 0)$ ]]; then
    if [[ " $_tizen_cmdline " =~ \ tizen\.console_size=([1-9][0-9]{0,3})x([1-9][0-9]{0,3})\  ]]; then
        stty rows "${BASH_REMATCH[1]}" cols "${BASH_REMATCH[2]}"
    else
        stty rows 24 cols 80
    fi
fi
case " $_tizen_cmdline " in *" tizen.console=managed "*) ;; *) unset _tizen_cmdline; return ;; esac
unset _tizen_cmdline

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
