# The owner home is shared with Tizen services and retains its platform label.
if [ "${USER:-}" = owner ]; then
    if [ ! -e "$HOME/.gitconfig" ] && [ ! -e "${XDG_CONFIG_HOME:-$HOME/.config}/git/config" ]; then
        export GIT_CONFIG_GLOBAL="${GIT_CONFIG_GLOBAL:-$HOME/workspace/.gitconfig}"
    fi
    if [ "${HISTFILE:-$HOME/.bash_history}" = "$HOME/.bash_history" ]; then
        HISTFILE=$HOME/workspace/.bash_history
    fi
    if [ -S "/run/user/$(id -u)/bus" ]; then
        export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
        export DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-unix:path=$XDG_RUNTIME_DIR/bus}"
    fi
fi
