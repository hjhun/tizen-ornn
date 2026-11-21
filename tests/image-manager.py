#!/usr/bin/env python3
"""Check image selection and recovery without building an entire guest."""
from contextlib import ExitStack
from importlib.machinery import SourceFileLoader
from importlib.util import module_from_spec, spec_from_loader
import json
import os
from types import SimpleNamespace
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

root = Path(__file__).resolve().parents[1]
loader = SourceFileLoader('image_manager', str(root / 'scripts/linux-backend'))
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
        self.stack.enter_context(patch.dict(manager.os.environ, {'TIZEN_ORNN_HOME': str(self.home)}))
        self.stack.enter_context(patch.object(manager, 'dependencies'))
        self.stack.enter_context(patch.object(manager, 'requested_image', return_value=(release, source)))
        self.builder = self.stack.enter_context(patch.object(manager, 'build_image', side_effect=self.build))
        self.copy = self.stack.enter_context(patch.object(manager, 'run_as_root'))

    def invoke(self):
        # Exercise the compiled parser, then apply its request to the fixture
        # backend in-process so locks, prompts and selected disks are observable.
        default = root / ('target/release/tizen-ornn' if (root / 'Cargo.toml').is_file() else 'tizen-ornn')
        binary = Path(os.environ.get('TIZEN_ORNN_TEST_CLI', default))
        directory = self.home / 'parser'
        backend = directory / 'scripts/linux-backend'
        if not backend.exists():
            backend.parent.mkdir(parents=True)
            (directory / 'tizen-ornn').write_bytes(binary.read_bytes())
            (directory / 'tizen-ornn').chmod(0o755)
            backend.write_text('#!/usr/bin/python3\nimport sys\nprint(sys.argv[1])\n')
            backend.chmod(0o755)
        output = subprocess.check_output([directory / 'tizen-ornn', *manager.sys.argv[1:]])
        manager.execute(SimpleNamespace(**json.loads(output)))

    def build(self, stage, version):
        for name in ('bzImage', 'tizen-rootfs.ext4', '.image-ready'):
            (stage / name).touch()
        (stage / '.tizen-ornn-image-revision').write_text((root / 'config/image-revision').read_text())
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
        (self.old / 'tizen-rootfs.ext4').write_bytes(b'resumed previous guest with new data')
        self.copy.side_effect = None
        self.update()
        self.assertEqual(self.builder.call_count, 2)

    def test_delete_managed_disk_and_restore_previous(self):
        new = self.update()
        disk = manager.disk_at(new)
        with manager.stopped(new), self.assertRaisesRegex(ValueError, 'in use'):
            manager.delete_image(self.home, new, new.name, True)
        with self.assertRaisesRegex(ValueError, 'cancelled'), patch('sys.stdin.isatty', return_value=False):
            manager.delete_image(self.home, new, new.name, False)
        self.assertTrue(disk.exists())
        manager.delete_image(self.home, new, new.name, True)
        self.assertFalse(disk.exists())
        self.assertEqual(manager.active_image(self.home), self.old)

    def test_delete_preserves_qcow2_backing_disk(self):
        new = self.update()
        overlay = new.parent / 'child'
        overlay.mkdir()
        disk = manager.disk_at(new)
        subprocess.run(['qemu-img', 'create', '-q', '-f', 'qcow2',
                                '-F', 'raw', '-b', disk, overlay / 'tizen-rootfs.qcow2'],
                               check=True)
        with self.assertRaisesRegex(ValueError, 'backing image'):
            manager.delete_image(self.home, new, new.name, True)
        self.assertTrue(disk.exists())
        self.assertEqual(manager.active_image(self.home), new)

    def test_delete_rejects_paths_outside_the_managed_store(self):
        new = self.update()
        alias = new.parent / 'alias'
        alias.symlink_to(self.old)
        for name in ('../dev', str(self.old), 'alias', '.', '.building-test'):
            with self.subTest(name=name), self.assertRaises(ValueError):
                manager.delete_image(self.home, new, name, True)
        self.assertTrue(manager.disk_at(self.old).is_file())

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
        with patch.dict(manager.os.environ, {'TIZEN_OUT': str(self.old), 'TIZEN_ORNN_HOME': ''}):
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
            (['shutdown'], ['guest-command', disk, 'shutdown']),
            (['install', 'codex', 'ed'], ['guest-command', disk, 'install', 'codex', 'ed']),
            (['update', '--packages'], ['update-packages', disk]),
            (['update', '--packages', '--snapshot', release, 'ed'], ['update-packages', disk, '--snapshot', release, 'ed']),
        ]
        for args, expected in examples:
            with self.subTest(args=args), patch.object(manager.sys, 'argv', ['tizen-ornn', *args]), \
                    patch.object(manager.os, 'execv', side_effect=SystemExit(0)) as run, self.assertRaises(SystemExit):
                try:
                    self.invoke()
                finally:
                    helper = str(manager.SCRIPTS / expected[0])
                    run.assert_called_once_with(helper, [helper, *expected[1:]])

    def interactive(self, args, answers):
        with patch.object(manager.sys, 'argv', ['tizen-ornn', *args]), \
                patch('sys.stdin.isatty', return_value=True), \
                patch('sys.stdout.isatty', return_value=True), \
                patch('builtins.input', side_effect=answers):
            self.invoke()

    def test_update_menu_migrates_selected_source(self):
        previous = self.update()
        self.interactive(['update'], ['2'])
        self.assertEqual(self.copy.call_args.args[0][-2], manager.disk_at(self.old))
        current = manager.active_image(self.home)
        self.assertNotEqual(current, previous)
        self.assertTrue(manager.disk_at(previous).is_file())
        manager.rollback(self.home, current)
        self.assertEqual(manager.active_image(self.home), self.old)

    def test_package_menu_dispatches_selected_disk_and_can_cancel(self):
        self.update()
        with patch.object(manager.os, 'execv', side_effect=SystemExit(0)) as run:
            self.interactive(['update', '--packages'], ['q'])
            run.assert_not_called()
            with self.assertRaises(SystemExit):
                self.interactive(['update', '--packages', 'ed'], ['2'])
            self.assertEqual(run.call_args.args[1][1:], [str(manager.disk_at(self.old)), 'ed'])

    def test_delete_menu_cancels_or_confirms_selected_arm_partitions(self):
        current = self.update()
        arm = self.home / 'armv7l-headed'
        disk = arm / 'disk'
        disk.mkdir(parents=True)
        for name in manager.ARM_PARTITIONS:
            (disk / f'{name}.img').touch()
        self.interactive(['delete'], ['q'])
        with self.assertRaisesRegex(ValueError, 'cancelled'):
            self.interactive(['delete', '--yes'], ['2', 'wrong'])
        self.assertEqual(len(list(disk.iterdir())), 4)
        with manager.lock_file(arm / 'build.lock', 'busy'), self.assertRaisesRegex(ValueError, 'in use'):
            self.interactive(['delete'], ['2'])
        self.interactive(['delete'], ['2', arm.name])
        self.assertFalse(disk.exists())
        self.assertTrue(manager.disk_at(current).exists())

    def test_build_reuses_existing_disk_and_run_selects_it(self):
        with patch.object(manager.sys, 'argv', ['tizen-ornn', 'build']):
            self.invoke()
        self.builder.assert_not_called()
        with patch.dict(manager.os.environ), patch.object(manager.sys, 'argv', ['tizen-ornn', 'run', '--gui']), \
                patch.object(manager.os, 'execv', side_effect=SystemExit(0)) as run, self.assertRaises(SystemExit):
            try:
                self.invoke()
            finally:
                self.assertEqual(manager.os.environ['TIZEN_OUT'], str(self.old))
                self.assertEqual(run.call_args.args[1][-1], '--gui')

    def test_display_mode_for_new_boot(self):
        for verb in ([], ['run'], ['start']):
            for flag, expected in (([], ['--gui']), (['--gui'], ['--gui']), (['--headless'], [])):
                for args in (verb + flag, flag + verb):
                    with self.subTest(args=args), patch.dict(manager.os.environ), \
                            patch.object(manager.sys, 'argv', ['tizen-ornn', *args]), \
                            patch.object(manager.os, 'execv', side_effect=SystemExit(0)) as run:
                        with self.assertRaises(SystemExit):
                            self.invoke()
                        script = manager.SCRIPTS / 'run-vm.sh'
                        run.assert_called_once_with(script, [str(script), *expected])

    def test_run_and_start_forward_share_paths_as_one_argument(self):
        folder = './project with spaces,commas'
        for verb, option in [('run', '--share'), ('start', '--share-ro')]:
            argv = ['tizen-ornn', verb, '--gui', '--host-browser', option, folder]
            with self.subTest(verb=verb), patch.dict(manager.os.environ), patch.object(manager.sys, 'argv', argv), \
                    patch.object(manager.os, 'execv', side_effect=SystemExit(0)) as run:
                with self.assertRaises(SystemExit):
                    self.invoke()
                self.assertEqual(run.call_args.args[1], [str(manager.SCRIPTS / 'run-vm.sh'), '--gui', '--host-browser', option, folder])

    def test_user_config_persists_partial_changes_and_validates_before_replacing(self):
        directory = self.home / 'xdg'
        with patch.dict(manager.os.environ, XDG_CONFIG_HOME=str(directory), TIZEN_IMAGE_CONFIG=''):
            manager.configure(source, release)
            dest = directory / 'tizen-ornn/image.toml'
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
