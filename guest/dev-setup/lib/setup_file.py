"""Publish completed installer files on POSIX and Windows-backed 9p homes."""
import filecmp
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile


def replace_closed(source, target):
    try:
        os.replace(source, target)
    except FileExistsError:
        # Windows 9p supports rename to an unused name, but not overwrite.
        if target.is_dir() and not target.is_symlink():
            raise IsADirectoryError(target)
        backup = source.with_name(source.name + ".previous")
        if backup.exists() or backup.is_symlink():
            raise FileExistsError(f"Previous setup backup needs review: {backup}")
        os.rename(target, backup)
        try:
            os.rename(source, target)
        except OSError:
            os.rename(backup, target)
            raise
        backup.unlink()


def publish(target, write, mode=0o600):
    target = Path(target)
    if target.is_symlink() or (target.exists() and not target.is_file()):
        raise ValueError(f"Expected a regular file at {target}")
    out = tempfile.NamedTemporaryFile(dir=target.parent, delete=False)
    tmp = Path(out.name)
    try:
        with out:
            write(out)
        if target.exists():
            if filecmp.cmp(tmp, target, shallow=False):
                return
            mode = stat.S_IMODE(target.stat().st_mode)
        tmp.chmod(mode)
        replace_closed(tmp, target)
    finally:
        tmp.unlink(missing_ok=True)


if __name__ == "__main__":
    if sys.argv[1] == "--replace-closed":
        replace_closed(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        with open(sys.argv[1], "rb") as source:
            publish(sys.argv[2], lambda out: shutil.copyfileobj(source, out), 0o644)
