"""Native failed-directory retry, Undo and cut-clipboard ownership across filesystems."""
import json
import os
import subprocess
import time


def verify(session, fixture, browser, secondary, scheme, scale):
    notifications = session.output / "notifications.json"
    checkpoints = []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((window for window in windows() if window["title"] == value), None)

    def wait_title(value):
        window = session.wait_for(lambda: title(value) and title(value)["is_focused"] and title(value), value)
        time.sleep(.25)
        return window

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "directory retry browser focus")
        time.sleep(.25)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name]); time.sleep(.25)

    def chord(modifier, key):
        session.run(["wtype", "-M", modifier, "-k", key, "-m", modifier]); time.sleep(.3)

    def open_operations():
        chord("ctrl", "j"); operations = wait_title("Operations — Canopa Files")
        session.run(["niri", "msg", "action", "set-window-width", "360"])
        time.sleep(.5)
        actual = next(item for item in windows() if item["id"] == operations["id"])
        assert actual["layout"]["window_size"][0] == 360, actual
        assert actual["is_focused"], actual

    def navigate(path):
        focus(browser); chord("ctrl", "l")
        session.run(["wtype", str(path)]); keys("Return")
        wait_title(f"{path.name} — Canopa Files")

    def events():
        return json.loads(notifications.read_text()) if notifications.exists() else []

    def wait_event(before, prefix):
        return session.wait_for(lambda: next((event for event in events()[before:] if event["body"].startswith(prefix)), None), prefix)

    def cut_payload():
        try:
            return session.run(["wl-paste", "--type", "x-special/gnome-copied-files"]).strip()
        except subprocess.CalledProcessError:
            return None

    def checkpoint(**values):
        checkpoints.append(values)
        (session.output / "retry-checkpoints.json").write_text(json.dumps(checkpoints, indent=2) + "\n")

    root = fixture / "Directory retry"; root.mkdir()
    target = secondary / "Destination"; target.mkdir()
    filesystems = {"source": root.stat().st_dev, "destination": target.stat().st_dev}
    assert len(set(filesystems.values())) == 2, filesystems

    def failed_move(name):
        parent = root / name; parent.mkdir()
        original = parent / "project"
        (original / "nested").mkdir(parents=True)
        os.mkfifo(original / "nested/blocked.txt")
        (original / "z.txt").write_text("Earlier moved sibling.\n")
        navigate(parent); keys("Home"); chord("ctrl", "x")
        assert original.as_uri() in cut_payload()
        navigate(target); before = len(events()); chord("ctrl", "v")
        wait_event(before, "Completed with errors")
        current = target / "project"
        assert (current / "z.txt").read_text() == "Earlier moved sibling.\n"
        assert original.is_dir() and (original / "nested/blocked.txt").exists()
        assert not (current / "nested/blocked.txt").exists()
        open_operations()
        return original, current

    original, current = failed_move("Original source")
    session.capture("directory-retry-01-failed.png")
    (original / "nested/blocked.txt").unlink()
    (original / "nested/blocked.txt").write_text("Repaired source returned by Undo.\n")
    before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
    time.sleep(.35)
    assert not original.exists(), "Retry must finish the captured source directory cleanup"
    assert (current / "nested/blocked.txt").read_text() == "Repaired source returned by Undo.\n"
    assert (current / "z.txt").read_text() == "Earlier moved sibling.\n"
    session.capture("directory-retry-02-completed.png")
    payload = cut_payload()
    checkpoint(stage="retry completed", original_removed=True, repaired_child=True, earlier_sibling=True, clipboard=payload)
    assert payload is None, "completed directory retry must clear its original cut clipboard selection"
    keys("Escape"); focus(browser)
    before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
    assert (original / "nested/blocked.txt").read_text() == "Repaired source returned by Undo.\n"
    assert (current / "z.txt").read_text() == "Earlier moved sibling.\n"
    navigate(original / "nested"); keys("Home", "space"); wait_title("blocked.txt — Quick Look")
    time.sleep(.4); session.capture("directory-retry-03-returned-preview.png"); keys("Escape")
    focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
    assert not current.exists()
    assert (original / "z.txt").read_text() == "Earlier moved sibling.\n"
    navigate(original); session.capture("directory-retry-04-original-restored.png")

    original, current = failed_move("Unavailable source")
    held = original.with_name("project held while unavailable")
    original.rename(held)
    (held / "nested/blocked.txt").unlink()
    (held / "nested/blocked.txt").write_text("Repaired unavailable source.\n")
    before = len(events()); chord("alt", "r"); wait_event(before, "Completed with errors")
    time.sleep(.35)
    assert (held / "nested/blocked.txt").read_text() == "Repaired unavailable source.\n"
    assert not (current / "nested/blocked.txt").exists()
    payload = cut_payload()
    checkpoint(stage="source unavailable", source_preserved=True, destination_untouched=True, clipboard=payload)
    session.capture("directory-retry-05-source-unavailable.png")
    assert original.as_uri() in (payload or ""), "an unavailable failed source must retain its cut selection for retry"
    held.rename(original)
    before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
    time.sleep(.35)
    assert not original.exists()
    assert (current / "nested/blocked.txt").read_text() == "Repaired unavailable source.\n"
    assert cut_payload() is None
    checkpoint(stage="source returned", original_removed=True, clipboard_cleared=True)
    session.capture("directory-retry-06-source-returned.png")
    keys("Escape"); focus(browser)
    for _ in range(2):
        before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
    assert not current.exists()
    assert (original / "nested/blocked.txt").read_text() == "Repaired unavailable source.\n"
    assert (original / "z.txt").read_text() == "Earlier moved sibling.\n"

    for mode in ["copy", "cut"]:
        original, current = failed_move(f"Source before newer own {mode}")
        (original / "nested/blocked.txt").unlink()
        (original / "nested/blocked.txt").write_text(f"Repaired source before newer {mode}.\n")
        keys("Escape")
        marker_parent = root / f"New {mode} clipboard source"; marker_parent.mkdir()
        marker = marker_parent / "marker.txt"; marker.write_text("Newer clipboard selection.\n")
        navigate(marker_parent); keys("Home"); chord("ctrl", "c" if mode == "copy" else "x")
        expected = cut_payload()
        assert expected.startswith(mode + "\n") and marker.as_uri() in expected
        open_operations()
        before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
        time.sleep(.35)
        assert not original.exists()
        assert (current / "nested/blocked.txt").read_text() == f"Repaired source before newer {mode}.\n"
        assert cut_payload() == expected, f"the old retry must preserve the newer own {mode} selection"
        checkpoint(stage=f"newer own {mode}", original_removed=True, newer_selection_preserved=True)
        session.capture(f"directory-retry-own-{mode}-preserved.png")
        keys("Escape"); focus(browser)
        for _ in range(2):
            before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
        assert not current.exists()
        assert marker.read_text() == "Newer clipboard selection.\n"
        assert cut_payload() == expected

    focus(browser); session.run(["niri", "msg", "action", "set-window-width", "360"]); time.sleep(.5)
    assert next(item for item in windows() if item["id"] == browser["id"])["layout"]["window_size"][0] == 360
    original, current = failed_move("Source with newer clipboard")
    session.capture("directory-retry-07-narrow-failure.png")
    (original / "nested/blocked.txt").unlink()
    (original / "nested/blocked.txt").write_text("Second repaired source.\n")
    session.start(["wl-copy", "--foreground", "--type", "text/plain", "New user clipboard content"], "new-clipboard.log")
    session.wait_for(lambda: session.run(["wl-paste"]).strip() == "New user clipboard content", "new clipboard ownership")
    before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
    time.sleep(.35)
    assert not original.exists()
    assert (current / "nested/blocked.txt").read_text() == "Second repaired source.\n"
    assert session.run(["wl-paste"]).strip() == "New user clipboard content"
    checkpoint(stage="retry with newer clipboard", original_removed=True, newer_clipboard_preserved=True)
    session.capture("directory-retry-08-new-clipboard-preserved.png")
    keys("Escape"); navigate(current / "nested"); keys("Home", "space")
    wait_title("blocked.txt — Quick Look"); time.sleep(.4)
    session.capture("directory-retry-09-narrow-preview.png"); keys("Escape")
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "Gtk-WARNING"]), log
    return {"status": "passed", "scheme": scheme, "scale": scale, "filesystems": filesystems, "checks": [
        "native Cut/Paste records an actual cross-filesystem directory move",
        "an unsupported child fails while its regular sibling moves independently",
        "the failed source child and its original directories remain available",
        "Operations Retry uses the engine's captured directory completion plan",
        "Operations fits a 360-pixel tile and Alt+R retries the same captured work",
        "successful retry removes owned empty source directories",
        "retry preserves the completed sibling and repaired file contents",
        "successful directory retry clears its own original cut selection",
        "Undo of the retry returns the repaired child without returning earlier sibling work",
        "Quick Look previews the returned repaired child",
        "older Undo returns the earlier sibling and removes its empty destination containers",
        "the restored original hierarchy is rendered with the preview closed",
        "an unavailable original source is preserved and receives no destination copy",
        "failed unavailable work retains its cut selection",
        "returning the original source allows the exact retry to finish and clear the selection",
        "Undo preserves source data after an unavailable-source retry",
        "a newer Copy in this application survives the old retry and Undo",
        "a newer Cut in this application survives the old retry and Undo",
        "the same native retry works from a 360-pixel browser tile",
        "a newer external clipboard selection survives completion of the old retry",
        "Quick Look remains useful independently of the narrow browser tile",
    ], "windows": windows()}
