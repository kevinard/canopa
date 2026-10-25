"""Real folder conflicts and keyboard review in a private Iced desktop."""
import json
import os
import time
from urllib.parse import quote_from_bytes
from PIL import ImageChops

BATCH = {
    "First": {"a.txt": b"First incoming A\n", "b.txt": b"First incoming B\n", "c.txt": b"First incoming C\n"},
    "Second": {"a.txt": b"Second incoming A\n", "b.txt": b"Second incoming B\n", "c.txt": b"Second incoming C\n"},
}


def setup(session, fixture, config):
    root = fixture / "Conflict fixtures"
    assert root.is_relative_to(session.runtime / "home")
    incoming = root / "Source/Photos"
    existing = root / "Destination/Photos"
    (incoming / "nested").mkdir(parents=True)
    existing.mkdir(parents=True)
    (incoming / "a.txt").write_bytes(b"Incoming photo notes\n")
    (incoming / "nested/deep.txt").write_bytes(b"Nested incoming notes\n")
    (existing / "a.txt").write_bytes(b"Existing photo notes\n")
    (existing / "kept.txt").write_bytes(b"Destination-only notes\n")
    for folder, entries in BATCH.items():
        source = root / "Batch source" / folder
        source.mkdir(parents=True)
        for name, content in entries.items():
            (source / name).write_bytes(content)
        for mode in ["keep", "replace", "skip"]:
            target = root / f"Batch {mode}" / folder
            target.mkdir(parents=True)
            (target / "a.txt").write_bytes(f"Existing {folder} A\n".encode())
            (target / "b.txt").write_bytes(f"Existing {folder} B\n".encode())
            (target / "kept.txt").write_bytes(b"Destination-only batch notes\n")


def verify(session, fixture, browser, scheme, scale, process, *, question_focus=False, move=False):
    root = fixture / "Conflict fixtures"
    incoming = root / "Source/Photos"
    existing = root / "Destination/Photos"
    operation = "Move" if move else "Copy"
    incoming_ids = {name: path.stat().st_ino for name, path in [("root", incoming), ("notes", incoming / "a.txt"), ("deep", incoming / "nested/deep.txt")]}

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

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
        session.wait_for(lambda: title(path.name), "conflict folder navigation")
        time.sleep(.3)

    def notices(prefix):
        path = session.output / "notifications.json"
        if not path.exists():
            return []
        events = json.loads(path.read_text())
        if prefix == "Undo":
            return [e for e in events if e["body"].split(" ·", 1)[0].endswith(" reversed")]
        return [e for e in events if e["title"].startswith(prefix)]

    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "conflict Browser focus")
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    navigate(incoming.parent)
    chord("a", "ctrl")
    chord("x" if move else "c", "ctrl")
    navigate(existing.parent)
    chord("v", "ctrl")
    question = session.wait_for(lambda: title("Folder conflict"), "actual folder conflict")
    assert question["is_floating"]
    question_id = question["id"]
    session.wait_for(lambda: title("Folder conflict")["is_focused"], "folder default Cancel focus")
    session.capture("conflict-folder-wide.png")
    if question_focus:
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "question parent focus for another Browser")
        chord("n", "ctrl")
        other = session.wait_for(lambda: next((w for w in windows() if w.get("app_id") == "org.canopa.Files"
                                             and w["id"] not in [browser["id"], question_id]), None), "third window for question focus return")
        session.wait_for(lambda: any(w["id"] == other["id"] and w["is_focused"] for w in windows()), "independent Browser focus")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(question_id)])
        session.wait_for(lambda: title("Folder conflict")["is_focused"], "question focus before Cancel")
        session.capture("question-focus-before-close.png")
        chord("Escape")
        session.wait_for(lambda: all(w["id"] != question_id for w in windows()), "question native surface closes")
        session.wait_for(lambda: notices(operation), "explicit question Cancel completion")
        assert (existing / "a.txt").read_bytes() == b"Existing photo notes\n"
        assert (existing / "kept.txt").read_bytes() == b"Destination-only notes\n"
        assert (incoming / "nested/deep.txt").read_bytes() == b"Nested incoming notes\n"
        assert set(existing.parent.iterdir()) == {existing}
        session.capture("question-focus-after-close.png")
        session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()),
                         "closing the question returns its own Browser parent", timeout=3)
        assert any(w["id"] == other["id"] for w in windows())
        return {"status": "passed", "scheme": scheme, "scale": scale,
                "checks": ["explicitly cancelling a question returns its own parent after another Browser was used, preserving both actual folder versions"]}
    session.run(["niri", "msg", "action", "set-window-width", "320"])
    session.run(["niri", "msg", "action", "set-window-height", "240"])
    question = session.wait_for(lambda: next((w for w in windows() if w["id"] == question_id
                               and w["layout"]["window_size"][0] <= 330
                               and w["layout"]["window_size"][1] <= 250), None), "minimum folder question")
    layout = question["layout"]
    x, y = (sum(pair) for pair in zip(layout["tile_pos_in_workspace_view"], layout["window_offset_in_tile"]))
    width = layout["window_size"][0]
    # Only facts: exclude the title, focus outline, action row and other windows.
    box = tuple(round(value * scale) for value in (x + 18, y + 53, x + width - 28, y + 92))

    def facts(name):
        current = title("Folder conflict")
        assert current and current["id"] == question_id, "keyboard review retired or replaced the question"
        return session.capture(name).convert("RGB").crop(box)

    chord("Home")
    top = facts("conflict-folder-minimum-start.png")
    chord("End")
    session.wait_for(lambda: ImageChops.difference(top, facts("conflict-folder-minimum-end.png")).getbbox(),
                     "keyboard review exposes the existing folder metadata", timeout=3)
    chord("Home")
    session.wait_for(lambda: ImageChops.difference(top, facts("conflict-folder-minimum-returned.png")).getbbox() is None,
                     "keyboard review returns to the incoming folder metadata", timeout=3)
    for forward, backward, name in [("Down", "Up", "line"), ("Next", "Prior", "viewport")]:
        chord(forward)
        session.wait_for(lambda: ImageChops.difference(top, facts(f"conflict-folder-{name}-down.png")).getbbox(),
                         f"question {name} scrolling advances", timeout=3)
        chord(backward)
        session.wait_for(lambda: ImageChops.difference(top, facts(f"conflict-folder-{name}-returned.png")).getbbox() is None,
                         f"question {name} scrolling returns", timeout=3)
    # Cancel, Merge, Keep both: reviewing the facts must not move choice focus.
    chord("Tab")
    chord("Tab")
    chord("Return")
    session.wait_for(lambda: title("Folder conflict") is None, "Keep both answers the folder conflict")
    kept = session.wait_for(lambda: next((p for p in existing.parent.glob("Photos*") if p != existing), None), "separate Keep both folder")
    session.wait_for(lambda: notices(operation), "folder Keep both completion")
    assert (kept / "a.txt").read_bytes() == b"Incoming photo notes\n"
    assert (kept / "nested/deep.txt").read_bytes() == b"Nested incoming notes\n"
    assert (existing / "a.txt").read_bytes() == b"Existing photo notes\n"
    assert (existing / "kept.txt").read_bytes() == b"Destination-only notes\n"
    if move:
        assert not incoming.exists()
        assert kept.stat().st_ino == incoming_ids["root"]
        assert (kept / "a.txt").stat().st_ino == incoming_ids["notes"]
        assert (kept / "nested/deep.txt").stat().st_ino == incoming_ids["deep"]
    else:
        assert (incoming / "a.txt").read_bytes() == b"Incoming photo notes\n"
        assert (incoming / "nested/deep.txt").read_bytes() == b"Nested incoming notes\n"
    session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "Browser focus after Keep both")
    session.capture("conflict-folder-kept.png")
    chord("z", "ctrl")
    session.wait_for(lambda: not kept.exists(), "Undo only the copied Keep both folder")
    session.wait_for(lambda: notices("Undo"), "folder Keep both Undo completion")
    assert (existing / "a.txt").read_bytes() == b"Existing photo notes\n"
    assert (existing / "kept.txt").read_bytes() == b"Destination-only notes\n"
    assert (incoming / "a.txt").read_bytes() == b"Incoming photo notes\n"
    assert (incoming / "nested/deep.txt").read_bytes() == b"Nested incoming notes\n"
    if move:
        assert incoming.stat().st_ino == incoming_ids["root"]
        assert (incoming / "a.txt").stat().st_ino == incoming_ids["notes"]
        assert (incoming / "nested/deep.txt").stat().st_ino == incoming_ids["deep"]
    checks = ["a floating folder conflict above a narrow Browser exposes both captured versions through all six scrolling keys at minimum size",
              "Keep both moves the actual nested folder and shared Undo returns its original identities" if move else
              "Keep both creates the real nested folder and shared Undo removes only that copy"]
    for choice in ["cancel", "skip"]:
        if move:
            navigate(incoming.parent)
            chord("a", "ctrl")
            chord("x", "ctrl")
            navigate(existing.parent)
        before = len(notices(operation))
        chord("v", "ctrl")
        session.wait_for(lambda: title("Folder conflict") and title("Folder conflict")["is_focused"], f"folder {choice} question")
        if choice == "skip":
            for _ in range(3):
                chord("Tab")
        chord("Return")
        session.wait_for(lambda: len(notices(operation)) > before, f"folder {choice} completion")
        assert (existing / "a.txt").read_bytes() == b"Existing photo notes\n"
        assert (existing / "kept.txt").read_bytes() == b"Destination-only notes\n"
        assert (incoming / "nested/deep.txt").read_bytes() == b"Nested incoming notes\n"
        assert set(existing.parent.iterdir()) == {existing}
        assert ("cancelled" if choice == "cancel" else "1 skipped") in notices(operation)[-1]["body"]
    checks.append("default folder Cancel and explicit Skip preserve both trees without changing either location")

    def apply_and_choose(name, index, capture):
        question = session.wait_for(lambda: title(name) and title(name)["is_focused"] and title(name), f"{name} choice focus")
        for _ in range(4):
            chord("Tab")
        chord("Return")
        session.capture(capture)
        for _ in range(4 - index):
            chord("Tab", "shift")
        chord("Return")
        return question["id"]

    for mode, index in [("keep", 2), ("replace", 1), ("skip", 3)]:
        destination = root / f"Batch {mode}"
        identities = {folder: (destination / folder).stat().st_ino for folder in BATCH}
        source_ids = {(folder, name): (root / "Batch source" / folder / name).stat().st_ino
                      for folder, entries in BATCH.items() for name in entries}
        navigate(root / "Batch source")
        chord("a", "ctrl")
        chord("x" if move else "c", "ctrl")
        navigate(destination)
        before = len(notices(operation))
        attention = len(notices("Operation needs a decision"))
        chord("v", "ctrl")
        folder_id = apply_and_choose("Folder conflict", 1, f"conflict-{mode}-merge-apply.png")
        file_id = apply_and_choose("File conflict", index, f"conflict-{mode}-file-apply.png")
        assert file_id != folder_id, "the nested file decision reused a stale folder question"
        session.wait_for(lambda: len(notices(operation)) > before, f"batch {mode} copy completion")
        assert title("Folder conflict") is None and title("File conflict") is None
        assert len(notices("Operation needs a decision")) == attention + 2, "batch choices leaked across kinds or repeated a question"
        assert set(p.name for p in destination.iterdir()) == set(BATCH), "a file choice leaked into the second folder conflict"
        for folder, entries in BATCH.items():
            target = destination / folder
            assert target.stat().st_ino == identities[folder], "Merge replaced an existing destination folder"
            for name, content in entries.items():
                original = root / "Batch source" / folder / name
                remains = not move or mode == "skip" and name in ["a.txt", "b.txt"]
                assert original.exists() == remains
                if remains:
                    assert original.read_bytes() == content
                elif move:
                    moved = next(p for p in target.iterdir() if p.is_file() and p.read_bytes() == content)
                    assert moved.stat().st_ino == source_ids[(folder, name)]
            if move:
                assert (root / "Batch source" / folder).exists() == (mode == "skip")
            if mode == "replace":
                assert (target / "a.txt").read_bytes() == entries["a.txt"]
                assert (target / "b.txt").read_bytes() == entries["b.txt"]
            else:
                assert (target / "a.txt").read_bytes() == f"Existing {folder} A\n".encode()
                assert (target / "b.txt").read_bytes() == f"Existing {folder} B\n".encode()
            assert (target / "c.txt").read_bytes() == entries["c.txt"]
            assert (target / "kept.txt").read_bytes() == b"Destination-only batch notes\n"
            # Replaced versions remain in engine-owned directories for Undo.
            # Check the public file payloads without interpreting those stores.
            extra = [p for p in target.iterdir() if p.is_file() and p.name not in ["a.txt", "b.txt", "c.txt", "kept.txt"]]
            assert sorted(p.read_bytes() for p in extra) == (sorted([entries["a.txt"], entries["b.txt"]]) if mode == "keep" else [])
        if mode == "skip":
            assert "4 skipped" in notices(operation)[-1]["body"]
        session.capture(f"conflict-{mode}-completed.png")
        before_undo = len(notices("Undo"))
        chord("z", "ctrl")
        session.wait_for(lambda: len(notices("Undo")) > before_undo, f"Undo batch {mode}")
        if not move:
            assert notices("Undo")[-1]["body"].startswith("2 reversed" if mode == "skip" else "6 reversed")
        for folder in BATCH:
            target = destination / folder
            assert target.stat().st_ino == identities[folder]
            assert set(p.name for p in target.iterdir() if p.is_file()) == {"a.txt", "b.txt", "kept.txt"}
            assert (target / "a.txt").read_bytes() == f"Existing {folder} A\n".encode()
            assert (target / "b.txt").read_bytes() == f"Existing {folder} B\n".encode()
            assert (target / "kept.txt").read_bytes() == b"Destination-only batch notes\n"
            for name, content in BATCH[folder].items():
                original = root / "Batch source" / folder / name
                assert original.read_bytes() == content
                if move:
                    assert original.stat().st_ino == source_ids[(folder, name)]
        checks.append(f"batch folder Merge and file {mode} stay scoped across two real folders, and shared Undo preserves both destination trees")
    assert process.poll() is None
    return {"status": "passed", "scheme": scheme, "scale": scale, "operation": operation, "checks": checks}
