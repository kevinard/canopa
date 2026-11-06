#!/usr/bin/env python3
"""Check compositor selection and immutable executable handoff without a desktop."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import native_compositor
from native_compositor import select_niri, sha256, stage_niri

ROOT = Path(__file__).resolve().parents[1]
NIRI = None


class Selection(unittest.TestCase):
    def setUp(self):
        self.storage = tempfile.TemporaryDirectory(prefix="canopa-runner-store-")
        self.addCleanup(self.storage.cleanup)
        self.store = Path(self.storage.name) / "store"
        replacement = patch.object(native_compositor, "SNAPSHOT_STORE", self.store)
        replacement.start()
        self.addCleanup(replacement.stop)

    def source(self, name="source"):
        source = Path(self.storage.name) / name
        source.write_text("#!/bin/sh\necho 'niri fixture'\n")
        source.chmod(0o700)
        return source

    def test_runs_share_one_frozen_copy_without_sharing_mutable_build_output(self):
        source = self.source()
        selected = select_niri(source)
        paths = [Path(stage_niri(selected, Path(self.storage.name) / str(i))["snapshot"])
                 for i in range(3)]
        self.assertTrue(all(path.is_symlink() for path in paths))
        self.assertEqual(len({path.resolve() for path in paths}), 1)
        canonical = paths[0].resolve()
        self.assertNotEqual(canonical.stat().st_ino, source.stat().st_ino)
        self.assertEqual(list(self.store.glob("*/niri")), [canonical])
        source.write_text("#!/bin/sh\necho 'niri replacement'\n")
        changed = stage_niri(select_niri(source), Path(self.storage.name) / "new")
        self.assertNotEqual(Path(changed["snapshot"]).resolve(), canonical)
        self.assertEqual(sha256(canonical), selected["sha256"])
        with self.assertRaisesRegex(ValueError, "changed while preparing"):
            stage_niri(selected, Path(self.storage.name) / "stale")

    def test_corrupt_or_symlinked_cached_snapshots_are_refused_without_overwriting(self):
        source = self.source()
        selected = select_niri(source)
        canonical = self.store / selected["sha256"] / "niri"
        canonical.parent.mkdir(parents=True)
        for symlink in (False, True):
            with self.subTest(symlink=symlink):
                if symlink:
                    canonical.symlink_to(source)
                else:
                    canonical.write_text("corrupt snapshot")
                with self.assertRaisesRegex(ValueError, "Corrupt"):
                    stage_niri(selected, Path(self.storage.name) / f"bad-{symlink}")
                self.assertEqual(source.read_text(), "#!/bin/sh\necho 'niri fixture'\n")
                canonical.unlink()

    def test_change_during_copy_leaves_no_partial_snapshot(self):
        source = self.source()
        selected = select_niri(source)
        with patch.object(native_compositor.shutil, "copyfileobj",
                          side_effect=lambda source, output: output.write(b"changed during copy")):
            with self.assertRaisesRegex(ValueError, "changed while preparing"):
                stage_niri(selected, Path(self.storage.name) / "changed")
        self.assertEqual(list(self.store.rglob("niri")), [])
        self.assertEqual(list(self.store.rglob(".niri-*")), [])

    def test_media_sessions_use_the_shared_store_and_check_explicit_frozen_sources(self):
        from screencast_media import MediaSession
        source = self.source()
        checksum = sha256(source)
        root = Path(self.storage.name)
        first = MediaSession(root / "runtime", root / "media-a", source, checksum)
        with patch("screencast_media.SNAPSHOT_STORE", self.store):
            second = MediaSession(root / "runtime", root / "media-b", first.compositor.resolve(),
                                  checksum, reference_frozen=True)
            with self.assertRaisesRegex(ValueError, "Only a content-addressed"):
                MediaSession(root / "runtime", root / "media-c", source, checksum, reference_frozen=True)
        self.assertEqual(first.compositor.resolve(), second.compositor.resolve())
        self.assertTrue(second.compositor.is_symlink())

    def refusal(self, arguments, message):
        with tempfile.TemporaryDirectory(prefix="canopa-runner-refusal-") as directory:
            output = Path(directory) / "artifacts"
            result = subprocess.run(
                [sys.executable, "-B", str(ROOT / "scripts/run-desktop-acceptance.py"),
                 "--case", "files-dnd-cancel-gtk-receiver", "--output", str(output), *arguments],
                capture_output=True, text=True, timeout=15,
            )
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertIn(message, result.stderr)
            self.assertFalse(output.exists(), "refused selection created acceptance artifacts")

    def test_list_needs_no_executable_or_desktop(self):
        result = subprocess.run(
            [sys.executable, "-B", str(ROOT / "scripts/run-desktop-acceptance.py"),
             "--list", "--niri", "/no-such-canopa-runner/niri"],
            capture_output=True, text=True, timeout=15,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("files-dnd-transport: verify-files-dnd-transport.py", result.stdout)

    def test_missing_executable_is_refused_before_startup(self):
        self.refusal(["--niri", "/no-such-canopa-runner/niri"], "No such file")

    def test_directory_is_refused_before_startup(self):
        self.refusal(["--niri", "/tmp"], "must be an executable file")

    def test_malformed_hash_is_refused_before_execution(self):
        self.refusal(["--niri", "/usr/bin/true", "--niri-sha256", "invalid"], "64 hexadecimal digits")

    def test_wrong_hash_is_refused_before_execution(self):
        self.refusal(["--niri", "/usr/bin/true", "--niri-sha256", "0" * 64], "checksum mismatch")

    def test_unrelated_executable_is_refused(self):
        self.refusal(["--niri", "/usr/bin/true"], "did not identify itself as Niri")

    def test_replacing_original_preserves_selected_executable(self):
        if NIRI is None:
            self.skipTest("Niri is required for actual executable handoff")
        with tempfile.TemporaryDirectory(prefix="canopa-runner-snapshot-") as directory:
            root = Path(directory)
            original = root / "niri source with spaces"
            shutil.copyfile(NIRI, original)
            original.chmod(0o700)
            selected = select_niri(original, sha256(original))
            staged = stage_niri(selected, root / "artifacts with spaces")
            original.write_bytes(b"Original build was replaced after selection.\n")
            snapshot = Path(staged["snapshot"])
            self.assertEqual(sha256(snapshot), selected["sha256"])
            self.assertEqual(subprocess.check_output([str(snapshot), "--version"], text=True,
                                                    timeout=10).strip(), selected["version"])
            self.assertFalse(snapshot.stat().st_mode & 0o222)

    def test_source_change_during_handoff_is_refused(self):
        if NIRI is None:
            self.skipTest("Niri is required for actual executable handoff")
        with tempfile.TemporaryDirectory(prefix="canopa-runner-changed-") as directory:
            root = Path(directory)
            original = root / "niri"
            shutil.copyfile(NIRI, original)
            original.chmod(0o700)
            selected = select_niri(original)
            original.write_bytes(b"Source changed before the snapshot.\n")
            with self.assertRaisesRegex(ValueError, "changed while preparing"):
                stage_niri(selected, root / "artifacts")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--niri", type=Path, help="Real Niri for executable handoff checks; default: PATH")
    arguments, remaining = parser.parse_known_args()
    NIRI = arguments.niri or shutil.which("niri")
    unittest.main(argv=[sys.argv[0], *remaining])
