#!/usr/bin/env python3
"""Verify focused-window icons update without unrelated application events.

Uses a private software-rendered Weston/Niri desktop and a controlled Niri IPC
stream. Checks fresh PNG and SVG icons, cached switches, both rotation directions,
and horizontal rendering. Requires Weston, Niri, D-Bus and Pillow.
"""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import socket
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

COLORS = {"Red": (255, 0, 0), "Blue": (0, 0, 255), "Green": (0, 255, 0)}
EDGES = ("left", "right", "top")


def capture(session, name, scale):
    session.screenshot(name)
    with Image.open(session.output / name) as screenshot:
        image = screenshot.convert("RGB")
    width, height = image.size
    inset, thickness = round(3 * scale), round(48 * scale)
    boxes = {
        "left": (inset, thickness + inset, thickness - inset, height - inset),
        "right": (width - thickness + inset, thickness + inset, width - inset, height - inset),
        "top": (thickness + inset, inset, width - thickness - inset, thickness - inset),
    }
    result = {}
    for edge, box in boxes.items():
        data = image.crop(box).tobytes()
        pixels = list(zip(data[0::3], data[1::3], data[2::3]))
        # Only white/gray title ink contributes; icon colors are saturated.
        ink = bytes(255 if min(pixel) > 100 and max(pixel) - min(pixel) < 8 else 0
                    for pixel in pixels)
        result[edge] = {
            "icons": {app: pixels.count(color) for app, color in COLORS.items()},
            "title_pixels": ink.count(255),
            "title_sha256": hashlib.sha256(ink).hexdigest(),
        }
    return result


def window(index, name):
    return {
        "id": index, "title": name + " focused window", "app_id": "org.canopa." + name,
        "pid": None, "workspace_id": 1, "is_focused": False, "is_floating": False,
        "is_urgent": False, "focus_timestamp": None,
        "layout": {"pos_in_scrolling_layout": None, "tile_size": [0, 0],
                   "window_size": [0, 0], "tile_pos_in_workspace_view": None,
                   "window_offset_in_tile": [0, 0]},
    }


def verify(binary, output, session, scale):
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
    session.run(["niri", "msg", "output", output_name, "scale", str(scale)])
    applications = runtime / "data/applications"
    applications.mkdir(parents=True)
    for name, color in COLORS.items():
        icon = runtime / (name + (".svg" if name == "Green" else ".png"))
        if icon.suffix == ".svg":
            icon.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32">'
                            '<path fill="#00ff00" d="M0 0h32v32H0z"/></svg>')
        else:
            Image.new("RGB", (32, 32), color).save(icon)
        (applications / f"org.canopa.{name}.desktop").write_text(
            f"[Desktop Entry]\nType=Application\nName={name}\nExec=true\nIcon={icon}\n"
        )
    config = runtime / "config/niribar/config.kdl"
    config.parent.mkdir(parents=True)
    config.write_text("".join(f'''bar "{edge}" {{
    edge "{edge}"
    collapsed-width 48
    style size=16 color="#ffffff"
    modules {{ center "focused-window"; }}
}}
''' for edge in EDGES) + 'modules { module "focused-window" {}; }\n')

    # No clock, GTK clients, or real compositor events may accidentally refresh
    # the bar after the renderer finishes loading a previously unseen image.
    with socket.socket(socket.AF_UNIX) as listener:
        fixture_socket = str(runtime / "fixture.sock")
        listener.bind(fixture_socket)
        listener.listen(1)
        listener.settimeout(15)
        real_socket = session.env["NIRI_SOCKET"]
        session.env["NIRI_SOCKET"] = fixture_socket
        try:
            session.start([str(binary)], "niribar.log")
        finally:
            session.env["NIRI_SOCKET"] = real_socket
        connection, _ = listener.accept()
        with connection:
            connection.settimeout(5)
            with connection.makefile("rb") as request:
                assert json.loads(request.readline()) == "EventStream"

            def send(event):
                connection.sendall((json.dumps(event) + "\n").encode())

            def focus(index):
                send({"WorkspacesChanged": {"workspaces": [{
                    "id": 1, "idx": 1, "name": None, "output": output_name,
                    "is_urgent": False, "is_active": True, "is_focused": True,
                    "active_window_id": index,
                }]}})

            send({"Ok": "Handled"})
            session.wait_for_layers(len(EDGES))
            names = ["Warmup", *COLORS]
            send({"WindowsChanged": {"windows": [window(i, name) for i, name in enumerate(names)]}})
            focus(0)
            attempts = 0

            def warmup():
                nonlocal attempts
                attempts += 1
                rows = capture(session, f"warmup-{attempts}.png", scale)
                return rows if all(row["title_pixels"] > 100 for row in rows.values()) else None

            previous = session.wait_for(warmup, "initial title", timeout=30)
            time.sleep(0.5)
            results = []
            for cycle, name in enumerate(("Red", "Blue", "Green", "Red", "Blue")):
                started = time.monotonic()
                focus(names.index(name))
                frame = 0
                passed = False
                while time.monotonic() - started < 3:
                    rows = capture(session, f"focus-{cycle}-{frame}.png", scale)
                    frame += 1
                    passed = all(
                        row["icons"][name] > 100
                        and all(count == 0 for app, count in row["icons"].items() if app != name)
                        and row["title_pixels"] > 100
                        and row["title_sha256"] != previous[edge]["title_sha256"]
                        for edge, row in rows.items()
                    )
                    if passed:
                        break
                    time.sleep(0.05)
                result = {"app": name, "scale": scale, "passed": passed,
                          "observed_seconds": round(time.monotonic() - started, 3), "bars": rows}
                results.append(result)
                desktop.write_json(output / "result.json", results)
                print(json.dumps(result), flush=True)
                assert passed, f"{name} icon/title did not update while the bar was idle"
                previous = rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scale", type=float, choices=[1.0, 1.25, 1.5, 2.0], default=1.25)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="canopa-focused-icons-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(binary, output, session, args.scale)
        finally:
            session.close()


if __name__ == "__main__":
    main()
