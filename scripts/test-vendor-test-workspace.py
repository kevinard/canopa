#!/usr/bin/env python3
"""Check repeatable fork-test source synchronization without compiling Rust."""
from pathlib import Path
import tempfile
import unittest

from vendor_test_workspace import sync_tree


class Synchronization(unittest.TestCase):
    def test_symlinked_destination_is_refused_without_touching_its_target(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, outside, target = root / "source", root / "outside", root / "copy"
            source.mkdir()
            outside.mkdir()
            (outside / "keep").write_text("retained")
            target.symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "must not be symlinks"):
                sync_tree(source, target)
            self.assertEqual((outside / "keep").read_text(), "retained")

    def test_unchanged_transformed_sources_keep_mtimes_and_changes_remove_stale_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, target = root / "source", root / "copy"
            (source / "src").mkdir(parents=True)
            (source / "Cargo.toml").write_text("original manifest")
            (source / "src/lib.rs").write_text("first source")
            (source / "retired").write_text("old source")
            (source / "script").write_text("executable source")
            (source / "script").chmod(0o755)
            overrides = {"Cargo.toml": b"prepared manifest"}
            sync_tree(source, target, overrides=overrides)
            before = {p.relative_to(target): p.stat().st_mtime_ns for p in target.rglob("*") if p.is_file()}
            sync_tree(source, target, overrides=overrides)
            self.assertEqual(before, {p.relative_to(target): p.stat().st_mtime_ns for p in target.rglob("*") if p.is_file()})
            self.assertEqual((target / "script").stat().st_mode & 0o777, 0o755)
            (source / "retired").unlink()
            (source / "src/lib.rs").write_text("changed source")
            (target / "src/lib.rs").chmod(0o444)
            sync_tree(source, target, overrides=overrides)
            self.assertFalse((target / "retired").exists())
            self.assertEqual((target / "src/lib.rs").read_text(), "changed source")
            self.assertEqual((target / "Cargo.toml").stat().st_mtime_ns, before[Path("Cargo.toml")])
            self.assertEqual((source / "Cargo.toml").read_text(), "original manifest")


if __name__ == "__main__":
    unittest.main()
