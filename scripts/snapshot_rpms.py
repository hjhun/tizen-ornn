#!/usr/bin/python3
"""Resolve bootstrap files from pinned Tizen RPM-MD and authenticate their bytes.

The native RPM transaction checks dependency versions, scripts and conflicts.
This selector is only for constructing this known snapshot's fixed bootstrap.
"""
import argparse
from collections import defaultdict, deque
from concurrent.futures import ThreadPoolExecutor
import gzip
import hashlib
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET

from snapshot_lock import read_lock

TOOLS = """bash coreutils diffutils file util-linux shadow-utils iproute2 toybox-symlinks-dhcp
rpm rpm-build smack ca-certificates ca-certificates-tizen zypper vim gdb strace
gcc gcc-c++ cpp glibc-devel libstdc++-devel glib2-devel cmake ninja make pkg-config
rust rust-devel cargo curl libcurl-devel libexpat-devel libopenssl3-devel zlib-devel
pcre2-devel perl gettext-tools""".split()


def repository_indexes(lock, aliases):
    return {alias: (lock["repos"][alias]["url"], lock["repos"][alias]["primary"])
            for alias in aliases}


def sha256(path):
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def fetch(url, digest, path):
    if path.exists() and sha256(path) == digest:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as dest:
        temp = Path(dest.name)
    try:
        subprocess.run(["curl", "-fLSs", "--retry", "3", "-o", str(temp), url], check=True)
        if sha256(temp) != digest:
            raise ValueError(f"Digest mismatch: {url}")
        temp.chmod(0o644)
        temp.replace(path)
    finally:
        temp.unlink(missing_ok=True)


def get_index(cache, repos, arch):
    packages = {}
    providers = defaultdict(list)
    for alias, (baseurl, digest) in repos.items():
        path = cache / "meta" / (alias + ".xml.gz")
        fetch(baseurl + f"repodata/{digest}-primary.xml.gz", digest, path)
        with gzip.open(path) as data:
            root = ET.parse(data).getroot()
        for node in root:
            name = node.findtext("{*}name")
            if node.findtext("{*}arch") not in (arch, "noarch") or name in packages:
                continue
            fmt = node.find("{*}format")
            location = node.find("{*}location").get("href")
            if Path(location).is_absolute() or ".." in Path(location).parts:
                raise ValueError(f"Invalid RPM location {location}")
            files = [item.text for item in fmt.findall("{*}file")]
            caps = [item.get("name") for item in fmt.findall("{*}provides/{*}entry")]
            for cap in set(files + caps):
                providers[cap].append(name)
            requires = [item.get("name") for item in fmt.findall("{*}requires/{*}entry")]
            packages[name] = (requires, baseurl + location, node.findtext("{*}checksum"))
    return packages, providers


def stage_packages(installed, repos, arch, tools, cache, manifest):
    if "filesystem" not in installed:
        raise ValueError("Expected the snapshot's initialized native RPM database")
    packages, providers = get_index(cache, repos, arch)
    pending = deque(tools)
    selected = set()
    while pending:
        name = pending.popleft()
        if name in selected or name in installed:
            continue
        requires, _, _ = packages[name]
        selected.add(name)
        for cap in requires:
            if cap.startswith("rpmlib("):
                continue
            candidates = providers.get(cap, [])
            if not candidates:
                raise ValueError(f"Missing provider for {name}: {cap}")
            if any(p in installed or p in selected for p in candidates):
                continue
            # Exact-name matches and the image's package choices have priority.
            pending.append(cap if cap in candidates else candidates[0])

    def download(name):
        _, url, digest = packages[name]
        filename = url.rsplit("/", 1)[1]
        fetch(url, digest, cache / "rpms" / filename)
        return filename

    with ThreadPoolExecutor(max_workers=8) as pool:
        filenames = list(pool.map(download, sorted(selected)))
    manifest.write_text("".join(name + "\n" for name in filenames))
    print(f"{len(installed)} installed packages; {len(filenames)} additional verified RPMs")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("cache", type=Path)
    parser.add_argument("manifest", type=Path)
    args = parser.parse_args()
    installed = set(subprocess.check_output(
        ["chroot", str(args.root), "/bin/rpm", "-qa", "--qf", "%{NAME}\\n"], text=True
    ).splitlines())
    repos = repository_indexes(read_lock(), ("tizen-emulator", "tizen-base"))
    stage_packages(installed, repos, "x86_64", TOOLS, args.cache, args.manifest)


if __name__ == "__main__":
    main()
