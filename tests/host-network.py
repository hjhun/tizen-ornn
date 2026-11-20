#!/usr/bin/python3
import contextlib
import io
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch

module = runpy.run_path(str(Path(__file__).resolve().parents[1] / "scripts/host-network"))
settings = module["settings"]


class HostNetworkTests(unittest.TestCase):
    def config(self, env, windows=None, resolver="nameserver 127.0.0.53\n"):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "resolv.conf"
            path.write_text(resolver)
            with patch.dict(settings.__globals__, windows_settings=lambda: windows or {}):
                return settings(env, path)

    def test_no_configuration(self):
        self.assertEqual(self.config({}), {"proxy": {}, "search": []})

    def test_linux_values_override_windows(self):
        data = self.config({"http_proxy": "http://localhost:3128", "HTTP_PROXY": "http://other:9",
                            "HTTPS_PROXY": "https://[::1]:8080", "no_proxy": "localhost,.example"},
                           {"enabled": True, "server": "elsewhere:1"})
        self.assertEqual(data["proxy"], {"http_proxy": "http://10.0.2.2:3128",
                                       "https_proxy": "https://10.0.2.2:8080",
                                       "no_proxy": "localhost,.example"})

    def test_manual_windows_proxy_and_bypass(self):
        data = self.config({}, {"enabled": True, "server": "http=corp:8080;https=corp:8443;socks=corp:1080",
                                "bypass": "*.intranet.example;127.0.0.1"})
        self.assertEqual(data["proxy"], {"http_proxy": "http://corp:8080", "https_proxy": "http://corp:8443",
                                        "all_proxy": "socks5h://corp:1080", "no_proxy": ".intranet.example,127.0.0.1"})

    def test_last_search_line_overrides_domain(self):
        data = self.config({}, resolver="domain old.example\nsearch corp.example team.example # suffixes\n")
        self.assertEqual(data["search"], ["corp.example", "team.example"])

    def test_credential_not_in_output_or_diagnostic(self):
        with contextlib.redirect_stderr(io.StringIO()) as log:
            data = self.config({"ALL_PROXY": "socks5h://example-user:example-password@[2001:db8::7]:1080"})
        self.assertEqual(data["proxy"], {"all_proxy": "socks5h://[2001:db8::7]:1080"})
        self.assertNotIn("example-user", log.getvalue())
        self.assertNotIn("example-password", log.getvalue())

    def test_invalid_values_cannot_become_guest_configuration(self):
        for name, value in (("http_proxy", "https://proxy/?token=hidden"), ("no_proxy", "localhost\nBAD=value"),
                            ("https_proxy", "http://1.2.3.4:99999")):
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.config({name: value})


if __name__ == "__main__":
    unittest.main()
