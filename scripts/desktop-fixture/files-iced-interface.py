"""Editing state must survive tiling and configuration changes in real Iced widgets."""
import json
import shlex
import sys
import time
from PIL import Image


def setup(session, fixture, config):
    (fixture / "caret parent/chosen-right").mkdir(parents=True)
    theme = session.runtime / "data/icons/hicolor"
    icons = theme / "18x18/actions"
    icons.mkdir(parents=True)
    (theme / "index.theme").write_text('[Icon Theme]\nName=Private Files icons\nDirectories=18x18/actions\n[18x18/actions]\nSize=18\nType=Fixed\n')
    Image.new("RGBA", (18, 18), (223, 53, 176, 255)).save(icons / "canopa-fixture.png")
    (icons / "canopa-fixture-symbolic.svg").write_text('<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18"><path fill="currentColor" d="M0 0h18v18H0z"/></svg>')
    helper = session.runtime / "interface-action.py"
    helper.write_text('import json, sys\nfrom pathlib import Path\n' +
        f'Path({str(session.runtime / "interface-action.json")!r}).write_text(json.dumps(sys.argv[1:]))\n')
    command = f'{shlex.quote(sys.executable)} {shlex.quote(str(helper))} %F'
    config.write_text(config.read_text() + 'files { action "inspect" name="Inspect" icon="canopa-fixture" toolbar=#true shortcut="<Control><Alt>u" command=' + json.dumps(command) + '; }\n')


def verify(session, fixture, browser, config, scheme, scale):
    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def wait_title(name):
        result = session.wait_for(lambda: title(name), name)
        time.sleep(.3)
        return result

    def key(name, count=1):
        session.run(["wtype", *[arg for _ in range(count) for arg in ("-k", name)]])
        time.sleep(.15)

    def chord(name, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", name]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.2)

    def select_suffix(count):
        session.run(["wtype", "-M", "shift", *[arg for _ in range(count) for arg in ("-k", "Left")], "-m", "shift"])
        time.sleep(.2)

    def width(value):
        session.run(["niri", "msg", "action", "set-window-width", str(value)])
        session.wait_for(lambda: next((w for w in windows() if w["is_focused"]), {})
                         .get("layout", {}).get("window_size", [None])[0] == value, "resized native window")
        time.sleep(.3)

    def magenta(frame, area):
        return [(x, y) for y in range(round(area[1]*scale), round(area[3]*scale))
                for x in range(round(area[0]*scale), round(area[2]*scale))
                if max(abs(a-b) for a,b in zip(frame.getpixel((x,y))[:3], (223,53,176))) <= 3]

    def raster_painted():
        frame = session.capture("interface-00-raster-toolbar.png")
        return len(magenta(frame, (0,35,350,165))) >= round(150*scale*scale)
    session.run(["wtype", "B-notes"])
    session.wait_for(raster_painted, "prepared raster action icon")

    initial_width = browser["layout"]["window_size"][0]
    chord("l", "ctrl")
    session.run(["wtype", str(fixture / "caret parent/wrong")])
    key("End")
    select_suffix(5)
    session.capture("interface-01-wide-location-selection.png")
    width(360)
    session.run(["wtype", "right"])
    session.capture("interface-02-narrow-location-replaced.png")
    width(initial_width)
    key("Left", 5)
    session.run(["wtype", "chosen-"])
    key("Return")
    wait_title("chosen-right")
    session.capture("interface-03-location-caret-survived.png")

    chord("l", "ctrl")
    session.run(["wtype", str(fixture)])
    key("Return")
    wait_title("Downloads")
    chord("a", "ctrl", "shift")
    wait_title("Custom Actions")
    key("Tab", 4)
    chord("a", "ctrl")
    session.run(["wtype", "draft part"])
    chord("a", "ctrl")
    chord("c", "ctrl")
    assert session.run(["wl-paste", "--no-newline"]) == "draft part"
    key("End")
    select_suffix(4)
    before = session.capture("interface-04-editor-selection.png")
    config.write_text(config.read_text() + "// external editor keeps its own change\n")
    region = (0, round(110 * scale), round(600 * scale), round(165 * scale))
    def notice_painted():
        frame = session.capture("interface-05-external-notice.png")
        return frame.crop(region).tobytes() != before.crop(region).tobytes()
    session.wait_for(notice_painted, "external configuration notice")
    session.run(["wtype", "kept"])
    chord("a", "ctrl")
    chord("c", "ctrl")
    actual = session.run(["wl-paste", "--no-newline"])
    assert actual == "draft kept", f"draft field lost native focus/selection on reload: {actual!r}"
    chord("s", "ctrl")
    time.sleep(.5)
    saved = config.read_text()
    assert 'name="Inspect"' in saved and "draft kept" not in saved
    assert saved.endswith("// external editor keeps its own change\n")
    session.capture("interface-06-stale-save-keeps-draft.png")
    chord("w", "ctrl")
    key("Tab")  # Keep editing -> explicit Discard.
    key("Return")
    session.wait_for(lambda: title("Custom Actions") is None, "explicit discard closes editor")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    width(360)
    session.run(["wtype", "B-notes"])
    time.sleep(.3)
    key("Menu")
    def menu_icon_painted():
        frame = session.capture("interface-07-menu-icon.png")
        return len(magenta(frame, (0,175,360,320))) >= round(150*scale*scale)
    session.wait_for(menu_icon_painted, "context menu action icon")
    key("Escape")
    before = session.capture("interface-07b-narrow-icon.png")
    points = magenta(before, (0,35,350,165))
    assert points
    bounds = (min(x for x,y in points)+2, min(y for x,y in points)+2,
              max(x for x,y in points)-1, max(y for x,y in points)-1)
    config.write_text(config.read_text().replace('"canopa-fixture"', '"canopa-fixture-symbolic"'))
    foreground = (237,244,240) if scheme == "dark" else (32,52,44)
    def foreground_count(frame):
        return sum(max(abs(a-b) for a,b in zip(pixel[:3], foreground)) <= 3
                   for pixel in frame.crop(bounds).getdata())
    area = (bounds[2]-bounds[0]) * (bounds[3]-bounds[1])
    def symbolic_painted():
        frame = session.capture("interface-08-symbolic-toolbar.png")
        return foreground_count(frame) > area * .9
    session.wait_for(symbolic_painted, "live SVG symbolic icon in the shared foreground")
    config.write_text(config.read_text().replace('"canopa-fixture-symbolic"', '"missing-canopa-fixture"'))
    def missing_falls_back():
        frame = session.capture("interface-09-missing-icon-label.png")
        return foreground_count(frame) < area * .5
    session.wait_for(missing_falls_back, "missing icon retains a text action")
    chord("u", "ctrl", "alt")
    result = session.runtime / "interface-action.json"
    session.wait_for(lambda: result.exists() and json.loads(result.read_text()) == [str(fixture / "B-notes.txt")], "missing icon does not prevent action invocation")
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "checks": ["wide-to-narrow location selection survives", "narrow-to-wide location caret survives and navigates the edited path", "external configuration notice preserves editor focus/selection and all draft text", "stale save preserves the external source and draft", "explicit keyboard discard closes the changed draft", "prepared raster toolbar icon", "prepared context menu icon", "live SVG symbolic icon follows shared foreground", "missing icon keeps the label and executable action"]}
