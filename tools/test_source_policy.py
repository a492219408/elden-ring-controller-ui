# SPDX-License-Identifier: GPL-3.0-only
import contextlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import source_policy as policy


class SourcePolicyTests(unittest.TestCase):
    def fixture(self, root):
        (root / 'packaging').mkdir()
        (root / 'src').mkdir()
        (root / policy.MANIFEST).write_text(f'{policy.MANIFEST}\nCargo.toml\nsrc/lib.rs\n', encoding='utf-8')
        (root / 'Cargo.toml').write_text('[package]\nversion="1.0.1"\nlicense="GPL-3.0-only"\n', encoding='utf-8')
        (root / 'src/lib.rs').write_text('// public source\n', encoding='utf-8')

    def test_private_paths_at_any_depth_are_rejected(self):
        for name in ['AGENTS.md', 'src/AGENTS.md', 'src/agents.MD', 'docs/a.md', 'publishing/a.txt',
                     'packaging/nexusmods/a.txt', 'packaging/github/a.txt', 'local/a.txt', '.git/config',
                     'assets/a.gfx', 'evidence.log', 'image.png', 'src/foo-Private/a.rs']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                policy.check_path(name)

    def test_path_traversal_and_windows_aliases_are_rejected(self):
        for name in ['../file.rs', '/file.rs', 'C:/file.rs', 'src\\file.rs', './src/lib.rs',
                     'src//lib.rs', 'src/../file.rs', 'src/lib.rs:stream', 'src/lib.rs\0',
                     '.git./config', 'src/AGENTS.md ', 'src/a\nb.rs']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                policy.check_path(name)

    def test_public_paths_are_allowed(self):
        for name in ['.github/workflows/ci.yml', 'src/lib.rs', 'tools/source_policy.py', 'README.zh-CN.md']:
            self.assertEqual(str(policy.check_path(name)), name)

    def test_duplicate_and_case_colliding_manifest_is_rejected(self):
        base = f'{policy.MANIFEST}\nCargo.toml\nsrc/lib.rs\n'
        for extra in ['src/lib.rs\n', 'SRC/lib.rs\n']:
            with self.assertRaises(ValueError):
                policy.parse_manifest(base + extra)

    def test_missing_source_member_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            (root / 'src/lib.rs').unlink()
            with self.assertRaises(FileNotFoundError):
                policy.read_sources(root)

    def test_local_agents_are_never_archived(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            (root / 'AGENTS.md').write_text('private local instructions', encoding='utf-8')
            (root / 'src/AGENTS.md').write_text('nested private instructions', encoding='utf-8')
            with contextlib.redirect_stdout(io.StringIO()):
                policy.pack(root, root / 'dist')
            archive = root / 'dist/elden-ring-controller-ui-1.0.1-source.zip'
            with zipfile.ZipFile(archive) as zipped:
                self.assertEqual(len(zipped.namelist()), 3)
                self.assertFalse(any('AGENTS.md' in name for name in zipped.namelist()))

    def test_existing_archive_cannot_be_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            with contextlib.redirect_stdout(io.StringIO()):
                policy.pack(root, root / 'dist')
            archive = root / 'dist/elden-ring-controller-ui-1.0.1-source.zip'
            before = archive.read_bytes()
            with self.assertRaises(ValueError):
                policy.pack(root, root / 'dist')
            self.assertEqual(archive.read_bytes(), before)

    def test_forced_tracked_agents_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / '.git').mkdir()
            with patch.object(policy, 'git', side_effect=[str(root).encode(), b'src/lib.rs\0AGENTS.md\0']):
                with self.assertRaisesRegex(ValueError, 'Tracked files'):
                    policy.check_git(root, ['src/lib.rs'])

    def test_dirty_worktree_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / '.git').mkdir()
            with patch.object(policy, 'git', side_effect=[str(root).encode(), b'src/lib.rs\0', b' M src/lib.rs\n']):
                with self.assertRaisesRegex(ValueError, 'clean source'):
                    policy.check_git(root, ['src/lib.rs'])

    def test_private_file_in_old_commit_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / '.git').mkdir()
            outputs = [str(root).encode(), b'src/lib.rs\0', b'', b'oldcommit\n',
                       b'100644 blob deadbeef\tAGENTS.md\0']
            with patch.object(policy, 'git', side_effect=outputs):
                with self.assertRaisesRegex(ValueError, 'Private path'):
                    policy.check_git(root, ['src/lib.rs'])

    def test_history_symlink_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / '.git').mkdir()
            outputs = [str(root).encode(), b'src/lib.rs\0', b'', b'commit\n',
                       b'120000 blob deadbeef\tsrc/lib.rs\0']
            with patch.object(policy, 'git', side_effect=outputs):
                with self.assertRaisesRegex(ValueError, 'Symlink'):
                    policy.check_git(root, ['src/lib.rs'])


if __name__ == '__main__':
    unittest.main()
