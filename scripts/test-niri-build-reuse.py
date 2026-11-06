#!/usr/bin/env python3
"""Check prepared-source reuse without invoking Cargo or creating a build cache."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("niri_builder", Path(__file__).with_name("build-niri-screencast.py"))
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class PreparedReuse(unittest.TestCase):
    def fixture(self, directory):
        work = Path(directory)
        source = work / "niri"
        source.mkdir()
        (source / "Cargo.lock").write_text("retained lock")
        (source / "lib.rs").write_text("retained source")
        sources = {"niri": source}
        record = {"sources": {"niri": {"revision": "pinned"}}}
        receipt = {"overlay": "fixture", "recipe_sha256": builder.vendor_sources.digest(json.dumps(record, sort_keys=True).encode()),
                   "sources": {"niri": str(source)}, "inputs": builder.source_inputs(sources), "status": "built"}
        (work / "canopa-build.json").write_text(json.dumps(receipt))
        return work, sources, record, receipt

    def test_unchanged_reuse_preserves_paths_contents_and_mtimes(self):
        with tempfile.TemporaryDirectory() as directory:
            work, sources, record, receipt = self.fixture(directory)
            before = {str(path): path.stat().st_mtime_ns for path in sources["niri"].iterdir()}
            self.assertEqual(builder.reuse_prepared(work, sources, "fixture", record), receipt)
            self.assertEqual(before, {str(path): path.stat().st_mtime_ns for path in sources["niri"].iterdir()})

    def test_changed_removed_and_added_inputs_are_refused(self):
        for change in ("edit", "remove", "add", "symlink"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as directory:
                work, sources, record, receipt = self.fixture(directory)
                path = sources["niri"] / "lib.rs"
                if change == "edit":
                    path.write_text("unexpected source")
                elif change == "remove":
                    path.unlink()
                elif change == "add":
                    path.with_name("extra.rs").write_text("extra")
                else:
                    path.with_name("alias.rs").symlink_to(path)
                with self.assertRaises(ValueError):
                    builder.reuse_prepared(work, sources, "fixture", record)

    def test_changed_recipe_or_overlay_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            work, sources, record, receipt = self.fixture(directory)
            with self.assertRaises(ValueError):
                builder.reuse_prepared(work, sources, "other", record)
            record["sources"]["niri"]["revision"] = "different"
            with self.assertRaises(ValueError):
                builder.reuse_prepared(work, sources, "fixture", record)


if __name__ == "__main__":
    unittest.main()
