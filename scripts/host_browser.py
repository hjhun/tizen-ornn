"""Open HTTPS requests from the optional Tizen guest browser port."""
import os
import re
import shutil
import subprocess
from urllib.parse import urlsplit

MAX_FRAME = 16384


def open_https(url):
    if not url.isascii() or not url.isprintable() or " " in url or "\\" in url:
        raise ValueError("Invalid URL")
    parts = urlsplit(url)
    if parts.scheme != "https" or not parts.hostname or parts.username is not None:
        raise ValueError("Expected an HTTPS URL")
    if "microsoft" in os.uname().release.lower() and shutil.which("powershell.exe"):
        # Pass the URL as data on stdin, including OAuth query parameters.
        command = ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command",
                   "$ErrorActionPreference='Stop'; "
                   "$p=New-Object System.Diagnostics.ProcessStartInfo; "
                   "$p.FileName=[Console]::In.ReadToEnd(); $p.UseShellExecute=$true; "
                   "[void][System.Diagnostics.Process]::Start($p)"]
        data = url.encode("ascii")
    else:
        command = ["gio", "open", url]
        data = b""
    subprocess.run(command, input=data, stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL, timeout=12, check=True)


def serve(port):
    with port, port.makefile("rb") as stream:
        while frame := stream.readline(MAX_FRAME + 1):
            if len(frame) > MAX_FRAME or not frame.endswith(b"\n"):
                # Discard the whole oversized request, including its tail.
                while frame and not frame.endswith(b"\n"):
                    frame = stream.readline(MAX_FRAME + 1)
                continue
            ident, _, payload = frame[:-1].partition(b" ")
            if not re.fullmatch(b"[0-9a-f]{32}", ident):
                continue
            try:
                open_https(payload.decode("ascii"))
                status = b"ok"
            except (ValueError, OSError, subprocess.SubprocessError):
                status = b"error"
            try:
                port.sendall(ident + b" " + status + b"\n")
            except (BrokenPipeError, ConnectionResetError):
                return
