"""Iced name prompts, actual mutations and shared Undo in private tiled windows."""
import json
import os
from pathlib import Path
import signal
import subprocess
import time
from urllib.parse import quote_from_bytes
from PIL import ImageChops


def setup(session, fixture, config, location_editing_only=False):
    root = fixture / "Operation fixtures"
    assert root.is_relative_to(session.runtime / "home")
    root.mkdir()
    (root / "rename-me.txt").write_bytes(b"Original file contents\n")
    (root / "existing.txt").write_bytes(b"Preserve existing contents\n")
    raw = os.fsencode(root) + b"/raw-\xff.txt"
    with open(raw, "wb") as stream:
        stream.write(b"Original raw filename\n")
    editing = ' shortcut "copy"; shortcut "location-focus" "<Ctrl>c";' if location_editing_only else ''
    config.write_text(config.read_text() + f'files {{ shortcut "operations" "<Alt>o";{editing} }}\n')


def verify(session, fixture, browser, scheme, scale, process, window_navigation_only=False,
           location_editing_only=False):
    root = fixture / "Operation fixtures"
    checks = []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def key(name):
        session.run(["wtype", "-k", name])
        time.sleep(.18)

    def chord(name, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", name]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.2)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "browser focus")
        time.sleep(.2)

    def navigate(path, burst=False):
        location = "file://" + quote_from_bytes(os.fsencode(path), safe="/")
        if burst:
            session.run(["wtype", "-M", "ctrl", "-k", "l", "-m", "ctrl", location])
        else:
            chord("l", "ctrl")
            session.run(["wtype", location])
        key("Return")
        directory = path if os.path.isdir(path) else os.path.dirname(path)
        name = os.fsdecode(os.path.basename(directory))
        session.wait_for(lambda: any(w["is_focused"] and w.get("title") == f"{name} — Canopa Files" for w in windows()), "operation folder navigation")
        time.sleep(.3)

    def prompt(name, shortcut, *modifiers):
        chord(shortcut, *modifiers) if modifiers else key(shortcut)
        dialog = session.wait_for(lambda: title(name), f"Iced {name} dialog")
        session.wait_for(lambda: any(w["id"] == dialog["id"] and w["is_focused"] for w in windows()), "name field focus")
        assert dialog["is_floating"], "name prompt remains comfortable above a narrow browser"
        time.sleep(.25)
        return dialog

    def commit(name, target):
        session.run(["wtype", name])
        key("Return")
        session.wait_for(target.exists, "actual committed filesystem change")
        session.wait_for(lambda: title("New folder") is None and title("Rename") is None, "name prompt closes after durable admission")
        time.sleep(.25)

    if location_editing_only:
        def copied_text():
            try:
                return session.run(["wl-paste", "--no-newline"])
            except subprocess.CalledProcessError:
                return None  # No text selection has been published yet.

        focus(browser)
        chord("c", "ctrl")
        marker = str(root / "Location text selection")
        session.run(["wtype", marker])
        chord("a", "ctrl")
        chord("c", "ctrl")
        session.wait_for(lambda: copied_text() == marker,
                         "native location text Copy takes precedence over rebound Location")
        key("Escape")
        chord("f", "ctrl")
        query = "Search text selection"
        session.run(["wtype", query])
        chord("a", "ctrl")
        chord("c", "ctrl")
        session.wait_for(lambda: copied_text() == query,
                         "native search text Copy takes precedence over rebound Location")
        key("Escape")
        location = "file://" + quote_from_bytes(os.fsencode(root / "rename-me.txt"), safe="/")
        session.run(["wtype", "-M", "ctrl", "-k", "c", "-m", "ctrl", location])
        key("Return")
        prompt("Rename", "F2")
        renamed = root / "editing-renamed.txt"
        commit(renamed.name, renamed)
        assert not (root / "rename-me.txt").exists()
        assert renamed.read_bytes() == b"Original file contents\n"
        focus(browser)
        chord("z", "ctrl")
        session.wait_for(lambda: (root / "rename-me.txt").exists() and not renamed.exists(),
                         "shared Undo after rebound Location navigation")
        assert (root / "rename-me.txt").read_bytes() == b"Original file contents\n"
        session.capture("location-native-editing.png")
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": [
            "native location text Copy takes precedence over the configured Location shortcut",
            "native search text Copy takes precedence over the configured Location shortcut",
            "the rebound Location shortcut still accepts a rapid file URI with actual Rename and shared Undo"],
            "scope": "configured Location shortcut and native text editing in the private session"}

    if window_navigation_only:
        focus(browser)
        session.run(["niri", "msg", "action", "set-window-width", "340"])
        session.wait_for(lambda: next(w for w in windows() if w["id"] == browser["id"])
                         ["layout"]["window_size"][0] == 340, "narrow original Browser")
        time.sleep(.3)
        navigate(root / "rename-me.txt")
        prompt("Rename", "F2")
        session.run(["wtype", "existing.txt"])
        key("Return")
        session.wait_for(lambda: title("Rename") is None, "collision request admitted")
        time.sleep(.4)
        assert (root / "rename-me.txt").read_bytes() == b"Original file contents\n"
        assert (root / "existing.txt").read_bytes() == b"Preserve existing contents\n"
        for attempt in range(1, 9):
            focus(browser)
            chord("n", "ctrl")
            other = session.wait_for(lambda: next((w for w in windows()
                if w.get("app_id") == "org.canopa.Files" and w["id"] != browser["id"]), None), "fresh Browser")
            focus(other)
            navigate(root / "rename-me.txt", burst=True)
            session.capture(f"window-navigation-{attempt:02d}-before-rename.png")
            prompt("Rename", "F2")
            shared = root / f"window-{attempt}.txt"
            commit(shared.name, shared)
            assert not (root / "rename-me.txt").exists()
            assert shared.read_bytes() == b"Original file contents\n"
            focus(browser)
            chord("z", "ctrl")
            session.wait_for(lambda: (root / "rename-me.txt").exists()
                             and not shared.exists(), "Undo from original Browser")
            assert (root / "rename-me.txt").read_bytes() == b"Original file contents\n"
            focus(other)
            chord("w", "ctrl")
            session.wait_for(lambda: not any(w["id"] == other["id"] for w in windows()), "retire fresh Browser")
            checks.append(f"fresh Browser{attempt} selects the file URI, renames the actual file and shares Undo")
        return {"status": "passed", "checks": checks, "scheme": scheme, "scale": scale,
                "scope": "repeated new-window file navigation and Rename after a refused collision; full operation acceptance remains separate"}

    focus(browser)
    navigate(root)
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    time.sleep(.3)
    prompt("New folder", "n", "ctrl", "shift")
    session.capture("operations-new-folder-narrow.png")
    created = root / "Created folder"
    commit(created.name, created)
    checks.append("New Folder opens a floating Iced prompt from a narrow tile and creates the actual directory")

    focus(browser)
    navigate(root / "rename-me.txt")
    prompt("Rename", "F2")
    renamed = root / "renamed.txt"
    commit(renamed.name, renamed)
    assert not (root / "rename-me.txt").exists()
    assert renamed.read_bytes() == b"Original file contents\n"
    checks.append("Rename starts with selected editable text and preserves actual file contents")
    focus(browser)
    chord("z", "ctrl")
    session.wait_for(lambda: (root / "rename-me.txt").exists() and not renamed.exists(), "Undo rename")
    chord("z", "ctrl")
    session.wait_for(lambda: not created.exists(), "Undo folder creation")
    checks.append("shared Ctrl+Z reverses completed Rename and New Folder")

    raw = os.fsencode(root) + b"/raw-\xff.txt"
    identity = os.stat(raw).st_ino
    navigate(raw)
    prompt("Rename", "F2")
    key("Return")
    session.wait_for(lambda: title("Rename") is None, "unchanged raw-name prompt closes")
    assert os.stat(raw).st_ino == identity
    assert not (root / "raw-�.txt").exists()
    checks.append("confirming an unchanged lossy raw filename is a byte-preserving no-op")

    focus(browser)
    prompt("New folder", "n", "ctrl", "shift")
    session.run(["wtype", "invalid/name"])
    key("Return")
    assert title("New folder") is not None
    assert not (root / "invalid").exists()
    session.capture("operations-name-validation.png")
    chord("a", "ctrl")
    fixed = root / "Fixed name"
    commit(fixed.name, fixed)
    focus(browser)
    chord("z", "ctrl")
    session.wait_for(lambda: not fixed.exists(), "Undo corrected folder creation")
    checks.append("basename validation keeps the draft and field focus for correction")

    navigate(root / "rename-me.txt")
    prompt("Rename", "F2")
    session.run(["wtype", "existing.txt"])
    key("Return")
    session.wait_for(lambda: title("Rename") is None, "collision request admitted")
    time.sleep(.4)
    assert (root / "rename-me.txt").read_bytes() == b"Original file contents\n"
    assert (root / "existing.txt").read_bytes() == b"Preserve existing contents\n"
    checks.append("a colliding Rename preserves both files and remains inspectable in Operations")
    focus(browser)
    chord("o", "alt")
    operations = session.wait_for(lambda: title("Operations"), "shared Operations")
    session.capture("operations-results.png")
    key("Escape")
    session.wait_for(lambda: title("Operations") is None, "close Operations")

    focus(browser)
    chord("n", "ctrl")
    other = session.wait_for(lambda: next((w for w in windows() if w.get("app_id") == "org.canopa.Files" and w["id"] != browser["id"]), None), "second Browser")
    focus(other)
    navigate(root / "rename-me.txt")
    prompt("Rename", "F2")
    shared = root / "shared.txt"
    commit(shared.name, shared)
    focus(browser)
    chord("z", "ctrl")
    session.wait_for(lambda: (root / "rename-me.txt").exists() and not shared.exists(), "Undo from another Browser")
    checks.append("one operation history is shared across independently tiled Browser windows")
    session.capture("operations-shared-windows.png")

    def events():
        path = session.output / "notifications.json"
        return json.loads(path.read_text()) if path.exists() else []

    def notice(name, before):
        return session.wait_for(lambda: next((event for event in events()[before:] if name in event["title"]), None), f"public completion notification for {name}")

    def review_folder(name):
        focus(browser)
        navigate(root)
        prompt("New folder", "n", "ctrl", "shift")
        folder = root / name
        commit(name, folder)
        (folder / "later.txt").write_bytes(b"Later work survives Undo\n")
        focus(browser)
        chord("z", "ctrl")
        question = session.wait_for(lambda: title("Undo needs a decision"), "floating Undo question")
        session.wait_for(lambda: any(w["id"] == question["id"] and w["is_focused"] for w in windows()), "Undo default Cancel focus")
        assert question["is_floating"]
        time.sleep(.25)
        return folder, question

    def queued_folder(directory, name):
        focus(browser)
        navigate(directory)
        prompt("New folder", "n", "ctrl", "shift")
        session.run(["wtype", name])
        key("Return")
        session.wait_for(lambda: title("New folder") is None, "queued request durably admitted")
        target = directory / name
        assert not target.exists(), "queued work ran past the waiting Undo"
        return target

    volume_root = Path(session.env["CANOPA_FIXTURE_VOLUMES"])
    volume = volume_root / "Fixture USB"
    focus(browser)
    navigate(volume)
    prompt("New folder", "n", "ctrl", "shift")
    mounted = volume / "Initially available"
    commit(mounted.name, mounted)
    focus(browser)
    chord("z", "ctrl")
    session.wait_for(lambda: not mounted.exists(), "Undo on initial observed volume")
    checks.append("the private GIO provider supplies an initial volume usable by actual Iced operations")

    folder, question = review_folder("Keep later work")
    session.run(["niri", "msg", "action", "set-window-width", "320"])
    session.run(["niri", "msg", "action", "set-window-height", "240"])
    compact = session.wait_for(lambda: next((w for w in windows() if w["id"] == question["id"]
                              and w["layout"]["window_size"][0] <= 330
                              and w["layout"]["window_size"][1] <= 250), None), "minimum Undo question")
    layout = compact["layout"]
    x, y = (sum(pair) for pair in zip(layout["tile_pos_in_workspace_view"], layout["window_offset_in_tile"]))
    width = layout["window_size"][0]
    box = tuple(round(value * scale) for value in (x + 18, y + 53, x + width - 28, y + 95))

    def undo_facts(name):
        current = title("Undo needs a decision")
        assert current and current["id"] == question["id"], "review retired or replaced the Undo question"
        return session.capture(name).convert("RGB").crop(box)

    key("Home")
    start = undo_facts("operations-undo-minimum-start.png")
    key("End")
    session.wait_for(lambda: ImageChops.difference(start, undo_facts("operations-undo-minimum-end.png")).getbbox(),
                     "keyboard review exposes recorded Undo metadata", timeout=3)
    key("Home")
    session.wait_for(lambda: ImageChops.difference(start, undo_facts("operations-undo-minimum-returned.png")).getbbox() is None,
                     "keyboard review returns to the Undo explanation", timeout=3)
    session.run(["niri", "msg", "action", "set-window-width", "640"])
    session.run(["niri", "msg", "action", "set-window-height", "520"])
    session.wait_for(lambda: title("Undo needs a decision")["layout"]["window_size"][0] >= 630
                     and title("Undo needs a decision")["layout"]["window_size"][1] >= 510, "restored Undo question")
    checks.append("Undo facts remain keyboard-readable at minimum size without answering or changing choice focus")
    session.capture("operations-undo-question.png")
    queued = queued_folder(volume, "Queued on original volume")
    before = len(events())
    (volume_root / "unplugged").touch()
    session.wait_for(lambda: "device disconnected" in (volume_root / "events").read_text(), "actual GIO disconnect signal")
    time.sleep(.4)
    focus(question)
    key("Tab")
    key("Return")
    session.wait_for(lambda: title("Undo needs a decision") is None, "Keep item resolves Undo")
    event = notice(queued.name, before)
    assert "unavailable" in event["body"].lower() or "changed" in event["body"].lower(), event
    assert not queued.exists()
    assert (folder / "later.txt").read_bytes() == b"Later work survives Undo\n"
    checks.append("a typed Undo decision preserves later work while an admitted request refuses a disconnected original volume")

    focus(other)
    chord("w", "ctrl")
    session.wait_for(lambda: not any(w["id"] == other["id"] for w in windows()), "close independent Browser")
    folder, question = review_folder("Survives source closure")
    queued = queued_folder(root, "Queued before observer loss")
    focus(browser)
    chord("w", "ctrl")
    session.wait_for(lambda: not any(w["id"] == browser["id"] for w in windows()), "close last Browser")
    operations = session.wait_for(lambda: title("Operations"), "active jobs keep Operations available")
    session.wait_for(lambda: title("Undo needs a decision") is None, "source closure hides only its question")
    assert process.poll() is None
    focus(operations)
    assert not queued.exists(), "source closure answered the independent Undo"
    assert (folder / "later.txt").read_bytes() == b"Later work survives Undo\n"
    session.capture("operations-source-closure-pending.png")
    # A new Operations window starts without a focused control. Traverse New
    # browser, Recovery, queued Cancel, queued Open folder, waiting Cancel,
    # waiting Review. The shared Undo command is disabled while work waits.
    for _ in range(6):
        key("Tab")
    key("Return")
    question = session.wait_for(lambda: title("Undo needs a decision"), "Review restores unanswered Undo after source closure")
    session.capture("operations-source-closure.png")
    checks.append("accepted work and its unanswered Undo survive the last Browser closing and remain reviewable in Operations")

    # Simulate only this application's exact private monitor dying. A pidfd
    # prevents PID reuse between identity checks and signalling.
    candidates = set()
    for task in Path(f"/proc/{process.pid}/task").iterdir():
        try:
            candidates.update(int(pid) for pid in (task / "children").read_text().split())
        except FileNotFoundError:
            pass
    executable = os.stat(f"/proc/{process.pid}/exe")
    helpers = []
    for pid in candidates:
        handle = None
        try:
            handle = os.pidfd_open(pid)
            args = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")[:-1]
            identity = os.stat(f"/proc/{pid}/exe")
            status = Path(f"/proc/{pid}/status").read_text().splitlines()
            parent = int(next(line.split()[1] for line in status if line.startswith("PPid:")))
            if parent == process.pid and len(args) == 2 and args[1] == b"--internal-volume-monitor" and (identity.st_dev, identity.st_ino) == (executable.st_dev, executable.st_ino):
                helpers.append(handle)
                handle = None
        except (FileNotFoundError, ProcessLookupError):
            pass
        finally:
            if handle is not None:
                os.close(handle)
    if len(helpers) != 1:
        for handle in helpers:
            os.close(handle)
    assert len(helpers) == 1, helpers
    handle = helpers[0]
    try:
        before = len(events())
        signal.pidfd_send_signal(handle, signal.SIGKILL)
    finally:
        os.close(handle)
    time.sleep(.5)
    focus(question)
    key("Escape")
    event = notice(queued.name, before)
    assert "unavailable" in event["body"].lower() or "monitor" in event["body"].lower(), event
    assert not queued.exists()
    assert (folder / "later.txt").read_bytes() == b"Later work survives Undo\n"
    focus(operations)
    session.capture("operations-observer-loss.png")
    assert process.poll() is None
    checks.append("losing the exact GIO helper invalidates already queued local work, preserves files and leaves Operations usable")
    return {"status": "passed", "checks": checks, "scheme": scheme, "scale": scale,
            "scope": "Iced name prompts and mutation/Undo foundation; full operation/device/exchange acceptance remains required"}
