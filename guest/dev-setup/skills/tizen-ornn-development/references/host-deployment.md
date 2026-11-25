# Develop on the host and run on Ornn

## Select the guest

Run the commands below from the host skill directory. The helper requires Python 3.8+ and Tizen Studio SDB on PATH. Supply `--sdb /path/to/sdb` or a Windows executable path to choose the SDK.

```sh
tizen-ornn images
python3 scripts/ornn_target.py devices
python3 scripts/ornn_target.py probe
```

Start a stopped VM with `tizen-ornn run --image-dir PATH`. Use `--gui` when the task needs a graphical session. Ornn 2.2.0 introduced automatic loopback SDB registration. Since 2.2.1 the VM advertises the model name `tizen-ornn`. Each boot can have a different serial. On 2.2.0 select the serial printed by the Ornn console explicitly, even if the model column reads `<unknown>`. If SDB was installed after boot, use `sdb connect SERIAL` with the printed address. For an existing running guest, `tizen-ornn console --image-dir PATH` reattaches to the developer console.

## Platform RPMs

Build the exact working revision with the project's existing host GBS profile and an x86_64 target. Use the matching Tizen snapshot and include uncommitted changes when the task concerns working-tree edits. Pass the actual output RPMs as separate arguments. Include the matching runtime and development subpackages needed by the scenario.

```sh
python3 scripts/ornn_target.py --serial SERIAL install-rpm path/to/runtime.rpm path/to/devel.rpm
python3 scripts/ornn_target.py --serial SERIAL exec rpm -q PACKAGE
```

For intentionally unsigned personal development RPMs, add `--allow-unsigned` **after** `install-rpm`. The helper checks SHA-256 after SDB push, queries RPM headers inside the guest, installs the set through the VM's privileged Zypper transaction, and compares each installed RPM header ID with its input. It accepts x86_64 and noarch. The RPM transaction needs guest administrative privileges. Some images require an authorized SDK root session (`sdb -s SERIAL root on`, then `root off` after the task). The helper uses the current session and does not change that setting. If root access is unavailable, transfer the artifacts with the shared folder and use the guest developer console for installation. Select the architecture-specific hardware or emulator for an ARM runtime test. Host `tizen-ornn install NAME` accepts repository package names. Use the helper when artifact identity matters.

## Signed native TPKs

Build and sign for the VM's Tizen platform using the project's SDK profile. Use a distributor certificate accepted by the target.

```sh
python3 scripts/ornn_target.py --serial SERIAL install-tpk path/to/app.tpk
python3 scripts/ornn_target.py --serial SERIAL exec pkginfo --list PACKAGE_ID
python3 scripts/ornn_target.py --serial SERIAL exec app_launcher -s APP_ID
python3 scripts/ornn_target.py --serial SERIAL exec app_launcher -r APP_ID
```

The TPK helper reads the package ID and version from the manifest, runs the SDK installer, then checks the guest package database. Package ID and application ID can differ. Use the manifest or `pkginfo --list PACKAGE_ID` for the application ID. Check an observable action in the running application. If launch fails, inspect the app's dlog entries on the same serial, for example `sdb -s SERIAL dlog -d -s TAG`. Uninstall an app with `sdb -s SERIAL uninstall PACKAGE_ID` when removal is part of the task.

## Focused checks and failures

`exec` accepts a program and its argument list and propagates guest failures. For pipes or shell syntax pass `sh -c '...'` explicitly. `sdb -s SERIAL shell` opens an interactive shell, normally as `owner`. A guest `tizen` checkout may be private to the developer. Use the developer console for its files. A running SDB transport alone establishes connectivity. Capture the package or process evidence needed for the requested test. If an install times out, check the installed state before repeating it.
