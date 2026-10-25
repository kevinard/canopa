"""Unavailable saved-version Undo using actual isolated files and native input."""
import json
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
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "saved-version workflow focus")
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

    source = fixture / "Saved source"; target = fixture / "Saved target"
    source.mkdir(); target.mkdir()

    def replace(name, moving):
        original = source / name; current = target / name
        incoming = f"incoming {name}\n"; previous = f"previous {name}\n"
        original.write_text(incoming); current.write_text(previous)
        navigate(source); session.run(["wtype", name]); chord("ctrl", "x" if moving else "c")
        navigate(target); chord("ctrl", "v"); wait_title("File conflict — Canopa Files")
        before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
        assert current.read_text() == incoming and original.exists() != moving
        # Test inputs: move the actual retained file in this private directory.
        saved = next(path for path in target.glob(".canopa-files-stage-*/previous") if path.read_text() == previous)
        held = fixture / f"Held {name}"; saved.rename(held)
        chord("ctrl", "z"); wait_title("Saved version unavailable — Canopa Files")
        return original, current, saved, held, incoming, previous

    original, current, saved, held, incoming, previous = replace("unavailable-copy.txt", False)
    session.capture("saved-01-unavailable-copy.png")
    before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    assert current.read_text() == original.read_text() == incoming
    assert held.read_text() == previous and not saved.exists()
    chord("ctrl", "z"); wait_title("Saved version unavailable — Canopa Files")
    before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
    chord("ctrl", "z"); wait_title("Saved version unavailable — Canopa Files")
    before = len(events()); chord("alt", "f"); finished = wait_event(before, "Completed")
    assert "0 changes undone" in finished["body"] and "1 saved version not restored" in finished["body"]
    assert current.read_text() == original.read_text() == incoming
    held.rename(saved)
    session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
    recovery = wait_title("Recovery — Canopa Files")
    focus(recovery); keys("F5"); time.sleep(.5); keys("Up", "space")
    wait_title("unavailable-copy.txt — Quick Look"); time.sleep(.4)
    session.capture("saved-02-finished-recovery-preview.png"); keys("Escape")
    focus(recovery); chord("ctrl", "r"); wait_title("Recover a copy")
    recovered = fixture / "Recovered unavailable saved version.txt"
    chord("ctrl", "a"); session.run(["wtype", str(recovered)])
    before = len(events()); keys("Return"); session.wait_for(recovered.exists, "protected saved-version recovery copy"); wait_event(before, "Completed")
    assert recovered.read_text() == previous and current.read_text() == incoming

    original, current, saved, held, incoming, previous = replace("unavailable-move.txt", True)
    session.capture("saved-03-return-only-review.png")
    before = len(events()); chord("alt", "r"); returned = wait_event(before, "Partly undone")
    assert "1 change undone" in returned["body"] and "1 saved version not restored" in returned["body"]
    assert original.read_text() == incoming and not current.exists() and held.read_text() == previous
    chord("ctrl", "z"); wait_title("Saved version unavailable — Canopa Files")
    session.capture("saved-04-pending-restoration.png")
    before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
    held.rename(saved)
    chord("ctrl", "z"); wait_title("Saved version changed — Canopa Files")
    before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
    assert current.read_text() == previous and original.read_text() == incoming
    navigate(target); session.run(["wtype", "unavailable-move.txt"]); keys("space")
    wait_title("unavailable-move.txt — Quick Look"); time.sleep(.4)
    session.capture("saved-05-restored-preview.png"); keys("Escape")
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "Gtk-WARNING"]), log
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": [
        "unavailable saved version shows recorded location and lookup reason",
        "Return safely skips unavailable saved version without changing files",
        "Escape cancels and preserves unavailable saved-version Undo",
        "Finish leaves current copy and source in place",
        "Finish reports zero reversals and the saved version not restored",
        "reappearing finished saved version is previewable in protected Recovery",
        "protected saved-version recovery copy preserves current files",
        "available moved item can return without its unavailable saved destination",
        "source-only return reports a reversal and a pending saved version separately",
        "pending saved restoration stays available through another Undo and Cancel",
        "reappearing pending saved version receives explicit current-version approval",
        "completing pending restoration preserves returned source and previews previous destination",
    ], "windows": windows()}
