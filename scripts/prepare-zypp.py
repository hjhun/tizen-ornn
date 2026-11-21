#!/usr/bin/python3
"""Stage signed copies of the two independently pinned public RPM-MD indexes."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

import snapshot_rpms as bootstrap
LOCK = bootstrap.read_lock()
REPOS = bootstrap.repository_indexes(LOCK, ("tizen-emulator", "tizen-base"))
PINS = {alias: LOCK["repos"][alias]["repomd"] for alias in REPOS}
root = Path(sys.argv[1])
destination = root / "usr/share/tizen-snapshot"
destination.mkdir(parents=True, exist_ok=True)
(destination / "snapshot.lock.json").write_text(json.dumps(LOCK, indent=2, sort_keys=True) + "\n")
seed = destination / "repos.d"
seed.mkdir(exist_ok=True)
live = root / "etc/zypp/repos.d"
live.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix="tizen-index-sign-") as home:
    # Private key exists only while countersigning these hash-pinned indexes.
    gpg = ["gpg", "--homedir", home, "--batch", "--pinentry-mode", "loopback",
           "--passphrase", ""]
    subprocess.run(gpg + ["--quick-generate-key", "Tizen developer image snapshot index", "rsa3072", "sign", "0"], check=True)
    key = subprocess.check_output(gpg + ["--armor", "--export"])
    (destination / "snapshot-index.asc").write_bytes(key)
    for alias, (url, _) in REPOS.items():
        folder = destination / alias / "repodata"
        folder.mkdir(parents=True, exist_ok=True)
        index = folder / "repomd.xml"
        bootstrap.fetch(url + "repodata/repomd.xml", PINS[alias], index)
        for node in ET.parse(index).getroot().findall("{*}data"):
            href = node.find("{*}location").get("href")
            digest = node.find("{*}checksum")
            if digest.get("type") != "sha256" or Path(href).parent != Path("repodata"):
                raise ValueError(f"Unexpected RPM-MD file {href}")
            bootstrap.fetch(url + href, digest.text, folder / Path(href).name)
        subprocess.run(gpg + ["--yes", "--armor", "--detach-sign", str(index)], check=True)
        (folder / "repomd.xml.key").write_bytes(key)
        for repos, base in [(seed, f"file:///usr/share/tizen-snapshot/{alias}/"), (live, url)]:
            priority = 1 if alias == "tizen-emulator" else 99
            content = (f"[{alias}]\nname={alias}\nenabled=1\nautorefresh=0\n"
                       f"baseurl={base}\ntype=rpm-md\npriority={priority}\ngpgcheck=1\n")
            (repos / f"{alias}.repo").write_text(content)
    subprocess.run(["gpgconf", "--homedir", home, "--kill", "all"], check=True)
