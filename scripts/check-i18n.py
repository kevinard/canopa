#!/usr/bin/env python3
"""Reject user-visible string literals in UI code that bypass Fluent.

A pragmatic heuristic, not a parser: it looks for word-like string literals
passed to the calls and struct fields that put text on screen (`text("…")`,
`button("…")`, `label: "…".into()`, notification summaries and so on) in the
crates' `src/` trees, outside test modules. Text belongs in
`i18n/en/<crate>.ftl` and is read with the crate's `fl!`.

Justified exceptions (brand names, protocol text, debug-only tools) go in
`scripts/i18n-allowlist.txt`, one per line:

    crates/canopa-x/src/file.rs: "Literal text"  # why it stays
    crates/canopa-x/src/debug_tool.rs: *         # why the whole file is exempt
    crates/canopa-x/src/legacy/*.rs: *           # paths may be fnmatch patterns

`--report` prints, per crate, the `fl!` lookups and the literals still found,
which is the conversion coverage.
"""
import argparse
import fnmatch
from collections import Counter
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]

# Crates whose text reaches people. Library crates without UI are left out.
UI_CRATES = [
    "canopa-ui", "canopa-session-ui", "canopa-session", "canopa-greeter",
    "canopa-polkit", "canopa-pinentry", "canopa-shell", "canopa-launcher",
    "canopa-notify", "canopa-settings", "canopa-files", "canopa-portal",
    "canopa-cli", "canopa-widgets",
]

# Calls whose first argument is shown (or read out by a screen reader).
CALLS = (
    "text|rich_text|span|muted|strong_text|plain_text|title_text|caption_text|"
    "label|set_label|with_label|button|text_button|primary_button|secondary_button|"
    "danger_button|text_input|search_input|placeholder|title|heading|subtitle|"
    "caption|tooltip|settings_group|section|fact|attention|note|notice|hint|"
    "show_error|error_text|warning_text|highlight|summary|set_summary|body|set_body|"
    "description|set_description|set_name|toast|banner|status_text|row_label|"
    "checkbox|toggler|radio|menu_item|command_button|tab_label"
)
FIELDS = (
    "label|title|summary|body|text|tooltip|description|placeholder|heading|"
    "message|caption|subtitle|hint|detail|status|prompt|action_label|button_label"
)
STRING = r'"((?:[^"\\\n]|\\.)*)"'
CALL = re.compile(rf'(?<![\w:.])(?:[A-Za-z_][\w]*::)*({CALLS})\s*\(\s*(?:&?format!\(\s*)?{STRING}', re.S)
METHOD = re.compile(rf'\.({CALLS})\s*\(\s*(?:&?format!\(\s*)?{STRING}', re.S)
FIELD = re.compile(
    rf'(?<![\w.])({FIELDS})\s*:\s*(?:Some\(\s*)?(?:String::from\(\s*|format!\(\s*)?{STRING}'
    r'\s*(\.(?:into|to_owned|to_string)\(\)|\)|,|\n\s*[}\]])',
    re.S,
)
# `tooltip(content, "Text", position)`: the text is the second argument.
TOOLTIP = re.compile(
    rf'(?<![\w.])(?:[A-Za-z_]\w*::)*(tooltip)\s*\((?:[^,()]|\([^()]*\))*,\s*(?:&?format!\(\s*)?{STRING}', re.S)
FL = re.compile(r'\bfl!\s*\(')
# Any literal, for the broader `--prose` sweep.
LITERAL = re.compile(STRING)
# Log lines, panics and internal errors are not translated.
INTERNAL = re.compile(
    r'(?:\b(?:trace|debug|info|warn|error|eprintln|println|eprint|print|panic|unreachable|todo|'
    r'assert|assert_eq|assert_ne|debug_assert|bail|ensure|anyhow|format_err|write|writeln)!\s*\(\s*'
    r'|\.(?:expect|context|with_context|wrap_err)\s*\(\s*(?:\|\|\s*)?(?:format!\(\s*)?'
    r'|#\[(?:error|doc|serde|arg|command|clap|value)\s*\(\s*(?:[\w]+\s*=\s*)?)$'
)
WORD = re.compile(r"[A-Za-z]{2,}")
IDENTIFIER = re.compile(r"^[\w.:/@+#%{}$<>=-]*$")


def user_visible(literal):
    """Whether a literal reads as words people see, not an ID, icon or format."""
    text = re.sub(r"\{[^{}]*\}", "", literal).strip()
    if not WORD.search(text):
        return False
    if " " not in text:
        # One word: user-visible only when it is capitalized prose ("Cancel"),
        # not an identifier, key name, icon, MIME type or format string.
        if IDENTIFIER.match(text) and not re.match(r"^[A-Z][a-z]+[.!?…:]?$", text):
            return False
        if text.endswith("-symbolic") or "%" in text:
            return False
        if re.match(r"^[A-Z][a-z]+$", text) and text in KEY_NAMES:
            return False
        return bool(re.match(r"^[A-Z][a-z]", text))
    if text.startswith(("org.", "com.", "net.", "/", "--", "http", "file:")):
        return False
    return bool(re.search(r"[a-z]", text))


# Key names in shortcut descriptions are XKB keysyms, not prose.
KEY_NAMES = {"Return", "Escape", "Tab", "Delete", "Home", "End", "Left", "Right",
             "Up", "Down", "Space", "Backspace", "Insert", "Menu", "Print"}


def strip_tests(source):
    """Drop `#[cfg(test)]` items, line comments and doc comments."""
    lines = source.splitlines()
    kept = []
    index = 0
    while index < len(lines):
        line = lines[index]
        stripped = line.strip()
        if stripped.startswith("#[cfg(test)]") or stripped.startswith("#[cfg(all(test"):
            # Skip the attribute and the following item, by brace depth.
            index += 1
            depth = 0
            opened = False
            while index < len(lines):
                code = re.sub(STRING, '""', lines[index])
                code = re.sub(r"'(?:[^'\\]|\\.)'", "''", code)
                code = code.split("//", 1)[0]
                depth += code.count("{") - code.count("}")
                opened = opened or "{" in code
                index += 1
                if (opened and depth <= 0) or (not opened and code.rstrip().endswith(";")):
                    break
            continue
        if stripped.startswith("//"):
            kept.append("")
        else:
            kept.append(line)
        index += 1
    return "\n".join(kept)


def test_file(path):
    parts = set(path.parts)
    name = path.name
    return (
        "tests" in parts or "benches" in parts or "examples" in parts
        or name in {"tests.rs", "testing.rs", "harness.rs", "test_support.rs", "fixtures.rs"}
        or name.endswith("_tests.rs") or name.startswith("test_")
    )


def load_allowlist(path):
    entries = {}
    if not path.exists():
        return entries
    for number, raw in enumerate(path.read_text().splitlines(), 1):
        line = raw.split("  #", 1)[0].strip()
        if not line or line.startswith("#"):
            continue
        if "  #" not in raw:
            raise SystemExit(f"{path}:{number}: every exception needs a `  # reason`")
        file, _, literal = line.partition(": ")
        literal = literal.strip()
        if literal != "*":
            if not (literal.startswith('"') and literal.endswith('"')):
                raise SystemExit(f"{path}:{number}: expected `path: \"literal\"` or `path: *`")
            literal = literal[1:-1]
        entries.setdefault(file.strip(), set()).add(literal)
    return entries


def prose(root, crates, allowlist):
    """Capitalized, multi-word literals outside logs, panics and tests."""
    found = []
    for crate in crates:
        source_root = root / "crates" / crate / "src"
        if not source_root.is_dir():
            continue
        for path in sorted(source_root.rglob("*.rs")):
            relative = path.relative_to(root)
            if test_file(relative):
                continue
            allowed = set().union(*(
                literals for pattern, literals in allowlist.items()
                if fnmatch.fnmatchcase(relative.as_posix(), pattern)
            ))
            if "*" in allowed:
                continue
            code = strip_tests(path.read_text(encoding="utf-8", errors="replace"))
            for match in LITERAL.finditer(code):
                literal = match.group(1)
                if literal in allowed or not re.match(r"^[A-Z][a-z’']*\b", literal) or " " not in literal:
                    continue
                if not user_visible(literal):
                    continue
                before = code[max(0, match.start() - 80):match.start()]
                if INTERNAL.search(before):
                    continue
                line = code.count("\n", 0, match.start()) + 1
                found.append((crate, relative.as_posix(), line, "literal", literal))
    return found


def findings(root, crates, allowlist):
    found = []
    lookups = Counter()
    for crate in crates:
        source_root = root / "crates" / crate / "src"
        if not source_root.is_dir():
            continue
        for path in sorted(source_root.rglob("*.rs")):
            relative = path.relative_to(root)
            if test_file(relative):
                continue
            allowed = set().union(*(
                literals for pattern, literals in allowlist.items()
                if fnmatch.fnmatchcase(relative.as_posix(), pattern)
            ))
            if "*" in allowed:
                continue
            source = path.read_text(encoding="utf-8", errors="replace")
            code = strip_tests(source)
            lookups[crate] += len(FL.findall(code))
            seen = set()
            for pattern in (CALL, METHOD, FIELD, TOOLTIP):
                for match in pattern.finditer(code):
                    literal = match.group(2)
                    if not user_visible(literal) or literal in allowed:
                        continue
                    line = code.count("\n", 0, match.start(2)) + 1
                    if (line, literal) in seen:
                        continue
                    seen.add((line, literal))
                    found.append((crate, relative.as_posix(), line, match.group(1), literal))
    found.sort(key=lambda item: (item[1], item[2]))
    return found, lookups


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--allowlist", type=Path)
    parser.add_argument("--crate", action="append", help="limit to these crates")
    parser.add_argument("--report", action="store_true", help="print coverage per crate and succeed")
    parser.add_argument("--prose", action="store_true",
                        help="list every capitalized multi-word literal (a broader, noisier sweep) and succeed")
    args = parser.parse_args(argv)
    allowlist = load_allowlist(args.allowlist or args.root / "scripts" / "i18n-allowlist.txt")
    crates = args.crate or UI_CRATES
    found, lookups = findings(args.root, crates, allowlist)
    if args.prose:
        for _, file, line, _, literal in prose(args.root, crates, allowlist):
            print(f'{file}:{line}: "{literal}"')
        return 0
    if args.report:
        remaining = Counter(item[0] for item in found)
        broad = Counter(item[0] for item in prose(args.root, crates, allowlist))
        print(f"{'crate':20} {'fl!':>6} {'lint hits':>9} {'prose left':>10} {'converted':>9}")
        for crate in crates:
            total = lookups[crate] + broad[crate]
            share = f"{100 * lookups[crate] / total:.0f}%" if total else "-"
            print(f"{crate:20} {lookups[crate]:>6} {remaining[crate]:>9} {broad[crate]:>10} {share:>9}")
        print("converted = fl! lookups / (fl! lookups + prose literals left); prose is a noisy upper bound.")
        return 0
    for _, file, line, sink, literal in found:
        print(f'{file}:{line}: "{literal}" passed to {sink}; move it to i18n/en/<crate>.ftl '
              f"and use fl!, or allowlist it in scripts/i18n-allowlist.txt with a reason")
    if found:
        print(f"{len(found)} user-visible literal(s) outside Fluent.", file=sys.stderr)
        return 1
    print("No user-visible literals outside Fluent in UI code.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except BrokenPipeError:
        sys.exit(1)
