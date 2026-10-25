"""Nonempty created-folder Undo and nested container choices."""
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
    root = fixture / "Folder Undo"; root.mkdir()

    def create(name):
        navigate(root)
        session.run(["wtype", "-M", "ctrl", "-M", "shift", "-k", "n", "-m", "shift", "-m", "ctrl"])
        wait_title("New folder"); chord("ctrl", "a"); session.run(["wtype", name])
        before = len(events()); keys("Return"); wait_event(before, "Completed")
        folder = root / name; assert folder.is_dir()
        return folder

    def undo():
        focus(browser); chord("ctrl", "z"); wait_title("Folder not empty — Canopa Files")

    folder = create("Keep folder"); added = folder / "new.txt"; added.write_text("New work added after creating the folder.\n")
    undo(); session.capture("folders-01-nonempty-review.png")
    before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    assert added.read_text() == "New work added after creating the folder.\n"
    undo(); before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
    undo(); before = len(events()); chord("alt", "k"); event = wait_event(before, "Completed")
    assert "1 item kept" in event["body"], event
    navigate(folder); keys("Home", "space"); wait_title("new.txt — Quick Look"); time.sleep(.4)
    session.capture("folders-02-kept-new-work.png"); keys("Escape")

    folder = create("Retry folder"); added = folder / "new.txt"; added.write_text("Work moved aside before retry.\n")
    undo(); before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    held = root / "new work elsewhere.txt"; added.rename(held)
    focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
    assert not folder.exists() and held.read_text() == "Work moved aside before retry.\n"

    folder = create("Stale folder"); (folder / "first.txt").write_text("First new work.\n")
    undo(); (folder / "later.txt").write_text("Work added during review.\n")
    before = len(events()); chord("alt", "k"); wait_event(before, "Failed")
    assert (folder / "later.txt").read_text() == "Work added during review.\n"
    undo(); before = len(events()); chord("alt", "k"); wait_event(before, "Completed")
    assert (folder / "first.txt").read_text() == "First new work.\n"

    source = root / "Incoming"; (source / "nested").mkdir(parents=True)
    (source / "nested/copied.txt").write_text("Original copied source.\n")
    target = root / "Destination"; target.mkdir()
    navigate(root); session.run(["wtype", "Incoming"]); chord("ctrl", "c")
    navigate(target); before = len(events()); chord("ctrl", "v"); wait_event(before, "Completed")
    copied = target / "Incoming/nested/copied.txt"; assert copied.read_text() == "Original copied source.\n"
    added = copied.parent / "new.txt"; added.write_text("New nested work stays here.\n")
    undo(); session.capture("folders-03-nested-review.png")
    before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    assert not copied.exists() and added.read_text() == "New nested work stays here.\n"
    undo(); before = len(events()); chord("alt", "k"); event = wait_event(before, "Completed")
    assert "2 items kept" in event["body"], event
    assert added.read_text() == "New nested work stays here.\n" and (source / "nested/copied.txt").read_text() == "Original copied source.\n"
    session.capture("folders-04-kept-containers.png")
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "Gtk-WARNING"]), log
    return {"status":"passed", "scheme":scheme, "scale":scale, "checks":[
        "new folder is created through its normal native dialog",
        "nonempty folder Undo shows its actual folder metadata and explicit choices",
        "Return defaults to Skip and preserves new contents",
        "Escape cancels without consuming the created-folder step",
        "Keep folder leaves new work in place and reports one kept item",
        "new work remains previewable after Keep folder",
        "Skip can be retried after contents are moved elsewhere",
        "an owned empty folder is then removed without a conflict",
        "a changed folder during review refuses the stale Keep decision",
        "a fresh review keeps the latest folder and contents",
        "copy Undo removes completed copied leaves before reviewing remaining new work",
        "Skip preserves created ancestor containers",
        "Keep consumes nested container steps together and reports both kept folders",
        "original copy sources and new nested contents remain intact",
    ], "windows":windows()}
