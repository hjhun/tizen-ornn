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

install -d -m 700 "$HOME/.ssh"
mkdir -p "$HOME/workspace"
link_setting bashrc .bashrc
link_setting bash_profile .bash_profile
link_setting vimrc .vimrc
mkdir -p "$base/vim"
link_setting vim .vim
link_setting ssh/config .ssh/config

tizen-dev-packages install basic build debug editor
"$base/bin/setup-vim"
for key in ui branch status diff commit; do
    if ! git config --global --get "color.$key" >/dev/null; then
        git config --global "color.$key" auto
    fi
done

for name in agy claude codex herdr; do
    if "$base/cli/bin/$name" --version 2>/dev/null; then
        if [[ $name == claude ]]; then "$base/bin/setup-claude-statusline"; fi
        continue
    fi
    if [[ $name == herdr ]]; then
        "$base/bin/setup-herdr"
    else
        "$base/bin/install-ai-clis" "$name"
    fi
done

"$base/bin/setup-graphify"
"$base/bin/setup-skills"

key=$HOME/.ssh/id_ed25519_tizen_gerrit
if [[ ${1:-} == --with-ssh-key && ! -e $key ]]; then
    "$base/bin/setup-gerrit-key"
fi
echo 'Development setup complete. Open a login shell to load the settings.'
