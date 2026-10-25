#!/usr/bin/env python3
"""Check compositor selection and immutable executable handoff without a desktop."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from native_compositor import select_niri, sha256, stage_niri

ROOT = Path(__file__).resolve().parents[1]
NIRI = None


class Selection(unittest.TestCase):
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
