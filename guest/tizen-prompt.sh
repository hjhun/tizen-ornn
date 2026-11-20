case $- in *i*) ;; *) return ;; esac
[ -n "${BASH_VERSION:-}" ] || return
# Branch text is expanded as data, including branch names containing '$'.
_tizen_git_prompt() {
    local status head line ahead=0 dirty=0
    _tizen_branch= _tizen_color= _tizen_reset=
    status=$(git --no-optional-locks status --porcelain=v2 --branch 2>/dev/null) || return 0
    while IFS= read -r line; do
        case $line in
            '# branch.oid '*) head=${line#\# branch.oid } ;;
            '# branch.head '*) _tizen_branch=${line#\# branch.head } ;;
            '# branch.ab +'[1-9]*) ahead=1 ;;
            '# '*) ;;
            *) dirty=1 ;;
        esac
    done <<< "$status"
    if [[ $_tizen_branch == '(detached)' ]]; then _tizen_branch=${head:0:7}; fi
    _tizen_branch=" ($_tizen_branch)"
    if [[ ${TERM:-dumb} != dumb && -z ${NO_COLOR:-} ]]; then
        if (( dirty )); then _tizen_color=$'\e[31m'
        elif (( ahead )); then _tizen_color=$'\e[36m'
        else _tizen_color=$'\e[32m'
        fi
        _tizen_reset=$'\e[0m'
    fi
}
case "${PROMPT_COMMAND[*]-}" in
    *'_tizen_git_prompt'*) ;;
    *) PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND$'\n'}_tizen_git_prompt" ;;
esac
PS1='\u:\w\[${_tizen_color-}\]${_tizen_branch-}\[${_tizen_reset-}\]> '
