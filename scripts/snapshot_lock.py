"""Read the optional source-image lock produced by scripts/tizen-image."""
import json
import os
from pathlib import Path
import sys


def read_lock():
    path = os.environ.get("TIZEN_SNAPSHOT_LOCK")
    if not path:
        return None
    lock = json.loads(Path(path).read_text())
    if lock.get("format") != 1:
        raise ValueError(f"Unsupported snapshot lock format in {path}")
    return lock


if __name__ == "__main__":
    lock = read_lock()
    section = sys.argv[1]
    if section == "cross":
        for sha, url in lock[section]:
            print(sha, url)
    else:
        keys = {"archive": ("sha256", "url"), "rust": ("sha256", "url", "version")}[section]
        print(*(lock[section][key] for key in keys), sep="\n")
