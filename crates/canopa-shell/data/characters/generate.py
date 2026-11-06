"""Regenerate the offline catalogs from Emoji 17.0 emoji-test.txt and CLDR 48 en.xml."""

import argparse
from pathlib import Path
import xml.etree.ElementTree as ET


def write_catalog(path, entries):
    with path.open("w", encoding="utf-8", newline="\n") as output:
        for glyph, name, keywords in entries:
            assert all(field == " ".join(field.split()) for field in (name, keywords))
            fields = (glyph, name, keywords) if keywords else (glyph, name)
            output.write("\t".join(fields) + "\n")


def generate(emoji_test, english_annotations):
    names, keywords = {}, {}
    for annotation in ET.parse(english_annotations).iter("annotation"):
        if annotation.get("alt") is not None:
            continue
        table = names if annotation.get("type") == "tts" else keywords
        table[annotation.attrib["cp"]] = " ".join(annotation.text.split())

    emoji, emoji_codepoints = [], set()
    for line in emoji_test.read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        codepoints, status = line.split("#", 1)[0].split(";")
        glyph = "".join(chr(int(code, 16)) for code in codepoints.split())
        status = status.strip()
        if status in ("fully-qualified", "component") and len(glyph) == 1:
            emoji_codepoints.add(glyph)
        if status == "fully-qualified":
            name = line.split("#", 1)[1].split(maxsplit=2)[2]
            key = glyph.replace("\ufe0f", "")
            # Reuse the base aliases for sequences that differ only in skin tone.
            base = key.translate(dict.fromkeys(range(0x1F3FB, 0x1F400)))
            emoji.append((glyph, name, keywords.get(key, keywords.get(base, ""))))

    symbols = {
        glyph: (name, keywords.get(glyph, ""))
        for glyph, name in names.items()
        if len(glyph) == 1 and glyph not in emoji_codepoints
    }
    symbols.update({
        "✓": ("check mark", "tick | done"),
        "✗": ("ballot x", "cross mark | x"),
        "⌘": ("command key", "command | cmd"),
        "⌥": ("option key", "option | alt"),
        "⌫": ("backspace", "delete | keyboard"),
        "⏎": ("return key", "enter | newline | keyboard"),
        "½": ("one half", "fraction | 1/2"),
        "¼": ("one quarter", "fraction | 1/4"),
        "¾": ("three quarters", "fraction | 3/4"),
        "Δ": ("capital delta", "greek letter | change"),
        "Ω": ("capital omega", "greek letter | ohm"),
        "Σ": ("capital sigma", "greek letter | sum"),
    })
    for glyph, name in zip(
        "αβγδεζηθικλμνξοπρστυφχψω",
        "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi "
        "omicron pi rho sigma tau upsilon phi chi psi omega".split(),
        strict=True,
    ):
        symbols.setdefault(glyph, (name, "greek letter"))

    output = Path(__file__).resolve().parent
    write_catalog(output / "emoji.tsv", emoji)
    common = "—–…•✓×÷±≈≠≤≥∞°π←↑→↓€£¥©®™"
    order = dict.fromkeys([*common, *symbols])
    write_catalog(output / "symbols.tsv", ((glyph, *symbols[glyph]) for glyph in order))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("emoji_test", type=Path)
    parser.add_argument("english_annotations", type=Path)
    args = parser.parse_args()
    generate(args.emoji_test, args.english_annotations)
