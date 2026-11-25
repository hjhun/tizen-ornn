# Develop inside the Ornn guest

On the host, use `tizen-ornn images` and select the image, then attach with `tizen-ornn console --image-dir PATH`. Inside that console, confirm `id -un` returns `tizen`. Guest `/home/tizen` holds the developer's local checkout. Guest `/home/tizen/share` maps the configured host folder. To edit on the host and compile in the VM, open the same checkout through that shared folder. Host `tizen-ornn share-dir PATH` selects the shared folder for the next boot.

To start the same build from your host terminal, pass the absolute **guest** path. The helper invokes the guest's installed `ornn` as `tizen`, collects the build output and returns the guest exit status. An SDB session that cannot switch to the developer account should use the Ornn console instead. Omit `--no-install` when the requested native build should install its RPMs.

```sh
python3 scripts/ornn_target.py --serial SERIAL build /home/tizen/workspace/bundle --no-install
```

From a Tizen platform checkout in the guest:

```sh
ornn build --help
ornn build
ornn status
```

The default native target is x86_64. A successful native build installs its subpackages system-wide, including same-version replacements. Use `ornn build --no-install` to build only. For a producer/consumer change, build the producer, then build and test the consumer in the same guest. For example, run `ornn build` in `bundle` before building `rpc-port`. The spec's `%check` controls CTest/gtest execution. Inspect actual test counts and skipped cases.

Use the output path reported by the installed guest tool. Released 2.2.1 guests default to the project's `Build/RPMS/`. A newer guest tool can report a shared build workspace that also retains CMakeCache.txt, objects and logs. Check `ornn --help` for named `install` and `uninstall` support before using those commands. `ornn clean` clears Ornn-owned build outputs. `ornn restore` reinstalls the configured server RPMs for the project and can take explicit package names. These actions have different effects on the guest RPM database.

For the preinstalled GUI example in a graphical guest:

```sh
cd /usr/share/tizen-dev/examples/efl-counter
./build.sh "$HOME/ornn-build/app"
tizen-ui-run "$HOME/ornn-build/app/tizen-ui"
```

Exercise the counter in its window. To test distribution of the signed TPK or a platform RPM, copy the artifact into the shared folder and follow the host deployment reference on the host.
