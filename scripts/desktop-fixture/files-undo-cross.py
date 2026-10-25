"""Original-location conflicts during Undo across two actual filesystems."""
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
        incoming = f"incoming {name}\n"; previous = f"previous {name}\n"; newer = f"newer original {name}\n"
        original.write_text(incoming); current.write_text(previous)
        navigate(source); session.run(["wtype", name]); chord("ctrl", "x")
        navigate(target); chord("ctrl", "v"); wait_title("File conflict — Canopa Files")
        before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
        assert not original.exists() and current.read_text() == incoming
        original.write_text(newer)
        chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
        return original, current, incoming, previous, newer

    original, current, incoming, previous, newer = move("keep.txt")
    session.capture("cross-01-original-conflict.png")
    before = len(events()); chord("alt", "s"); wait_event(before, "Partly undone")
    assert original.read_text() == newer and current.read_text() == incoming
    chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
    assert original.read_text() == newer and current.read_text() == incoming
    chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    returned = source / "keep (2).txt"
    assert original.read_text() == newer and returned.read_text() == incoming and current.read_text() == previous
    navigate(source); keys("Home", "space")
    wait_title("keep (2).txt — Quick Look"); time.sleep(.4)
    session.capture("cross-02-kept-return-preview.png"); keys("Escape")

    original, current, incoming, previous, newer = move("replace.txt")
    before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
    assert original.read_text() == incoming and current.read_text() == previous
    session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
    recovery = wait_title("Recovery — Canopa Files")
    focus(recovery); keys("F5"); time.sleep(.5); keys("Up", "space")
    wait_title("replace.txt — Quick Look"); time.sleep(.4)
    session.capture("cross-03-replaced-newer-preview.png"); keys("Escape")
    focus(recovery); chord("ctrl", "r"); wait_title("Recover a copy")
    recovered = fixture / "Recovered cross-filesystem newer work.txt"
    chord("ctrl", "a"); session.run(["wtype", str(recovered)])
    before = len(events()); keys("Return"); session.wait_for(recovered.exists, "cross-filesystem displaced work recovery"); wait_event(before, "Completed")
    assert recovered.read_text() == newer and original.read_text() == incoming and current.read_text() == previous
    wait_title("Operations — Canopa Files")

    original, current, incoming, previous, newer = move("stale.txt")
    original.write_text("changed during original-location review\n")
    before = len(events()); chord("alt", "r"); wait_event(before, "Failed")
    assert original.read_text() == "changed during original-location review\n" and current.read_text() == incoming
    chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    assert original.read_text() == "changed during original-location review\n"
    assert (source / "stale (2).txt").read_text() == incoming and current.read_text() == previous
    session.capture("cross-04-retried-return.png")
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "Gtk-WARNING"]), log
    return {"status": "passed", "scheme": scheme, "scale": scale, "filesystems": filesystems, "checks": [
        "cut and paste forces a move across different actual filesystem device IDs",
        "cross-filesystem original-location conflict shows both real file versions",
        "Skip preserves destination copy and newer original-location work",
        "Escape cancels the return without consuming its history",
        "Return defaults to Keep both and restores the previous destination",
        "returned file at its new name is previewable",
        "Replace returns incoming data and restores the previous destination",
        "displaced newer original-location work is previewable in protected Recovery",
        "recovering displaced work preserves returned source and previous destination",
        "stale original-location review refuses Replace and keeps both files",
        "fresh review can retry Keep both after the refused return",
    ], "windows": windows()}
