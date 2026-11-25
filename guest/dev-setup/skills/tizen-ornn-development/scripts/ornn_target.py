#!/usr/bin/env python3
"""Select an Ornn VM and verify operations on its SDB transport."""
import argparse
import hashlib
from pathlib import Path
import re
import shlex
import subprocess
import sys
import uuid
import xml.etree.ElementTree as ET
from zipfile import BadZipFile, ZipFile


def invoke(args, timeout=45):
    result = subprocess.run(args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=timeout)
    out = result.stdout.decode(errors="replace").replace("\r\n", "\n")
    if result.returncode:
        raise ValueError(out + result.stderr.decode(errors="replace"))
    return out


def remote(client, command, timeout=45):
    # Some SDB clients return zero even when the target command fails.
    tag = "ORNN_RESULT_" + uuid.uuid4().hex
    script = f"( {command} ); printf '\\n{tag}=%d\\n' $?"
    out = invoke([*client, "shell", script], timeout)
    match = re.search(rf"\n{tag}=(\d+)\s*\Z", out)
    if not match:
        raise ValueError("Incomplete SDB response: " + out)
    output = out[:match.start()].strip()
    if int(match[1]):
        raise ValueError(f"Guest command exited {match[1]}: {output}")
    return output


def select(sdb, serial):
    rows = [line.split() for line in invoke([sdb, "devices"]).splitlines()]
    online = [row for row in rows if len(row) >= 3 and row[1] == "device"]
    if serial:
        candidates = [row for row in online if row[0] == serial]
    else:
        candidates = [row for row in online if row[2] == "tizen-ornn"]
    if len(candidates) != 1:
        raise ValueError("Select an online Ornn serial with --serial. Run sdb devices.")
    client = [sdb, "-s", candidates[0][0]]
    remote(client, 'test -x /usr/local/bin/ornn && test "$(uname -m)" = x86_64')
    print(f"Target {client[-1]} ({candidates[0][2]})", file=sys.stderr)
    return client


def install_rpms(client, files, allow_unsigned):
    stage = remote(client, "mktemp -d /home/owner/share/tmp/ornn-deploy.XXXXXXXX")
    if not re.fullmatch(r"/home/owner/share/tmp/ornn-deploy\.[A-Za-z0-9]+", stage):
        raise ValueError("Unexpected guest staging path: " + stage)
    identities = {}
    paths = []
    try:
        for i, file in enumerate(files):
            path = f"{stage}/{i}.rpm"
            invoke([*client, "push", str(file), path], 180)
            with file.open("rb") as source:
                digest = hashlib.sha256()
                for block in iter(lambda: source.read(1024 * 1024), b""):
                    digest.update(block)
            if remote(client, f"sha256sum {path}").split()[0] != digest.hexdigest():
                raise ValueError(f"Transfer checksum differs for {file}")
            name, arch, identity = remote(client, f"rpm -qp --qf '%{{NAME}} %{{ARCH}} %{{SIGMD5}}' {path}").split()
            if arch not in ("x86_64", "noarch") or not re.fullmatch(r"[a-fA-F0-9]{32}", identity):
                raise ValueError(f"Unsupported guest RPM: {name} {arch} {identity}")
            key = f"{name}.{arch}"
            if key in identities:
                raise ValueError(f"Duplicate RPM for {key}")
            identities[key] = identity
            paths.append(path)
        args = ["/usr/bin/zypper", "--non-interactive", "install", "--no-recommends", "--force"]
        if allow_unsigned:
            args.append("--allow-unsigned-rpm")
        print(remote(client, "su - root -c " + shlex.quote(shlex.join([*args, "--", *paths])) + " </dev/null", 300))
        for key, expected in identities.items():
            actual = remote(client, "rpm -q --qf '%{SIGMD5}' -- " + shlex.quote(key))
            if actual != expected:
                raise ValueError(f"Installed RPM differs for {key}: {actual}")
            print(f"Verified {key} {actual}")
    finally:
        remote(client, f"rm -rf -- {stage}")


def install_tpk(client, file):
    with ZipFile(file) as package:
        manifest = ET.fromstring(package.read("tizen-manifest.xml"))
    name, version = manifest.attrib["package"], manifest.attrib["version"]
    out = invoke([*client, "install", str(file)], 180)
    print(out)
    if not (f"pkgid[{name}] key[end] val[ok]" in out or
            re.search(r"^processing result\s*:\s*success\s*$", out, re.I | re.M)):
        raise ValueError("SDB did not confirm installation")
    info = remote(client, "pkginfo --pkg " + shlex.quote(name))
    if f"Version: {version}" not in info.splitlines() or "Type: tpk" not in info.splitlines():
        raise ValueError(f"Expected TPK {name} {version}; guest reported:\n{info}")
    print(f"Verified TPK registration {name} {version}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sdb", default="sdb", help="host SDK sdb executable")
    parser.add_argument("--serial", help="serial from the first column of sdb devices")
    sub = parser.add_subparsers(dest="action", required=True)
    sub.add_parser("devices", help="list the SDK's devices")
    sub.add_parser("probe", help="read guest account, architecture and Tizen release")
    command = sub.add_parser("exec", help="execute a program on the guest as the SDB user")
    command.add_argument("argv", nargs=argparse.REMAINDER)
    build = sub.add_parser("build", help="build a guest checkout as the developer via SDB")
    build.add_argument("checkout", help="absolute guest checkout path")
    build.add_argument("--spec", help="spec path relative to the checkout or absolute")
    build.add_argument("--target", choices=["x86_64", "armv7l", "aarch64"])
    build.add_argument("--no-install", action="store_true", help="only produce RPMs")
    build.add_argument("--out", help="guest RPM export directory")
    build.add_argument("--timeout", type=int, default=1800, metavar="SECONDS")
    rpm = sub.add_parser("install-rpm", help="install and verify a set of host RPMs")
    rpm.add_argument("--allow-unsigned", action="store_true", help="accept these unsigned local development RPMs")
    rpm.add_argument("files", nargs="+", type=Path)
    tpk = sub.add_parser("install-tpk", help="install a signed native TPK and check registration")
    tpk.add_argument("file", type=Path)
    args = parser.parse_args()
    if args.action == "devices":
        print(invoke([args.sdb, "devices"]), end="")
        return
    client = select(args.sdb, args.serial)
    if args.action == "probe":
        print(remote(client, 'id -un && uname -m && cat /etc/os-release'))
    elif args.action == "exec":
        if not args.argv:
            parser.error("exec requires a guest program")
        print(remote(client, shlex.join(args.argv)))
    elif args.action == "build":
        if not args.checkout.startswith("/") or args.timeout <= 0:
            parser.error("build requires an absolute guest checkout path and positive timeout")
        argv = ["ornn", "build"]
        for option in ("spec", "target", "out"):
            if getattr(args, option):
                argv.extend(["--" + option, getattr(args, option)])
        if args.no_install:
            argv.append("--no-install")
        script = "cd " + shlex.quote(args.checkout) + " && exec " + shlex.join(argv)
        print(remote(client, "su - tizen -c " + shlex.quote(script) + " </dev/null", args.timeout))
    elif args.action == "install-rpm":
        install_rpms(client, [p.resolve(strict=True) for p in args.files], args.allow_unsigned)
    else:
        install_tpk(client, args.file.resolve(strict=True))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, BadZipFile, ET.ParseError, subprocess.TimeoutExpired) as error:
        sys.exit(str(error))
