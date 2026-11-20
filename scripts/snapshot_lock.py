"""Read the source-image lock produced by scripts/tizen-image."""
import json
import os
from pathlib import Path
import subprocess
import sys


def read_lock():
    path = os.environ.get("TIZEN_SNAPSHOT_LOCK")
    if not path:
        # Standalone staging tools use the same inputs as a full build.
        saved = Path(os.environ.get("TIZEN_OUT", Path(__file__).resolve().parents[1] / "out")) / "snapshot.lock.json"
        if not saved.is_file():
            return json.loads(subprocess.check_output([Path(__file__).with_name("tizen-image"), "--print-lock"]))
        path = saved
    lock = json.loads(Path(path).read_text())
    if lock.get("format") != 1:
        raise ValueError(f"Unsupported snapshot lock format in {path}")
    return lock


if __name__ == "__main__":
    lock = read_lock()
    section = sys.argv[1]
    if section == "source":
        base, sep, tail = lock["archive"]["url"].rpartition("/Tizen-10.1-Unified/")
        if not sep or not tail.startswith(lock["release"] + "/"):
            raise ValueError("snapshot lock contains an unexpected archive URL")
        print(base + "/")
    elif section == "cross":
        for sha, url in lock[section]:
            print(sha, url)
    else:
        keys = {"archive": ("sha256", "url"), "rust": ("sha256", "url", "version")}[section]
        print(*(lock[section][key] for key in keys), sep="\n")
