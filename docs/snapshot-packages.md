# Native Tizen packages

The default R4 image uses two official x86_64 RPM repositories. `tizen-emulator` points to the
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
in the selected snapshot lock (or the dated defaults in `scripts/prepare-zypp.py`), then checks each referenced metadata file.
It signs the verified indexes with a temporary local key, imports the public
key into Tizen, and discards the private key. The key identifies the image's
local verification step. The published RPM checksums come from Tizen.
The refresh helper validates these stored, signed indexes with native
libzypp. Both live repository definitions retain `gpgcheck=1`. A forced
network refresh reaches the original unsigned index, so use
`tizen-refresh-pinned` for this image.

The snapshot metadata remains usable offline for queries. Installing an
uncached package requires a network connection and continued availability of
the dated upstream RPM. To change snapshots, select a dated Unified image with `scripts/tizen-image` and rebuild.
The builder resolves its matching Base release. [Snapshot selection](snapshot-build.md) Native Git is provided by a separate
local RPM repository.
