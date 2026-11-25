# Tizen development account

From the guest console run `~/dev-setup/install.sh`. The installer offers the selected snapshot RPMs through zypper, configures Bash and Vim, and installs Gemini CLI, Claude Code, Codex and Herdr in `~/dev-setup/cli/bin`. Open another login shell after setup. In Vim, F7 opens the tag list and F9 opens the file tree.

## Gerrit

1. Run `~/dev-setup/install.sh --with-ssh-key` or `setup-gerrit-key`. Enter the passphrase in the terminal and register the public key in Gerrit under Settings → SSH Keys.
2. Add your Gerrit `User` to the host entry in `~/.ssh/config`. Verify the server fingerprint through your usual trusted channel, then use `ssh tizen-gerrit gerrit version` to check authentication.
3. Configure your own `git config --global user.name` and `user.email` values.
4. Run `gerrit-clone platform/core/appfw/tizen-action`. The clone goes under `~/workspace` and the helper obtains the commit-msg hook from Gerrit.
5. Build and test according to the project's instructions. For supported CMake/spec projects, `tizen-build test` builds and runs CTest in the guest.
6. After committing, check the Change-Id with `git log -1`. For a review targeting the tizen branch, use `git push origin HEAD:refs/for/tizen`. Amend the commit to retain the Change-Id on later patchsets.

## Tools

To build native RPMs inside the x86_64 guest, run `ornn build` from the checkout containing the spec and its source files. Use `--spec FILE` to select a spec and `--out DIR` to choose the output directory (default `./Build/RPMS`). If RPM reports missing `BuildRequires`, Ornn uses the guest's configured Zypper repositories to install the requirements and their dependencies, then builds as the calling user. A requirement with no provider stops the build.

A successful native build installs its RPMs in the guest so dependent projects see the new headers and libraries immediately. Use `ornn build --no-install` to produce RPMs only. From the project root, run `ornn restore` to reinstall its last installed package set from the configured HTTP(S) repositories, or name packages explicitly, for example `ornn restore bundle bundle-devel`. The per-project list in `${XDG_DATA_HOME:-~/.local/share}/ornn/projects/<key>/.installed-rpms` survives `ornn clean`. RPM checks installed build dependencies against the guest package database.

Run `ornn status` from the project root to compare the last build with the installed RPM identities. A different ID indicates a different build. After an interrupted install, retry the same package set or run `ornn restore`. Clean removes only a `Build/` created and marked by Ornn. Pre-existing directories are preserved.

Use `tizen-dev-packages list` to view package groups and `tizen-dev-packages plan build ctags` to resolve a selection. Use `download` to cache individual RPMs and `install` to install with dependencies.

Run `install-ai-clis` to reinstall the three AI clients, or pass one name such as `install-ai-clis codex`. Run `setup-herdr` to reinstall the pinned Herdr release.

The interactive Bash commands `claude` and `codex` use their permission-bypass flags. Invoke `command claude` or `command codex` for the upstream defaults. Installing Claude merges the display command from `claude/statusline.sh` into the user's Claude settings. It shows the model, working directory and branch, context usage, cost, duration and available rate limits.

The guest exposes the Ornn skill at `~/.agents/skills/ornn/SKILL.md` on login. The installer links the ornn, coding-style and development-style skills from `~/dev-setup/skills` into `~/.agents/skills`, `~/.agent/skills`, and `~/.claude/skills`. The distributed copies contain reusable engineering rules.

Graphify is installed in a dedicated Python environment under `/usr/local/lib/tizen-ornn` so Tizen can load its native parsers. Its skill comes from the matching graphifyy distribution, and the Herdr skill comes from the installed Herdr binary. Both are linked alongside the two development skills. Setup leaves graph builds and project hooks under the developer's control.

For ARM RPMs, use `ornn build --target armv7l` or `ornn build --target aarch64` inside the x86_64 guest. The build uses the selected target’s C/C++ toolchain and sysroot. Supply any additional target libraries in that sysroot.
