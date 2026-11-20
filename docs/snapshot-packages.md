# Native Tizen packages

The default 10.1 image uses two official x86_64 RPM repositories. `tizen-emulator` points to the
Tizen 10.1 Unified emulator snapshot `20261119.053314` with priority 1.
`tizen-base` points to the matching Base snapshot `20261118.084133` with
priority 99. Both use fixed HTTPS addresses. Run installation commands as
root inside Tizen.

```sh
zypper repos --details
zypper search --details glib2-devel
zypper --non-interactive install --no-recommends ed
rpm -q ed
```

`zypper` resolves dependencies from the preloaded snapshot metadata and
downloads RPMs over HTTPS. Repository metadata auto-refresh is disabled for
these two immutable versions. If the caches have been cleaned, restore them
before searching or installing.

```sh
tizen-refresh-pinned
```

The public snapshot URLs distribute unsigned RPM-MD indexes. During image
creation the build checks the SHA-256 digest of each index against the pins
in the selected snapshot lock (resolved by `scripts/tizen-image` from the configured mirror), then checks each referenced metadata file.
It signs the verified indexes with a temporary local key, imports the public
key into Tizen, and discards the private key. The key identifies the image's
local verification step. The published RPM checksums come from Tizen.
The refresh helper validates these stored, signed indexes with native
libzypp. Both live repository definitions retain `gpgcheck=1`. A forced
network refresh reaches the original unsigned index, so use
`tizen-refresh-pinned` for this image.

The snapshot metadata remains usable offline for queries. Installing an uncached package requires network access to its dated upstream RPM. Native Git comes from a separate local RPM repository.

## Update installed packages

Start the selected VM, then use a second host terminal. These commands refresh the VM's package repositories and update its installed native RPMs.

```sh
tizen-vm update --packages ed
tizen-vm update --packages --snapshot tizen-10.1-unified_20261119.053314
```

Omitting the snapshot selects the configured mirror's latest dated Unified build and its matching Base build. Supplying RPM names limits the transaction to installed packages and their required dependencies. With no names, zypper updates all eligible installed packages. Local repositories keep their existing definitions. The source image identity, kernel and ARM cross sysroot belong to the image build. [Snapshot selection](snapshot-build.md)

Some Tizen rebuilds keep the same RPM name, epoch, version, release and architecture. The updater compares the recorded package checksum against the new primary index. It explicitly reinstalls changed builds, remembers successful replacements, and keeps the old checksum for unselected installed packages. After an interrupted transaction, repeat the command to finish it. The active package snapshot is recorded in `/var/lib/tizen-vm/rpm-updates/active.json`. The original image lock remains at `/usr/share/tizen-snapshot/snapshot.lock.json`.
