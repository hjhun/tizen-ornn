"""Register this portable copy with the current user's application menu."""
import os
from pathlib import Path
import shutil
import sys

launcher, runtime = map(Path, sys.argv[1:])
data = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")
apps = data / "applications"
icons = data / "icons/hicolor/256x256/apps"
apps.mkdir(parents=True, exist_ok=True)
icons.mkdir(parents=True, exist_ok=True)
shutil.copyfile(runtime / "packaging/art/tizen-ornn.png", icons / "tizen-ornn-portable.png")
# Exec quoting is decoded after the desktop-entry string escapes.
argument = str(launcher).replace("%", "%%")
for char in ('\\', '"', '`', '$'):
    argument = argument.replace(char, '\\' + char)
argument = argument.replace('\\', '\\\\').replace('\n', '\\n').replace('\r', '\\r')
entry = (runtime / "packaging/tizen-ornn.desktop").read_text()
entry = entry.replace("Exec=tizen-ornn-gui", f'Exec="{argument}"')
entry = entry.replace("Icon=tizen-ornn\n", "Icon=tizen-ornn-portable\n")
entry = entry.replace("StartupWMClass=tizen-ornn\n", "StartupWMClass=tizen-ornn-portable\n")
(apps / "tizen-ornn-portable.desktop").write_text(entry)
