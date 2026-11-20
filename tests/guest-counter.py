#!/usr/bin/python3
"""Check an actual 0 -> 1 counter click on the demo's 1024x768 display."""
from itertools import groupby
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from PIL import Image, ImageChops

display = Path(__file__).with_name("headed-display.py")
qmp = sys.argv[1]


def display_command(*args):
    subprocess.run([sys.executable, display, qmp, *args], check=True, stdout=subprocess.DEVNULL)


def read_counter(frame):
    display_command("shot", frame)
    with Image.open(frame) as picture:
        picture = picture.convert("RGB")
    # This example paints one dark-blue button and a black numeral above it.
    button = ImageChops.difference(picture, Image.new("RGB", picture.size, (10, 13, 74)))
    box = button.convert("L").point(lambda value: 255 if value == 0 else 0).getbbox()
    if not box or not 50 <= box[2] - box[0] <= 200 or not 20 <= box[3] - box[1] <= 90:
        return None
    x, y = (box[0] + box[2]) // 2, (box[1] + box[3]) // 2
    # A horizontal scan through the middle of 0 crosses two strokes; 1 has one.
    middle = box[1] - 36
    strokes = sum(1 for ink, _ in groupby(max(picture.getpixel((i, middle))) < 120
                                          for i in range(x - 30, x + 31)) if ink)
    return strokes, x, y


def await_strokes(frame, expected):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        observation = read_counter(frame)
        if observation and observation[0] == expected:
            return observation[1:]
        time.sleep(0.25)
    raise RuntimeError("The EFL demo did not render the expected counter digit.")


with tempfile.TemporaryDirectory() as tmp:
    frame = Path(tmp) / "display.png"
    x, y = await_strokes(frame, 2)
    display_command("click", str(x), str(y))
    await_strokes(frame, 1)
    print("The EFL counter changed from 0 to 1 after a tablet click.")
