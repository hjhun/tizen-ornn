#!/usr/bin/python3
"""Exercise rejected and interrupted uploads on a running disposable guest."""
import base64
import fcntl
import json
from pathlib import Path
import socket
import subprocess
import sys
import uuid

scripts = Path(__file__).resolve().parents[1] / "scripts"
disk = sys.argv[1]
prefix = subprocess.check_output([scripts / "vm-runtime", disk], text=True).strip()
with open(prefix + ".request.lock", "a") as lock:
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    other = subprocess.run([scripts / "guest-command", disk, "install", "ed"], capture_output=True)
    assert other.returncode and b"Another host package operation" in other.stderr

    with socket.socket(socket.AF_UNIX) as port:
        port.settimeout(25)
        port.connect(prefix + ".packages")
        ident = uuid.uuid4().hex

        def send(**fields):
            port.sendall(json.dumps(dict(fields, id=ident)).encode() + b"\n")

        with port.makefile("rb") as stream:
            def expect(kind):
                while line := stream.readline():
                    if line.strip():
                        msg = json.loads(line)
                        if msg.get("id") == ident:
                            assert msg["type"] == kind, msg
                            return msg
                raise RuntimeError("Guest closed the package channel")

            send(op="ping")
            expect("ready")
            send(op="exec")
            assert b"Unknown" in base64.b64decode(expect("output")["data"])
            assert expect("exit")["code"] != 0
            send(op="update", size=64 * 1024 * 1024 + 1, sha256="0" * 64)
            expect("output")
            assert expect("exit")["code"] != 0
            send(op="update", size=3, sha256="0" * 64)
            expect("upload")
            send(data="YWJj")
            send(end=True)
            assert b"corrupt" in base64.b64decode(expect("output")["data"])
            assert expect("exit")["code"] != 0
            send(op="update", size=3, sha256="0" * 64)
            expect("upload")
            port.sendall(b'{"data":"partial')  # next client must recover immediately
