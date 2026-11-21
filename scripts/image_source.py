"""Select a dated Tizen 10.1 image from the configured HTTPS snapshot tree."""
from dataclasses import dataclass
import os
from pathlib import Path
import re
import subprocess
import tomllib
from urllib.parse import urlsplit
import xml.etree.ElementTree as ET

PROJECT = Path(__file__).resolve().parent.parent
PROFILE = "tizen-headed-emulator64-wayland"
DATED = r"[0-9]{8}\.[0-9]{6}"
RELEASE = rf"tizen-10\.1-unified_{DATED}"


def read_url(url):
    return subprocess.check_output(
        ["curl", "-fLsS", "--retry", "2", "--proto", "=https", "--proto-redir", "=https", url]
    )


def user_config():
    override = os.environ.get("TIZEN_IMAGE_CONFIG")
    if override:
        return Path(override).expanduser().absolute()
    directory = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    return directory / "tizen-ornn/image.toml"


def config_path():
    user = user_config()
    if os.environ.get("TIZEN_IMAGE_CONFIG") or user.exists():
        return user
    legacy = user.parent.parent / "tizen-vm/image.toml"
    if legacy.is_file():
        return legacy
    system = Path("/etc/tizen-ornn/image.toml")
    return system if system.exists() else PROJECT / "config/image.toml"


@dataclass(frozen=True)
class ImageSource:
    base_url: str
    snapshot: str

    @classmethod
    def load(cls, path=None):
        path = path or config_path()
        with path.open("rb") as config:
            values = tomllib.load(config)
        if set(values) != {"repository", "snapshot"}:
            raise ValueError(f"{path}: expected repository and snapshot")
        return cls.validated(values["repository"], values["snapshot"], path)

    @classmethod
    def validated(cls, url, release, path):
        parts = urlsplit(url) if isinstance(url, str) else None
        if (not parts or parts.scheme != "https" or not parts.hostname
                or parts.username is not None or parts.password is not None
                or "?" in url or "#" in url or re.search(r"[\x00-\x20\x7f]", url)):
            raise ValueError(f"{path}: repository must be an HTTPS base URL without credentials, query or fragment")
        if not isinstance(release, str) or not re.fullmatch(RELEASE, release):
            raise ValueError(f"{path}: snapshot must be a dated Tizen 10.1 Unified ID")
        return cls(url.rstrip("/") + "/", release)

    def archive_url(self, release):
        return (f"{self.base_url}Tizen-10.1-Unified/{release}/images/emulator/{PROFILE}/"
                f"{release}_{PROFILE}.tar.gz")

    def latest(self):
        url = self.base_url + "Tizen-10.1-Unified/latest/build.xml"
        release = ET.fromstring(read_url(url)).findtext("id", "")
        if not re.fullmatch(RELEASE, release):
            raise ValueError(f"{url} did not identify a dated Tizen 10.1 Unified release")
        return release

    def resolve(self, value):
        if value is None:
            return self.snapshot
        if value == "latest":
            return self.latest()
        match = re.search(RELEASE, value)
        if match and value in (match[0], self.archive_url(match[0])):
            return match[0]
        raise ValueError("expected latest, a dated Tizen 10.1 Unified ID, or its configured emulator archive URL")
