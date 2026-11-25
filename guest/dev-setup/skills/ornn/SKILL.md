---
name: ornn
description: Build Tizen platform RPMs inside an Ornn QEMU guest, use locally built APIs in dependent checkouts, and restore server packages with the guest ornn CLI.
---

# Develop RPMs inside the Ornn guest

Run `ornn` as the guest developer from the package checkout root. On the host, `tizen-ornn console` attaches to a running guest. Check `ornn build -h` and `ornn restore -h` for the options supported by the installed guest tool.

- Use `ornn build` for an x86_64 development build by default. Ornn resolves missing spec BuildRequires from the configured repositories and installs the resulting RPMs system-wide after the build succeeds. To build and test without installing the result, use `ornn build --no-install`. A cross build produces RPMs for the selected target.
- Build a producer before its consumer. For example, build `bundle`, then run `ornn build` in the `rpc-port` checkout. The consumer uses the installed headers and libraries. For the same version and release, compare the RPM file and installed package with `rpm -qp --qf '%{SIGMD5}\n' FILE.rpm` and `rpm -q --qf '%{SIGMD5}\n' NAME`.
- RPM runs the spec's `%check` during the build. For a gtest suite registered with CTest, check that this section invokes `ctest` in the configured build directory and propagates its failure. Report the tests and skips observed in the output. A package without a test command can still produce an RPM.
- When Ornn generates a missing local `%{name}-%{version}.tar.gz`, it uses current contents of Git-tracked files. New source files enter the archive after `git add`. Use `--spec FILE` to disambiguate multiple specs.
- Default output is `Build/RPMS/<arch>/`. Run `ornn clean` to clear the project’s `Build/` directory. Output selected with `--out` outside that directory remains in place. The guest RPM database tracks installed dependencies, and the project install list lives under `~/.local/share/ornn/projects/` (or `$XDG_DATA_HOME/ornn/projects/`).
- To switch back to the configured server RPMs, run `ornn restore` from the original project directory, including after clearing `Build/`. Use `ornn restore NAME...` for specific installed packages. Restore fetches the repository version for each installed architecture. A local-only package needs a separate decision if the server has no counterpart.
- Run `ornn status` from the project root to compare the last build with the installed RPM identities. A different ID indicates a different build. After an interrupted install, retry the same package set or run `ornn restore`. Clean removes only a `Build/` created and marked by Ornn. Pre-existing directories are preserved.
