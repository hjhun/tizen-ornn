# Tizen development account

From the guest console run `~/dev-setup/install.sh`. The installer offers the selected snapshot RPMs through zypper, configures Bash and Vim, and installs agy, Claude Code, Codex and Herdr in `~/dev-setup/cli/bin`. Open another login shell after setup. In Vim, F7 opens the tag list and F9 opens the file tree.

## Gerrit

1. Run `~/dev-setup/install.sh --with-ssh-key` or `setup-gerrit-key`. Enter the passphrase in the terminal and register the public key in Gerrit under Settings → SSH Keys.
2. Add your Gerrit `User` to the host entry in `~/.ssh/config`. Verify the server fingerprint through your usual trusted channel, then use `ssh tizen-gerrit gerrit version` to check authentication.
3. Configure your own `git config --global user.name` and `user.email` values.
4. Run `gerrit-clone platform/core/appfw/tizen-action`. The clone goes under `~/workspace` and the helper obtains the commit-msg hook from Gerrit.
5. Build and test according to the project's instructions. For supported CMake/spec projects, `tizen-build test` builds and runs CTest in the guest.
6. After committing, check the Change-Id with `git log -1`. For a review targeting the tizen branch, use `git push origin HEAD:refs/for/tizen`. Amend the commit to retain the Change-Id on later patchsets.

## Tools

Use `tizen-dev-packages list` to view package groups and `tizen-dev-packages plan build ctags` to resolve a selection. Use `download` to cache individual RPMs and `install` to install with dependencies.

Run `install-ai-clis` to reinstall the three AI clients, or pass one name such as `install-ai-clis codex`. Run `setup-herdr` to reinstall the pinned Herdr release.

The interactive Bash commands `claude`, `codex` and `agy` use their permission-bypass flags. Invoke `command claude`, `command codex` or `command agy` for the upstream defaults. Installing Claude merges the display command from `claude/statusline.sh` into the user's Claude settings. It shows the model, working directory and branch, context usage, cost, duration and available rate limits.

The installer links the coding-style and development-style skills from `~/dev-setup/skills` into `~/.agents/skills`, `~/.agent/skills`, and `~/.claude/skills`. The distributed copies contain reusable engineering rules.

Graphify is installed in a dedicated Python environment under `/usr/local/lib/tizen-vm` so Tizen can load its native parsers. Its skill comes from the matching graphifyy distribution, and the Herdr skill comes from the installed Herdr binary. Both are linked alongside the two development skills. Setup leaves graph builds and project hooks under the developer's control.
