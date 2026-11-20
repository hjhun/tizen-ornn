# AI coding tools in the Tizen VM

From the Ubuntu or WSL host, install the CLI you want into the running VM. Each command downloads the provider's current Linux x86_64 package and prints the installed version.

| Host command | Guest command | Provider |
| --- | --- | --- |
| `tizen-vm install codex` | `codex` | [OpenAI Codex CLI](https://developers.openai.com/codex/cli/) |
| `tizen-vm install gemini` | `gemini` | [Google Gemini CLI](https://geminicli.com/docs/get-started/installation/) |
| `tizen-vm install claude` | `claude` | [Anthropic Claude Code](https://code.claude.com/docs/en/setup) |
| `tizen-vm install agy` | `agy` | [Google Antigravity CLI](https://antigravity.google/docs/cli-getting-started) |

At the guest console, open the tool shell and run your installed CLI from a project directory. For example:

```sh
tizen-ai-shell
cd /home/owner/workspace/my-project
codex
```

Follow the provider's own sign-in instructions when prompted. Headless sessions may require the provider's device or remote authentication flow. To run a single command at the guest console, use `tizen-ai-shell codex --version`. You can also install from inside the guest with `tizen-ai-install codex`.

## Sign in to Antigravity from a VM

With the updated host CLI and guest image, start the VM with browser access enabled.

```sh
tizen-vm run --host-browser
```

Install `agy` from a second host terminal with `tizen-vm install agy`, then run `tizen-ai-shell agy` at the guest console. Selecting Google sign-in sends the HTTPS URL to the Windows default browser on WSL or the desktop browser on Linux. Complete sign-in there and paste the authorization code into the guest when prompted. The browser port is available to the guest tizen, owner and root accounts for the lifetime of that boot. To enable it on an already running VM, stop the VM and start it again with the option.

On an older image or a host without a desktop browser, run `tizen-ai-shell agy acp --login`. Open the printed link on your host and paste the returned code or callback URL into the guest prompt. Then start `tizen-ai-shell agy`.

## Guest crash reports

The kernel invokes Tizen's `crash-manager` on a native crash. As guest root, inspect the newest local report and list its contents.

```sh
ls -lt /opt/usr/share/crash/dump
python3 -m zipfile -l /opt/usr/share/crash/dump/REPORT.zip
```

The installer puts the Antigravity executable in the owner's `~/.tizen-vm-tools` directory and links it into the AI shell's PATH. The `crash_worker` account can traverse this executable path. Credentials and personal settings stay in the private AI home. After migrating an older image, run `tizen-vm install agy` again to move the executable to this layout.

## Files and permissions

The shell runs as the Tizen `owner` user. Its private home is `/home/owner/workspace/.ai-home`, where npm, the CLIs and their sign-in flows can write settings without changing the SMACK label of the platform home. User-installed binaries live under `.ai-home/.local/bin`. The default image contains the runtime and install helpers. Each developer installs the desired clients and supplies their own account.

A normal VM restart retains these files. The image updater copies the owner workspace, including this directory, into the replacement image. Treat workspace backups that contain a signed-in CLI home as private.

## Runtime and verification

The image supplies Node.js 26.13.1 LTS with npm 11.20.0 and npx, ripgrep 15.2.0, bubblewrap 0.13.1 and socat 1.8.1.3. The builder verifies SHA-256 digests, installs the upstream Node.js and ripgrep distributions, and compiles bubblewrap and socat against the guest's glibc. Licenses ship with the tools. The development image also has Git, curl, jq, CA certificates and the native build toolchain. [Node.js release](https://nodejs.org/dist/v26.13.1/), [ripgrep release](https://github.com/BurntSushi/ripgrep/releases/tag/15.2.0), [bubblewrap release](https://github.com/containers/bubblewrap/releases/tag/v0.13.1), [socat source package](https://packages.debian.org/source/sid/socat).

The root login finds `node`, `npm` and `npx` under `/root/.local/bin`. Other users run the same distribution through `/usr/local/bin`. Image updates refresh these three root commands while migrating other user-installed tools.

Run `tizen-ai-check` at the guest console to create and execute an offline npm package under a temporary developer prefix, start a read-only bubblewrap namespace and exercise a local socat relay. An optional account-free Codex check after installation is `tizen-ai-shell codex sandbox -- /bin/echo hello`. Claude Code uses bubblewrap and socat for its Linux sandbox. [Claude sandbox documentation](https://code.claude.com/docs/en/sandboxing).

The Tizen 10.1 x86_64 guest test installed and started Codex 0.171.0, Gemini CLI 0.70.0, Claude Code 2.1.329 and Antigravity CLI 1.5.13. Codex also ran a command inside its sandbox as the SMACK-labeled owner. Account sign-in, model responses and graphical browser flows were outside this test.
