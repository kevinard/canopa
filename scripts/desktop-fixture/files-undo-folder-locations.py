"""Changed and unavailable created-folder locations during Undo."""
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
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "folder Undo workflow focus")
        time.sleep(.25)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name]); time.sleep(.25)

    def chord(modifier, key):
        session.run(["wtype", "-M", modifier, "-k", key, "-m", modifier]); time.sleep(.3)

    def events():
        return json.loads(notifications.read_text()) if notifications.exists() else []

    def wait_event(before, prefix):
        return session.wait_for(lambda: next((event for event in events()[before:] if event["body"].startswith(prefix)), None), prefix)

    def navigate(location):
        focus(browser); chord("ctrl", "l")
        session.run(["wtype", str(location)]); keys("Return")
        wait_title(f"{location.name} — Canopa Files")

    pointer = session.build_pointer(); session.run([str(pointer), "1500", "900", "move"])
    root = fixture / "Folder locations"; root.mkdir()

    def create(name):
        navigate(root)
        session.run(["wtype", "-M", "ctrl", "-M", "shift", "-k", "n", "-m", "shift", "-m", "ctrl"])
        wait_title("New folder"); chord("ctrl", "a"); session.run(["wtype", name])
        before = len(events()); keys("Return"); wait_event(before, "Completed")
        path = root / name; assert path.is_dir()
        (path / "work.txt").write_text(f"Original work in {name}.\n")
        return path

    def undo(expected):
        focus(browser); chord("ctrl", "z"); wait_title(expected)

    path = create("changed.txt"); held = root / "changed-original"; path.rename(held)
    path.write_text("The replacement file stays here.\n")
    undo("Created folder changed — Canopa Files"); session.capture("folder-locations-01-replacement-review.png")
    before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    assert path.read_text() == "The replacement file stays here.\n"
    undo("Created folder changed — Canopa Files"); before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
    undo("Created folder changed — Canopa Files"); before = len(events()); chord("alt", "k"); event = wait_event(before, "Completed")
    assert "0 changes undone" in event["body"] and "1 item kept" in event["body"], event
    assert (held / "work.txt").read_text() == "Original work in changed.txt.\n"
    navigate(root); session.run(["wtype", "changed.txt"]); keys("space"); wait_title("changed.txt — Quick Look"); time.sleep(.4)
    session.capture("folder-locations-02-kept-replacement.png"); keys("Escape")

    path = create("retry-folder"); held = root / "retry-original"; path.rename(held)
    undo("Created folder unavailable — Canopa Files"); session.capture("folder-locations-03-missing-review.png")
    before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    undo("Created folder unavailable — Canopa Files"); before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
    saved = root / "retry-work.txt"; (held / "work.txt").rename(saved); held.rename(path)
    focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
    assert not path.exists() and saved.read_text() == "Original work in retry-folder.\n"

    path = create("finish-folder"); held = root / "finished-original"; path.rename(held)
    undo("Created folder unavailable — Canopa Files"); before = len(events()); chord("alt", "f"); event = wait_event(before, "Completed")
    assert "0 changes undone" in event["body"] and "1 folder not removed" in event["body"] and "not returned" not in event["body"], event
    assert not path.exists() and (held / "work.txt").read_text() == "Original work in finish-folder.\n"

    path = create("unreadable-folder"); path.chmod(0)
    try:
        undo("Created folder unavailable — Canopa Files"); session.capture("folder-locations-04-unreadable-review.png")
        before = len(events()); chord("alt", "f"); event = wait_event(before, "Completed")
        assert "0 changes undone" in event["body"] and "1 folder not removed" in event["body"], event
    finally:
        if path.exists(): path.chmod(0o700)
    assert (path / "work.txt").read_text() == "Original work in unreadable-folder.\n"

    path = create("stale.txt"); held = root / "stale-original"; path.rename(held); path.write_text("First replacement.\n")
    undo("Created folder changed — Canopa Files"); path.write_text("Later replacement work.\n")
    before = len(events()); chord("alt", "k"); wait_event(before, "Failed")
    assert path.read_text() == "Later replacement work.\n"
    undo("Created folder changed — Canopa Files"); before = len(events()); chord("alt", "k"); wait_event(before, "Completed")
    assert path.read_text() == "Later replacement work.\n" and (held / "work.txt").read_text() == "Original work in stale.txt.\n"

    path = create("reappeared-folder"); held = root / "reappeared-original"; path.rename(held)
    undo("Created folder unavailable — Canopa Files"); held.rename(path)
    before = len(events()); chord("alt", "f"); wait_event(before, "Failed")
    assert (path / "work.txt").read_text() == "Original work in reappeared-folder.\n"
    undo("Folder not empty — Canopa Files"); before = len(events()); chord("alt", "k"); wait_event(before, "Completed")

    source = root / "Project"; (source / "nested").mkdir(parents=True)
    target = root / "Destination"; target.mkdir()
    navigate(root); session.run(["wtype", "Project"]); chord("ctrl", "c")
    navigate(target); before = len(events()); chord("ctrl", "v"); wait_event(before, "Completed")
    child = target / "Project/nested"; (child / "new.txt").write_text("New nested work.\n")
    held = root / "nested-original"; child.rename(held)
    undo("Created folder unavailable — Canopa Files")
    before = len(events()); chord("alt", "f"); event = wait_event(before, "Partly undone")
    assert "0 changes undone" in event["body"] and "1 folder not removed" in event["body"], event
    assert (target / "Project").is_dir()
    focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
    assert not (target / "Project").exists() and (held / "new.txt").read_text() == "New nested work.\n" and (source / "nested").is_dir()
    session.capture("folder-locations-05-ancestor-retry.png")
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "Gtk-WARNING"]), log
    return {"status":"passed", "scheme":scheme, "scale":scale, "checks":[
        "folders are created through the normal native dialog",
        "replacement review shows the actual current item metadata",
        "Return defaults to Skip for a changed created-folder location",
        "Escape cancels replacement review without consuming history",
        "Keep item reports zero reversed changes and one kept item",
        "the original folder elsewhere and its work stay intact",
        "kept replacement data remains previewable",
        "missing-folder review shows the recorded path and actual error",
        "missing-folder Skip and Cancel preserve its retry",
        "reappeared owned empty folders can be removed normally",
        "Finish reports a folder not removed, separately from moved items not returned",
        "unreadable-folder Finish preserves its contents after access returns",
        "changes during replacement review refuse stale Keep",
        "fresh replacement review preserves the latest version",
        "reappearance during missing-folder review refuses stale Finish",
        "fresh nonempty-folder review keeps the reappeared original",
        "finishing an unavailable nested child leaves its ancestor retryable",
        "ancestor retry removes only the owned empty ancestor",
        "original copy directories and external nested work stay intact",
    ], "windows":windows()}
