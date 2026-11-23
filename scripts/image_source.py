"""Select a dated Tizen 10.1 image from the configured HTTPS snapshot tree."""
from dataclasses import dataclass
import gzip
import hashlib
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
    system = Path("/etc/tizen-ornn/image.toml")
    return system if system.exists() else PROJECT / "config/image.toml"


@dataclass(frozen=True)
class ImageSource:
    base_url: str
    snapshot: str
    profile: str = PROFILE

    @classmethod
    def load(cls, path=None):
        path = path or config_path()
        with path.open("rb") as config:
            values = tomllib.load(config)
        if set(values) not in ({"repository", "snapshot"}, {"repository", "snapshot", "profile"}):
            raise ValueError(f"{path}: expected repository and snapshot")
        return cls.validated(values["repository"], values["snapshot"], path, values.get("profile", PROFILE))

    @classmethod
    def validated(cls, url, release, path, profile=PROFILE):
        parts = urlsplit(url) if isinstance(url, str) else None
        if (not parts or parts.scheme != "https" or not parts.hostname
                or parts.username is not None or parts.password is not None
                or "?" in url or "#" in url or re.search(r"[\x00-\x20\x7f]", url)):
            raise ValueError(f"{path}: repository must be an HTTPS base URL without credentials, query or fragment")
        if not isinstance(release, str) or not re.fullmatch(RELEASE, release):
            raise ValueError(f"{path}: snapshot must be a dated Tizen 10.1 Unified ID")
        if not isinstance(profile, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,100}", profile):
            raise ValueError(f"{path}: invalid emulator profile")
        return cls(url.rstrip("/") + "/", release, profile)

    def archive_url(self, release):
        return (f"{self.base_url}Tizen-10.1-Unified/{release}/images/emulator/{self.profile}/"
                f"{release}_{self.profile}.tar.gz")

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


def primary_index(url):
    md = read_url(url + "repodata/repomd.xml")
    primary = ET.fromstring(md).find("{*}data[@type='primary']")
    if primary is None:
        raise ValueError(f"repository has no primary index: {url}")
    digest = primary.findtext("{*}checksum[@type='sha256']")
    name = f"repodata/{digest}-primary.xml.gz"
    if not digest or not re.fullmatch("[0-9a-f]{64}", digest) or primary.find("{*}location").get("href") != name:
        raise ValueError(f"expected SHA-256 pinned primary.xml.gz in {url}")
    data = read_url(url + name)
    if hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f"primary index checksum failed for {url}")
    return md, digest, ET.fromstring(gzip.decompress(data))
