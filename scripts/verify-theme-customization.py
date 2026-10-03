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


def verify(binary, session):
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
                       'hotkey-overlay { skip-at-startup; }\nlayout { gaps 0; }\n')
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

    color = (85, 34, 51)
    checks = []
    for edge in ["top", "bottom", "left", "right"]:
        for align in ["start", "center", "end"]:
            save(config(edge, align))
            span = 1280 if edge in ("top", "bottom") else 800
            start = {"start": 12, "center": (span-400)//2, "end": span-412}[align]
            x, y = ((start+35, 32 if edge == "top" else 768) if edge in ("top", "bottom")
                    else (32 if edge == "left" else 1248, start+35))
            name = f"{edge}-{align}.png"
            session.wait_for(lambda: session.capture(name).getpixel((x, y)) == color, name)
            image = session.capture(name)
            assert image.getpixel((3, 3)) != color, "outer screen margin was painted"
            checks.append(f"{edge} {align}")

    save(config())
    mark = len(log())
    replacement = theme.with_suffix(".new")
    replacement.write_text('palette { background "#224466"; accent "#33cc99"; }\n'
                           'typography { family "monospace"; body-size 20; label-size 18; title-size 24; }\n')
    replacement.replace(theme)
    session.wait_for(lambda: "configuration reloaded" in log()[mark:], "theme-only reload")
    session.wait_for(lambda: session.capture("theme-reloaded.png").getpixel((470, 32)) == (34, 68, 102), "theme repaint")
    mark = len(log())
    theme.write_text('ui { bar { padding -1; }; }\n')
    session.wait_for(lambda: "reload rejected" in log()[mark:], "invalid theme rejection")
    assert session.capture("invalid-keeps-theme.png").getpixel((470, 32)) == (34, 68, 102)
    theme.write_text('palette { background "#552233"; }\n')
    session.wait_for(lambda: session.capture("recovered.png").getpixel((470, 32)) == color, "theme recovery")

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
    for x, y in [(100, 32), (441, 13)]:
        session.run([str(pointer), str(round(x*1600/1280)), str(round(y*1000/800))])
        session.wait_for(clicked.exists, "transparent bar input pass-through")
        clicked.unlink()
    session.run([str(pointer), "800", "40"])
    session.wait_for_layers(2)
    session.capture("short-bar-panel.png")
    assert shell.poll() is None
    checks += ["theme-only atomic replacement", "invalid theme retained", "theme recovery",
               "transparent host and rounded corner click-through", "short bar panel activation"]
    desktop.write_json(session.output / "result.json", {"passed": True, "checks": checks})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/canopa-shell")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="canopa-theme-desktop-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(args.binary.resolve(strict=True), session)
        finally:
            session.close()
    print(f"Theme desktop checks passed: {output}")


if __name__ == "__main__":
    main()
