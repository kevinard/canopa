"""Directory returns across filesystems, using a location change or a native Move."""
import json
import time


def verify(session, fixture, browser, secondary, scheme, scale, directory_moves=False):
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
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "cross-filesystem folder workflow focus")
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

    root = fixture / "Cross folder returns"; root.mkdir()
    seed = root / "Seed"; seed.mkdir(); (seed / "a.txt").write_text("Earlier copied child.\n")
    moved = (secondary if directory_moves else root) / "Moved folders"; moved.mkdir()
    filesystems = ({"original": root.stat().st_dev, "current": moved.stat().st_dev} if directory_moves
                   else {"incoming": moved.stat().st_dev, "destination": secondary.stat().st_dev})
    assert len(set(filesystems.values())) == 2, filesystems

    def copy_seed(destination):
        navigate(seed); session.run(["wtype", "a.txt"]); chord("ctrl", "c")
        navigate(destination); before = len(events()); chord("ctrl", "v"); wait_event(before, "Completed")
        assert (destination / "a.txt").read_text() == "Earlier copied child.\n"

    def move_project(parent):
        navigate(parent); session.run(["wtype", "project"]); chord("ctrl", "x")
        navigate(moved); before = len(events()); chord("ctrl", "v"); wait_event(before, "Completed")
        current = moved / "project"
        assert current.is_dir() and not (parent / "project").exists()
        if directory_moves:
            return current
        # The native move itself is within one filesystem. Replacing only this
        # private, now-empty location makes its explicit Undo Merge cross devices.
        foreign = secondary / parent.name; foreign.mkdir()
        parent.rmdir(); parent.symlink_to(foreign, target_is_directory=True)
        return current

    parent = root / "Original tree"; parent.mkdir(); original = parent / "project"
    (original / "b-nested/deep").mkdir(parents=True); (original / "a-empty").mkdir()
    (original / "b-nested/deep/note.txt").write_text("Nested child returned across filesystems.\n")
    (original / "z.txt").write_text("Incoming final child.\n")
    copy_seed(original); current = move_project(parent)
    original.mkdir(); (original / "z.txt").write_text("Newer destination work.\n")
    owned = original.stat().st_ino
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    session.capture("cross-directories-01-merge-review.png")
    before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
    wait_title("Undo conflict — Canopa Files")
    session.capture("cross-directories-02-child-collision.png")
    def returned_siblings():
        assert (original / "a.txt").read_text() == "Earlier copied child.\n"
        assert (original / "a-empty").is_dir() and not any((original / "a-empty").iterdir())
        assert (original / "b-nested/deep/note.txt").read_text() == "Nested child returned across filesystems.\n"
    if not directory_moves:
        returned_siblings()
    before = len(events()); chord("alt", "s"); wait_event(before, "Partly undone")
    returned_siblings()
    assert (current / "z.txt").read_text() == "Incoming final child.\n"
    assert not (current / "b-nested").exists() and original.stat().st_ino == owned
    navigate(original / "b-nested/deep"); keys("Home", "space")
    wait_title("note.txt — Quick Look"); time.sleep(.4)
    session.capture("cross-directories-03-nested-preview.png"); keys("Escape")
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    assert not current.exists() and original.stat().st_ino == owned
    assert (original / "z.txt").read_text() == "Newer destination work.\n"
    assert (original / "z (2).txt").read_text() == "Incoming final child.\n"
    focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
    assert not (original / "a.txt").exists() and (seed / "a.txt").read_text() == "Earlier copied child.\n"
    assert (original / "z.txt").read_text() == "Newer destination work.\n"
    navigate(original); keys("Home", "Down", "Down", "space")
    wait_title("z (2).txt — Quick Look"); time.sleep(.4)
    session.capture("cross-directories-04-returned-file-preview.png"); keys("Escape")

    parent = root / "Original colliding tree"; parent.mkdir(); original = parent / "project"
    (original / "nested").mkdir(parents=True); copy_seed(original / "nested")
    current = move_project(parent)
    (original / "nested").mkdir(parents=True)
    (original / "nested/newer.txt").write_text("Newer colliding folder work.\n")
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
    wait_title("Undo conflict — Canopa Files")
    session.capture("cross-directories-05-folder-keep-both.png")
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    assert not current.exists()
    returned = original / "nested (2)/a.txt"
    assert returned.read_text() == "Earlier copied child.\n"
    assert (original / "nested/newer.txt").read_text() == "Newer colliding folder work.\n"
    navigate(returned.parent); keys("Home", "space"); wait_title("a.txt — Quick Look"); time.sleep(.4)
    session.capture("cross-directories-06-folder-alias-preview.png"); keys("Escape")
    focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
    assert not returned.exists()
    assert (original / "nested/newer.txt").read_text() == "Newer colliding folder work.\n"
    if directory_moves:
        tree_seed = root / "Tree seed"; (tree_seed / "project").mkdir(parents=True)
        (tree_seed / "project/a.txt").write_text("Earlier copied directory child.\n")
        parent = root / "Owned original tree"; parent.mkdir(); original = parent / "project"
        navigate(tree_seed); keys("Home"); chord("ctrl", "c")
        navigate(parent); before = len(events()); chord("ctrl", "v"); wait_event(before, "Completed")
        current = move_project(parent)
        original.mkdir(); (original / "newer.txt").write_text("Newer root folder work.\n")
        owned = original.stat().st_ino
        focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
        session.capture("cross-directories-07-root-keep-both.png")
        before = len(events()); keys("Return"); wait_event(before, "Completed")
        returned = parent / "project (2)"
        assert not current.exists() and (returned / "a.txt").read_text() == "Earlier copied directory child.\n"
        assert original.stat().st_ino == owned and (original / "newer.txt").read_text() == "Newer root folder work.\n"
        navigate(returned); keys("Home", "space"); wait_title("a.txt — Quick Look"); time.sleep(.4)
        session.capture("cross-directories-08-root-alias-preview.png"); keys("Escape")
        focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
        assert not returned.exists() and original.stat().st_ino == owned
        assert (original / "newer.txt").read_text() == "Newer root folder work.\n"
        assert (tree_seed / "project/a.txt").read_text() == "Earlier copied directory child.\n"
        parent = root / "Saved forward tree"; parent.mkdir(); original = parent / "project"
        (original / "nested").mkdir(parents=True); copy_seed(original / "nested")
        replacement = root / "Forward replacement"; replacement.mkdir()
        (replacement / "a.txt").write_text("Incoming forward replacement.\n")
        navigate(replacement); keys("Home"); chord("ctrl", "c")
        navigate(original / "nested"); chord("ctrl", "v"); wait_title("File conflict — Canopa Files")
        before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
        current = move_project(parent)
        assert (current / "nested/a.txt").read_text() == "Incoming forward replacement.\n"
        saved = next((current / "nested").glob(".canopa-files-stage-*/previous"))
        assert saved.read_text() == "Earlier copied child.\n" and saved.stat().st_dev == moved.stat().st_dev
        session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
        recovery = wait_title("Recovery — Canopa Files")
        time.sleep(.5); keys("Up", "space"); wait_title("a.txt — Quick Look"); time.sleep(.4)
        session.capture("cross-directories-09-forward-backup-preview.png"); keys("Escape")
        focus(recovery); chord("ctrl", "r"); wait_title("Recover a copy")
        recovered = root / "Recovered forward backup.txt"
        chord("ctrl", "a"); session.run(["wtype", str(recovered)])
        before = len(events()); keys("Return"); session.wait_for(recovered.exists, "forward moved backup Recovery copy")
        wait_event(before, "Completed")
        assert recovered.read_text() == "Earlier copied child.\n" and saved.exists()
        focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
        assert not recovered.exists() and saved.exists()
        before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
        assert not current.exists() and (original / "nested/a.txt").read_text() == "Incoming forward replacement.\n"
        focus(recovery); keys("F5"); time.sleep(.5); keys("Up", "space")
        wait_title("a.txt — Quick Look"); time.sleep(.4)
        session.capture("cross-directories-10-forward-returned-backup-preview.png"); keys("Escape")
        focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
        assert (original / "nested/a.txt").read_text() == "Earlier copied child.\n"
        before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
        assert not (original / "nested/a.txt").exists()
        assert (replacement / "a.txt").read_text() == "Incoming forward replacement.\n"
        assert (seed / "a.txt").read_text() == "Earlier copied child.\n"
    else:
        parent = root / "Saved original tree"; parent.mkdir(); original = parent / "project"
        (original / "nested").mkdir(parents=True)
        copy_seed(original / "nested")
        replacement = root / "Replacement seed"; replacement.mkdir()
        (replacement / "a.txt").write_text("Incoming replacement.\n")
        navigate(replacement); keys("Home"); chord("ctrl", "c")
        navigate(original / "nested"); chord("ctrl", "v"); wait_title("File conflict — Canopa Files")
        before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
        saved = next((original / "nested").glob(".canopa-files-stage-*/previous"))
        assert saved.read_text() == "Earlier copied child.\n"
        name = saved.parent.name
        current = move_project(parent)
        occupied = original / "nested" / name; occupied.mkdir(parents=True)
        (occupied / "previous").write_text("Unrelated destination work.\n")
        focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
        before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
        wait_title("Undo conflict — Canopa Files")
        before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
        wait_title("Undo conflict — Canopa Files")
        session.capture("cross-directories-07-saved-container-collision.png")
        before = len(events()); chord("alt", "s"); wait_event(before, "Partly undone")
        assert (current / "nested" / name / "previous").read_text() == "Earlier copied child.\n"
        assert (occupied / "previous").read_text() == "Unrelated destination work.\n"
        session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
        recovery = wait_title("Recovery — Canopa Files")
        time.sleep(.5); keys("Up", "space"); wait_title("a.txt — Quick Look"); time.sleep(.4)
        session.capture("cross-directories-08-pending-backup-preview.png"); keys("Escape")
        focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
        before = len(events()); keys("Return"); wait_event(before, "Completed")
        assert not current.exists()
        assert (original / "nested/a.txt").read_text() == "Incoming replacement.\n"
        assert (occupied / "previous").read_text() == "Unrelated destination work.\n"
        returned = next(path for path in (original / "nested").glob(".canopa-files-stage-*/previous")
                        if path.read_text() == "Earlier copied child.\n")
        assert returned.parent != occupied and returned.stat().st_dev == secondary.stat().st_dev
        focus(recovery); keys("F5"); time.sleep(.5); keys("Up", "space")
        wait_title("a.txt — Quick Look"); time.sleep(.4)
        session.capture("cross-directories-09-returned-backup-preview.png"); keys("Escape")
        focus(recovery); chord("ctrl", "r"); wait_title("Recover a copy")
        recovered = root / "Recovered returned backup.txt"
        chord("ctrl", "a"); session.run(["wtype", str(recovered)])
        before = len(events()); keys("Return"); session.wait_for(recovered.exists, "returned backup Recovery copy")
        wait_event(before, "Completed")
        assert recovered.read_text() == "Earlier copied child.\n" and returned.exists()
        focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
        assert not recovered.exists() and returned.exists()
        before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
        assert (original / "nested/a.txt").read_text() == "Earlier copied child.\n"
        before = len(events()); chord("ctrl", "z"); wait_event(before, "Completed")
        assert not (original / "nested/a.txt").exists()
        assert (occupied / "previous").read_text() == "Unrelated destination work.\n"
        assert (seed / "a.txt").read_text() == "Earlier copied child.\n"
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "Gtk-WARNING"]), log
    checks = [
        "native Copy and Cut/Paste record the child and folder history",
        ("native forward directory Move and Undo return files across actual filesystems" if directory_moves
         else "explicit Undo Merge returns children between different actual filesystem device IDs"),
        ("recorded siblings, nested and empty folders return after skipping a child conflict" if directory_moves
         else "disjoint nested and empty folders return before a later file collision"),
        "Skip retains the unfinished child after returning its siblings",
        "Merge preserves the newer destination folder identity",
        "Quick Look previews the returned nested child",
        "retry reviews only the unfinished file and defaults to Keep both",
        "completion removes only the owned empty incoming folder",
        "newer destination data and the returned colliding file both survive",
        "older Copy Undo owns the returned file at its new physical identity",
        "older Undo preserves the source seed and newer destination work",
        "Quick Look previews the returned colliding file after older Undo",
        "a folder conflict defaults to Keep both across filesystems",
        "the incoming folder returns under the chosen new name",
        "Quick Look previews the child returned in the renamed folder",
        "older child Copy Undo follows the folder alias while preserving newer work",
    ]
    if directory_moves:
        checks.extend([
            "root Keep both returns the moved directory under a new name",
            "Quick Look previews the child in the returned root folder",
            "older directory Copy Undo removes the entire owned returned tree",
            "newer original folder identity, newer work and original seed survive older Undo",
            "a native forward folder Move carries its active backup across filesystems",
            "Recovery and Quick Look inspect the forward moved backup",
            "Recovery copies the forward moved backup without consuming it",
            "Undo of the Recovery copy preserves the backup",
            "Undo returns the moved folder and its catalog-owned backup",
            "Recovery refresh and Quick Look inspect the returned backup",
            "older replacement Undo restores the saved version after the forward Move and return",
            "older Copy Undo preserves both original seed files",
        ])
    else:
        checks.extend([
            "saved-container collisions offer Skip and Keep both during a cross-filesystem return",
            "Skip leaves the active backup available for Quick Look in Recovery",
            "Keep both returns the saved container without merging unrelated destination work",
            "Recovery refresh and Quick Look follow the returned physical saved version",
            "Recovery copies the returned backup without consuming it",
            "Undo of the Recovery copy leaves the backup available",
            "older replacement Undo restores the returned saved data",
            "successive older Copy Undo preserves unrelated destination work and the seed",
        ])
    return {"status": "passed", "scheme": scheme, "scale": scale, "filesystems": filesystems,
            "checks": checks, "windows": windows()}
