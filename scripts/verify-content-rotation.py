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


def capture_title_pixels(session, output, scale, name):
    session.screenshot(name)
    with Image.open(output / name) as screenshot:
        image = screenshot.convert("RGB")
    width, height = image.size
    inset, thickness = round(3 * scale), round(48 * scale)
    boxes = {
        "left": (inset, thickness + inset, thickness - inset, height - inset),
        "right": (width - thickness + inset, thickness + inset, width - inset, height - inset),
        "top": (thickness + inset, inset, width - thickness - inset, thickness - inset),
    }
    results = []
    for edge, box in boxes.items():
        colors = image.crop(box).getcolors(width * height)
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
            "passed": color_count >= 32 and ink_pixels > 100 and icon_pixels > 100,
            "pixels_sha256": hashlib.sha256(image.crop(box).tobytes()).hexdigest(),
        })
    return results


def verify(binary, output, session, scales):
    runtime = session.runtime
    session.env["ICED_BACKEND"] = "wgpu"
    bus = runtime / "bus.conf"
    # No inherited service activation in this disposable session.
    bus.write_text(f'''<busconfig>
<type>session</type><listen>unix:tmpdir={runtime}</listen>
<policy context="default"><allow send_destination="*"/><allow receive_sender="*"/>
<allow own="*"/></policy></busconfig>''')
    session.start([
        "dbus-daemon", f"--config-file={bus}", "--nofork", "--print-address=1",
    ], "dbus.log")
    session.env["DBUS_SESSION_BUS_ADDRESS"] = session.wait_for(
        lambda: (output / "dbus.log").read_text().strip(), "private bus",
    ).splitlines()[0]
    niri_config = runtime / "niri.kdl"
    niri_config.write_text(
        "prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n"
        "hotkey-overlay { skip-at-startup; }\n"
    )
    session.start([
        "weston", "--backend=headless", "--renderer=pixman", "--fake-seat",
        "--width=1280", "--height=800", "--socket=wayland-parent", "--no-config",
        "--idle-time=0",
    ], "weston.log")
    session.wait_for(lambda: (runtime / "wayland-parent").is_socket(), "Weston")
    session.env["WAYLAND_DISPLAY"] = "wayland-parent"
    session.start(["niri", "-c", str(niri_config)], "niri.log")
    session.env["NIRI_SOCKET"] = str(session.wait_for(
        lambda: list(runtime.glob("niri.*.sock")), "Niri IPC",
    )[0])
    session.env["WAYLAND_DISPLAY"] = str(session.wait_for(
        lambda: [p for p in runtime.glob("wayland-*")
                 if p.is_socket() and p.name != "wayland-parent"], "Niri display",
    )[0])
    output_name = next(iter(json.loads(session.run(["niri", "msg", "--json", "outputs"]))))
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
    session.start([str(binary)], "niribar.log")
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
    desktop.write_json(output / "result.json", results)
    print(json.dumps(results, indent=2), flush=True)
    assert all(result["passed"] for result in results), "Missing or aliased focused-window text"


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
