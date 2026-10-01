#!/usr/bin/env python3
"""Check rotated glyph quality in a private software-rendered Niri session.

Requires Weston, Niri, dbus-daemon, Pillow, and Python GI with GTK 3.
The live desktop is never contacted. Screenshots and results are retained.
"""

import argparse
import hashlib
import importlib.util
import itertools
import json
from pathlib import Path
import sys
import tempfile
import time

from PIL import Image

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "desktop", Path(__file__).with_name("validate-desktop.py")
)
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def capture_title_pixels(session, output, scale, name, *, thickness=48, require_icon=True):
    session.screenshot(name)
    with Image.open(output / name) as screenshot:
        image = screenshot.convert("RGB")
    width, height = image.size
    inset, thickness = round(3 * scale), round(thickness * scale)
    boxes = {
        "left": (inset, thickness + inset, thickness - inset, height - inset),
        "right": (width - thickness + inset, thickness + inset, width - inset, height - inset),
        "top": (thickness + inset, inset, width - thickness - inset, thickness - inset),
    }
    results = []
    for edge, box in boxes.items():
        crop = image.crop(box)
        colors = crop.getcolors(width * height)
        ink = Image.new("L", crop.size)
        ink.putdata([255 if min(color) > 128 and max(color) - min(color) < 12 else 0
                     for color in crop.getdata()])
        ink_bounds = ink.getbbox()
        icon_pixels = sum(count for count, color in colors if color == (255, 0, 0))
        # Exclude the red icon (including its edges) from the text-quality test.
        colors = [(count, color) for count, color in colors
                  if color[0] <= max(color[1], color[2]) + 8]
        # Native glyph rasterization provides grayscale coverage. Merely
        # smoothing outline meshes with MSAA yields only five levels and
        # still makes small rotated letters look uneven.
        color_count = len(colors)
        ink_pixels = sum(count for count, _ in colors) - max(count for count, _ in colors)
        results.append({
            "edge": edge, "scale": scale, "colors": color_count,
            "ink_pixels": ink_pixels, "icon_pixels": icon_pixels,
            "passed": color_count >= 32 and ink_pixels > 100 and (not require_icon or icon_pixels > 100),
            "ink_bounds": ink_bounds,
            "pixels_sha256": hashlib.sha256(image.crop(box).tobytes()).hexdigest(),
        })
    return results


def verify(binary, output, session, scales):
    runtime = session.runtime
    session.env["ICED_BACKEND"] = "wgpu"
    output_name = session.start_niri(
        "prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n"
        "hotkey-overlay { skip-at-startup; }\n"
    )
    icon = runtime / "application.svg"
    icon.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16">'
                    '<path fill="#ff0000" d="M0 0h16v16H0z"/></svg>')
    applications = runtime / "data/applications"
    applications.mkdir(parents=True)
    (applications / "org.canopa.RotationFixture.desktop").write_text(
        f"[Desktop Entry]\nType=Application\nName=Rotation Fixture\nExec=true\nIcon={icon}\n"
    )
    config = runtime / "config/niribar/config.kdl"
    config.parent.mkdir(parents=True)
    config.write_text("".join(f'''bar "{edge}" {{
    edge "{edge}"
    collapsed-width 48
    style size=16 color="#ffffff"
    modules {{ center "focused-window"; }}
}}
''' for edge in ("left", "right", "top")) + '''modules {
    module "focused-window" {}
}
''')
    bar = session.start([str(binary)], "canopa-shell.log")
    session.wait_for_layers(3)
    titles = ["Focused window: Quarterly planning - Firefox", "Updated title: iii WWW — Résumé"]
    title_file = runtime / "title"
    title_file.write_text(titles[0])
    client = runtime / "client.py"
    client.write_text('''import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, GLib
from pathlib import Path
title = Path(__file__).with_name("title")
GLib.set_prgname("org.canopa.RotationFixture")
window = Gtk.Window(title=title.read_text())
window.set_default_size(600, 400)
window.show_all()
def update_title():
    current = title.read_text()
    if current != window.get_title():
        window.set_title(current)
    return True
GLib.timeout_add(50, update_title)
Gtk.main()
''')
    session.env["NO_AT_BRIDGE"] = "1"
    session.env["GTK_USE_PORTAL"] = "0"
    session.start(["/usr/bin/python3", str(client)], "client.log")
    session.wait_for(
        lambda: json.loads(session.run(["niri", "msg", "--json", "windows"])),
        "focused fixture window",
    )
    results = []
    for scale in scales:
        session.run(["niri", "msg", "output", output_name, "scale", str(scale)])
        session.wait_for(
            lambda: json.loads(session.run(["niri", "msg", "--json", "outputs"]))
            [output_name]["logical"]["scale"] == scale, "output scale",
        )
        previous = None
        for title_index, title in enumerate(titles):
            pending = runtime / "next-title"
            pending.write_text(title)
            pending.replace(title_file)
            session.wait_for(
                lambda: any(window["title"] == title for window in json.loads(
                    session.run(["niri", "msg", "--json", "windows"]))), "updated window title",
            )
            # Wait for actual ink and title replacement, independently of
            # smoothing, to accommodate initial shader compilation.
            attempts = itertools.count()

            def populated():
                rows = capture_title_pixels(
                    session, output, scale, f"settling-{scale}-{title_index}-{next(attempts)}.png",
                )
                ready = all(row["ink_pixels"] > 100 and
                            (previous is None or row["pixels_sha256"] != previous[row["edge"]])
                            for row in rows)
                return rows if ready else None

            session.wait_for(populated, "populated focused-window text", timeout=30)
            for frame in range(2):
                rows = capture_title_pixels(
                    session, output, scale, f"rotation-{scale}-{title_index}-{frame}.png",
                )
                results.extend(row | {"frame": frame, "title_index": title_index} for row in rows)
                previous = {row["edge"]: row["pixels_sha256"] for row in rows}
                time.sleep(0.2)
    # Exercise an ordinary Module through the shared presentation boundary as
    # well as the rich focused-window image path. Restart to apply each config.
    desktop.stop(bar)
    session.processes.remove(bar)
    session.wait_for_layers(0)
    for rotation in ("auto", "clockwise"):
        config.write_text("".join(f'''bar "{edge}" {{
    edge "{edge}"
    collapsed-width 96
    style size=16 color="#ffffff"
    modules {{ center "idle-inhibitor"; }}
}}
''' for edge in ("left", "right", "top")) + f'''modules {{
    module "idle-inhibitor" {{
        content-rotation "{rotation}"
        format "Ordinary module rendering: iii WWW - Resume"
    }}
}}
''')
        bar = session.start([str(binary)], f"ordinary-{rotation}.log")
        session.wait_for_layers(3)
        for scale in scales:
            session.run(["niri", "msg", "output", output_name, "scale", str(scale)])
            session.wait_for(
                lambda: json.loads(session.run(["niri", "msg", "--json", "outputs"]))
                [output_name]["logical"]["scale"] == scale, "ordinary output scale",
            )
            attempts = itertools.count()

            def populated_ordinary():
                rows = capture_title_pixels(session, output, scale,
                    f"ordinary-{rotation}-{scale}-{next(attempts)}.png",
                    thickness=96, require_icon=False)
                for result in rows:
                    bounds = result["ink_bounds"]
                    if not bounds:
                        return None
                    ink_width, ink_height = bounds[2] - bounds[0], bounds[3] - bounds[1]
                    vertical = rotation == "clockwise" or result["edge"] != "top"
                    result["orientation_passed"] = (ink_height > ink_width if vertical
                                                     else ink_width > ink_height)
                    result["passed"] = result["passed"] and result["orientation_passed"]
                    result["module"] = "idle-inhibitor"
                    result["rotation"] = rotation
                return rows if all(result["passed"] for result in rows) else None

            results.extend(session.wait_for(populated_ordinary, "ordinary rotated glyphs", timeout=30))
        desktop.stop(bar)
        session.processes.remove(bar)
        session.wait_for_layers(0)
    desktop.write_json(output / "result.json", results)
    print(json.dumps(results, indent=2), flush=True)
    assert all(result["passed"] for result in results), "Missing, misoriented or aliased module text"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scale", nargs="+", type=float, choices=[1.0, 1.25, 1.5, 2.0],
                        default=[1.0, 1.25])
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="canopa-content-rotation-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(binary, output, session, args.scale)
        finally:
            session.close()


if __name__ == "__main__":
    main()
