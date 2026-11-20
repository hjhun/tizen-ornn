# The owner account keeps its writable development home under workspace.
case "$(id -un)" in
    root) _tizen_bin=/root/.local/bin ;;
    owner) _tizen_bin="$(getent passwd owner | cut -d: -f6)/workspace/.ai-home/.local/bin" ;;
    dev) _tizen_bin="$HOME/.local/bin" ;;
    *) _tizen_bin= ;;
esac
# Move each default to the front, including on a repeated login.
_tizen_prepend() {
    _tizen_path=:$PATH:
    while :; do
        case $_tizen_path in
            *:"$1":*) _tizen_path=${_tizen_path%%:"$1":*}:${_tizen_path#*:"$1":} ;;
            *) break ;;
        esac
    done
    _tizen_path=${_tizen_path#:}
    _tizen_path=${_tizen_path%:}
    PATH=$1${_tizen_path:+:$_tizen_path}
}
_tizen_prepend /usr/local/bin
[ -z "$_tizen_bin" ] || _tizen_prepend "$_tizen_bin"
export PATH
unset _tizen_bin _tizen_path
unset -f _tizen_prepend
