"""Check declarative image inputs against the published partition and RPM inventories."""
import fnmatch
import re
import shlex
import subprocess
import sys
from pathlib import Path


def package_rows(lines):
    rows = set()
    for line in lines:
        fields = line.split()
        if not fields:
            continue
        if (len(fields) < 2 or not re.fullmatch(r"[A-Za-z0-9_+.~-]+\.(?:x86_64|noarch|i[3-6]86)", fields[0])
                or not re.fullmatch(r"[A-Za-z0-9_+.~:^%-]+", fields[1])):
            raise ValueError(f"invalid x86 emulator package record: {line}")
        row = tuple(fields[:2])
        if row in rows:
            raise ValueError(f"duplicate package record: {row}")
        rows.add(row)
    if not rows:
        raise ValueError("empty package inventory")
    return rows


def validate_kickstart(text, image, packages):
    names = {pkg.rsplit(".", 1)[0] for pkg, _ in packages}
    partitions = {}
    requested = []
    erased = set()
    section = None
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line == "%end":
            section = None
            continue
        if section == "%post":
            # Recognize literal MIC cleanup in the chroot. Never run repo scripts.
            removal = re.fullmatch(r"rpm -e ([A-Za-z0-9_+.~-]+)(?: --nodeps)?", line)
            if removal:
                erased.add(removal[1])
        if section in ("%post", "%pre", "%posttrans", "%post --nochroot"):
            continue
        if line.startswith("%"):
            section = line.split()[0]
            if section == "%post" and "--nochroot" in line.split():
                section = "%post --nochroot"
            if section not in ("%packages", "%post", "%pre", "%posttrans", "%post --nochroot"):
                raise ValueError(f"unsupported kickstart directive: {section}")
            continue
        if section == "%packages":
            pattern, = shlex.split(line, comments=True)
            exclude = pattern.startswith("-")
            if any(fnmatch.fnmatchcase(name, pattern.removeprefix("-")) for name in names) == exclude:
                raise ValueError(f"kickstart selection disagrees with .packages: {pattern}")
            requested.append(pattern)
        elif line.split()[0] in ("part", "partition"):
            _, mount, *options = shlex.split(line, comments=True)
            args = dict(opt[2:].split("=", 1) for opt in options if opt.startswith("--") and "=" in opt)
            mount = mount.rstrip("/") or "/"
            if mount in partitions:
                raise ValueError(f"duplicate kickstart partition: {mount}")
            partitions[mount] = (args.get("fstype"), args.get("label"), args.get("size"))
    published = {p.get("mountpoint"): (p.get("fstype"), p.get("fslabel"), p.get("size"))
                 for p in image.findall("partition")}
    if partitions != published or not requested or section is not None:
        raise ValueError("kickstart partition layout or package selection does not match the image")
    return requested, sorted(erased)


def verify_installed(root, expected):
    output = subprocess.check_output(
        ["chroot", str(root), "/bin/rpm", "-qa", "--qf", "%{NAME}.%{ARCH} %{VERSION}-%{RELEASE}\\n"],
        text=True)
    actual = package_rows(output.splitlines())
    for description, difference in (("missing or changed", expected - actual),
                                    ("unexpected", actual - expected)):
        if difference:
            sample = ", ".join(" ".join(row) for row in sorted(difference)[:10])
            raise ValueError(f"original image RPM inventory: {len(difference)} {description}: {sample}")
    print(f"Verified {len(actual)} original image RPMs before adding development tools.", flush=True)


if __name__ == "__main__":
    from snapshot_lock import read_lock
    recipe = read_lock().get("recipe")
    if not recipe:
        sys.exit("Image build requires a snapshot lock with verified recipe inputs. Select a fresh output directory.")
    verify_installed(Path(sys.argv[1]), {tuple(row) for row in recipe["packages"]})
