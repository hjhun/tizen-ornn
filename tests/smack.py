#!/usr/bin/env python3
"""Exercise a real access decision with identical DAC permissions."""
import errno
import os
import pathlib
import tempfile


def try_read(directory, subject, expected_denial):
    pid = os.fork()
    if pid == 0:
        try:
            pathlib.Path("/proc/self/attr/current").write_bytes(subject)
            os.setgroups([])
            os.setgid(65534)
            os.setuid(65534)
            assert pathlib.Path("/proc/self/attr/current").read_bytes().rstrip(b"\0\n") == subject
            capabilities = next(line for line in pathlib.Path("/proc/self/status").read_text().splitlines()
                                if line.startswith("CapEff:"))
            assert int(capabilities.split()[1], 16) == 0
            try:
                fd = os.open("fixture", os.O_RDONLY, dir_fd=directory)
            except OSError as exc:
                if not expected_denial or exc.errno != errno.EACCES:
                    raise
            else:
                assert not expected_denial and os.read(fd, 2) == b"ok"
                os.close(fd)
            os._exit(0)
        except BaseException as exc:
            print(f"{subject!r}: {exc!r}", flush=True)
            os._exit(1)
    _, status = os.waitpid(pid, 0)
    assert os.waitstatus_to_exitcode(status) == 0


with tempfile.TemporaryDirectory() as tmp:
    os.chmod(tmp, 0o755)
    os.setxattr(tmp, "security.SMACK64", b"_")
    f = pathlib.Path(tmp, "fixture")
    f.write_bytes(b"ok")
    os.chmod(f, 0o644)
    os.setxattr(f, "security.SMACK64", b"TizenDev::Reader")
    d = os.open(tmp, os.O_RDONLY | os.O_DIRECTORY)
    try:
        try_read(d, b"TizenDev::Reader", False)
        try_read(d, b"TizenDev::Other", True)
    finally:
        os.close(d)
print("SMACK allow + EACCES (uid 65534, effective capabilities zero)")
