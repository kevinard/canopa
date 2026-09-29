#!/usr/bin/env python3
"""Check panel transition frames and idle CPU in a private Weston/Niri desktop.

Uses deliberately slow transitions to sample actual native frames, not FPS.
Requires validate-desktop.py's dependencies, grim and Pillow. Never uses the live
desktop. Both presentations, every edge, disabled motion and rapid reopen are
covered. --scale also exercises physical capture size on scaled outputs.
--default-timings samples normal-speed entry and exit; compositor screenshots may
skip intermediate frames at that speed, so slowed cases assert their presence.
"""
import argparse
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from PIL import Image, ImageChops, ImageStat

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def verify(binary, output, session, scale, only, default_timings=False):
    runtime = session.runtime
    bus = runtime / "bus.conf"
    bus.write_text(f'<busconfig><type>session</type><listen>unix:tmpdir={runtime}</listen>'
                   '<policy context="default"><allow send_destination="*"/>'
                   '<allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
    session.start(["dbus-daemon", f"--config-file={bus}", "--nofork", "--print-address=1"], "bus.log")
    session.env["DBUS_SESSION_BUS_ADDRESS"] = session.wait_for(
        lambda: (output / "bus.log").read_text().strip(), "private bus").splitlines()[0]
    niri = runtime / "niri.kdl"
    niri.write_text('animations { off; }\nxwayland-satellite { off; }\n'
                    'hotkey-overlay { skip-at-startup; }\n')
    session.start(["weston", "--backend=headless", "--renderer=pixman", "--fake-seat",
                   "--width=1280", "--height=800", "--socket=wayland-parent", "--no-config",
                   "--idle-time=0"], "weston.log")
    session.wait_for(lambda: (runtime / "wayland-parent").is_socket(), "Weston")
    session.env["WAYLAND_DISPLAY"] = "wayland-parent"
    session.start(["niri", "-c", str(niri)], "niri.log")
    session.env["NIRI_SOCKET"] = str(session.wait_for(
        lambda: list(runtime.glob("niri.*.sock")), "Niri IPC")[0])
    session.env["WAYLAND_DISPLAY"] = str(session.wait_for(
        lambda: [p for p in runtime.glob("wayland-*")
                 if p.is_socket() and p.name != "wayland-parent"], "Niri display")[0])
    name = next(iter(json.loads(session.run(["niri", "msg", "--json", "outputs"]))))
    session.run(["niri", "msg", "output", name, "scale", str(scale)])
    session.start(["wtype", "-s", "600000"], "keyboard.log")
    config = runtime / "config/niribar/config.kdl"
    config.parent.mkdir(parents=True)
    results = []

    def capture(name):
        raw = subprocess.check_output(["grim", "-t", "ppm", "-"], env=session.env,
                                      stderr=subprocess.PIPE, timeout=5)
        frame = Image.open(io.BytesIO(raw)).convert("RGB")
        frame.save(output / f"{name}.png")
        return frame

    def difference(a, b):
        return sum(ImageStat.Stat(ImageChops.difference(a, b)).mean)

    cases = [(edge, form, 1000) for edge in ["left", "right", "top", "bottom"]
             for form in (["auto", "drawer"] if edge in ["left", "right"] else ["auto"])]
    cases.append(("left", "drawer", 0))
    for edge, form, duration in cases:
        case = f"{edge}-{form}-{duration}"
        if only and only != case:
            continue
        animation = ("" if default_timings and duration else
                     f"animation {{ open-ms {duration}; close-ms {duration}; }}")
        config.write_text(f'''bar "main" {{
 edge "{edge}"
 collapsed-width 48
 modules {{ start "button"; }}
 panel {{ presentation "{form}"; width 360; height 300; start "button";
         {animation} }}
}}
modules {{ module "button" {{ label "Panel motion"; icon "B"; actions {{ click {{ toggle-panel; }} }} }} }}
''')
        process = session.start([str(binary)], f"{case}.log")
        session.wait_for_layers(1)
        session.wait_for(lambda: (runtime / "niribar.sock").is_socket(), "bar IPC")
        time.sleep(1)
        # Warm the software renderer before sampling an already-running shell.
        session.run([str(binary), "--toggle-panel"])
        session.wait_for_layers(2)
        time.sleep(1.2)
        session.run(["wtype", "-k", "Escape"])
        session.wait_for_layers(1)
        def bar_ready():
            frame = capture(f"{case}-ready")
            inset = round(24 * scale)
            point = {"left": (inset, frame.height // 2),
                     "right": (frame.width - inset, frame.height // 2),
                     "top": (frame.width // 2, inset),
                     "bottom": (frame.width // 2, frame.height - inset)}[edge]
            return frame.getpixel(point) == (30, 30, 46)
        session.wait_for(bar_ready, "first bar frame")
        closed = capture(f"{case}-closed")
        session.run([str(binary), "--toggle-panel"])
        session.wait_for_layers(2)
        entry_frames = []
        started = time.monotonic()
        while time.monotonic() - started < 1.4:
            frame = capture(f"{case}-entering-{len(entry_frames)}")
            entry_frames.append((time.monotonic() - started, frame))
            time.sleep(.04)
        opened = capture(f"{case}-open")
        assert difference(opened, closed) > 1, f"{case}: panel is blank"
        samples = [(elapsed, difference(frame, closed)) for elapsed, frame in entry_frames]
        if duration and not default_timings:
            intermediate = [frame for _, frame in entry_frames
                            if .1 < difference(frame, closed) < difference(opened, closed) - .05]
            assert intermediate, f"{case}: no intermediate entry frame"
            entering = intermediate[0]
        elif not duration:
            entering = entry_frames[-1][1]
            assert difference(entering, opened) < .01, f"{case}: disabled entry changed"
        else:
            entering = entry_frames[0][1]
        idle = desktop.measure(process.pid, 2)
        assert idle["cpu_percent_one_core"] < 5, f"{case}: animation remained busy at rest"
        session.run(["wtype", "-k", "Escape"])
        if duration and not default_timings:
            time.sleep(.2)
            assert len(session.layers()) == 2, f"{case}: exit removed too early"
            exiting = capture(f"{case}-exiting")
            assert difference(exiting, opened) > .05, f"{case}: no intermediate exit frame"
            assert .1 < difference(exiting, closed) < difference(opened, closed) - .05, f"{case}: exit did not fade"
            # Reopen before the old exit's completion, then let that completion arrive.
            session.run([str(binary), "--toggle-panel"])
            time.sleep(1.3)
            assert len(session.layers()) == 2, f"{case}: stale close removed new panel"
            reopened = capture(f"{case}-reopened")
            assert difference(reopened, opened) < .01, f"{case}: reopening changed content"
            session.run(["wtype", "-k", "Escape"])
        # Record the whole exit, including its last visible frames. A single
        # early screenshot misses an abrupt fade at the end of the transition.
        exit_frames = []
        started = time.monotonic()
        while time.monotonic() - started < (1.2 if duration and not default_timings else .4):
            frame = capture(f"{case}-exiting-{len(exit_frames)}")
            exit_frames.append((time.monotonic() - started, frame))
        exit_samples = [(elapsed, difference(frame, closed)) for elapsed, frame in exit_frames]
        session.wait_for_layers(1)
        time.sleep(.1)
        final = capture(f"{case}-final")
        assert difference(final, closed) < .01, f"{case}: closing left pixels behind"
        if case == "left-auto-1000":
            # An IPC client must receive cancellation before the visual exit ends.
            client = subprocess.Popen([str(binary), "--dmenu"], env=session.env,
                                      stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                      stderr=subprocess.PIPE, text=True)
            try:
                client.stdin.write("First\nSecond\n")
                client.stdin.close()
                session.wait_for_layers(2)
                time.sleep(1.2)
                session.run(["wtype", "-k", "Escape"])
                client.wait(timeout=.5)
                if not default_timings:
                    assert len(session.layers()) == 2, "IPC cancellation waited for the exit"
                session.wait_for_layers(1)
            finally:
                if client.poll() is None:
                    client.kill()
                client.wait()
        results.append({"edge": edge, "presentation": form,
                        "duration_ms": None if default_timings and duration else duration,
                        "scale": scale, "idle_cpu_percent": idle["cpu_percent_one_core"],
                        "default_timings": default_timings,
                        "entry_samples_seconds_and_difference": samples,
                        "exit_samples_seconds_and_difference": exit_samples,
                        "entry_difference": difference(entering, opened)})
        session.retire(process)
        session.wait_for_layers(0)
        print(f"PASS: {case} at {scale}x", flush=True)
    desktop.write_json(output / "result.json", results)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/niribar")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scale", type=float, default=1.0)
    parser.add_argument("--case", help="Run one named case, such as left-auto-1000")
    parser.add_argument("--default-timings", action="store_true",
                        help="Sample configured defaults instead of slowed transitions")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="canopa-motion-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(args.binary.resolve(), output, session, args.scale, args.case,
                   args.default_timings)
        finally:
            session.close()


if __name__ == "__main__":
    main()
