#!/usr/bin/env python3
"""Capture a QEMU display or click a guest pixel through its virtual tablet."""
import argparse
import json
from pathlib import Path
import socket
import struct
import tempfile


def request(stream, name, **arguments):
    stream.write(json.dumps({"execute": name, "arguments": arguments}).encode() + b"\n")
    stream.flush()
    while line := stream.readline():
        reply = json.loads(line)
        if "error" in reply:
            raise RuntimeError(f"{name}: {reply['error']['desc']}")
        if "return" in reply:
            return reply["return"]
    raise RuntimeError(f"QEMU disconnected during {name}")


def capture(stream, path):
    request(stream, "screendump", filename=str(path.resolve()), format="png")
    with path.open("rb") as image:
        header = image.read(24)
    if header[:8] != b"\x89PNG\r\n\x1a\n" or header[12:16] != b"IHDR":
        raise ValueError("QEMU produced an invalid PNG header")
    return struct.unpack(">II", header[16:24])


def click(stream, x, y):
    with tempfile.TemporaryDirectory() as work:
        width, height = capture(stream, Path(work) / "size.png")
    if not (0 <= x < width and 0 <= y < height and width > 1 and height > 1):
        raise ValueError(f"Point ({x}, {y}) outside {width} x {height} display")
    events = [
        {"type": "abs", "data": {"axis": "x", "value": x * 32767 // (width - 1)}},
        {"type": "abs", "data": {"axis": "y", "value": y * 32767 // (height - 1)}},
    ]
    request(stream, "input-send-event", events=events)
    for down in (True, False):
        event = {"type": "btn", "data": {"button": "left", "down": down}}
        request(stream, "input-send-event", events=[event])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("socket", help="Unix socket set with TIZEN_QMP")
    actions = parser.add_subparsers(dest="action", required=True)
    actions.add_parser("shot").add_argument("output", type=Path)
    pointer = actions.add_parser("click")
    pointer.add_argument("x", type=int)
    pointer.add_argument("y", type=int)
    args = parser.parse_args()
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(10)
        client.connect(args.socket)
        with client.makefile("rwb") as stream:
            if "QMP" not in json.loads(stream.readline()):
                raise ValueError("Expected QEMU QMP greeting")
            request(stream, "qmp_capabilities")
            if args.action == "shot":
                width, height = capture(stream, args.output)
                print(f"{args.output} ({width} x {height})")
            else:
                click(stream, args.x, args.y)


if __name__ == "__main__":
    main()
