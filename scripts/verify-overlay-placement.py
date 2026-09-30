#!/usr/bin/env python3
"""Check panel placement with two independent bars in a private Niri session.

Requires Weston, Niri, wtype, and Pillow. Never connects to the live desktop.
"""

import argparse
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import time

from PIL import Image, ImageChops

sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location(
    "desktop", Path(__file__).with_name("validate-desktop.py")
)
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def verify(binary, output, session, edge, bar_count, scale):
    niri_config = session.runtime / "niri.kdl"
    niri_config.write_text(
        'prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
        'hotkey-overlay { skip-at-startup; }\n'
    )
    # No service directories: test clients must not auto-activate desktop
    # services (or FUSE mounts) inherited from the user's installation.
    bus_config = session.runtime / "bus.conf"
    bus_config.write_text(f'''<busconfig>
<type>session</type><listen>unix:tmpdir={session.runtime}</listen>
<policy context="default"><allow send_destination="*"/><allow receive_sender="*"/>
<allow own="*"/></policy></busconfig>''')
    session.start(["dbus-daemon", f"--config-file={bus_config}", "--nofork", "--print-address=1"], "dbus.log")
    session.env["DBUS_SESSION_BUS_ADDRESS"] = session.wait_for(
        lambda: (output / "dbus.log").read_text().strip(), "private bus"
    ).splitlines()[0]
    session.start([
        "weston", "--backend=headless", "--renderer=pixman", "--fake-seat",
        "--width=1280", "--height=800", "--socket=wayland-parent", "--no-config",
        "--idle-time=0",
    ], "weston.log")
    session.wait_for(lambda: (session.runtime / "wayland-parent").is_socket(), "Weston")
    session.env["WAYLAND_DISPLAY"] = "wayland-parent"
    session.start(["niri", "-c", str(niri_config)], "niri.log")
    session.env["NIRI_SOCKET"] = str(session.wait_for(
        lambda: list(session.runtime.glob("niri.*.sock")), "Niri IPC"
    )[0])
    display = session.wait_for(
        lambda: [p for p in session.runtime.glob("wayland-*")
                 if p.is_socket() and p.name != "wayland-parent"], "Niri socket"
    )[0]
    session.env["WAYLAND_DISPLAY"] = str(display)
    outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
    output_name = next(iter(outputs))
    session.run(["niri", "msg", "output", output_name, "scale", str(scale)])
    session.wait_for(
        lambda: json.loads(session.run(["niri", "msg", "--json", "outputs"]))
        [output_name]["logical"]["scale"] == scale, "output scale",
    )
    compositor_env = session.env.copy()
    bars = []
    thicknesses = [38, 54][:bar_count]
    reserved = round(sum(thicknesses) * scale)
    # A fractional origin and extent can round independently when rasterized
    # (e.g. x=852.5 and width=312.5 become 853 and 313 physical pixels).
    rounding_tolerance = 0 if scale.is_integer() else 1
    for index, thickness in enumerate(thicknesses):
        runtime = session.runtime / f"bar-{index}"
        runtime.mkdir(mode=0o700)
        config = runtime / "config/niribar/config.kdl"
        config.parent.mkdir(parents=True)
        config.write_text(f'''bar "test" {{
    edge "{edge}"
    collapsed-width {thickness}
    modules {{ start "button"; }}
    panel {{ width 250; height 180; start "button"; }}
}}
modules {{
    module "button" {{
        label "Bar {index + 1}"
        icon "B"
        actions {{ click {{ toggle-panel; }} }}
    }}
}}
''')
        session.env = compositor_env | {
            "XDG_RUNTIME_DIR": str(runtime), "XDG_CONFIG_HOME": str(runtime / "config"),
        }
        session.start([str(binary)], f"bar-{index}.log")
        session.wait_for_layers(index + 1)
        session.wait_for(lambda: (runtime / "canopa-shell.sock").is_socket(), "bar IPC")
        bars.append(session.env.copy())
    session.env = compositor_env
    # Layer registration precedes the first GPU frame. Wait for both bars to
    # be visible so the before/after difference contains only the panel.
    def bars_visible():
        session.screenshot("settling.png")
        frame = Image.open(output / "settling.png").convert("RGB")
        points = {
            "top": [(640, 10), (640, reserved - 10)],
            "bottom": [(640, 790), (640, 810 - reserved)],
            "left": [(10, 400), (reserved - 10, 400)],
            "right": [(1270, 400), (1290 - reserved, 400)],
        }[edge]
        return all(frame.getpixel(point) == (30, 30, 46) for point in points)
    session.wait_for(bars_visible, "both bar frames")
    results = []
    for index, environment in enumerate(bars):
        session.env = environment
        session.screenshot(f"bar-{index}-closed.png")
        session.run([str(binary), "--toggle-panel"])
        session.wait_for_layers(bar_count + 1)
        time.sleep(1)
        session.screenshot(f"bar-{index}-open.png")
        before = Image.open(output / f"bar-{index}-closed.png").convert("RGB")
        after = Image.open(output / f"bar-{index}-open.png").convert("RGB")
        bounds = ImageChops.difference(before, after).getbbox()
        if bounds is None:
            raise AssertionError("Panel produced no visible change")
        clearance = {
            "top": bounds[1], "bottom": after.height - bounds[3],
            "left": bounds[0], "right": after.width - bounds[2],
        }[edge]
        results.append({"bar": index, "scale": scale, "bounds": bounds, "clearance": clearance,
                        "required_clearance": reserved, "rounding_tolerance_px": rounding_tolerance,
                        "passed": abs(clearance - reserved) <= rounding_tolerance})
        session.run(["wtype", "-k", "Escape"])
        session.wait_for_layers(bar_count)
        time.sleep(0.2)
    desktop.write_json(output / "result.json", results)
    print(json.dumps(results, indent=2), flush=True)
    assert all(result["passed"] for result in results), "Panel is not adjacent to the reserved bar area"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--edge", nargs="+", choices=["top", "bottom", "left", "right"], default=["top"])
    parser.add_argument("--bars", type=int, choices=[1, 2], default=2)
    parser.add_argument("--scale", type=float, choices=[1.0, 1.25, 1.5, 2.0], default=1.0)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    for edge in args.edge:
        case_output = output / edge
        case_output.mkdir()
        with tempfile.TemporaryDirectory(prefix="canopa-shell-placement-") as directory:
            session = desktop.Session(Path(directory), case_output)
            try:
                verify(binary, case_output, session, edge, args.bars, args.scale)
            finally:
                session.close()


if __name__ == "__main__":
    main()
