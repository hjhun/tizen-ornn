---
name: tizen-ornn-development
description: Develop and verify Tizen applications and platform RPMs on a tizen-ornn x86_64 QEMU VM, either by deploying host-built artifacts over SDB or by building inside its developer console.
---

# Tizen Ornn development

Choose the loop that fits the user's checkout and artifacts.

- **Host checkout or existing RPM/TPK** — keep the project's SDK or GBS build on the host. Select the Ornn instance, install the resulting artifact, and exercise it in the guest. Read [host deployment](references/host-deployment.md).
- **Guest checkout, spec edits, or dependent platform packages** — use the Ornn developer console and the guest's own compiler, RPM database and test dependencies. Read [guest development](references/guest-development.md).

On the **host**, `python3 scripts/ornn_target.py probe` from this skill's directory identifies a single online device named `tizen-ornn` and prints its user, architecture and OS. Use `py -3` on Windows. With several VMs, pass `--serial SERIAL` **before** the subcommand. In the **guest**, start with `id -un`, `uname -m` and `ornn --help`.

A deployment can replace guest system packages. Use the artifact and VM the user selected. Report the host artifact, target serial, installed identity and the behavior actually observed. For a library, exercise a consumer. For an app, distinguish package registration, launch and the requested UI behavior. For a service, check the service operation the change affects.
