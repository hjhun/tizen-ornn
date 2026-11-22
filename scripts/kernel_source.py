"""Resolve the QEMU kernel source from a matching Tizen boot inventory."""
import gzip
import hashlib
import re
import subprocess
import sys
from pathlib import Path

from image_source import ImageSource, primary_index, read_url


def resolve(base, release, arch):
    source = ImageSource.validated(base, release, "kernel repository")
    tree = f"{source.base_url}Tizen-10.1-Unified/{release}/"
    profile = f"tizen-boot-{arch}-rpi"
    folder = f"{tree}images/standard/{profile}/"
    name = f"{release}_{profile}.packages"
    inventory = read_url(folder + name)
    digest = hashlib.sha256(inventory).hexdigest()
    if [digest, name] not in [line.split() for line in read_url(folder + "SHA256SUMS").decode().splitlines()]:
        raise ValueError(f"inventory checksum missing or invalid for {name}")
    prefix = {"armv7l": "arm", "aarch64": "arm64"}[arch]
    rows = [line.split() for line in inventory.decode().splitlines()
            if re.match(rf"^{prefix}-[\w.-]+-linux-kernel\.{arch}\s", line)]
    if len(rows) != 1 or len(rows[0]) != 3:
        raise ValueError(f"expected one kernel with version and VCS revision in {name}")
    _, version, vcs = rows[0]
    if not re.fullmatch(r"[\w./+-]+#[0-9a-f]{40}", vcs):
        raise ValueError(f"unsupported kernel revision in {name}")
    repo = tree + "repos/standard/source/"
    _, _, entries = primary_index(repo)
    matches = []
    for pkg in entries:
        ver = pkg.find("{*}version")
        if (pkg.findtext("{*}arch") == "src" and ver.get("vcs") == vcs
                and f"{ver.get('ver')}-{ver.get('rel')}" == version):
            href = pkg.find("{*}location").get("href")
            sha = pkg.findtext("{*}checksum[@type='sha256']", "")
            if not re.fullmatch(r"[\w.+~-]+\.src\.rpm", href) or not re.fullmatch(r"[a-f0-9]{64}", sha):
                raise ValueError(f"invalid kernel source package record for {vcs}")
            matches.append((repo + href, sha, vcs, ver.get("ver")))
    if len(matches) != 1:
        raise ValueError(f"expected one source RPM for {version} {vcs} in {repo}")
    return matches[0]


def prepare(version, dest):
    # Interpret only the source extraction and patch directives in the RPi spec.
    # Repository-supplied shell commands must never execute on the build host.
    spec, = Path.cwd().glob("*.spec")
    text = spec.read_text()
    headers = dict(re.findall(r"^(Source0|Patch\d+):\s+(\S+)\s*$", text, re.M | re.I))
    def local(value):
        value = value.replace("%{version}", version)
        if not re.fullmatch(r"[\w.+~-]+", value):
            raise ValueError(f"unsupported kernel source filename {value}")
        return Path(value)
    lines = text.split("\n%prep\n", 1)[1].split("\n%build", 1)[0].splitlines()
    extracted = False
    for raw in lines:
        line = raw.strip()
        if not line or line.startswith("#") or line == "cp %{SOURCE1001} .":
            continue
        if line == "%setup -q" and not extracted:
            subprocess.run(["tar", "-xf", local(headers["Source0"]), "-C", dest, "--strip-components=1"], check=True)
            extracted = True
            continue
        match = re.fullmatch(r"%patch(\d+) -p(\d+)", line)
        if not match or not extracted:
            raise ValueError(f"unsupported kernel spec preparation directive: {line}")
        patch = local(headers["Patch" + match[1]])
        with (gzip.open(patch) if patch.suffix == ".gz" else patch.open("rb")) as stream:
            command = ["patch", "--batch", "--forward", "-s", "-d", dest, "-p" + match[2]]
            subprocess.run(command, input=stream.read(), check=True)
    if not extracted:
        raise ValueError("missing kernel source archive in spec")


if __name__ == "__main__":
    try:
        if sys.argv[1] == "prepare":
            prepare(*sys.argv[2:])
        else:
            print(*resolve(*sys.argv[1:]), sep="\n")
    except (OSError, ValueError, KeyError, IndexError, subprocess.CalledProcessError) as error:
        sys.exit(f"kernel source: {error}")
