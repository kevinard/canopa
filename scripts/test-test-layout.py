#!/usr/bin/env python3
"""Guard test discovery and the workspace's integration-binary budget."""
import json
from pathlib import Path
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]


def modules(suite):
    for relative, name in re.findall(r'(?:#\[path = "([^"]+)"\]\n)?^[ \t]*mod (\w+);$', suite.read_text(), re.MULTILINE):
        path = suite.parent / (relative or f"{name}.rs")
        if not relative and not path.exists():
            path = suite.parent / name / "mod.rs"
        yield name, path


class TestLayout(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"],
            cwd=ROOT, text=True,
        ))

    def test_workspace_integration_binary_budget(self):
        targets = [(package["name"], target["name"])
                   for package in self.metadata["packages"]
                   for target in package["targets"] if "test" in target["kind"]]
        # Includes the custom-harness Files native fixtures, which host the GIO
        # helper entrypoint themselves (see docs/testing.md).
        self.assertLessEqual(len(targets), 18, targets)

    def test_every_test_file_is_registered_exactly_once(self):
        for package in self.metadata["packages"]:
            directory = Path(package["manifest_path"]).parent / "tests"
            if not directory.exists():
                continue
            with self.subTest(package=package["name"]):
                targets = [Path(target["src_path"]) for target in package["targets"]
                           if "test" in target["kind"]]
                suite = directory / "integration.rs"
                self.assertIn(suite, targets)
                sources = list(modules(suite))
                self.assertTrue(all(path.exists() for _, path in sources), sources)
                registered = targets + [path for _, path in sources if path.parent == directory]
                files = set(directory.glob("*.rs"))
                self.assertEqual(set(registered), files)
                self.assertEqual(len(registered), len(files), "duplicate test registration")

    def test_subprocess_helpers_use_their_module_qualified_test_names(self):
        for suite in ROOT.glob("crates/*/tests/integration.rs"):
            for module, path in modules(suite):
                text = path.read_text()
                if "current_exe()" not in text:
                    continue
                names = re.findall(r"#\[test\]\s*(?:#\[[^\]]*\]\s*)*(?:async )?fn (\w+)", text)
                for name in names:
                    self.assertNotIn(f'"{name}"', text,
                                     f"{path}: subprocess filters must use {module}::{name}")


if __name__ == "__main__":
    unittest.main()
