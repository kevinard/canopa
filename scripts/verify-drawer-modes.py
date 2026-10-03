#!/usr/bin/env python3
"""Verify drawer placement, bar input and unchanged clients in a private Niri session.

Requires Weston, Niri, wtype, Pillow, GTK 3 Python bindings, cc and wayland-scanner.
Never connects to the live desktop. Keeps screenshots and results in --output.
"""

import argparse
import importlib.util
import json
import subprocess
from pathlib import Path
import sys
import tempfile
import time

from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def verify(binary, output, session, scale):
    runtime = session.runtime
    session.start_niri(
        'prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
        'hotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; '
        'focus-ring { off; }; default-column-width { proportion 1.0; }; }\n', scale=scale,
    )
    session.start(["wtype", "-s", "600000"], "keyboard.log")
    click = session.build_pointer()
    client = runtime / "client.py"
    client.write_text('''import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, GLib
from pathlib import Path
GLib.set_prgname("org.canopa.DrawerFixture")
window = Gtk.Window(title="Drawer fixture")
area = Gtk.EventBox()
area.connect("button-press-event", lambda *args: Path(__file__).with_name("clicked").touch())
window.add(area)
window.show_all()
Gtk.main()
''')
    client_process = session.start(["python3", str(client)], "client.log")
    session.wait_for(lambda: json.loads(session.run(["niri", "msg", "--json", "windows"])), "client")
    config = runtime / "config/niribar/config.kdl"
    config.parent.mkdir(parents=True)
    results = []

    def capture(name):
        (output / name).unlink(missing_ok=True)
        session.screenshot(name)
        return Image.open(output / name).convert("RGB")

    def layout():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))[0]["layout"]

    for edge in ["left", "right"]:
        for mode in ["overlay", "push-bar"]:
            case = f"{edge}-{mode}"
            config.write_text(f'''bar "top" {{ edge "top"; collapsed-width 32; }}
bar "main" {{
 edge "{edge}"
 collapsed-width 48
 modules {{ start "button"; }}
 panel {{ presentation "drawer"; drawer-mode "{mode}"; width 260; height 220; start "button"; }}
}}
modules {{ module "button" {{ label "Open"; icon "B"; actions {{ click {{ toggle-panel; }} }} }} }}
''')
            process = session.start([str(binary)], f"{case}.log")
            session.wait_for_layers(2)
            session.wait_for(lambda: (runtime / "canopa-shell.sock").is_socket(), "bar IPC")
            time.sleep(1)
            def bars_ready():
                frame = capture(f"{case}-ready.png")
                x = round(24 * scale) if edge == "left" else frame.width - round(24 * scale)
                return frame.getpixel((x, round(300 * scale))) == (16, 25, 29)
            session.wait_for(bars_ready, "bar frame")
            before = capture(f"{case}-closed.png")
            client_layout = layout()
            # Activate the side bar by pointer, then use its displaced icon to close.
            x = 24 * scale if edge == "left" else before.width - 24 * scale
            y = 48 * scale

            def pointer(px, py):
                session.run([str(click), str(round(px * 1600 / before.width)),
                             str(round(py * 1000 / before.height))])

            pointer(x, y)
            session.wait_for_layers(3)
            time.sleep(.5)
            opened = capture(f"{case}-open.png")
            assert layout() == client_layout, f"{case}: application geometry changed"
            displacement = 260 * scale if mode == "push-bar" else 0
            moved_x = x + displacement if edge == "left" else x - displacement
            # Compare an unhovered section of the bar; its pixels move intact.
            def strip(cx):
                return (round(cx - 20 * scale), round(300 * scale),
                        round(cx + 20 * scale), round(400 * scale))
            assert not ImageChops.difference(before.crop(strip(x)), opened.crop(strip(moved_x))).getbbox(), case
            pointer(moved_x, y)
            session.wait_for_layers(2)
            time.sleep(.3)
            assert layout() == client_layout, f"{case}: application geometry changed on close"
            restored = capture(f"{case}-restored.png")
            assert not ImageChops.difference(before.crop(strip(x)), restored.crop(strip(x))).getbbox(), case
            pointer(x, y)
            session.wait_for_layers(3)
            # Empty panel chrome is inside: it must not dismiss the drawer.
            inside_x = 120 * scale if edge == "left" else before.width - 120 * scale
            pointer(inside_x, 180 * scale)
            session.wait_for_layers(3)
            # A capped drawer leaves space below it. The first outside click
            # dismisses only; the following click can reach the application.
            pointer(inside_x, 450 * scale)
            session.wait_for_layers(2)
            assert not (runtime / "clicked").exists(), f"{case}: dismissal activated client"
            pointer(inside_x, 450 * scale)
            session.wait_for(lambda: (runtime / "clicked").exists(), "input restored after dismissal")
            (runtime / "clicked").unlink()
            pointer(x, y)
            session.wait_for_layers(3)
            session.run(["wtype", "-k", "Escape"])
            session.wait_for_layers(2)
            results.append({"edge": edge, "mode": mode, "scale": scale,
                            "client_layout_unchanged": True, "bar_click_and_escape": True,
                            "outside_click_dismissal": True,
                            "inside_click_preserved": True,
                            "dismissal_click_consumed": True})
            session.retire(process)
            session.wait_for_layers(0)
            print(f"PASS: {case} at {scale}x", flush=True)
    # Empty desktop clicks do not necessarily change keyboard focus. Exercise
    # them separately, including bars stacked on the Overlay layer and menus
    # opened over IPC without a preceding pointer grab.
    session.retire(client_process)
    for edge in ["left", "right", "top", "bottom"]:
        config.write_text(f'''bar "main" {{
 edge "{edge}"
 layer "overlay"
 collapsed-width 48
 modules {{ start "button"; }}
 panel {{ presentation "auto"; width 260; height 220; start "button"; }}
}}
modules {{ module "button" {{ label "Open"; icon "B"; actions {{ click {{ toggle-panel; }} }} }} }}
''')
        case = f"{edge}-empty-desktop"
        process = session.start([str(binary)], f"{case}.log")
        session.wait_for_layers(1)
        session.wait_for(lambda: (runtime / "canopa-shell.sock").is_socket(), "bar IPC")
        time.sleep(.5)
        for button in ["left", "right"]:
            session.run([str(binary), "--toggle-panel"])
            session.wait_for_layers(2)
            time.sleep(.3)
            session.run([str(click), "800", "500", button])
            session.wait_for_layers(1)
        # An outside click on an overlay-layer bar must only dismiss, even if
        # the module underneath would toggle or replace the panel.
        session.run([str(binary), "--toggle-panel"])
        session.wait_for_layers(2)
        time.sleep(.3)
        frame = capture(f"{case}-open.png")
        inset = 24 * scale
        px, py = {"left": (inset, inset), "right": (frame.width - inset, inset),
                  "top": (inset, inset), "bottom": (inset, frame.height - inset)}[edge]
        session.run([str(click), str(round(px * 1600 / frame.width)),
                     str(round(py * 1000 / frame.height))])
        session.wait_for_layers(1)
        menu = subprocess.Popen([str(binary), "--dmenu"], env=session.env,
                                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, text=True)
        try:
            menu.stdin.write("First\nSecond\n")
            menu.stdin.close()
            session.wait_for_layers(2)
            time.sleep(.3)
            session.run([str(click), "800", "500"])
            session.wait_for_layers(1)
            menu.wait(timeout=2)
            assert menu.stdout.read() == "", f"{case}: dismissal selected a menu entry"
        finally:
            if menu.poll() is None:
                menu.kill()
            menu.wait()
        results.append({"edge": edge, "empty_desktop_dismissal": True,
                        "secondary_click_dismissal": True, "overlay_bar_dismissal": True,
                        "dmenu_cancellation": True})
        session.retire(process)
        session.wait_for_layers(0)
        print(f"PASS: {case} at {scale}x", flush=True)
    desktop.write_json(output / "result.json", results)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/canopa-shell")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scale", type=float, default=1.0)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="canopa-drawer-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(args.binary.resolve(), output, session, args.scale)
        finally:
            session.close()


if __name__ == "__main__":
    main()
