"""Real Iced folder views, shared sparse preferences and private-state failures."""
import json
import os
from pathlib import Path
import re
import shlex
import sys
import time
from urllib.parse import quote_from_bytes
from PIL import Image, ImageChops

DEFAULT_VIEW = 'view mode="columns" sort="name" descending=#false column-width=240 { details "size" "modified"; }'
UPDATED_VIEW = 'view mode="list" sort="modified" descending=#true column-width=360 { details "kind"; }'


def setup(session, fixture, config):
    root = fixture / "View fixtures"
    root.mkdir()
    (root / "A-child").mkdir()
    (root / "A-child/child.txt").write_text("A child folder view.\n")
    raw = os.fsencode(root) + b"/raw-\xff prefs"
    os.mkdir(raw)
    with open(raw + b"/note.txt", "wb") as stream:
        stream.write(b"Raw parent bytes remain unchanged.\n")
    for index in range(2000):
        path = root / f"item-{index:03}.txt"
        path.write_bytes(b"x" * (54321 if index == 156 else index + 32))
        os.utime(path, (1262304000 + index * 60,) * 2)
    (root / "link.txt").symlink_to("item-001.txt")
    (root / ".hidden.txt").write_text("Hidden.\n")
    helper = session.runtime / "views-record-selection.py"
    helper.write_text('import json, os, sys\nfrom pathlib import Path\n' +
                     f'Path({str(session.runtime / "views-selection.json")!r}).write_text(json.dumps([os.fsencode(p).hex() for p in sys.argv[1:]]))\n')
    command = f'{shlex.quote(sys.executable)} {shlex.quote(str(helper))} %F'
    config.write_text(config.read_text() + 'files {\n sidebar "always";\n ' + DEFAULT_VIEW + '\n' +
        ' action "record-selection" name="Record selection" shortcut="<Control><Alt>p" command=' + json.dumps(command) +
        ' { selection min=1 files=#true directories=#true; }\n}\n')


def verify(session, fixture, browser, launched, config, scheme, scale, binary, process, case="full"):
    root = fixture / "View fixtures"
    raw = os.fsencode(root) + b"/raw-\xff prefs"
    state = Path(session.env["XDG_STATE_HOME"]) / "canopa-files/views.json"

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def key(name, count=1):
        session.run(["wtype", *[arg for _ in range(count) for arg in ("-k", name)]])
        time.sleep(.2)

    def chord(name, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", name]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.25)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "browser focus")
        time.sleep(.25)

    def navigate(path):
        chord("l", "ctrl")
        value = os.fsencode(path)
        session.run(["wtype", "file://" + quote_from_bytes(value, safe="/")])
        key("Return")
        name = os.path.basename(value).decode(errors="replace")
        if value.endswith(b".txt"):
            name = os.path.basename(os.path.dirname(value)).decode(errors="replace")
        session.wait_for(lambda: title(name), name)
        time.sleep(.3)

    def width(value):
        session.run(["niri", "msg", "action", "set-window-width", str(value)])
        session.wait_for(lambda: next(w for w in windows() if w["is_focused"])["layout"]["window_size"][0] == value, "native tile width")
        time.sleep(.35)

    def read_state():
        try:
            return json.loads(state.read_text())
        except (FileNotFoundError, IsADirectoryError, json.JSONDecodeError):
            return {"folders": []}

    def stored(path):
        target = os.fsencode(path)
        for folder in read_state()["folders"]:
            encoded = folder["path"]
            actual = bytes(encoded["unix_bytes"]) if isinstance(encoded, dict) else os.fsencode(encoded)
            if actual == target:
                return folder["view"]
        return None

    def saved(path, **expected):
        session.wait_for(lambda: (value := stored(path)) is not None and all(value.get(k) == v for k, v in expected.items()), "persisted folder choices")
        time.sleep(.15)  # Disk publication can precede the next painted frame.

    def expect_open(path):
        launched.unlink(missing_ok=True)
        key("Return")
        def actual():
            try:
                return [os.fsencode(p) for p in json.loads(launched.read_text())] == [os.fsencode(path)]
            except (FileNotFoundError, json.JSONDecodeError):
                return False
        session.wait_for(actual, "actual file opened from the current view")

    def options():
        key("Menu")
        key("End")
        key("Return")

    def quick_look(start, following, capture):
        navigate(root / start)
        key("space")
        def preview(name):
            return next((w for w in windows() if w.get("title") == f"Quick Look: {name} — Canopa Files"), None)
        session.wait_for(lambda: (w := preview(start)) and w["is_floating"], "floating preview from the chosen folder view")
        key("Down")
        session.wait_for(lambda: preview(following), "preview navigation follows the source view's row stride")
        session.capture(capture)
        key("Escape")
        session.wait_for(lambda: preview(following) is None and any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "preview closes to its source browser")
        expect_open(root / following)

    foreground = (237, 244, 240) if scheme == "dark" else (32, 52, 44)
    selected = (61, 80, 78) if scheme == "dark" else (208, 223, 213)
    def pixels(frame, area, color, tolerance=4):
        box = tuple(round(v * scale) for v in area)
        return sum(max(abs(a-b) for a, b in zip(pixel[:3], color)) <= tolerance
                   for pixel in frame.crop(box).getdata())

    def selection_visible(name):
        def painted():
            frame = session.capture(name)
            focused = next(w for w in windows() if w["is_focused"])
            width = focused["layout"]["window_size"][0]
            return pixels(frame, (0, 165, width, frame.height / scale - 40), selected) > 500 * scale * scale
        session.wait_for(painted, "actual selected tile is painted in the viewport")

    def selection_border_visible(name):
        def painted():
            frame = session.capture(name).convert("RGB")
            focused = next(w for w in windows() if w["is_focused"])
            width = round(focused["layout"]["window_size"][0] * scale)
            top = round(165 * scale)
            region = frame.crop((0, top, width, frame.height - round(40 * scale)))
            difference = ImageChops.difference(region, Image.new("RGB", region.size, selected))
            channels = difference.split()
            maximum = ImageChops.lighter(ImageChops.lighter(channels[0], channels[1]), channels[2])
            mask = maximum.point([255] * 5 + [0] * 251).tobytes()
            rows = []
            for y in range(region.height):
                runs = list(re.finditer(b"\xff+", mask[y * width:(y + 1) * width]))
                if runs:
                    run = max(runs, key=lambda r: r.end() - r.start())
                    rows.append((run.end() - run.start(), run.end(), y + top))
            longest = max((length for length, _, _ in rows), default=0)
            body = [(right, y) for length, right, y in rows if length >= longest * .95]
            if longest < 50 * scale or not body or max(y for _, y in body) - min(y for _, y in body) < 70 * scale:
                return False
            def outlined(right, y):
                if right >= width:
                    return False
                contrast = sum(frame.getpixel((right, y))) - sum(selected)
                return contrast > 90 if scheme == "dark" else contrast < -75
            return sum(outlined(right, y) for right, y in body) >= len(body) * .7
        session.wait_for(painted, "complete selected icon-card right border beside the scrollbar")

    def list_details_visible(name):
        def painted():
            frame = session.capture(name)
            height = frame.height / scale
            return pixels(frame, (100, height - 135, 240, height - 32), foreground, tolerance=24) > 80 * scale * scale
        session.wait_for(painted, "actual detail text is painted beyond the filename")

    initial_width = browser["layout"]["window_size"][0]
    if case in ("reflow", "gutter"):
        focus(browser)
        navigate(root)
        width(360)
        session.run(["wtype", "item-159"])
        time.sleep(.3)
        chord("3", "ctrl")
        saved(root, mode="icons")
        selection_visible("reflow-00-narrow-selected.png")
        if case == "gutter":
            selection_border_visible("gutter-01-narrow-border.png")
            expect_open(root / "item-159.txt")
            width(initial_width)
            selection_border_visible("gutter-02-wide-border.png")
            expect_open(root / "item-159.txt")
            width(360)
            selection_border_visible("gutter-03-returned-narrow-border.png")
            expect_open(root / "item-159.txt")
            return {"status": "passed", "scheme": scheme, "scale": scale,
                    "case": case, "checks": ["rightmost selected icon has a complete border beside the scrollbar",
                        "wide/narrow reflow retains the border and actual selected-file launch target"],
                    "scope": "icon-grid scrollbar gutter regression"}
        expect_open(root / "item-159.txt")
        for cycle in range(1, 4):
            width(initial_width)
            selection_visible(f"reflow-{cycle:02}-wide-selected.png")
            expect_open(root / "item-159.txt")
            width(360)
            selection_visible(f"reflow-{cycle:02}-narrow-selected.png")
            expect_open(root / "item-159.txt")
        return {"status": "passed", "scheme": scheme, "scale": scale,
                "case": case, "checks": ["actual selected item-159 is painted in the narrow icon grid",
                    "three wide/narrow reflows retain the painted selection and actual launch target"],
                "scope": "reduced reflow reproduction, not complete view acceptance"}
    focus(browser)
    navigate(root)
    width(360)
    session.run(["wtype", "item-159"])
    time.sleep(.3)
    session.capture("views-01-columns.png")
    chord("2", "ctrl")
    saved(root, mode="list")
    list_details_visible("views-02-list-details.png")
    expect_open(root / "item-159.txt")
    before = session.capture("views-empty-search-before.png")
    height = before.height / scale
    area = (10, height - 90, 330, height - 40)
    reference = pixels(before, area, selected)
    assert reference > 500 * scale * scale, reference
    chord("f", "ctrl")
    def search_bar_painted():
        frame = session.capture("views-empty-search-open.png")
        box = tuple(round(v * scale) for v in (10, 126, 330, 215))
        changed = sum(max(abs(a-b) for a, b in zip(old[:3], new[:3])) > 8
                      for old, new in zip(before.crop(box).getdata(), frame.crop(box).getdata()))
        return changed > 300 * scale * scale
    session.wait_for(search_bar_painted, "the empty Search bar is actually painted")
    time.sleep(.2)
    key("Escape")
    def middle_scroll_restored():
        frame = session.capture("views-empty-search-restored.png")
        return pixels(frame, area, selected) >= reference * .9
    session.wait_for(middle_scroll_restored, "empty Search restores the middle-folder scroll, without end clamping")
    quick_look("item-010.txt", "item-011.txt", "views-preview-list.png")
    navigate(root / "item-159.txt")
    chord("3", "ctrl")
    saved(root, mode="icons")
    selection_visible("views-03-icon-selection.png")
    key("Home")
    def grid_painted():
        frame = session.capture("views-04-two-column-icons.png")
        return all(pixels(frame, area, foreground) > 100 * scale * scale
                   for area in ((64, 164, 116, 220), (244, 164, 296, 220)))
    session.wait_for(grid_painted, "two distinct file icons across the grid row")
    key("Down")
    expect_open(root / "item-000.txt")
    key("Home")
    chord("Down", "shift")
    chord("p", "ctrl", "alt")
    recorded = session.runtime / "views-selection.json"
    expected = sorted([os.fsencode(root / "A-child").hex(), raw.hex(), os.fsencode(root / "item-000.txt").hex()])
    session.wait_for(lambda: recorded.exists() and json.loads(recorded.read_text()) == expected, "grid range selection keeps raw paths")
    navigate(root / "item-159.txt")
    width(initial_width)
    selection_visible("views-05-wide-grid-selection.png")
    if case in ("reflow-prefix", "reflow-stress"):
        if case == "reflow-stress":
            for cycle in range(1, 21):
                width(360)
                selection_visible(f"reflow-stress-{cycle:02}-narrow.png")
                width(initial_width)
                selection_visible(f"reflow-stress-{cycle:02}-wide.png")
        expect_open(root / "item-159.txt")
        return {"status": "passed", "scheme": scheme, "scale": scale,
                "case": case, "checks": ["original list/search/preview/range-action prelude retains a painted and actionable selection after widening"],
                "additional_reflow_cycles": 20 if case == "reflow-stress" else 0,
                "scope": "original reflow prefix reproduction, not complete view acceptance"}
    width(360)
    selection_visible("views-06-narrow-grid-selection.png")
    selection_border_visible("views-06-narrow-grid-border.png")
    quick_look("item-010.txt", "item-012.txt", "views-preview-icons.png")
    navigate(root / "item-159.txt")
    key("Left")
    session.wait_for(lambda: title("Downloads"), "parent from the icon view")
    key("Right")
    session.wait_for(lambda: title("View fixtures"), "return through the selected child")
    time.sleep(.3)
    expect_open(root / "item-159.txt")

    old_ids = {w["id"] for w in windows()}
    chord("n", "ctrl")
    second = session.wait_for(lambda: next((w for w in windows() if w["id"] not in old_ids and w.get("title") == "View fixtures — Canopa Files"), None), "second browser of the same folder")
    time.sleep(.35)
    focus(second)
    chord("2", "ctrl")
    saved(root, mode="list")
    chord("w", "ctrl")
    session.wait_for(lambda: not any(w["id"] == second["id"] for w in windows()), "second browser closes")
    focus(browser)
    expect_open(root / "item-159.txt")
    session.capture("views-07-shared-list.png")
    width(360)
    options()
    key("Tab", 5)  # Close, modes, Name, Size.
    key("Return")
    saved(root, sort="size")
    key("Tab", 8)  # Kind detail.
    key("space")
    saved(root, details=["size", "modified", "kind"])
    chord("Tab", "shift")
    chord("Tab", "shift")
    chord("Tab", "shift")  # Width +.
    key("Return")
    saved(root, column_width=260)
    chord("Tab", "shift")
    chord("Tab", "shift")  # Reverse order.
    key("space")
    saved(root, descending=True)
    session.capture("views-08-folder-options.png")
    key("Escape")
    key("Home")
    key("Down", 2)  # The two folders precede the largest file.
    expect_open(root / "item-156.txt")
    session.capture("views-09-sorted-list.png")

    navigate(raw)
    chord("3", "ctrl")
    saved(raw, mode="icons")
    assert stored(raw) == {"mode": "icons"}, stored(raw)
    text = config.read_text()
    assert DEFAULT_VIEW in text
    chord("f", "ctrl")
    session.run(["wtype", "note part"])
    time.sleep(.35)
    key("End")
    session.run(["wtype", "-M", "shift", "-k", "Left", "-k", "Left", "-k", "Left", "-k", "Left", "-m", "shift"])
    before = session.capture("views-10a-search-selection.png")
    def notice_changed(before, name):
        def painted():
            frame = session.capture(name)
            box = tuple(round(v * scale) for v in (0, 126, 350, 220))
            changed = sum(max(abs(a-b) for a, b in zip(old[:3], new[:3])) > 8
                          for old, new in zip(before.crop(box).getdata(), frame.crop(box).getdata()))
            return changed > 300 * scale * scale  # Caret blinking is much smaller.
        session.wait_for(painted, "actual configuration notice transition")
    config.write_text(text + "files {\n")
    notice_changed(before, "views-10b-invalid-config-notice.png")
    session.run(["wtype", "kept"])
    chord("a", "ctrl")
    chord("c", "ctrl")
    assert session.run(["wl-paste", "--no-newline"]) == "note kept"
    key("End")
    session.run(["wtype", "-M", "shift", "-k", "Left", "-k", "Left", "-k", "Left", "-k", "Left", "-m", "shift"])
    before = session.capture("views-10c-retained-search-selection.png")
    config.write_text(text.replace(DEFAULT_VIEW, UPDATED_VIEW))
    notice_changed(before, "views-10d-valid-config-restored.png")
    session.run(["wtype", "live"])
    chord("a", "ctrl")
    chord("c", "ctrl")
    assert session.run(["wl-paste", "--no-newline"]) == "note live"
    key("Escape")
    options()
    session.capture("views-10-inherited-options.png")
    chord("Tab", "shift")  # Last enabled control: Use defaults.
    key("Return")
    session.wait_for(lambda: stored(raw) is None, "Use defaults clears the raw folder override")
    key("Escape")
    session.capture("views-11-raw-default-list.png")
    assert stored(root) == {"mode": "list", "sort": "size", "descending": True,
                            "details": ["size", "modified", "kind"], "column_width": 260}
    # Batched view changes followed immediately by the last Browser close must
    # retain the latest accepted snapshot through actual natural process exit.
    session.processes.remove(process)
    try:
        session.run(["wtype", "-M", "ctrl", "-k", "3", "-k", "2", "-k", "3", "-k", "w", "-m", "ctrl"])
        session.wait_for(lambda: process.poll() is not None, "natural exit after the final view save")
    finally:
        if process.poll() is None:
            session.processes.append(process)
    assert process.returncode == 0, process.returncode
    assert stored(raw) == {"mode": "icons"}, stored(raw)
    assert not [w for w in windows() if w.get("app_id") == "org.canopa.Files"]
    reopened = session.start([str(binary), str(root)], "files-reopened.log")
    browser = session.wait_for(lambda: title("View fixtures"), "reopened browser")
    time.sleep(.4)
    focus(browser)
    width(360)
    key("Home")
    key("Down", 2)
    expect_open(root / "item-156.txt")
    session.capture("views-12-restarted-sorted-list.png")
    navigate(raw)
    session.capture("views-13-restarted-raw-icons.png")

    backup = state.with_suffix(".saved")
    state.replace(backup)
    state.mkdir()
    chord("2", "ctrl")
    def warning_painted():
        frame = session.capture("views-14-state-write-failure.png")
        return pixels(frame, (300, 132, 350, 166), foreground, tolerance=24) > 20 * scale * scale
    session.wait_for(warning_painted, "view-state failure and Retry are visible")
    state.rmdir()
    backup.replace(state)
    pointer = session.build_pointer()
    outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
    logical = next(o["logical"] for o in outputs.values() if o.get("logical"))
    session.run([str(pointer), str(round(325 * 1600 / logical["width"])), str(round(147 * 1000 / logical["height"]))])
    saved(raw, mode="list")
    session.capture("views-15-retry-saved.png")
    assert reopened.poll() is None
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": [
        "columns remain the initial global default", "Ctrl+2 shows a detailed list and keeps the actual selected launch target",
        "opening and closing empty Search preserves a middle-folder scroll position",
        "Ctrl+3 shows distinct file icons in two columns at 360px", "Up/Down follows grid rows while Left/Right retains folder navigation",
        "List Quick Look opens as a floating source preview", "List preview navigation returns to the exact following file",
        "Icons Quick Look opens independently of the narrow tile", "Icons preview navigation uses the source grid stride and returns its exact file",
        "Shift+Down grid ranges preserve files, folders and raw Unix paths through a real command",
        "wide/narrow reflow keeps the actual focused tile painted and actionable",
        "rightmost icon-card focus border stays visible beside the scrollbar",
        "parent/child return restores the icon folder and selection",
        "a view change in another Browser publishes to the original Browser", "folder options support batched keyboard traversal",
        "sorting changes the actual opened file in the expected order", "details, direction and column width persist only deliberate fields",
        "live defaults retain sparse raw-folder overrides", "Use defaults removes every override and renders the inherited list",
        "invalid source notices preserve native Search focus and partial selection", "valid defaults reload preserves the same native Search editing state",
        "rapid view changes and last-window close persist the latest snapshot through natural exit",
        "restart restores actual sort order and byte-preserving folder choices", "failed private-state writes stay visible and Retry saves the retained choice"],
        "remaining": ["global Views/Toolbar/Locations/Shortcuts editor", "full large-folder, focus and accessibility matrix", "remaining operations/devices/media/native integration"]}
