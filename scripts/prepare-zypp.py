#!/usr/bin/python3
"""Stage signed copies of the two independently pinned public RPM-MD indexes."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

import snapshot_rpms as bootstrap
PINS = {
    "tizen-emulator": "50482f0c776da41297d82614ecc602208a3c2a618fffa6d44cbf47c235e30537",
    "tizen-base": "f031627bfbf35b8b071783f36e97a31b450229b7371704772be12c7276b4aa13",
}
if bootstrap.LOCK:
    PINS = {alias: bootstrap.LOCK["repos"][alias]["repomd"] for alias in bootstrap.REPOS}
root = Path(sys.argv[1])
destination = root / "usr/share/tizen-snapshot"
destination.mkdir(parents=True, exist_ok=True)
if bootstrap.LOCK:
    (destination / "snapshot.lock.json").write_text(json.dumps(bootstrap.LOCK, indent=2, sort_keys=True) + "\n")
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
    for alias, (url, _) in bootstrap.REPOS.items():
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
