"""Real Iced row gestures, transfers and shared Undo in the private desktop."""
import hashlib
import json
import os
import re
import signal
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Drop fixtures"
    assert root.is_relative_to(session.runtime / "home")
    (root / "Source").mkdir(parents=True)
    (root / "Destination/Folder").mkdir(parents=True)
    (root / "Source/a.txt").write_bytes(b"Original drag bytes\n")
    (root / "Source" / os.fsdecode(b"b raw \xff.txt")).write_bytes(b"Raw-name group bytes\n")
    (root / "Scroll source").mkdir()
    for index in range(40):
        (root / "Scroll source" / f"file-{index:02}.txt").write_bytes(f"Scrolled file {index}\n".encode())
        (root / "Scroll destination" / f"Folder-{index:02}").mkdir(parents=True)


def workflow(session, fixture, browser, scheme, scale, process):
    root = fixture / "Drop fixtures"
    source, destination = root / "Source", root / "Destination"
    a, raw = source / "a.txt", source / os.fsdecode(b"b raw \xff.txt")
    expected = {a.name: b"Original drag bytes\n", raw.name: b"Raw-name group bytes\n"}
    checks = []
    pointer_geometry = []

    def check(message):
        checks.append(message)
        (session.output / "drop-progress.json").write_text(json.dumps(checks, indent=2) + "\n")

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "drop Browser focus")

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
        session.wait_for(lambda: title(directory.name), "drop navigation")
        time.sleep(.3)

    def notices():
        path = session.output / "notifications.json"
        return json.loads(path.read_text()) if path.exists() else []

    def completed(prefix):
        records = notices()
        if prefix == "Undo":
            return [record for record in records if record["body"].split(" ·", 1)[0].endswith(" reversed")]
        return [record for record in records if record["title"].startswith(prefix)
                and record["body"].split(" ·", 1)[0].endswith(" completed")
                and "0 failed" in record["body"] and "0 remaining" in record["body"]]

    def restored(paths):
        return all(path.exists() and path.read_bytes() == expected[path.name] for path in paths)

    def undo(paths, removed):
        before = len(completed("Undo"))
        focus(target)
        chord("z", "ctrl")
        session.wait_for(lambda: restored(paths) and all(not path.exists() for path in removed), "actual shared Undo of the drop")
        session.wait_for(lambda: len(completed("Undo")) > before, "saved drop Undo completion")
        time.sleep(.3)

    focus(browser)
    navigate(a)
    chord("2", "ctrl")
    session.run(["niri", "msg", "action", "set-column-width", "50%"])
    source_window = title("Source")
    chord("n", "ctrl")
    session.wait_for(lambda: len(windows()) == 2, "second drop Browser")
    session.run(["niri", "msg", "action", "set-column-width", "50%"])
    navigate(destination)
    chord("2", "ctrl")
    target = title("Destination")
    focus(source_window)
    session.run(["niri", "msg", "action", "move-column-to-first"])
    time.sleep(.4)
    pointer = session.build_pointer("drag")
    session.capture("iced-drop-01-tiles.png")

    def namespace():
        return {os.fsencode(path.relative_to(root)).hex():
                ("folder", path.stat().st_ino) if path.is_dir() else
                ("file", path.stat().st_ino, path.stat().st_mode, hashlib.sha256(path.read_bytes()).hexdigest())
                for path in root.rglob("*")}

    def drag(name, target_y=350, source_y=144, source_x=100, target_x=100,
             modifier=None, cancel=False, stalled=False):
        before_hover = namespace()
        before_notices = len(notices())
        focus(source_window)
        session.run(["niri", "msg", "action", "center-visible-columns"])
        actual = sorted(windows(), key=lambda w: w["layout"]["pos_in_scrolling_layout"])
        output = next(o["logical"] for o in json.loads(session.run(["niri", "msg", "--json", "outputs"])).values() if o.get("logical"))
        total_width = sum(w["layout"]["tile_size"][0] for w in actual)
        assert len(actual) == 2 and total_width <= output["width"] + 1
        modifier_peer = None
        ready, release = session.runtime / f"{name}.ready", session.runtime / f"{name}.release"
        def point(window, x, y):
            index = next(i for i, w in enumerate(actual) if w["id"] == window["id"])
            layout = actual[index]["layout"]
            # This Niri version reports positions only for floating windows.
            # Explicit centering above fixes this two-column, zero-gap layout.
            left = (output["width"] - total_width) / 2 + sum(w["layout"]["tile_size"][0] for w in actual[:index])
            left += layout["window_offset_in_tile"][0]
            top = layout["window_offset_in_tile"][1]
            return str(round((left + x) * 1600 / output["width"])), str(round((top + y) * 1000 / output["height"]))
        first, last = point(source_window, source_x, source_y), point(target, target_x, target_y)
        pointer_geometry.append({"name": name, "output": output, "windows": actual, "source": first, "target": last})
        (session.output / "drop-pointer-geometry.json").write_text(json.dumps(pointer_geometry, indent=2) + "\n")
        wire_offset = len((session.output / "files.log").read_text())
        helper = session.start([str(pointer), *first, *last, str(ready), str(release)], f"{name}-pointer.log")
        session.wait_for(ready.exists, "held Iced row drag")
        time.sleep(.4)
        assert restored([a])
        assert namespace() == before_hover, "a held drag changed the private filesystem before release"
        wire = session.output / "files.log"
        sources = list(re.finditer(r"-> wl_data_device#\d+\.start_drag\(wl_data_source#(\d+)", wire.read_text()[wire_offset:]))
        assert sources, "Files did not start a new native drag"
        source_id, offset = sources[-1][1], wire_offset + sources[-1].end()
        if modifier:
            # wtype's modifier mask is synthetic: Niri does not carry it over
            # a keyboard-focus change. Change the live drag action after focus
            # follows the pointer, keeping this test independent of that quirk.
            focus(target)
            modifier_peer = session.start(["wtype", "-M", modifier, "-s", "60000", "-m", modifier], f"{name}-modifier.log")
        expected_action = "1" if modifier == "ctrl" else "2"
        def negotiated():
            actions = re.findall(rf"wl_data_source#{source_id}\.action\((\d+)\)", wire.read_text()[offset:])
            return actions and actions[-1] == expected_action
        session.wait_for(negotiated, "receiver has negotiated the expected native action")
        if name == "iced-drop-02-move":
            # wtype maps this logical key to the same synthetic native code as
            # Escape. Only interpreted Escape may cancel a held drag.
            chord("F12")
            session.wait_for(negotiated, "an unrelated logical key preserves the held drag")
        session.capture(f"{name}-held.png")
        assert namespace() == before_hover, "native hover negotiation admitted a filesystem change"
        assert len(notices()) == before_notices, "hover admitted a backend operation before release"
        if stalled:
            os.kill(process.pid, signal.SIGSTOP)
        try:
            if cancel:
                source_id, offset = sources[-1][1], wire_offset + sources[-1].end()
                chord("Escape")
                if not stalled:
                    session.wait_for(lambda: f"wl_data_source#{source_id}.destroy()" in wire.read_text()[offset:], "source handles Escape before physical release")
            release.touch()
            helper.wait(timeout=10)
        finally:
            if stalled:
                os.kill(process.pid, signal.SIGCONT)
        assert helper.returncode == 0
        session.processes.remove(helper)
        if modifier_peer:
            session.retire(modifier_peer)
        time.sleep(.3)

    before = len(completed("Move"))
    drag("iced-drop-02-move")
    session.wait_for(lambda: not a.exists() and (destination / a.name).exists(), "default same-filesystem native Move")
    session.wait_for(lambda: len(completed("Move")) > before, "saved native Move result")
    assert (destination / a.name).read_bytes() == expected[a.name]
    check("a real Iced row drag moves only after release and saves its normal backend result")
    undo([a], [destination / a.name])
    check("another Browser reverses the native Move through shared Ctrl+Z")

    focus(source_window)
    navigate(a)
    before = len(completed("Copy"))
    drag("iced-drop-03-control-copy", modifier="ctrl")
    session.wait_for(lambda: (destination / a.name).exists(), "Ctrl forces native Copy")
    session.wait_for(lambda: len(completed("Copy")) > before, "saved forced Copy result")
    assert restored([a]) and (destination / a.name).read_bytes() == expected[a.name]
    undo([a], [destination / a.name])
    check("Ctrl changes the live native action to Copy without consuming the source")

    focus(source_window)
    navigate(a)
    before = len(completed("Move"))
    drag("iced-drop-04-folder-row", target_y=144)
    nested = destination / "Folder/a.txt"
    session.wait_for(lambda: nested.exists() and not a.exists(), "folder-row target wins over containing column")
    session.wait_for(lambda: len(completed("Move")) > before, "saved folder-row Move")
    assert nested.read_bytes() == expected[a.name] and not (destination / a.name).exists()
    undo([a], [nested])
    check("actual folder-row hit testing takes priority over its containing column")

    focus(source_window)
    navigate(a)
    chord("a", "ctrl")
    before = len(completed("Move"))
    drag("iced-drop-05-group")
    session.wait_for(lambda: all(not path.exists() and (destination / path.name).exists() for path in [a, raw]), "original multiple selection moves together")
    session.wait_for(lambda: len(completed("Move")) > before, "saved group Move")
    assert all((destination / name).read_bytes() == data for name, data in expected.items())
    undo([a, raw], [destination / a.name, destination / raw.name])
    check("pressing an already selected row preserves the whole group, including raw filename bytes")

    scrolled = root / "Scroll source/file-39.txt"
    expected[scrolled.name] = b"Scrolled file 39\n"
    focus(source_window)
    navigate(scrolled)
    chord("End")
    focus(target)
    navigate(root / "Scroll destination")
    chord("End")
    session.capture("iced-drop-06-scrolled-tiles.png")
    before = len(completed("Move"))
    source_height = next(w["layout"]["window_size"][1] for w in windows() if w["id"] == source_window["id"])
    target_height = next(w["layout"]["window_size"][1] for w in windows() if w["id"] == target["id"])
    drag("iced-drop-06-scrolled", source_x=420, target_x=420, source_y=source_height - 100, target_y=target_height - 100)
    nested = root / "Scroll destination/Folder-39/file-39.txt"
    session.wait_for(lambda: nested.exists() and not scrolled.exists(), "visible scrolled source and destination rows")
    session.wait_for(lambda: len(completed("Move")) > before, "saved scrolled-row Move")
    assert nested.read_bytes() == expected[scrolled.name]
    undo([scrolled], [nested])
    check("scrolled source and folder rows use their rendered bounds and transfer the exact visible file")

    focus(target)
    navigate(destination)
    focus(source_window)
    navigate(a)
    before = len(completed("Move")) + len(completed("Copy"))
    drag("iced-drop-06-escape", cancel=True)
    time.sleep(.5)
    assert restored([a, raw]) and not (destination / a.name).exists()
    assert len(completed("Move")) + len(completed("Copy")) == before
    check("Escape invalidates the original backend drag before a phantom native Drop can admit work")
    focus(source_window)
    navigate(a)
    before = len(completed("Move")) + len(completed("Copy"))
    drag("iced-drop-07-buffered-escape", cancel=True, stalled=True)
    time.sleep(.6)
    assert restored([a, raw]) and not (destination / a.name).exists(), "buffered Escape was overtaken by a later native Drop"
    assert len(completed("Move")) + len(completed("Copy")) == before
    check("Escape queued before release still refuses admission while the GUI process was temporarily stopped")
    focus(source_window)
    chord("1", "ctrl")
    session.run(["niri", "msg", "action", "set-window-width", "360"])
    focus(target)
    chord("1", "ctrl")
    session.run(["niri", "msg", "action", "set-window-width", "360"])
    time.sleep(.5)
    assert all(w["layout"]["window_size"][0] == 360 for w in windows())
    before = len(completed("Move"))
    drag("iced-drop-08-narrow", source_y=178)
    session.wait_for(lambda: not a.exists() and (destination / a.name).exists(), "actual drop between narrow column browsers")
    session.wait_for(lambda: len(completed("Move")) > before, "saved narrow-browser Move")
    undo([a], [destination / a.name])
    check("column browsers adapt to 360px tiles while native Move and shared Undo retain their meaning")
    session.capture("iced-drop-08-complete.png")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "actual same-runtime Iced gestures, backend transfers and Undo; external/drop-lifetime/full-product acceptance remains separate"}


def verify(session, fixture, browser, scheme, scale, process):
    try:
        return workflow(session, fixture, browser, scheme, scale, process)
    finally:
        root = fixture / "Drop fixtures"
        tree = []
        for path in sorted(root.rglob("*")):
            if path.is_file():
                data = path.read_bytes()
                tree.append({"path_bytes": os.fsencode(path.relative_to(root)).hex(), "bytes": len(data),
                             "sha256": hashlib.sha256(data).hexdigest()})
        (session.output / "drop-filesystem.json").write_text(json.dumps(tree, indent=2) + "\n")
