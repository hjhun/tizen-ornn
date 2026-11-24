#!/bin/bash
set -eu

case "${1:-}" in
    -h|--help) echo 'Usage: ~/dev-setup/install.sh [--with-ssh-key]'; exit 0 ;;
    ''|--with-ssh-key) ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
esac
[[ $# -le 1 && $(id -un) == tizen ]] || {
    echo 'Run as tizen: ~/dev-setup/install.sh [--with-ssh-key]' >&2
    exit 2
}
base=$HOME/dev-setup
export PATH="$base/bin:$base/cli/bin:$PATH"

link_setting() {
    local source=$base/$1 target=$HOME/$2
    [[ -e "$source" ]] || { echo "Missing setting: $source" >&2; return 1; }
    if [[ "$target" -ef "$source" ]]; then return; fi
    if [[ -e "$target" || -L "$target" ]]; then
        echo "Existing setting needs manual merging: $target (source: $source)" >&2
        return 1
    fi
    ln -s "$source" "$target"
}

failed=()
step() {
    local label=$1
    shift
    if "$@"; then return 0; fi
    failed+=("$label")
    printf 'Failed: %s\n' "$label" >&2
}

step ".ssh directory" install -d -m 700 "$HOME/.ssh"
step "workspace" mkdir -p "$HOME/workspace"
step ".bashrc" link_setting bashrc .bashrc
step ".bash_profile" link_setting bash_profile .bash_profile
step ".vimrc" link_setting vimrc .vimrc
step "vim" mkdir -p "$base/vim"
step ".vim" link_setting vim .vim
step ".ssh/config" link_setting ssh/config .ssh/config

step "development packages" tizen-dev-packages install basic build debug editor
step Vim "$base/bin/setup-vim"
for key in ui branch status diff commit; do
    if ! git config --global --get "color.$key" >/dev/null; then
        step "git color.$key" git config --global "color.$key" auto
    fi
done

for name in gemini claude codex herdr; do
    if "$base/cli/bin/$name" --version 2>/dev/null; then
        if [[ $name == claude ]]; then step "Claude status line" "$base/bin/setup-claude-statusline"; fi
        continue
    fi
    if [[ $name == herdr ]]; then
        step "$name" "$base/bin/setup-herdr"
    else
        step "$name" "$base/bin/install-ai-clis" "$name"
    fi
done

step Graphify "$base/bin/setup-graphify"
step skills "$base/bin/setup-skills"

key=$HOME/.ssh/id_ed25519_tizen_gerrit
if [[ ${1:-} == --with-ssh-key && ! -e $key ]]; then
    step "Gerrit SSH key" "$base/bin/setup-gerrit-key"
fi
if (( ${#failed[@]} )); then
    printf 'Development setup finished with failed steps:\n' >&2
    printf '  - %s\n' "${failed[@]}" >&2
    exit 1
fi
echo 'Development setup complete. Open a login shell to load the settings.'
