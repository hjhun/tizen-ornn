#!/usr/bin/env python3
"""Check snapshot selection and mirror routing without contacting upstream."""
import os
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))
from image_source import ImageSource

repositories = runpy.run_path(str(SCRIPTS / "tizen-image"))["repositories"]
RELEASE = "tizen-10.1-unified_20261119.053314"
BASE = "tizen-10.1-base_20261118.084133"
MIRROR = "https://mirror.example.test/platform/10.1/"


class ImageSourceTest(unittest.TestCase):
    def config(self, repository):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        path = Path(temp.name) / "image.toml"
        path.write_text(f'repository = "{repository}"\nsnapshot = "{RELEASE}"\n')
        with patch.dict(os.environ, TIZEN_IMAGE_CONFIG=str(path)):
            return ImageSource.load()

    def test_config_selects_origin_and_dated_archive(self):
        source = self.config(MIRROR.rstrip("/"))
        self.assertEqual(source.base_url, MIRROR)
        self.assertEqual(source.resolve(None), RELEASE)
        self.assertEqual(source.resolve(source.archive_url(RELEASE)), RELEASE)
        with self.assertRaises(ValueError):
            source.resolve(source.archive_url(RELEASE).replace("mirror.", "other."))

    def test_rejects_credentials_and_ambiguous_repositories(self):
        for repository in ("http://mirror.example.test", "https://u:p@server/",
                           "https://server/tree?token=123", "https://server/tree#a"):
            with self.subTest(repository=repository), self.assertRaises(ValueError):
                self.config(repository)

    def test_latest_resolves_to_immutable_release(self):
        source = self.config(MIRROR)
        with patch("image_source.read_url", return_value=f"<build><id>{RELEASE}</id></build>".encode()) as read:
            self.assertEqual(source.resolve("latest"), RELEASE)
            read.assert_called_once_with(MIRROR + "Tizen-10.1-Unified/latest/build.xml")
        with patch("image_source.read_url", return_value=b"<build><id>latest</id></build>"), self.assertRaises(ValueError):
            source.resolve("latest")

    def test_original_build_metadata_on_a_mirror(self):
        build = f"""
        <build><id>{RELEASE}</id><base_id url="http://download.tizen.org/snapshots/TIZEN/Tizen-10.1/Tizen-10.1-Base/">{BASE}</base_id>
        <buildtargets>
        <buildtarget name="emulator"><archs><arch>x86_64</arch></archs><repo type="binary">repos/emulator/packages</repo></buildtarget>
        <buildtarget name="standard"><archs><arch>armv7l</arch></archs><repo type="binary">repos/standard/packages</repo></buildtarget>
        </buildtargets></build>"""
        urls = repositories(build, RELEASE, MIRROR)
        self.assertEqual(urls["tizen-base"], MIRROR + f"Tizen-10.1-Base/{BASE}/repos/standard/packages/")
        for name in ("emulator", "standard"):
            self.assertEqual(urls["tizen-" + name], MIRROR + f"Tizen-10.1-Unified/{RELEASE}/repos/{name}/packages/")


if __name__ == "__main__":
    unittest.main()
