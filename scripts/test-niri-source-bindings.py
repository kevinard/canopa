#!/usr/bin/env python3
"""Verify pinned source rebinding without resolving or building another Cargo graph."""
from pathlib import Path
import tempfile
import tomllib
import unittest

from niri_source_bindings import bind_sources

URL = "https://github.com/Smithay/smithay.git"
PINS = ("ff5fa7df392cecfba049ffed55cdaa4e98a8e7ef", "8a270232f1bfc416c781273c98a3f52a812a0bba")
PACKAGES = ("smithay", "smithay-drm-extras")


class SourceBindings(unittest.TestCase):
    def inputs(self, directory, revision):
        root = Path(directory)
        niri, smithay = root / "niri", root / "smithay with spaces"
        niri.mkdir()
        (smithay / "smithay-drm-extras").mkdir(parents=True)
        manifest = "[workspace]\nmembers = []\n"
        for name in PACKAGES:
            manifest += f'[workspace.dependencies.{name}]\ngit = "{URL}"\nrev = "{revision}"\nfeatures = ["wayland_frontend"]\ndefault-features = false\n'
        manifest += '[workspace.dependencies.serde]\nversion = "1.0.0"\nfeatures = ["derive"]\n'
        (niri / "Cargo.toml").write_text(manifest)
        lock = "version = 4\n"
        for name in ("niri", "serde", "smithay-client-toolkit", *PACKAGES):
            lock += f'\n[[package]]\nname = "{name}"\nversion = "1.0.0"\n'
            if name in PACKAGES:
                lock += f'source = "git+{URL}?rev={revision}#{revision}"\ndependencies = ["serde"]\n'
            elif name in ("serde", "smithay-client-toolkit"):
                lock += 'source = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "retained fixture checksum"\n'
        (niri / "Cargo.lock").write_text(lock)
        return {"niri":niri, "smithay":smithay}, {"sources":{"smithay":{"revision":revision}}}

    def assert_refused_without_writes(self, sources, record):
        paths = [sources["niri"] / name for name in ("Cargo.toml", "Cargo.lock")]
        before = [path.read_bytes() for path in paths]
        with self.assertRaises(ValueError):
            bind_sources(sources, record)
        self.assertEqual([path.read_bytes() for path in paths], before)

    def test_both_real_pins_preserve_every_other_manifest_and_lock_value(self):
        for revision in PINS:
            with self.subTest(revision=revision), tempfile.TemporaryDirectory(prefix="canopa source binding ") as directory:
                sources, record = self.inputs(directory, revision)
                manifest = sources["niri"] / "Cargo.toml"
                lock = sources["niri"] / "Cargo.lock"
                expected_manifest = tomllib.loads(manifest.read_text())
                expected_lock = tomllib.loads(lock.read_text())
                for name, path in (("smithay", sources["smithay"]), ("smithay-drm-extras", sources["smithay"] / "smithay-drm-extras")):
                    dependency = expected_manifest["workspace"]["dependencies"][name]
                    del dependency["git"], dependency["rev"]
                    dependency["path"] = str(path)
                for package in expected_lock["package"]:
                    if package["name"] in PACKAGES:
                        del package["source"]
                receipt = bind_sources(sources, record)
                self.assertEqual(tomllib.loads(manifest.read_text()), expected_manifest)
                self.assertEqual(tomllib.loads(lock.read_text()), expected_lock)
                self.assertEqual(receipt["local_source_packages"], ["smithay", "smithay-drm-extras"])
                self.assertTrue(receipt["other_locked_dependencies_unchanged"])

    def test_changed_manifest_revision_or_repository_refuses_both_writes(self):
        for old, new in [(PINS[1], PINS[0]), (URL, "https://invalid.example/smithay.git")]:
            with self.subTest(value=new), tempfile.TemporaryDirectory() as directory:
                sources, record = self.inputs(directory, PINS[1])
                manifest = sources["niri"] / "Cargo.toml"
                manifest.write_text(manifest.read_text().replace(old, new))
                self.assert_refused_without_writes(sources, record)

    def test_a_different_locked_repository_with_the_right_revision_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            sources, record = self.inputs(directory, PINS[1])
            lock = sources["niri"] / "Cargo.lock"
            lock.write_text(lock.read_text().replace(URL, "https://invalid.example/smithay.git"))
            self.assert_refused_without_writes(sources, record)

    def test_missing_or_duplicate_locked_packages_refuse_both_writes(self):
        for duplicate in (False, True):
            with self.subTest(duplicate=duplicate), tempfile.TemporaryDirectory() as directory:
                sources, record = self.inputs(directory, PINS[1])
                lock = sources["niri"] / "Cargo.lock"
                text = lock.read_text()
                last = "[[package]]" + text.split("[[package]]")[-1]
                lock.write_text(text + last if duplicate else text[:-len(last)])
                self.assert_refused_without_writes(sources, record)


if __name__ == "__main__":
    unittest.main()
