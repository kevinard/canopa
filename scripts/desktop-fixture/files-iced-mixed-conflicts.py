"""Review mixed file/folder collisions after a batch file Replace decision."""
import json
import os
import time
from urllib.parse import quote_from_bytes
from PIL import ImageChops


def setup(session, fixture, config):
    root = fixture / "Mixed conflicts"
    assert root.is_relative_to(session.runtime / "home")
    source, destination = root / "Source", root / "Destination"
    (source / "b-folder").mkdir(parents=True)
    (destination / "c-file").mkdir(parents=True)
    for name in ["a.txt", "c-file", "d.txt"]:
        (source / name).write_bytes(f"Incoming {name}\n".encode())
    (source / "b-folder/child.txt").write_bytes(b"Incoming folder child\n")
    for name in ["a.txt", "b-folder", "d.txt"]:
        (destination / name).write_bytes(f"Previous {name}\n".encode())
    (destination / "c-file/kept.txt").write_bytes(b"Existing folder child\n")


def verify(session, fixture, browser, scheme, scale, process, *, move=False):
    root = fixture / "Mixed conflicts"
    source, destination = root / "Source", root / "Destination"
    operation = "Move" if move else "Copy"
    source_ids = {name: (source / name).stat().st_ino
                  for name in ["a.txt", "b-folder", "b-folder/child.txt", "c-file", "d.txt"]}
    destination_ids = {name: (destination / name).stat().st_ino
                       for name in ["a.txt", "b-folder", "c-file", "c-file/kept.txt", "d.txt"]}

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
        session.wait_for(lambda: title(path.name), "mixed-conflict navigation")
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
    session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "mixed-conflict Browser focus")
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    navigate(source)
    chord("a", "ctrl")
    chord("x" if move else "c", "ctrl")
    navigate(destination)
    attention = len(notices("Operation needs a decision"))
    chord("v", "ctrl")
    first = session.wait_for(lambda: title("File conflict") and title("File conflict")["is_focused"] and title("File conflict"), "ordinary file Replace question")
    session.capture("mixed-first-file-replace.png")
    # Four actions followed by Apply. Apply Replace only to ordinary files.
    for _ in range(4):
        chord("Tab")
    chord("Return")
    for _ in range(3):
        chord("Tab", "shift")
    chord("Return")

    def mixed_question():
        if notices(operation):
            raise AssertionError(f"batch finished without reviewing the file/folder collision: {notices(operation)}")
        current = title("File conflict")
        return current if current and current["is_focused"] and current["id"] != first["id"] else None

    mixed = session.wait_for(mixed_question, "mixed collision requires a fresh decision after batch Replace")
    assert mixed["is_floating"]
    assert (destination / "a.txt").read_bytes() == b"Incoming a.txt\n"
    assert (destination / "b-folder").read_bytes() == b"Previous b-folder\n"
    assert (destination / "c-file/kept.txt").read_bytes() == b"Existing folder child\n"
    session.run(["niri", "msg", "action", "set-window-width", "320"])
    session.run(["niri", "msg", "action", "set-window-height", "240"])
    compact = session.wait_for(lambda: next((w for w in windows() if w["id"] == mixed["id"]
                               and w["layout"]["window_size"][0] <= 330
                               and w["layout"]["window_size"][1] <= 250), None), "minimum mixed conflict")
    layout = compact["layout"]
    x, y = (sum(pair) for pair in zip(layout["tile_pos_in_workspace_view"], layout["window_offset_in_tile"]))
    box = tuple(round(value * scale) for value in (x + 18, y + 53, x + layout["window_size"][0] - 28, y + 92))

    def facts(name):
        assert title("File conflict")["id"] == mixed["id"]
        return session.capture(name).convert("RGB").crop(box)

    chord("Home")
    top = facts("mixed-minimum-start.png")
    chord("End")
    session.wait_for(lambda: ImageChops.difference(top, facts("mixed-minimum-end.png")).getbbox(), "mixed metadata can scroll to both item kinds")
    chord("Home")
    session.wait_for(lambda: ImageChops.difference(top, facts("mixed-minimum-returned.png")).getbbox() is None, "mixed metadata returns without moving choice focus")
    # Cancel, Keep both, Skip, Apply: no Replace or Merge on mixed types.
    for _ in range(3):
        chord("Tab")
    chord("Return")
    session.capture("mixed-keep-both-apply.png")
    for _ in range(2):
        chord("Tab", "shift")
    chord("Return")
    session.wait_for(lambda: notices(operation), "mixed batch completion")
    assert "0 failed" in notices(operation)[-1]["body"] and "0 remaining" in notices(operation)[-1]["body"]
    assert len(notices("Operation needs a decision")) == attention + 2, "batch choices crossed ordinary-file and mixed-type groups"
    kept_folder = next(p for p in destination.glob("b-folder*") if p.is_dir())
    kept_file = next(p for p in destination.glob("c-file*") if p.is_file())
    assert (kept_folder / "child.txt").read_bytes() == b"Incoming folder child\n"
    assert kept_file.read_bytes() == b"Incoming c-file\n"
    for name in ["a.txt", "d.txt"]:
        assert (destination / name).read_bytes() == f"Incoming {name}\n".encode()
    assert (destination / "b-folder").read_bytes() == b"Previous b-folder\n"
    assert (destination / "c-file/kept.txt").read_bytes() == b"Existing folder child\n"
    for name in ["b-folder", "c-file", "c-file/kept.txt"]:
        assert (destination / name).stat().st_ino == destination_ids[name]
    if move:
        assert not any((source / name).exists() for name in ["a.txt", "b-folder", "c-file", "d.txt"])
        for name, actual in [("a.txt", destination / "a.txt"), ("b-folder", kept_folder),
                             ("b-folder/child.txt", kept_folder / "child.txt"), ("c-file", kept_file), ("d.txt", destination / "d.txt")]:
            assert actual.stat().st_ino == source_ids[name]
    else:
        for name, identity in source_ids.items():
            assert (source / name).stat().st_ino == identity
    session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "mixed completion parent focus")
    session.capture("mixed-batch-completed.png")
    chord("z", "ctrl")
    session.wait_for(lambda: notices("Undo"), "shared Undo of mixed batch")
    assert not kept_folder.exists() and not kept_file.exists()
    for name, identity in destination_ids.items():
        assert (destination / name).stat().st_ino == identity
    for name in ["a.txt", "c-file", "d.txt"]:
        assert (source / name).read_bytes() == f"Incoming {name}\n".encode()
    assert (source / "b-folder/child.txt").read_bytes() == b"Incoming folder child\n"
    for name in ["a.txt", "b-folder", "d.txt"]:
        assert (destination / name).read_bytes() == f"Previous {name}\n".encode()
    assert (destination / "c-file/kept.txt").read_bytes() == b"Existing folder child\n"
    if move:
        for name, identity in source_ids.items():
            assert (source / name).stat().st_ino == identity
    assert process.poll() is None
    return {"status": "passed", "scheme": scheme, "scale": scale, "operation": operation,
            "checks": ["batch Replace for ordinary files asks a separate mixed file/folder question with readable metadata and only safe choices",
                       "mixed Keep both stays scoped across both type orientations while ordinary file Replace remains active",
                       f"actual {operation} results and shared Undo preserve both original destination kinds and source payloads"]}
