#!/usr/bin/env python3
"""Check image selection and recovery without building an entire guest."""
from contextlib import ExitStack
from importlib.machinery import SourceFileLoader
from importlib.util import module_from_spec, spec_from_loader
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

root = Path(__file__).resolve().parents[1]
loader = SourceFileLoader('image_manager', str(root / 'tizen-vm'))
manager = module_from_spec(spec_from_loader(loader.name, loader))
loader.exec_module(manager)
source = 'https://mirror.example/Tizen-10.1/'
release = 'tizen-10.1-unified_20261119.053314'


class ImageSelection(unittest.TestCase):
    def setUp(self):
        self.stack = ExitStack()
        self.addCleanup(self.stack.close)
        self.home = Path(self.stack.enter_context(TemporaryDirectory()))
        self.old = self.home / 'dev'
        self.old.mkdir()
        (self.old / 'bzImage').write_bytes(b'old kernel')
        (self.old / 'tizen-rootfs.ext4').write_bytes(b'original disk contents')
        self.stack.enter_context(patch.dict(manager.os.environ, {'TIZEN_VM_HOME': str(self.home)}))
        self.stack.enter_context(patch.object(manager, 'dependencies'))
        self.stack.enter_context(patch.object(manager, 'requested_image', return_value=(release, source)))
        self.builder = self.stack.enter_context(patch.object(manager, 'build_image', side_effect=self.build))
        self.copy = self.stack.enter_context(patch.object(manager, 'run_as_root'))

    def build(self, stage, version):
        for name in ('bzImage', 'tizen-rootfs.ext4', '.image-ready'):
            (stage / name).touch()
        (stage / '.tizen-vm-image-revision').write_text((root / 'config/image-revision').read_text())
        (stage / 'snapshot.lock.json').write_text(json.dumps({
            'release': version,
            'archive': {'url': f'{source}Tizen-10.1-Unified/{version}/images/file.tar.gz'},
        }))

    def update(self):
        manager.prepare(self.home, manager.active_image(self.home), 'latest')
        return manager.active_image(self.home)

    def test_update_noop_and_rollback_leave_both_disks_intact(self):
        new = self.update()
        self.assertNotEqual(new, self.old)
        self.copy.assert_called_once()
        self.assertEqual(new, self.update())
        self.builder.assert_called_once()
        (new / 'tizen-rootfs.ext4').write_bytes(b'new disk edits')
        manager.rollback(self.home, new)
        self.assertEqual(manager.active_image(self.home), self.old)
        self.assertEqual((self.old / 'tizen-rootfs.ext4').read_bytes(), b'original disk contents')
        manager.rollback(self.home, self.old)
        self.assertEqual(manager.active_image(self.home), new)
        self.assertEqual((new / 'tizen-rootfs.ext4').read_bytes(), b'new disk edits')

    def test_disk_lock_blocks_update_and_rollback(self):
        with manager.stopped(self.old), self.assertRaisesRegex(ValueError, 'in use'):
            self.update()
        self.builder.assert_not_called()
        new = self.update()
        with manager.stopped(new), self.assertRaisesRegex(ValueError, 'in use'):
            manager.rollback(self.home, new)
        self.assertEqual(manager.active_image(self.home), new)

    def test_interrupted_copy_retries_without_exposing_half_an_image(self):
        self.copy.side_effect = OSError('copy failed')
        with self.assertRaisesRegex(OSError, 'copy failed'):
            self.update()
        self.assertEqual(manager.active_image(self.home), self.old)
        self.copy.side_effect = None
        self.assertNotEqual(self.update(), self.old)
        self.builder.assert_called_once()
        self.assertEqual(self.copy.call_count, 2)

    def test_interrupted_copy_from_changed_guest_uses_a_fresh_disk(self):
        self.copy.side_effect = OSError('interrupted')
        with self.assertRaises(OSError):
            self.update()
        (self.old / 'tizen-rootfs.ext4').write_bytes(b'resumed previous guest')
        self.copy.side_effect = None
        self.update()
        self.assertEqual(self.builder.call_count, 2)

    def test_legacy_real_current_directory_is_retained(self):
        self.old.rename(self.home / 'current')
        new = self.update()
        self.assertTrue((self.home / '.tizen-current').is_symlink())
        self.assertFalse((self.home / 'current').is_symlink())
        manager.rollback(self.home, new)
        self.assertEqual(manager.active_image(self.home), self.home / 'current')

    def test_first_build_failure_preserves_empty_selection(self):
        self.old.rename(self.home / 'unselected')
        self.builder.side_effect = manager.subprocess.CalledProcessError(1, 'build')
        with self.assertRaises(manager.subprocess.CalledProcessError):
            self.update()
        self.assertIsNone(manager.active_image(self.home))
        self.builder.side_effect = self.build
        new = self.update()
        self.assertTrue(new.is_dir())
        self.copy.assert_not_called()

    def test_legacy_tizen_out_remains_launchable_after_managed_update(self):
        with patch.dict(manager.os.environ, {'TIZEN_OUT': str(self.old), 'TIZEN_VM_HOME': ''}):
            home = manager.data_home()
            self.assertEqual(home, self.old)
            self.assertEqual(manager.active_image(home), self.old)
            manager.prepare(home, self.old, 'latest')
            self.assertNotEqual(manager.active_image(home), self.old)
            manager.rollback(home, manager.active_image(home))
            self.assertEqual(manager.active_image(home), self.old)

    def test_matching_release_requires_same_source_and_recipe_revision(self):
        image = self.update()
        revision = (root / 'config/image-revision').read_text().strip()
        self.assertFalse(manager.matches(image, release, 'https://other.example/', revision))
        self.assertFalse(manager.matches(image, release, source, 'old'))

    def test_live_commands_select_disk_and_forward_arguments(self):
        disk = str(self.old / 'tizen-rootfs.ext4')
        examples = [
            (['install', 'codex', 'ed'], ['guest-command', disk, 'install', 'codex', 'ed']),
            (['update', '--packages'], ['update-packages', disk]),
            (['update', '--packages', '--snapshot', release, 'ed'], ['update-packages', disk, '--snapshot', release, 'ed']),
        ]
        for args, expected in examples:
            with self.subTest(args=args), patch.object(manager.sys, 'argv', ['tizen-vm', *args]), \
                    patch.object(manager.os, 'execv', side_effect=SystemExit(0)) as run, self.assertRaises(SystemExit):
                try:
                    manager.main()
                finally:
                    helper = str(manager.SCRIPTS / expected[0])
                    run.assert_called_once_with(helper, [helper, *expected[1:]])

    def test_build_reuses_existing_disk_and_run_selects_it(self):
        with patch.object(manager.sys, 'argv', ['tizen-vm', 'build']):
            manager.main()
        self.builder.assert_not_called()
        with patch.dict(manager.os.environ), patch.object(manager.sys, 'argv', ['tizen-vm', 'run', '--gui']), \
                patch.object(manager.os, 'execv', side_effect=SystemExit(0)) as run, self.assertRaises(SystemExit):
            try:
                manager.main()
            finally:
                self.assertEqual(manager.os.environ['TIZEN_OUT'], str(self.old))
                self.assertEqual(run.call_args.args[1][-1], '--gui')

    def test_run_and_start_forward_share_paths_as_one_argument(self):
        folder = './project with spaces,commas'
        for verb, option in [('run', '--share'), ('start', '--share-ro')]:
            argv = ['tizen-vm', verb, '--gui', option, folder]
            with self.subTest(verb=verb), patch.dict(manager.os.environ), patch.object(manager.sys, 'argv', argv), \
                    patch.object(manager.os, 'execv', side_effect=SystemExit(0)) as run:
                with self.assertRaises(SystemExit):
                    manager.main()
                self.assertEqual(run.call_args.args[1], [str(manager.SCRIPTS / 'run-vm.sh'), '--gui', option, folder])

    def test_user_config_persists_partial_changes_and_validates_before_replacing(self):
        directory = self.home / 'xdg'
        with patch.dict(manager.os.environ, XDG_CONFIG_HOME=str(directory), TIZEN_IMAGE_CONFIG=''):
            manager.configure(source, release)
            dest = directory / 'tizen-vm/image.toml'
            self.assertEqual(manager.config_path(), dest)
            prior = dest.read_bytes()
            with self.assertRaisesRegex(ValueError, 'HTTPS'):
                manager.configure('http://invalid', None)
            self.assertEqual(dest.read_bytes(), prior)
            newer = 'tizen-10.1-unified_20261120.000000'
            manager.configure(None, newer)
            saved = manager.ImageSource.load()
            self.assertEqual((saved.base_url, saved.snapshot), (source, newer))
            self.assertEqual(dest.stat().st_mode & 0o777, 0o600)


if __name__ == '__main__':
    unittest.main()
