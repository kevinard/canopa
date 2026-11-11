#!/usr/bin/env python3
"""Positive and negative cases for scripts/check-i18n.py."""
import importlib.util
import io
from contextlib import redirect_stdout, redirect_stderr
from pathlib import Path
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("check_i18n", ROOT / "scripts" / "check-i18n.py")
check_i18n = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check_i18n)


class CheckI18n(unittest.TestCase):
    def run_lint(self, files, allowlist="", *arguments):
        with tempfile.TemporaryDirectory(prefix="canopa-check-i18n-") as temporary:
            root = Path(temporary)
            for relative, text in files.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(textwrap.dedent(text))
            (root / "scripts").mkdir()
            (root / "scripts" / "i18n-allowlist.txt").write_text(allowlist)
            output, errors = io.StringIO(), io.StringIO()
            with redirect_stdout(output), redirect_stderr(errors):
                status = check_i18n.main(["--root", str(root), "--crate", "canopa-ui", *arguments])
            return status, output.getvalue()

    def test_literals_in_text_sinks_are_rejected(self):
        status, output = self.run_lint({"crates/canopa-ui/src/view.rs": '''
            fn view() -> Element<'static, Message> {
                column![
                    text("Open the folder"),
                    button(text(format!("Delete {name}?"))).on_press(Message::Delete),
                    widget::tooltip(icon, "Show hidden files", Position::Bottom),
                    Node::button("Save changes").key("settings-apply"),
                    text_input("Search files", &self.query),
                ]
            }
            fn toast() -> Toast {
                Toast { summary: "Copy finished".into(), body: format!("{count} files copied"), id: 1 }
            }
            fn caption() -> Label {
                Label { label: "Cancel", key: "cancel" }
            }
        '''})
        self.assertEqual(status, 1)
        for literal in ["Open the folder", "Delete {name}?", "Show hidden files", "Save changes",
                        "Search files", "Copy finished", "{count} files copied", "Cancel"]:
            self.assertIn(f'"{literal}"', output)
        self.assertIn("crates/canopa-ui/src/view.rs:4:", output)

    def test_translated_identifiers_tests_and_logs_pass(self):
        status, output = self.run_lint({"crates/canopa-ui/src/view.rs": '''
            fn view() -> Element<'static, Message> {
                tracing::warn!("Could not open the folder");
                let _ = std::fs::read("x").context("Cannot read the folder");
                column![
                    text(fl!("open-folder")),
                    text("%H:%M"),
                    button(icon("go-next-symbolic")),
                    text("org.freedesktop.portal.Desktop"),
                    text("settings-page-display"),
                    text("·"),
                    text(name),
                    label("Return"),
                ]
            }
            // text("Commented out text")
            #[cfg(test)]
            mod tests {
                #[test]
                fn shows_text() { assert_eq!(text("Open the folder"), "Open the folder"); }
            }
        ''', "crates/canopa-ui/src/tests.rs": 'fn t() { text("Words in a test file"); }\n'})
        self.assertEqual(status, 0, output)

    def test_allowlist_entries_need_a_reason_and_can_cover_files(self):
        files = {"crates/canopa-ui/src/brand.rs": 'fn v() { text("Canopa Files"); title("Debug Window"); }\n',
                 "crates/canopa-ui/src/legacy/old.rs": 'fn v() { text("Old window"); }\n'}
        status, output = self.run_lint(files, '''
crates/canopa-ui/src/brand.rs: "Canopa Files"  # product name
crates/canopa-ui/src/legacy/*.rs: *  # not compiled
''')
        self.assertEqual(status, 1)
        self.assertNotIn("Canopa Files", output)
        self.assertNotIn("Old window", output)
        self.assertIn('"Debug Window"', output)
        with self.assertRaises(SystemExit):
            self.run_lint(files, 'crates/canopa-ui/src/brand.rs: "Canopa Files"\n')

    def test_report_counts_lookups_and_remaining_prose(self):
        status, output = self.run_lint({"crates/canopa-ui/src/view.rs": '''
            fn view() {
                text(fl!("open-folder"));
                text(fl!("close-folder"));
                let help = "Press Enter to open";
            }
        '''}, "", "--report")
        self.assertEqual(status, 0)
        line = next(line for line in output.splitlines() if line.startswith("canopa-ui"))
        self.assertEqual(line.split()[1:], ["2", "0", "1", "67%"])

    def test_the_converted_tree_passes(self):
        output, errors = io.StringIO(), io.StringIO()
        with redirect_stdout(output), redirect_stderr(errors):
            status = check_i18n.main([])
        self.assertEqual(status, 0, output.getvalue()[-4000:])


if __name__ == "__main__":
    unittest.main()
