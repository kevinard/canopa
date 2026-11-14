"""Real Iced global preference drafts, saves, stale sources and raw bookmarks."""
import json
import os
from pathlib import Path
import re
import shlex
import sys
import time
from urllib.parse import quote_from_bytes


def personal_locations(session):
    fixture_home = os.fsencode(session.runtime / "home")
    locations = [
        ("Desktop", fixture_home, "proof-desktop.txt"),
        ("Documents", os.fsencode(session.runtime / 'Documents "ailleurs" $budget'), "proof-documents.txt"),
        ("Downloads", fixture_home + "/Téléchargements #1".encode(), "proof-downloads.txt"),
        ("Pictures", fixture_home + b"/Images-\xff", "proof-pictures.txt"),
    ]
    private_root = os.fsencode(session.runtime)
    assert all(os.path.commonpath([private_root, folder]) == private_root for _, folder, _ in locations)
    return locations


def setup(session, fixture, config, case="full"):
    if case == "locations":
        assert Path(session.env["HOME"]) == session.runtime / "home"
        assert Path(session.env["XDG_CONFIG_HOME"]).is_relative_to(session.runtime)
        locations = personal_locations(session)
        for _, folder, name in locations:
            os.makedirs(folder, exist_ok=True)
            with open(folder + b"/" + name.encode(), "wb") as stream:
                stream.write(b"Actual personal-folder navigation.\n")
        def escaped(value):
            return b"".join((b"\\" if c in b'\\"$`' else b"") + bytes([c]) for c in value)
        definitions = b'XDG_DESKTOP_DIR="$HOME/"\n'
        definitions += b'XDG_DOCUMENTS_DIR="' + escaped(locations[1][1]) + b'"\n'
        definitions += b'XDG_DOWNLOAD_DIR="$HOME/' + escaped(os.path.basename(locations[2][1])) + b'"\n'
        definitions += b'XDG_PICTURES_DIR="' + escaped(locations[3][1]) + b'"\n'
        (Path(session.env["XDG_CONFIG_HOME"]) / "user-dirs.dirs").write_bytes(definitions)
    raw = os.fsencode(fixture) + b"/raw-\xff bookmark"
    os.mkdir(raw)
    with open(raw + b"/note.txt", "w") as stream:
        stream.write("Raw bookmark navigation.\n")
    helper = session.runtime / "preferences-action.py"
    helper.write_text('import json,os,sys\nfrom pathlib import Path\n' +
        f'Path({str(session.runtime / "preferences-action.json")!r}).write_text(json.dumps([os.fsencode(p).hex() for p in sys.argv[1:]]))\n')
    command = f'{shlex.quote(sys.executable)} {shlex.quote(str(helper))} %F'
    config.write_text(config.read_text() + 'notifications { }\nfiles { action "marker" name="Marker" shortcut="<Control><Alt>m" command=' + json.dumps(command) + '; }\n')


def verify(session, fixture, browser, config, scheme, scale, case="full", launched=None):
    checks = []
    raw = os.fsencode(fixture) + b"/raw-\xff bookmark"
    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))
    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)
    def wait_title(name):
        value = session.wait_for(lambda: title(name), name)
        time.sleep(.3)
        return value
    def key(name, count=1):
        session.run(["wtype", *[arg for _ in range(count) for arg in ("-k", name)]])
        time.sleep(.18)
    def chord(name, *mods):
        session.run(["wtype", *[arg for mod in mods for arg in ("-M", mod)], "-k", name,
                     *[arg for mod in reversed(mods) for arg in ("-m", mod)]])
        time.sleep(.25)
    def type_text(value):
        session.run(["wtype", value])
        time.sleep(.15)
    def replace(value):
        chord("a", "ctrl")
        type_text(value)
    def settle():
        # A lagging renderer can batch a page switch with the following Tabs,
        # routing them to the previous page. Wait for a stable painted frame.
        # A busy renderer also paints nothing, so also allow queued input time
        # to drain before requiring two identical frames well apart.
        time.sleep(1)
        previous = None
        def stable():
            nonlocal previous
            time.sleep(.4)
            frame = session.capture("preferences-settling.png").tobytes()
            same, previous = frame == previous, frame
            return same
        session.wait_for(stable, "settled Preferences frame")
    def page(n):
        chord(str(n), "alt")
        settle()
    def width(value):
        session.run(["niri", "msg", "action", "set-window-width", str(value)])
        session.wait_for(lambda: next(w for w in windows() if w["is_focused"])["layout"]["window_size"][0] == value, "native tile width")
        time.sleep(.3)
    def saved(predicate):
        session.wait_for(lambda: predicate(config.read_text()), "saved complete Files preferences", timeout=15 if case == "full" else 4)
        time.sleep(.35)
    def nodes(name):
        return [shlex.split(line.strip().rstrip(';')) for line in config.read_text().splitlines()
                if re.match(r'^\s*' + re.escape(name) + r'\s+', line)]
    def has_shortcut(binding):
        return ["shortcut", "search", binding] in nodes("shortcut")
    def toolbar():
        match = re.search(r'(?m)^\s*toolbar\s+([^\n]+)', config.read_text())
        assert match, config.read_text()
        return shlex.split(match.group(1).rstrip(";"))
    def focus_browser():
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "source browser focus")
        time.sleep(.25)
    def navigate(path):
        chord("l", "ctrl")
        type_text("file://" + quote_from_bytes(os.fsencode(path), safe="/"))
        key("Return")
        wait_title(os.path.basename(os.fsencode(path)).decode(errors="replace"))
    def shortcut(value, customized=False):
        # Four pages, the command filter, then the filtered Search row: its
        # label, Record, Reset (only once customized) and its shortcut field.
        page(4)
        key("Tab", 5)
        type_text("filenames")  # Only Search filenames matches.
        settle()
        key("Tab")
        key("Return")
        settle()
        # Selecting the command renews focus identities: traverse afresh.
        key("Tab", 9 if customized else 8)
        replace(value)

    if case == "locations":
        assert launched is not None
        pointer = session.build_pointer()
        outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
        logical = next(item["logical"] for item in outputs.values() if item.get("logical"))
        def click(x, y):
            session.run([str(pointer), str(round(x / logical["width"] * 1600)),
                         str(round(y / logical["height"] * 1000))])
            time.sleep(.25)
        def expect_file(folder, name):
            wait_title(os.path.basename(folder).decode(errors="replace"))
            type_text(name)
            launched.unlink(missing_ok=True)
            key("Return")
            def opened():
                try:
                    return [os.fsencode(p) for p in json.loads(launched.read_text())] == [folder + b"/" + name.encode()]
                except (FileNotFoundError, json.JSONDecodeError):
                    return False
            session.wait_for(opened, "actual GIO launch target in configured personal folder")
        locations = personal_locations(session)
        assert browser["layout"]["window_size"][0] >= 960
        for index, (label, folder, name) in enumerate(locations, 1):
            click(65, 140 + index * 40)
            expect_file(folder, name)
            session.capture(f"locations-wide-{label.lower()}.png")
            checks.append(f"wide sidebar {label} navigates the XDG path and opens its actual file")
        width(360)
        for index, (label, folder, name) in enumerate(locations, 2):
            key("Menu")
            key("End")
            key("Up")
            key("Return")
            key("Down", index)
            key("Return")
            expect_file(folder, name)
            session.capture(f"locations-narrow-{label.lower()}.png")
            checks.append(f"narrow Locations {label} uses the same byte-preserving path and actual file")
        return {"status":"passed", "scheme":scheme, "scale":scale, "case":case,
                "checks":checks, "scope":"XDG personal folders from a startup snapshot; full GUI migration remains required"}

    chord("comma", "ctrl")
    wait_title("Preferences")
    session.capture("preferences-01-views-wide.png")
    width(360)
    page(1)
    # Four pages, three views, four sort keys, two switches, narrower, width.
    key("Tab", 15)
    replace("12x")
    before = config.read_text()
    chord("s", "ctrl")
    time.sleep(.3)
    assert config.read_text() == before
    session.capture("preferences-02-invalid-width.png")
    replace("420")
    page(1)
    key("Tab", 6)
    key("Return")
    chord("s", "ctrl")
    saved(lambda s: any("mode=list" in n or 'mode="list"' in n for n in s.splitlines()) and 'column-width=420' in s)
    checks.append("360px Views form saves its actual uncommitted width and default List; invalid width leaves disk unchanged")

    # ADR0169: toolbar choices save as they change. Four pages, then the
    # default six items' enabled Earlier/Later/Remove controls (15), then Add.
    page(2)
    key("Tab", 20)
    settle()
    session.capture("preferences-02a-toolbar-add-focus.png")
    key("Return")  # Add opens the command picker and its filter.
    settle()
    session.capture("preferences-02b-toolbar-picker.png")
    # Opening the picker renews the form's focus identities: traverse afresh to
    # its filter, which follows Add.
    key("Tab", 21)
    type_text("Refresh")
    settle()
    session.capture("preferences-02c-toolbar-filtered.png")
    key("Tab")
    key("Return")
    saved(lambda s: "refresh" in toolbar())
    assert toolbar()[-1] == "refresh", toolbar()
    if case != "full":
        session.capture("preferences-toolbar-result.png")
        return {"status": "passed", "scheme": scheme, "scale": scale, "case": case,
                "checks": checks + ["actual Refresh addition through filtered keyboard traversal"],
                "scope": "reduced reproduction; not full preference acceptance"}
    page(2)
    key("Tab", 21)  # Seven rows' controls end with final Refresh's Earlier.
    key("Return")
    saved(lambda s: toolbar()[-2:] == ["refresh", "view"])
    checks.append("toolbar command search adds Refresh and Earlier changes its actual saved order")

    page(2)
    key("Tab", 23)  # Seven rows (18 controls), then Add.
    key("Return")
    settle()
    key("Tab", 24)  # A fresh traversal to the picker's filter after Add.
    type_text("Marker")
    settle()
    key("Tab")
    key("Return")
    saved(lambda s: "action:marker" in toolbar() and 'toolbar=#true' in s)
    session.capture("preferences-03-toolbar-action.png")
    checks.append("explicit custom-action toolbar placement enables the existing action and preserves its command")

    shortcut("<Control>l")
    before = config.read_text()
    chord("s", "ctrl")
    time.sleep(.3)
    assert config.read_text() == before
    session.capture("preferences-04-shortcut-conflict.png")
    settle()
    chord("Tab", "shift")  # Reset, enabled by the customized draft.
    chord("Tab", "shift")  # Record shortcut.
    key("Return")
    settle()
    key("F8")
    saved(lambda s: has_shortcut("F8"))
    checks.append("shortcut conflicts leave disk unchanged; actual keyboard recorder saves F8")
    shortcut("<Alt>f", customized=True)
    chord("s", "ctrl")
    saved(lambda s: has_shortcut("<Alt>f"))
    session.capture("preferences-05-shortcuts.png")
    chord("w", "ctrl")
    session.wait_for(lambda: title("Preferences") is None, "clean preference close")
    focus_browser()
    width(360)
    chord("f", "alt")
    type_text("B-notes")
    session.capture("preferences-06-remapped-search.png")
    key("Escape")
    checks.append("saved Search binding actually opens filename search in the narrow source browser")

    navigate(raw)
    chord("d", "ctrl")
    wait_title("Preferences")
    width(360)
    key("Tab", 43)  # Four pages, three sidebar choices, eleven places' controls (32), the bookmark's Earlier/Remove/label, name.
    replace("draft part")
    chord("a", "ctrl")
    chord("c", "ctrl")
    assert session.run(["wl-paste", "--no-newline"]) == "draft part"
    key("End")
    session.run(["wtype", "-M", "shift", *[arg for _ in range(4) for arg in ("-k", "Left")], "-m", "shift"])
    before = session.capture("preferences-07-bookmark-selection.png")
    config.write_text(config.read_text() + "// external preference writer\n")
    box = tuple(round(v * scale) for v in (0, 95, 360, 200))
    def notice_painted():
        current = session.capture("preferences-08-source-notice.png")
        return current.crop(box).tobytes() != before.crop(box).tobytes()
    session.wait_for(notice_painted, "external source notice")
    type_text("kept")
    chord("a", "ctrl")
    chord("c", "ctrl")
    actual = session.run(["wl-paste", "--no-newline"])
    assert actual == "draft kept", actual
    before_source = config.read_text()
    chord("s", "ctrl")
    time.sleep(.4)
    assert config.read_text() == before_source
    checks.append("source notice preserves bookmark caret/partial selection and stale Save preserves the external file")
    chord("r", "ctrl")
    key("Return")  # Keep editing is default.
    chord("a", "ctrl")
    chord("c", "ctrl")
    assert title("Preferences")
    chord("r", "ctrl")
    key("Tab")
    key("Return")  # Explicit Reload.
    time.sleep(.4)
    chord("w", "ctrl")
    session.wait_for(lambda: title("Preferences") is None, "explicit reload discards draft")
    checks.append("dirty Reload defaults to Keep editing and requires explicit discard")

    focus_browser()
    chord("d", "ctrl")
    wait_title("Preferences")
    key("Tab", 43)  # Four pages, three sidebar choices, eleven places' controls (32), the bookmark's Earlier/Remove/label, name.
    replace("Raw bookmark")
    chord("s", "ctrl")
    saved(lambda s: any(n[1] == "Raw bookmark" and "path-bytes=" + raw.hex() in n for n in nodes("bookmark")))
    assert "notifications" in config.read_text() and 'command=' in config.read_text()
    session.capture("preferences-09-raw-bookmark-saved.png")
    chord("w", "ctrl")
    session.wait_for(lambda: title("Preferences") is None, "saved bookmark close")
    focus_browser()
    navigate(fixture)
    # Locations is in the toolbar's application menu (ADR0171); the Menu key
    # opens the selection's own menu. Press the narrow window's More button.
    pointer = session.build_pointer()
    outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
    logical = next(item["logical"] for item in outputs.values() if item.get("logical"))
    current = next(w for w in windows() if w["id"] == browser["id"])
    session.run([str(pointer), str(round((current["layout"]["window_size"][0] - 25) / logical["width"] * 1600)),
                 str(round(64 / logical["height"] * 1000))])
    settle()
    key("End")
    key("Up", 6)  # Preferences, Custom Actions, Operations, Trash, hidden files, View options.
    key("Return")  # Locations in the same popup.
    settle()
    session.capture("preferences-10-locations-menu.png")
    key("Return")  # Back to original popup.
    settle()
    key("End")
    key("Up", 6)
    key("Return")
    settle()
    key("Home")
    key("Down", 12)  # Back, eleven built-in places, then the bookmark.
    key("Return")
    wait_title(os.path.basename(raw).decode(errors="replace"))
    type_text("note")
    chord("m", "ctrl", "alt")
    result = session.runtime / "preferences-action.json"
    expected = [(raw + b"/note.txt").hex()]
    session.wait_for(lambda: result.exists() and json.loads(result.read_text()) == expected, "actual action launches exact raw-bookmarked file")
    session.capture("preferences-11-raw-bookmark-browser.png")
    checks.append("raw bookmark persists as original Unix bytes and narrow Locations/Back navigates it before actual command execution")
    (session.output / "preferences-config.kdl").write_text(config.read_text())
    return {"status":"passed","scheme":scheme,"scale":scale,"checks":checks,
            "remaining":["full preference focus/accessibility/source-authority and toolbar/location matrix","remaining Iced operations/devices/media/native exchange and durable runtime"]}
