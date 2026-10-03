#!/usr/bin/env python3
"""Check theme reload, short-bar geometry and click-through in a private desktop."""
import argparse
import importlib.util
import json
from pathlib import Path
import tempfile
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def config(edge="top", align="center", reserve=True, length="400"):
    return f'''schema-version 1
appearance {{ theme "theme.kdl"; }}
shell {{
    bar "main" {{
        edge "{edge}"
        thickness 40
        length {length}
        align "{align}"
        margin 12
        reserve-space #{str(reserve).lower()}
        ui {{ radius 10; border width=2 color="accent"; padding 4; }}
        modules {{ center "button"; }}
    }}
    modules {{ module "button" {{ label "Theme"; icon "B"; actions {{ click {{ toggle-panel; }} }}; }}; }}
}}
'''


def verify(binary, session, scale):
    output_name = session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
                       'hotkey-overlay { skip-at-startup; }\nlayout { gaps 0; }\n',
                       width=round(1280*scale), height=round(800*scale), scale=scale)
    logical = json.loads(session.run(["niri", "msg", "--json", "outputs"]))[output_name]["logical"]
    width, height = logical["width"], logical["height"]
    session.env["GDK_BACKEND"] = "wayland"
    session.start(["wtype", "-s", "600000"], "keyboard.log")
    main = session.runtime / "config/canopa/config.kdl"
    main.parent.mkdir(parents=True)
    theme = main.with_name("theme.kdl")
    theme.write_text('palette { background "#552233"; accent "#33cc99"; }\n')
    main.write_text(config())
    session.env["RUST_LOG"] = "canopa_shell::configuration=info,canopa_shell::surface=debug"
    shell = session.start([str(binary)], "shell.log")
    session.wait_for_layers(1)
    log = lambda: (session.output / "shell.log").read_text()

    def save(text):
        mark = len(log())
        main.write_text(text)
        session.wait_for(lambda: "configuration reloaded" in log()[mark:], "configuration reload")

    def pixel(image, point):
        return image.getpixel(tuple(round(v*scale) for v in point))

    color = (85, 34, 51)
    checks = []
    for edge in ["top", "bottom", "left", "right"]:
        for align in ["start", "center", "end"]:
            save(config(edge, align))
            span = width if edge in ("top", "bottom") else height
            start = {"start": 12, "center": (span-400)//2, "end": span-412}[align]
            x, y = ((start+35, 32 if edge == "top" else height-32) if edge in ("top", "bottom")
                    else (32 if edge == "left" else width-32, start+35))
            name = f"{edge}-{align}.png"
            session.wait_for(lambda: pixel(session.capture(name), (x, y)) == color, name)
            image = session.capture(name)
            assert pixel(image, (3, 3)) != color, "outer screen margin was painted"
            checks.append(f"{edge} {align}")

    save(config())
    mark = len(log())
    replacement = theme.with_suffix(".new")
    replacement.write_text('palette { background "#224466"; accent "#33cc99"; }\n'
                           'typography { family "monospace"; body-size 20; label-size 18; title-size 24; }\n')
    replacement.replace(theme)
    session.wait_for(lambda: "configuration reloaded" in log()[mark:], "theme-only reload")
    theme_sample = (width/2-170, 32)
    session.wait_for(lambda: pixel(session.capture("theme-reloaded.png"), theme_sample) == (34, 68, 102), "theme repaint")
    mark = len(log())
    theme.write_text('ui { bar { padding -1; }; }\n')
    session.wait_for(lambda: "reload rejected" in log()[mark:], "invalid theme rejection")
    assert pixel(session.capture("invalid-keeps-theme.png"), theme_sample) == (34, 68, 102)
    theme.write_text('palette { background "#552233"; }\n')
    session.wait_for(lambda: pixel(session.capture("recovered.png"), theme_sample) == color, "theme recovery")

    # A maximized client covers the unreserved transparent host and rounded corner.
    save(config(reserve=False))
    client = session.runtime / "client.py"
    client.write_text('''import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk
from pathlib import Path
window=Gtk.Window(title="Theme input fixture")
area=Gtk.EventBox()
area.connect("button-press-event",lambda *args: Path(__file__).with_name("clicked").touch())
window.add(area)
window.show_all()
Gtk.main()
''')
    session.start(["python3", str(client)], "client.log")
    session.wait_for(lambda: json.loads(session.run(["niri", "msg", "--json", "windows"])), "client")
    session.run(["niri", "msg", "action", "maximize-column"])
    pointer = session.build_pointer()
    clicked = session.runtime / "clicked"
    for x, y in [(100, 32), (width/2-199, 13)]:
        session.run([str(pointer), str(round(x*1600/width)), str(round(y*1000/height))])
        session.wait_for(clicked.exists, "transparent bar input pass-through")
        clicked.unlink()
    session.run([str(pointer), "800", str(round(32*1000/height))])
    session.wait_for_layers(2)
    session.wait_for(lambda: pixel(session.capture("short-bar-panel.png"), (width/2, 100)) == color,
                     "panel painted beyond the short bar")
    session.run([str(binary), "--toggle-panel"])
    session.wait_for_layers(1)
    layout=lambda: json.loads(session.run(["niri", "msg", "--json", "windows"]))[0]["layout"]
    unreserved=layout()
    save(config())
    reserved=session.wait_for(lambda: (current if (current:=layout()) != unreserved else None), "window space reservation")
    for align in ["start", "end", "center"]:
        save(config(align=align))
        assert layout()==reserved, "alignment changed edge-wide reservation"
    save(config(length='"50%"'))
    percentage_start = width/2-(width-24)/4
    session.wait_for(lambda: pixel(session.capture("percentage.png"),(percentage_start+20,32))==color, "percentage bar length")
    assert pixel(session.capture("percentage.png"),(percentage_start-20,32))!=color
    save(config(length="32768"))
    session.wait_for(lambda: pixel(session.capture("clamped.png"),(width-40,32))==color, "clamped fixed length")
    desktop.write_json(session.output / "window-reservation.json", {"unreserved":unreserved,"reserved":reserved})
    checks += ["alignment-independent window reservation", "percentage length", "oversized fixed length fitting"]
    assert shell.poll() is None
    checks += ["theme-only atomic replacement", "invalid theme retained", "theme recovery",
               "transparent host and rounded corner click-through", "short bar panel activation"]
    desktop.write_json(session.output / "result.json", {"passed": True, "checks": checks})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/canopa-shell")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scale", type=float, default=1.0)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="canopa-theme-desktop-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(args.binary.resolve(strict=True), session, args.scale)
        finally:
            session.close()
    print(f"Theme desktop checks passed: {output}")


if __name__ == "__main__":
    main()
