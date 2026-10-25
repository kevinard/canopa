"""Native reviewed-move Undo against actual files in the private desktop."""
import json
import shutil
import time


def verify(session, fixture, browser, scheme, scale):
    notifications = session.output / "notifications.json"

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((window for window in windows() if window["title"] == value), None)

    def wait_title(value):
        window = session.wait_for(lambda: title(value), value)
        session.wait_for(lambda: title(value) and title(value)["is_focused"], f"focus of {value}")
        time.sleep(.25)
        return window

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "move workflow focus")
        time.sleep(.25)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name]); time.sleep(.25)

    def chord(modifier, key):
        session.run(["wtype", "-M", modifier, "-k", key, "-m", modifier]); time.sleep(.3)

    def events():
        return json.loads(notifications.read_text())

    def completed(before):
        return any(event["body"].startswith("Completed") for event in events()[before:])

    source = fixture / "Moved source"
    target = fixture / "Moved target"
    source.mkdir(); target.mkdir()

    def navigate(location):
        focus(browser); chord("ctrl", "l")
        session.run(["wtype", str(location)]); keys("Return")
        wait_title(f"{location.name} — Canopa Files")

    def move_replacing(name, incoming, previous, replacement=None, collision=False):
        original = source / name
        current = target / name
        external = fixture / f"Original moved {name}"
        original.write_text(incoming); current.write_text(previous)
        navigate(source); session.run(["wtype", name]); chord("ctrl", "x")
        navigate(target); chord("ctrl", "v"); wait_title("File conflict — Canopa Files")
        before = len(events()); chord("alt", "r")
        session.wait_for(lambda: completed(before), "completed replacement move")
        assert not original.exists() and current.read_text() == incoming
        current.rename(external)
        if replacement is not None: current.write_text(replacement)
        if collision: original.write_text("newer original-location work\n")
        time.sleep(.4); chord("ctrl", "z")
        wait_title("Moved item unavailable — Canopa Files" if replacement is None else "Moved item changed — Canopa Files")
        return original, current, external

    original, current, external = move_replacing(
        "return.txt", "original moving document\n", "previous return destination\n", "selected replacement to return\n")
    session.capture("moves-01-current-review.png")
    before = len(events()); keys("Return")
    session.wait_for(lambda: any(event["body"].startswith("Partly undone") for event in events()[before:]), "safe Skip default for replaced move")
    assert not original.exists() and current.read_text() == "selected replacement to return\n"
    chord("ctrl", "z"); wait_title("Moved item changed — Canopa Files")
    before = len(events()); keys("Escape")
    session.wait_for(lambda: any(event["body"].startswith("Cancelled") for event in events()[before:]), "cancel replaced move Undo")
    assert not original.exists() and current.read_text() == "selected replacement to return\n"
    chord("ctrl", "z"); wait_title("Moved item changed — Canopa Files")
    before = len(events()); chord("alt", "r")
    session.wait_for(lambda: completed(before), "explicit return of reviewed item")
    assert original.read_text() == "selected replacement to return\n"
    assert current.read_text() == "previous return destination\n"
    assert external.read_text() == "original moving document\n"
    navigate(source); session.run(["wtype", "return.txt"]); keys("space")
    wait_title("return.txt — Quick Look"); time.sleep(.4)
    session.capture("moves-02-returned-preview.png"); keys("Escape")

    original, current, external = move_replacing(
        "keep.txt", "second original moving document\n", "previous kept destination\n", "replacement kept at target\n")
    before = len(events()); chord("alt", "k")
    session.wait_for(lambda: completed(before), "keep reviewed replacement")
    assert any("1 item kept" in event["body"] for event in events()[before:])
    assert not original.exists() and current.read_text() == "replacement kept at target\n"
    assert external.read_text() == "second original moving document\n"
    session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
    recovery = wait_title("Recovery — Canopa Files")
    focus(recovery); keys("F5"); time.sleep(.5)
    keys("Up", "space"); wait_title("keep.txt — Quick Look"); time.sleep(.4)
    session.capture("moves-03-kept-previous-preview.png"); keys("Escape")
    focus(recovery); chord("ctrl", "r"); wait_title("Recover a copy")
    recovered = fixture / "Recovered kept previous destination.txt"
    chord("ctrl", "a"); session.run(["wtype", str(recovered)])
    before = len(events()); keys("Return")
    session.wait_for(recovered.exists, "recover previous destination preserved by Keep item")
    session.wait_for(lambda: completed(before), "completed kept-version recovery copy")
    assert recovered.read_text() == "previous kept destination\n"
    assert current.read_text() == "replacement kept at target\n"

    original, current, external = move_replacing(
        "collision.txt", "third original moving document\n", "previous collision destination\n", "replacement returned with a new name\n", collision=True)
    chord("alt", "r"); wait_title("Undo conflict — Canopa Files")
    session.capture("moves-04-original-location-conflict.png")
    before = len(events()); keys("Return")
    session.wait_for(lambda: completed(before), "Keep both after reviewed return")
    assert original.read_text() == "newer original-location work\n"
    assert (source / "collision (2).txt").read_text() == "replacement returned with a new name\n"
    assert current.read_text() == "previous collision destination\n"
    assert external.read_text() == "third original moving document\n"
    original, current, external = move_replacing(
        "unavailable-restore.txt", "missing incoming document\n", "saved unavailable destination\n")
    session.capture("moves-05-unavailable-review.png")
    before = len(events()); keys("Return")
    session.wait_for(lambda: any(event["body"].startswith("Partly undone") for event in events()[before:]), "safe Skip default for unavailable move")
    assert not original.exists() and not current.exists()
    assert external.read_text() == "missing incoming document\n"
    chord("ctrl", "z"); wait_title("Moved item unavailable — Canopa Files")
    before = len(events()); keys("Escape")
    session.wait_for(lambda: any(event["body"].startswith("Cancelled") for event in events()[before:]), "cancel unavailable move review")
    assert not original.exists() and not current.exists()
    chord("ctrl", "z"); wait_title("Moved item unavailable — Canopa Files")
    before = len(events()); chord("alt", "r")
    session.wait_for(lambda: completed(before), "restore only the saved previous destination")
    assert any("1 item not returned" in event["body"] for event in events()[before:])
    assert not original.exists() and current.read_text() == "saved unavailable destination\n"
    assert external.read_text() == "missing incoming document\n"
    navigate(target); session.run(["wtype", "unavailable-restore.txt"]); keys("space")
    wait_title("unavailable-restore.txt — Quick Look"); time.sleep(.4)
    session.capture("moves-06-saved-only-preview.png"); keys("Escape")

    original, current, external = move_replacing(
        "unavailable-finish.txt", "another missing incoming document\n", "previous destination protected by Finish\n")
    before = len(events()); chord("alt", "f")
    session.wait_for(lambda: completed(before), "finish unavailable step without restoration")
    assert any("0 changes undone" in event["body"] and "1 item not returned" in event["body"] for event in events()[before:])
    assert not original.exists() and not current.exists()
    assert external.read_text() == "another missing incoming document\n"
    session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
    recovery = wait_title("Recovery — Canopa Files")
    focus(recovery); keys("F5"); time.sleep(.5)
    keys("Up", "Up", "Up", "Down", "space")
    wait_title("unavailable-finish.txt — Quick Look"); time.sleep(.4)
    session.capture("moves-07-finished-unavailable-preview.png"); keys("Escape")
    focus(recovery); chord("ctrl", "r"); wait_title("Recover a copy")
    recovered = fixture / "Recovered unavailable previous destination.txt"
    chord("ctrl", "a"); session.run(["wtype", str(recovered)])
    before = len(events()); keys("Return")
    session.wait_for(recovered.exists, "recover previous destination preserved by Finish this step")
    session.wait_for(lambda: completed(before), "completed unavailable-version recovery copy")
    assert recovered.read_text() == "previous destination protected by Finish\n"
    assert external.read_text() == "another missing incoming document\n"
    assert not original.exists() and not current.exists()

    original, current, external = move_replacing(
        "unavailable-retry.txt", "incoming that will reappear\n", "previous retry destination\n")
    before = len(events()); keys("Return")
    session.wait_for(lambda: any(event["body"].startswith("Partly undone") for event in events()[before:]), "skip before moved item reappears")
    external.rename(current)
    before = len(events()); chord("ctrl", "z")
    session.wait_for(lambda: completed(before), "ordinary Undo after item reappears")
    assert original.read_text() == "incoming that will reappear\n"
    assert current.read_text() == "previous retry destination\n"
    assert not external.exists()
    older = fixture / "Ownership copy source"
    older.mkdir()
    for return_incoming in (True, False):
        name = "ownership-return.txt" if return_incoming else "ownership-saved-only.txt"
        (older / name).write_text("identical ownership document\n")
        original = source / name
        current = target / name
        external = fixture / f"Ownership incoming elsewhere {name}"
        navigate(older); session.run(["wtype", name]); chord("ctrl", "c")
        navigate(target); before = len(events()); chord("ctrl", "v")
        session.wait_for(lambda: completed(before), "copy before external move")
        current.rename(original)
        shutil.copy2(original, current)
        navigate(source); session.run(["wtype", name]); chord("ctrl", "x")
        navigate(target); chord("ctrl", "v"); wait_title("File conflict — Canopa Files")
        before = len(events()); chord("alt", "r")
        session.wait_for(lambda: completed(before), "replace independent destination")
        if return_incoming:
            before = len(events()); chord("ctrl", "z")
            session.wait_for(lambda: completed(before), "return incoming and restore independent destination")
            original.rename(external)
        else:
            current.rename(external)
            chord("ctrl", "z"); wait_title("Moved item unavailable — Canopa Files")
            before = len(events()); chord("alt", "r")
            session.wait_for(lambda: completed(before), "restore only independent destination")
        assert not original.exists()
        assert current.read_text() == external.read_text() == "identical ownership document\n"
        chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
        session.capture(f"moves-ownership-{'returned' if return_incoming else 'saved-only'}.png")
        before = len(events()); chord("alt", "s")
        session.wait_for(lambda: any(event["body"].startswith("Partly undone") for event in events()[before:]), "Skip foreign copy after ownership review")
        assert current.read_text() == external.read_text() == (older / name).read_text()
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "Gtk-WARNING"]), log
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": [
        "changed-move current metadata and original location",
        "Return safely skips replaced move and preserves Undo",
        "Escape cancels replaced-move Undo without moving files",
        "Return this item restores the explicitly reviewed replacement",
        "returning a replacement restores the saved previous destination",
        "externally moved original item remains untouched",
        "returned replacement is previewable",
        "Keep item leaves replacement in place and reports it separately",
        "Keep item preserves the previous destination in previewable Recovery",
        "recovering the kept previous destination preserves the current replacement",
        "returning a replacement asks separately about its occupied original location",
        "Keep both preserves newer original-location work and the previous destination",
        "unavailable move shows recorded path, lookup error and available saved metadata",
        "Return safely skips unavailable move and preserves Undo",
        "Escape cancels unavailable move without restoring files",
        "Restore previous destination restores only the explicitly shown saved version",
        "unreturned unavailable items are reported separately from actual reversals",
        "saved-only restoration is previewable",
        "Finish this step leaves the unavailable moved item untouched",
        "Finish protects the saved destination for native preview and recovery copy",
        "reappearing moved item remains normally undoable after Skip",
        "older Copy reviews an independent destination after normal move Undo",
        "older Copy reviews an independent destination after saved-only move Undo",
        "Skip preserves both independent destinations and external incoming items",
    ], "windows": windows()}
