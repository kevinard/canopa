"""Changed and unavailable move destinations across two actual filesystems."""
import json
import time


def verify(session, fixture, browser, secondary, scheme, scale):
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
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "cross-filesystem workflow focus")
        time.sleep(.25)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name]); time.sleep(.25)

    def chord(modifier, key):
        session.run(["wtype", "-M", modifier, "-k", key, "-m", modifier]); time.sleep(.3)

    def events():
        return json.loads(notifications.read_text())

    def wait_event(before, prefix):
        return session.wait_for(lambda: next((event for event in events()[before:] if event["body"].startswith(prefix)), None), prefix)

    def navigate(location):
        focus(browser); chord("ctrl", "l")
        session.run(["wtype", str(location)]); keys("Return")
        wait_title(f"{location.name} — Canopa Files")

    source = fixture / "Cross source"; target = secondary / "Cross target"
    source.mkdir(); target.mkdir()
    filesystems = {"source": source.stat().st_dev, "destination": target.stat().st_dev}
    assert filesystems["source"] != filesystems["destination"], filesystems

    def move(name):
        original = source / name; current = target / name
        incoming = f"incoming {name}\n"; previous = f"previous {name}\n"
        original.write_text(incoming); current.write_text(previous)
        navigate(source); session.run(["wtype", name]); chord("ctrl", "x")
        navigate(target); chord("ctrl", "v"); wait_title("File conflict — Canopa Files")
        before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
        assert not original.exists() and current.read_text() == incoming
        return original, current, incoming, previous

    def undo(expected):
        focus(browser); chord("ctrl", "z"); wait_title(expected)

    original, current, incoming, previous = move("changed.txt")
    current.write_text("destination edits chosen for return\n")
    undo("Moved destination changed — Canopa Files")
    session.capture("cross-current-01-changed-review.png")
    before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    assert not original.exists() and current.read_text() == "destination edits chosen for return\n"
    undo("Moved destination changed — Canopa Files")
    before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
    undo("Moved destination changed — Canopa Files")
    before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
    assert original.read_text() == "destination edits chosen for return\n" and current.read_text() == previous
    navigate(source); keys("Home", "space"); wait_title("changed.txt — Quick Look"); time.sleep(.4)
    session.capture("cross-current-02-returned-preview.png"); keys("Escape")

    original, current, incoming, previous = move("keep.txt")
    current.write_text("destination edits kept here\n")
    undo("Moved destination changed — Canopa Files")
    before = len(events()); chord("alt", "k"); wait_event(before, "Completed")
    assert not original.exists() and current.read_text() == "destination edits kept here\n"
    session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
    recovery = wait_title("Recovery — Canopa Files")
    focus(recovery); keys("F5"); time.sleep(.5); keys("Up", "space")
    wait_title("keep.txt — Quick Look"); time.sleep(.4)
    session.capture("cross-current-03-kept-backup-preview.png"); keys("Escape")
    focus(recovery); chord("ctrl", "r"); wait_title("Recover a copy")
    recovered = fixture / "Recovered previous destination.txt"
    chord("ctrl", "a"); session.run(["wtype", str(recovered)])
    before = len(events()); keys("Return"); session.wait_for(recovered.exists, "kept backup recovery"); wait_event(before, "Completed")
    assert recovered.read_text() == previous and current.read_text() == "destination edits kept here\n"
    wait_title("Operations — Canopa Files")

    original, current, incoming, previous = move("missing.txt")
    external = target / "missing-incoming-elsewhere.txt"; current.rename(external)
    original.write_text("newer original-location work\n")
    undo("Moved destination unavailable — Canopa Files")
    session.capture("cross-current-04-missing-review.png")
    before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    assert not current.exists() and external.read_text() == incoming
    undo("Moved destination unavailable — Canopa Files")
    before = len(events()); chord("alt", "r"); event = wait_event(before, "Completed")
    assert "1 item not returned" in event["body"], event
    assert original.read_text() == "newer original-location work\n" and current.read_text() == previous and external.read_text() == incoming

    original, current, incoming, previous = move("finish.txt")
    external = target / "finished-incoming-elsewhere.txt"; current.rename(external)
    undo("Moved destination unavailable — Canopa Files")
    before = len(events()); chord("alt", "f"); event = wait_event(before, "Completed")
    assert "1 item not returned" in event["body"] and "1 saved version not restored" in event["body"], event
    assert not original.exists() and not current.exists() and external.read_text() == incoming

    original, current, incoming, previous = move("stale.txt")
    current.write_text("reviewed edits\n")
    undo("Moved destination changed — Canopa Files")
    current.write_text("edits made during review\n")
    before = len(events()); chord("alt", "r"); wait_event(before, "Failed")
    assert not original.exists() and current.read_text() == "edits made during review\n"
    undo("Moved destination changed — Canopa Files")
    before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
    assert original.read_text() == "edits made during review\n" and current.read_text() == previous
    session.capture("cross-current-05-retried-return.png")
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "Gtk-WARNING"]), log
    return {"status": "passed", "scheme": scheme, "scale": scale, "filesystems": filesystems, "checks": [
        "cut and paste moves across different actual filesystem device IDs",
        "changed destination review shows current version and original location",
        "Return defaults to Skip and preserves both move steps",
        "Escape cancels a changed-version return",
        "Return this version restores edited data and its saved previous destination",
        "returned destination edits are previewable",
        "Keep item preserves destination edits and finishes both move steps",
        "saved predecessor after Keep is previewable in protected Recovery",
        "Recovery copy preserves kept edits and the saved predecessor",
        "missing destination review shows its saved previous version",
        "Return defaults to Skip for a missing destination",
        "saved-only restoration preserves unrelated original work and external incoming data",
        "saved-only outcome reports that the incoming item was not returned",
        "Finish preserves current files and reports the unrestored predecessor",
        "changed destination during review refuses a stale return",
        "fresh review retries and returns the latest selected version",
    ], "windows": windows()}
