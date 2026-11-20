#!/usr/bin/python3
"""Exercise the transport format and explicit same-NEVRA reinstall selection."""
import io
from pathlib import Path
import runpy
import tarfile
import tempfile
import unittest

helper = runpy.run_path(str(Path(__file__).resolve().parents[1] / "guest/tizen-update-rpms"))
extract = helper["extract"]
replacements = helper["replacements"]


class PackageUpdateTests(unittest.TestCase):
    def test_selective_reinstalls_only_changed_installed_payloads(self):
        k = lambda name: name + "|x86_64|0|1.0|1"
        installed = {k("rebuilt"), k("unrelated"), k("equal"), k("local"), k("new-version")}
        known = {k("rebuilt"): "a", k("unrelated"): "a", k("equal"): "b", k("new-version"): "a"}
        new = {k(name): ("tizen-emulator", "b") for name in ("rebuilt", "unrelated", "equal", "added")}
        new["new-version|x86_64|0|2.0|1"] = ("tizen-emulator", "c")
        self.assertEqual(replacements(installed, known, new, ["rebuilt"]), [k("rebuilt")])
        known[k("rebuilt")] = "b"
        self.assertEqual(replacements(installed, known, new, ["rebuilt"]), [])
        self.assertEqual(replacements(installed, known, new, []), [k("unrelated")])

    def assert_bad_archive(self, rows, message):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            with tarfile.open(base / "metadata", "w:gz") as archive:
                for name, kind in rows:
                    member = tarfile.TarInfo(name)
                    member.type = kind
                    member.size = 1 if kind == tarfile.REGTYPE else 0
                    member.linkname = "/tmp/invalid" if kind in (tarfile.LNKTYPE, tarfile.SYMTYPE) else ""
                    archive.addfile(member, io.BytesIO(b"x"))
            (base / "stage").mkdir()
            with self.assertRaisesRegex(ValueError, message):
                extract(base / "metadata", base / "stage")
            self.assertFalse((base / "escape").exists())

    def test_member_paths_and_types(self):
        for name in ("../escape", "/etc/shadow", "usr/share/tizen-snapshot/../../escape", "etc/zypp/repos.d/other.repo"):
            with self.subTest(name=name):
                self.assert_bad_archive([(name, tarfile.REGTYPE)], "invalid archive member")
        for kind in (tarfile.LNKTYPE, tarfile.SYMTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE, tarfile.DIRTYPE):
            with self.subTest(kind=kind):
                self.assert_bad_archive([("packages.json", kind)], "invalid archive member")

    def test_duplicate_and_incomplete_bundles(self):
        self.assert_bad_archive([("packages.json", tarfile.REGTYPE)] * 2, "invalid archive member")
        self.assert_bad_archive([("packages.json", tarfile.REGTYPE)], "incomplete")


if __name__ == "__main__":
    unittest.main()
