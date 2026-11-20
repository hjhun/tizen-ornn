#!/usr/bin/python3
"""Exercise the guest browser protocol without launching a desktop application."""
from pathlib import Path
import socket
import sys
from threading import Thread
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import host_browser


class BrowserTests(unittest.TestCase):
    def test_only_https_urls_reach_the_opener(self):
        for value in ("file:///tmp/a", "https:///path", "https://user@host/path",
                      "https://host/a b", "https://host/\nb", "https://host\\path"):
            with self.subTest(value=value), patch("subprocess.run") as opener:
                with self.assertRaises(ValueError):
                    host_browser.open_https(value)
                opener.assert_not_called()

    def test_failed_and_partial_requests_do_not_replay_stale_urls(self):
        server, client = socket.socketpair()
        ident = b"1" * 32
        valid = b"https://example.org/test?a=1&b=2"
        with patch.object(host_browser, "open_https", side_effect=[OSError(), None]) as opener:
            worker = Thread(target=host_browser.serve, args=(server,))
            worker.start()
            try:
                with client, client.makefile("rb") as stream:
                    client.settimeout(2)
                    client.sendall(b"x" * (host_browser.MAX_FRAME + 1) + ident + b" " + valid + b"\n")
                    client.sendall(ident + b" " + valid + b"\n")
                    self.assertEqual(stream.readline(), ident + b" error\n")
                    client.sendall(ident + b" ")
                    client.sendall(valid + b"\n")
                    self.assertEqual(stream.readline(), ident + b" ok\n")
            finally:
                worker.join(2)
            self.assertFalse(worker.is_alive())
            self.assertEqual([call.args for call in opener.call_args_list], [(valid.decode(),)] * 2)


if __name__ == "__main__":
    unittest.main()
