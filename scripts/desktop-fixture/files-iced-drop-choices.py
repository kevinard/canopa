"""Actual native placement choices and drag sources/targets in Iced browsers."""
import hashlib
import json
import os
from pathlib import Path
import re
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Drop choices"
    assert root.is_relative_to(session.runtime / "home")
    for directory in ["Source", "Destination", "Bookmark destination", "Search root/Nested", "Ancestor/Inside"]:
        (root / directory).mkdir(parents=True)
    (root / "Source/a.txt").write_bytes(b"Placement and presentation bytes\n")
    (root / "Search root/Nested/needle.txt").write_bytes(b"Recursive search drag bytes\n")
    (root / "Ancestor/Inside/child.txt").write_bytes(b"Ancestor folder drag bytes\n")
    config.write_text(config.read_text() + 'files { sidebar "never"; shortcut "search-subfolders" "<Alt>s"; }\n')


def workflow(session, fixture, browser, scheme, scale, process, cross_destination):
    root = fixture / "Drop choices"
    source = root / "Source/a.txt"
    destination = root / "Destination"
    cross = cross_destination
    assert cross and cross.is_dir() and cross.name.startswith(".canopa-native-cross-")
    assert source.stat().st_dev != cross.stat().st_dev, "native placement acceptance requires two actual filesystems"
    checks, geometry = [], []
    pointer = session.build_pointer("drag")
    wire = session.output / "files.log"
    original = b"Placement and presentation bytes\n"

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "choice Browser focus")

    def chord(key, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", key]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.2)

    def navigate(path):
        chord("l", "ctrl")
        session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(path), safe="/")])
        chord("Return")
        directory = path if path.is_dir() else path.parent
        result = session.wait_for(lambda: title(directory.name), "choice location")
        time.sleep(.3)
        return result

    def notices():
        path = session.output / "notifications.json"
        return json.loads(path.read_text()) if path.exists() else []

    def completed(kind):
        if kind == "Undo":
            return [e for e in notices() if e["body"].split(" ·", 1)[0].endswith(" reversed")]
        return [e for e in notices() if e["title"].startswith(kind)
                and e["body"].split(" ·", 1)[0].endswith(" completed")
                and "0 failed" in e["body"] and "0 remaining" in e["body"]]

    def inventory():
        result = {}
        for label, directory in [("home", root), ("cross", cross)]:
            for path in directory.rglob("*"):
                key = label + "/" + os.fsencode(path.relative_to(directory)).hex()
                result[key] = ("directory", path.stat().st_ino) if path.is_dir() else (
                    "file", path.stat().st_ino, hashlib.sha256(path.read_bytes()).hexdigest())
        return result

    def check(message):
        checks.append(message)
        (session.output / "choices-progress.json").write_text(json.dumps(checks, indent=2) + "\n")
        (session.output / f"choices-checkpoint-{len(checks):02}.json").write_text(json.dumps(
            {"checks": checks, "windows": windows(), "notices": notices(), "namespace": inventory()}, indent=2) + "\n")

    def undo(predicate):
        before = len(completed("Undo"))
        focus(target)
        chord("z", "ctrl")
        session.wait_for(predicate, "shared Undo of actual drag choice")
        session.wait_for(lambda: len(completed("Undo")) > before, "saved choice Undo")

    focus(browser)
    browser = navigate(source)
    chord("2", "ctrl")
    session.run(["niri", "msg", "action", "set-column-width", "50%"])
    chord("n", "ctrl")
    session.wait_for(lambda: len(windows()) == 2, "two choice browsers")
    target = next(w for w in windows() if w["id"] != browser["id"])
    session.run(["niri", "msg", "action", "set-column-width", "50%"])
    navigate(cross)
    chord("2", "ctrl")
    focus(browser)
    session.run(["niri", "msg", "action", "move-column-to-first"])
    session.run(["niri", "msg", "action", "center-visible-columns"])
    (session.output / "choices-filesystems.json").write_text(json.dumps(
        {"source_device": source.stat().st_dev, "cross_device": cross.stat().st_dev,
         "source": str(source), "cross": str(cross)}, indent=2) + "\n")

    def selected_row(painted, top, bottom, record=None):
        # The source toolbar and search filters can wrap. Locate the single
        # selected row's painted band at an x beyond its short label (in the
        # left-hand source tile), ignoring thin borders, glyphs and chrome
        # already present at the scan's first row.
        x = round(260 * scale)
        background = painted.getpixel((x, round(min(bottom + 90, 600) * scale)))
        runs, start = [], None
        for y in range(round(top * scale), round(bottom * scale) + 1):
            selected = y < round(bottom * scale) and max(
                abs(a - b) for a, b in zip(painted.getpixel((x, y)), background)) > 8
            if selected and start is None:
                start = y
            elif not selected and start is not None:
                if y - start >= round(20 * scale) and start > round(top * scale):
                    runs.append((start, y))
                start = None
        assert len(runs) == 1, f"expected one painted selected row: {runs}"
        logical_y = sum(runs[0]) / (2 * scale)
        if record:
            (session.output / record).write_text(json.dumps(
                {"x": x, "background": background, "selected_band": runs[0], "logical_y": logical_y}, indent=2) + "\n")
        return logical_y

    def align_browsers():
        # center-visible-columns only centers columns already fully visible.
        # After resizing, reveal the target first so both fitting tiles are in
        # the viewport before centering and calculating absolute coordinates.
        focus(target)
        focus(browser)
        session.run(["niri", "msg", "action", "center-visible-columns"])

    def drag(name, kind, source_x=100, source_y=144, target_x=None, target_y=350, modifier=None):
        align_browsers()
        actual = sorted(windows(), key=lambda w: w["layout"]["pos_in_scrolling_layout"])
        assert len(actual) == 2
        output = next(o["logical"] for o in json.loads(session.run(["niri", "msg", "--json", "outputs"])).values() if o.get("logical"))
        total = sum(w["layout"]["tile_size"][0] for w in actual)
        assert total <= output["width"] + 1
        def point(window, x, y):
            index = next(i for i, w in enumerate(actual) if w["id"] == window["id"])
            layout = actual[index]["layout"]
            left = (output["width"] - total) / 2 + sum(w["layout"]["tile_size"][0] for w in actual[:index])
            dx, dy = layout["window_offset_in_tile"]
            return str(round((left + dx + x) * 1600 / output["width"])), str(round((dy + y) * 1000 / output["height"]))
        current_target_x = target_x if target_x is not None else next(w["layout"]["window_size"][0] - 100 for w in actual if w["id"] == target["id"])
        first, last = point(browser, source_x, source_y), point(target, current_target_x, target_y)
        geometry.append({"name": name, "output": output, "windows": actual, "source": first, "target": last})
        (session.output / "choices-geometry.json").write_text(json.dumps(geometry, indent=2) + "\n")
        before, notification_count, wire_offset = inventory(), len(notices()), len(wire.read_text())
        ready, release = session.runtime / f"{name}.ready", session.runtime / f"{name}.release"
        helper = session.start([str(pointer), *first, *last, str(ready), str(release)], name + "-pointer.log")
        session.wait_for(ready.exists, "held placement/presentation drag")
        starts = list(re.finditer(r"-> wl_data_device#\d+\.start_drag\(wl_data_source#(\d+)", wire.read_text()[wire_offset:]))
        assert starts, "the visible source did not start a native drag"
        source_id, offset = starts[-1][1], wire_offset + starts[-1].end()
        modifier_owner = None
        if modifier:
            focus(target)
            modifier_owner = session.start(["wtype", "-M", modifier, "-s", "60000", "-m", modifier], name + "-modifier.log")
        action = "1" if kind == "Copy" else "2"
        def negotiated():
            actions = re.findall(rf"wl_data_source#{source_id}\.action\((\d+)\)", wire.read_text()[offset:])
            return actions and actions[-1] == action
        session.wait_for(negotiated, "actual negotiated placement action")
        assert inventory() == before and len(notices()) == notification_count, "hover admitted file work"
        session.capture(name + "-held.png")
        count = len(completed(kind))
        release.touch()
        helper.wait(timeout=10)
        assert helper.returncode == 0
        session.processes.remove(helper)
        if modifier_owner:
            session.retire(modifier_owner)
        session.wait_for(lambda: len(completed(kind)) > count, "saved actual drag result")

    drag("choice-01-cross-copy", "Copy")
    assert source.read_bytes() == original and (cross / source.name).read_bytes() == original
    undo(lambda: source.read_bytes() == original and not (cross / source.name).exists())
    check("different actual filesystems negotiate Copy by default, preserve the source and use shared Undo")

    drag("choice-02-cross-move", "Move", modifier="shift")
    assert not source.exists() and (cross / source.name).read_bytes() == original
    undo(lambda: source.exists() and source.read_bytes() == original and not (cross / source.name).exists())
    check("Shift forces a real cross-filesystem Move and shared Undo restores the original file bytes")

    focus(target)
    navigate(destination)
    chord("1", "ctrl")
    focus(browser)
    navigate(source)
    chord("3", "ctrl")
    session.capture("choice-03-icons-before.png")
    drag("choice-03-icon-header", "Move", source_y=175, target_y=108)
    assert not source.exists() and (destination / source.name).read_bytes() == original
    undo(lambda: source.exists() and source.read_bytes() == original and not (destination / source.name).exists())
    check("an icon row drags the selected file to a column header's folder and shares the same transfer/Undo workflow")

    search_root = root / "Search root"
    needle = search_root / "Nested/needle.txt"
    focus(browser)
    navigate(search_root)
    chord("f", "ctrl")
    session.run(["wtype", "needle"])
    query_title = "Search: needle — Search root — Canopa Files"
    session.wait_for(lambda: any(w["title"] == query_title for w in windows()), "initial filename search")
    chord("s", "alt")
    session.wait_for(lambda: any(w["title"] == query_title for w in windows()), "recursive filename search")
    session.run(["wtype", "x"])
    session.wait_for(lambda: any(w["title"] == "Search: needlex — Search root — Canopa Files" for w in windows()), "scope shortcut preserves query editing focus")
    chord("BackSpace")
    session.wait_for(lambda: any(w["title"] == query_title for w in windows()), "query editing after recursive toggle")
    chord("Return")
    chord("Down")
    searched = session.capture("choice-04-search-before.png")
    # The result row follows the location bar, the search filters (which wrap
    # in narrower tiles) and the results heading: find its painted selection.
    drag("choice-04-search-source", "Move", source_x=260, source_y=selected_row(searched, 155, 420))
    assert not needle.exists() and (destination / needle.name).read_bytes() == b"Recursive search drag bytes\n"
    undo(lambda: needle.exists() and needle.read_bytes() == b"Recursive search drag bytes\n" and not (destination / needle.name).exists())
    check("a recursive filename-search result captures its actual nested source and moves with shared Undo")

    # Sidebar appearance is changed through the real watched shared config.
    config = session.runtime / "config/canopa/config.kdl"
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n'
        'files { sidebar "always"; shortcut "search-subfolders" "<Alt>s"; places { '
        f'bookmark "Drop here" path={json.dumps(str(root / "Bookmark destination"))}; }} }}\n')
    # Show when there is room still hides below 560 logical pixels. At 125%,
    # two equal tiles are only 512px: give the bookmark target enough room and
    # keep the source narrow, rather than aiming at an invisible sidebar.
    focus(browser)
    session.run(["niri", "msg", "action", "set-column-width", "40%"])
    focus(target)
    session.run(["niri", "msg", "action", "set-column-width", "60%"])
    session.wait_for(lambda: any(w["id"] == target["id"] and w["layout"]["window_size"][0] >= 560
                                for w in windows()), "bookmark target has room for its sidebar")
    focus(browser)
    navigate(source)
    chord("2", "ctrl")
    time.sleep(.7)
    align_browsers()
    painted = session.capture("choice-05-sidebar-before.png")
    source_y = selected_row(painted, 100, 260, "choices-sidebar-source-paint.json")
    # The bookmark is the only place, listed below the Places heading.
    drag("choice-05-sidebar-target", "Move", source_x=260, source_y=source_y, target_x=70, target_y=152)
    bookmarked = root / "Bookmark destination" / source.name
    assert not source.exists() and bookmarked.read_bytes() == original and not (destination / source.name).exists()
    assert title(destination.name)["id"] == target["id"], "dropping onto a bookmark navigated the destination Browser"
    undo(lambda: source.exists() and source.read_bytes() == original and not bookmarked.exists())
    check("a live-configured sidebar bookmark accepts the drop without navigating or changing the displayed destination")

    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\nfiles {{ sidebar "never"; }}\n')
    focus(target)
    session.run(["niri", "msg", "action", "set-column-width", "50%"])
    focus(browser)
    session.run(["niri", "msg", "action", "set-column-width", "50%"])
    navigate(root / "Ancestor/Inside")
    chord("1", "ctrl")
    time.sleep(.7)
    align_browsers()
    session.capture("choice-06-ancestor-before.png")
    # The column trail opens with Ancestor's listing, whose first row is Inside.
    drag("choice-06-ancestor-source", "Move", source_x=100, source_y=115)
    moved_folder = destination / "Inside"
    assert not (root / "Ancestor/Inside").exists() and (moved_folder / "child.txt").read_bytes() == b"Ancestor folder drag bytes\n"
    undo(lambda: (root / "Ancestor/Inside/child.txt").exists() and not moved_folder.exists())
    check("an ancestor-column row can move its folder without first navigating, while the accepted operation survives the source view becoming unavailable")
    session.capture("choice-07-complete.png")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "actual cross-filesystem choices and icon/search/header/bookmark/ancestor gestures; device/stale/delayed/full-product gates remain separate"}


def verify(session, fixture, browser, scheme, scale, process, cross_destination):
    try:
        return workflow(session, fixture, browser, scheme, scale, process, cross_destination)
    finally:
        result = {}
        for label, directory in [("home", fixture / "Drop choices"), ("cross", cross_destination)]:
            result[label] = [{"path_bytes": os.fsencode(p.relative_to(directory)).hex(), "bytes": p.stat().st_size,
                              "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
                             for p in sorted(directory.rglob("*")) if p.is_file()]
        (session.output / "choices-filesystem.json").write_text(json.dumps(result, indent=2) + "\n")
