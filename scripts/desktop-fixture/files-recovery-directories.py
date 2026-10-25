"""Native directory Recovery copy, cancellation, exact continuation and Undo."""
import json
import threading
import time


def verify(session, fixture, browser, scheme, scale, expiry=False, tiling=False):
    notifications = session.output / "notifications.json"

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
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "directory Recovery focus")
        time.sleep(.25)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name]); time.sleep(.25)

    def chord(modifier, key):
        session.run(["wtype", "-M", modifier, "-k", key, "-m", modifier]); time.sleep(.3)

    def events():
        return json.loads(notifications.read_text()) if notifications.exists() else []

    def wait_event(before, prefix="Completed"):
        return session.wait_for(lambda: next((event for event in events()[before:] if event["body"].startswith(prefix)), None), prefix)

    def navigate(path):
        focus(browser); chord("ctrl", "l")
        session.run(["wtype", str(path)]); keys("Return")
        wait_title(f"{path.name} — Canopa Files")

    note_name = ("b" + "long" * 32 + ".txt") if tiling else "b-note.txt"
    copy_name = note_name[:-4] + " (2).txt"
    root = fixture / "Directory Recovery"; root.mkdir()
    seed = root / "Seed"; seed.mkdir()
    target = root / "Target"; target.mkdir()
    source = seed / "Saved report.txt"; source.write_text("Incoming replacement.\n")
    current = target / source.name; current.write_text("Previous saved file.\n")
    navigate(seed); keys("Home"); chord("ctrl", "c")
    navigate(target); chord("ctrl", "v"); wait_title("File conflict — Canopa Files")
    before = len(events()); chord("alt", "r"); wait_event(before)
    assert current.read_text() == "Incoming replacement.\n"
    saved = next(target.glob(".canopa-files-stage-*/previous"))
    held = root / "Held original saved file.txt"; saved.rename(held)
    saved.mkdir()
    # A substantial first child leaves an observable interval for the independent
    # user output arrival. Exclusive creation below can never overwrite a copy.
    (saved / "a-padding.txt").write_bytes(b"a" * (16 * 1024 * 1024))
    (saved / note_name).write_text("Recovered directory document.\n")
    (saved / "nested").mkdir()
    (saved / "nested/c-note.txt").write_text("Nested recovered document.\n")
    (saved / "nested").chmod(0o555)
    saved.chmod(0o555)
    session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
    recovery = session.wait_for(lambda: title("Recovery — Canopa Files"), "Recovery window")
    focus(recovery); keys("F5"); time.sleep(.5); keys("Up")
    session.run(["niri", "msg", "action", "set-window-width", "360"]); time.sleep(.4)
    session.capture("directory-recovery-01-protected-folder.png")
    chord("ctrl", "r"); wait_title("Recover a copy")
    recovered = root / "Recovered directory"
    chord("ctrl", "a"); session.run(["wtype", str(recovered)])
    arrival = []

    def incoming_work():
        deadline = time.monotonic() + 20
        while True:
            if time.monotonic() >= deadline:
                arrival.append("No writable copy interval for independent output arrival"); return
            try:
                with (recovered / note_name).open("x") as document:
                    document.write("Newer output document.\n")
                arrival.append("created"); return
            except (FileNotFoundError, PermissionError):
                # A newly copied read-only root grants temporary owner access
                # only while a child is copied. Wait for the substantial first
                # child; never chmod or overwrite the application's output.
                time.sleep(.001)
            except Exception as error:
                arrival.append(str(error)); return

    thread = threading.Thread(target=incoming_work, daemon=True); thread.start()
    keys("Return"); wait_title("File conflict — Canopa Files")
    thread.join(timeout=1); assert arrival == ["created"], arrival
    session.capture("directory-recovery-02-child-conflict.png")
    before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
    assert (recovered / "a-padding.txt").stat().st_size == 16 * 1024 * 1024
    assert (recovered / note_name).read_text() == "Newer output document.\n"
    assert not (recovered / "nested").exists()
    assert recovered.stat().st_mode & 0o777 == 0o555
    focus(browser); chord("ctrl", "j"); operations = wait_title("Operations — Canopa Files")
    if tiling:
        for width in (360, 640, 360):
            session.run(["niri", "msg", "action", "set-window-width", str(width)])
            time.sleep(.5)
            session.capture(f"operations-width-{width}.png")
            actual = next(item for item in windows() if item["id"] == operations["id"])
            assert actual["layout"]["window_size"][0] == width, actual
            assert actual["is_focused"], actual
    session.capture("directory-recovery-03-cancelled-prefix.png")
    held_output = root / "Held Recovery output"
    recovered.rename(held_output)
    recovered.mkdir(); (recovered / "new.txt").write_text("Unrelated replacement folder.\n")
    before = len(events()); chord("alt", "c")
    refused = session.wait_for(lambda: next((event for event in events()[before:]
        if event["body"].startswith(("Paused", "Failed"))), None), "refused output continuation")
    session.capture("directory-recovery-03a-output-refused.png")
    assert refused["body"].startswith("Paused"), refused
    assert list(recovered.iterdir()) == [recovered / "new.txt"]
    assert (held_output / "a-padding.txt").stat().st_size == 16 * 1024 * 1024
    unrelated = root / "Unrelated output folder"; recovered.rename(unrelated)
    held_output.rename(recovered)
    if expiry:
        navigate(root)
        for ordinal in range(105):
            session.wait_for(lambda: any(item["id"] == browser["id"] and item["is_focused"] for item in windows()), "browser after history folder request")
            before = len(events())
            session.run(["wtype", "-M", "ctrl", "-M", "shift", "-k", "n", "-m", "shift", "-m", "ctrl"])
            session.wait_for(lambda: title("New folder") and title("New folder")["is_focused"], "native history folder prompt")
            session.run(["wtype", f"History work {ordinal:03}", "-k", "Return"])
            wait_event(before)
            assert (root / f"History work {ordinal:03}").is_dir()
            if ordinal in (0, 24, 49, 74, 99, 104):
                (session.output / "history-checkpoint.json").write_text(json.dumps({"native_folders_created": ordinal + 1}) + "\n")
        # A known containing-folder Rename also invalidates the captured path.
        # Resume must resolve the same saved bytes through catalog ownership.
        focus(browser); session.run(["wtype", "Target"]); keys("F2"); wait_title("Rename")
        session.run(["wtype", "Target moved"])
        before = len(events()); keys("Return"); wait_event(before)
        moved = root / "Target moved"
        saved = moved / saved.relative_to(target)
        current = moved / current.relative_to(target)
        assert not target.exists() and saved.is_dir()
        focus(operations); session.capture("directory-recovery-03b-expired-before-continue.png")
    focus(operations); chord("alt", "c"); wait_title("File conflict — Canopa Files")
    if tiling:
        chord("alt", "d")
        session.wait_for(lambda: not title("File conflict — Canopa Files"), "decision put aside")
        focus(operations); session.capture("operations-narrow-review.png")
        chord("alt", "v"); wait_title("File conflict — Canopa Files")
    session.capture("directory-recovery-04-continued-conflict.png")
    before = len(events()); keys("Return"); completed = wait_event(before)
    assert completed["title"] == "Resume: Recover Saved report.txt", completed
    assert (recovered / copy_name).read_text() == "Recovered directory document.\n"
    assert (recovered / "nested/c-note.txt").read_text() == "Nested recovered document.\n"
    assert (recovered / note_name).read_text() == "Newer output document.\n"
    assert recovered.stat().st_mode & 0o777 == 0o555
    assert (recovered / "nested").stat().st_mode & 0o777 == 0o555
    focus(operations); session.capture("directory-recovery-05-continued-result.png")
    navigate(recovered)
    session.run(["niri", "msg", "action", "set-window-width", "360"]); time.sleep(.4)
    assert title("Recovered directory — Canopa Files")["layout"]["window_size"][0] == 360
    # Space invokes Quick Look, including while typing a filename. Choose the
    # recovered row with normal directory-first keyboard navigation instead.
    keys("Home", "Down", "Down", "space"); wait_title(f"{copy_name} — Quick Look")
    time.sleep(.4); session.capture("directory-recovery-06-narrow-preview.png"); keys("Escape")
    focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before)
    assert not (recovered / "nested").exists() and not (recovered / copy_name).exists()
    assert (recovered / "a-padding.txt").exists()
    before = len(events()); chord("ctrl", "z")
    if expiry:
        # The original copy's Undo has legitimately expired too. The next Undo
        # returns the containing-folder Rename, preserving the old prefix.
        wait_event(before)
        saved = target / saved.relative_to(moved)
        current = target / current.relative_to(moved)
        assert target.is_dir() and not moved.exists()
        assert (recovered / "a-padding.txt").stat().st_size == 16 * 1024 * 1024
        session.capture("directory-recovery-07-expired-prefix-preserved.png")
    else:
        wait_title("Folder not empty — Canopa Files")
        session.capture("directory-recovery-07-undo-preserves-new-work.png")
        chord("alt", "k"); wait_event(before)
        assert list(recovered.iterdir()) == [recovered / note_name]
    assert (recovered / note_name).read_text() == "Newer output document.\n"
    assert (saved / note_name).read_text() == "Recovered directory document.\n"
    assert (unrelated / "new.txt").read_text() == "Unrelated replacement folder.\n"
    assert current.read_text() == "Incoming replacement.\n" and held.read_text() == "Previous saved file.\n"
    assert recovered.stat().st_mode & 0o777 == 0o555
    assert saved.stat().st_mode & 0o777 == 0o555
    assert (saved / "nested").stat().st_mode & 0o777 == 0o555
    focus(recovery); keys("F5"); time.sleep(.5); keys("Up")
    session.capture("directory-recovery-08-source-still-available.png")
    for path in session.output.glob("files*.log"):
        log = path.read_text()
        assert not any(marker in log for marker in ("panicked", "CRITICAL", "Gtk-WARNING")), log
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": [
        "native Copy Replace retains an actual old file",
        "unexpected saved directory stays protected and enables Recovery Copy",
        "Recovery copy chooser creates a new folder from a 360-pixel tile",
        "new output child triggers a Recovery conflict without Replace",
        "Escape cancels with the completed prefix and newer output intact",
        "a replaced output folder pauses Continue without adopting unrelated work",
        "returning the original output folder leaves Continue available",
        "Operations Continue resumes the exact remaining Recovery work",
        "repeated continuation keeps a stable operation title",
        "Return uses Keep both and preserves newer output bytes",
        "nested recovered files complete without consuming the saved directory",
        "floating Quick Look previews a recovered file beside a narrow browser",
        "first Undo removes only the continuation's completed files",
        ("Undo of the containing-folder Rename preserves the expired copy prefix" if expiry else
         "older Undo offers a nonempty-folder review and preserves newer work"),
        "source directory, current replacement and held old file remain intact",
        "GTK logs have no warnings, criticals or panics",
        "cancelled Recovery copies retain the source folder's read-only permissions",
        "Continue preserves read-only root and nested folder modes",
        "Undo preserves read-only modes on kept output and the protected source",
    ] + ([
        "105 actual native folder operations expire the original saved-record key",
        "unfinished Continue survives pruning of completed Operations rows",
        "containing-folder Rename is followed before storage preflight and remaining copy",
    ] if expiry else []) + ([
        "Operations fits360 pixels with Continue and Undo both available",
        "unbroken long filenames wrap while preserving exact copy names",
        "widening then narrowing preserves operation focus and continuation",
        "Alt+V reviews a deferred decision from narrow Operations",
    ] if tiling else []), "windows": windows()}
