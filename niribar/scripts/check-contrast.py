#!/usr/bin/env python3
"""Check documented default theme pairs using WCAG relative luminance."""

import argparse
from pathlib import Path
import re


def luminance(rgb):
    channels = [value / 255 for value in rgb]
    linear = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in channels]
    return sum(weight * value for weight, value in zip((0.2126, 0.7152, 0.0722), linear))


def ratio(foreground, background):
    light, dark = sorted((luminance(foreground), luminance(background)), reverse=True)
    return (light + 0.05) / (dark + 0.05)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--markdown", action="store_true", help="print the measured documentation table")
    args = parser.parse_args()
    source = (Path(__file__).resolve().parents[1] / "src/theme.rs").read_text()
    colors = {
        name: tuple(int(channel, 16) for channel in (r, g, b))
        for name, r, g, b in re.findall(
            r"pub const (\w+): Color = Color::from_rgb8\(0x([\da-f]+), 0x([\da-f]+), 0x([\da-f]+)\);",
            source,
        )
    }
    pairs = [
        (fg, bg, 4.5)
        for fg in ("TEXT", "SUBTEXT", "ACCENT", "CRITICAL", "WARNING", "SUCCESS")
        for bg in ("BASE", "SURFACE0")
    ] + [("TEXT", "SURFACE1", 4.5), ("BASE", "ACCENT", 4.5), ("BASE", "CRITICAL", 4.5)]
    pairs += [("ACCENT", bg, 3.0) for bg in ("BASE", "SURFACE0", "SURFACE1")]
    pairs += [("SUBTEXT", bg, 3.0) for bg in ("BASE", "SURFACE0", "SURFACE1")]
    if args.markdown:
        print("| Foreground | Background | Ratio | Required | Result |")
        print("| --- | --- | ---: | ---: | --- |")
    failed = False
    for fg, bg, minimum in pairs:
        value = ratio(colors[fg], colors[bg])
        passed = value >= minimum  # Compare before rounding for display.
        failed |= not passed
        if args.markdown:
            print(f"| {fg} | {bg} | {value:.2f}:1 | {minimum:g}:1 | {'Pass' if passed else 'FAIL'} |")
        elif not passed:
            print(f"FAIL: {fg} on {bg}: {value:.4f}:1 < {minimum:g}:1")
    if failed:
        parser.exit(1, "Default theme contrast check failed.\n")
    if not args.markdown:
        print(f"All {len(pairs)} documented default theme contrast pairs pass.")


if __name__ == "__main__":
    main()
