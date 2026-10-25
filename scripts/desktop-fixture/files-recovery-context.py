"""Recovery context ownership and v15-compatible recorded-location input."""
import json
import sqlite3
import time


def v15_fields(value):
    """Prepare old input fields without carrying newer permission authority."""
    if isinstance(value, list):
        return [v15_fields(item) for item in value]
    if not isinstance(value, dict):
        return value
    result = {key: v15_fields(item) for key, item in value.items()
              if key not in {"parent_access", "source_access", "destination_parent",
                             "access_parent", "destination_mode"}}
    if result.get("kind") == "created-folder":
        result.pop("mode", None)
    return result


def verify(session, fixture, browser, secondary, scheme, scale, binary, process):
    notifications = session.output / "notifications.json"

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((window for window in windows() if window["title"] == value), None)

    def wait_title(value):
        window = session.wait_for(lambda: title(value) and title(value)["is_focused"] and title(value), value)
        time.sleep(.3)
        return window

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "Recovery context workflow focus")
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

    def navigate(location):
        focus(browser); chord("ctrl", "l")
        session.run(["wtype", str(location)]); keys("Return")
        wait_title(f"{location.name} — Canopa Files")

    def transfer(source, destination, move):
        navigate(source.parent); session.run(["wtype", source.name]); chord("ctrl", "x" if move else "c")
        navigate(destination); before = len(events()); chord("ctrl", "v"); wait_event(before)

    def recovery(capture, preview=False):
        session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
        # A remote action has no injected user's activation token. Select the
        # existing tile explicitly before keyboard input; presenting a window
        # alone does not establish compositor focus after a modal return.
        window = session.wait_for(lambda: title("Recovery — Canopa Files"), "existing Recovery window")
        focus(window)
        keys("F5"); time.sleep(.6); keys("Up")
        if preview:
            keys("space"); wait_title("protected.txt — Quick Look"); time.sleep(.4)
        session.capture(capture)
        if preview:
            keys("Escape")
        return window

    root = fixture / "Recovery context"; root.mkdir()
    seed = root / "Seed"; seed.mkdir(); (seed / "protected.txt").write_text("Initial copied seed.\n")
    parent = root / "Original"; nested = parent / "project/nested"; nested.mkdir(parents=True)
    moved = secondary / "Moved"; moved.mkdir()
    current = moved / "project/nested"
    filesystems = {"original": nested.stat().st_dev, "current": moved.stat().st_dev}
    assert len(set(filesystems.values())) == 2, filesystems
    protected = "Protected edits from the original folder.\n"
    unrelated = "Unrelated work created after the folder move.\n"
    transfer(seed / "protected.txt", nested, False)
    (nested / "protected.txt").write_text(protected)
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); chord("alt", "m"); wait_event(before)
    assert not (nested / "protected.txt").exists()
    transfer(parent / "project", moved, True)
    assert not (parent / "project").exists()
    saved = next(current.glob(".canopa-files-stage-*/previous"))
    assert saved.read_text() == protected
    recovery("context-01-moved-location.png")
    recovery("context-02-moved-preview.png", preview=True)

    nested.mkdir(parents=True); (nested / "protected.txt").write_text(unrelated)
    transfer(nested / "protected.txt", current, True)
    focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before)
    assert (nested / "protected.txt").read_text() == unrelated
    assert not (current / "protected.txt").exists() and saved.read_text() == protected
    window = recovery("context-03-after-unrelated-undo.png")
    focus(window); chord("ctrl", "r"); wait_title("Recover a copy")
    recovered = root / "Recovered identified version.txt"
    chord("ctrl", "a"); session.run(["wtype", str(recovered)])
    before = len(events()); keys("Return"); wait_event(before)
    assert recovered.read_text() == protected and saved.read_text() == protected
    focus(browser); before = len(events()); chord("ctrl", "z"); wait_event(before)
    assert not recovered.exists() and saved.exists()

    session.retire(process)
    session.wait_for(lambda: not any(window["app_id"] == "org.canopa.Files" for window in windows()), "closed Files before preparing legacy input")
    # Fixture input only: v15 maps contain paths without history-step or parent
    # authority. All upgrade outputs are observed through native UI and files.
    history = session.runtime / "state/canopa-files/operations.sqlite3"
    with sqlite3.connect(history) as input_history:
        # Newer Stage/Movement authority must be absent in a v15 input. Paths
        # and stage identities do not change, so their derived indexes remain.
        for operation, ordinal, body in input_history.execute("SELECT operation,ordinal,body FROM changes").fetchall():
            change = v15_fields(json.loads(body))
            input_history.execute("UPDATE changes SET body=? WHERE operation=? AND ordinal=?", (json.dumps(change), operation, ordinal))
            leaf = change
            while leaf.get("kind") == "directory-merge":
                leaf = leaf["change"]
            if leaf.get("kind") in ("created-folder", "copied"):
                input_history.execute("DELETE FROM identity_refs WHERE operation=? AND ordinal=? AND role=1", (operation, ordinal))
        rows = input_history.execute("SELECT id,body FROM garbage").fetchall()
        prepared = 0
        shared = 0
        for entry, body in rows:
            record = v15_fields(json.loads(body))
            for movement in record.get("context_moves", []):
                for field in ("container_step", "owner_step", "parent_identity"):
                    movement.pop(field, None)
                prepared += 1
            input_history.execute("UPDATE garbage SET body=? WHERE id=?", (json.dumps(record), entry))
            # A valid legacy archive may share its actual saved container with
            # another record. Expiring an unprotected reference must leave the
            # protected record and its bytes available after this startup.
            if record["retained"]["recovery"]:
                unprotected = json.loads(json.dumps(record))
                unprotected["retained"]["recovery"] = False
                duplicate = input_history.execute("INSERT INTO garbage(body) VALUES(?)", (json.dumps(unprotected),)).lastrowid
                input_history.execute("INSERT INTO saved_stage_records(stage,entry) SELECT stage,? FROM saved_stage_records WHERE entry=?", (duplicate, entry))
                input_history.execute("INSERT INTO garbage_paths(path,entry) SELECT path,? FROM garbage_paths WHERE entry=?", (duplicate, entry))
                shared += 1
        assert prepared, "the input must contain actual v15 path maps"
        assert shared, "the input must contain an unprotected shared archive"
        input_history.executescript("DROP TRIGGER recovery_context_deleted; DROP TABLE recovery_context_cleanup; DROP TABLE recovery_context_steps; DROP TABLE namespace_moves; DROP INDEX identity_refs_expected; DROP TABLE directory_access; PRAGMA user_version=15;")
    session.start([str(binary), str(fixture)], "files-context-restarted.log")
    browser = wait_title("Downloads — Canopa Files")
    window = recovery("context-04-legacy-recorded-location.png")
    recovery("context-05-legacy-preview.png", preview=True)
    focus(browser); before = len(events()); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    keys("Return"); wait_event(before)
    returned = parent / "project (2)/nested"
    assert not (moved / "project").exists()
    returned_saved = next(returned.glob(".canopa-files-stage-*/previous"))
    assert returned_saved.read_text() == protected
    assert (nested / "protected.txt").read_text() == unrelated
    window = recovery("context-06-returned-recorded-location.png")
    recovery("context-07-returned-legacy-preview.png", preview=True)
    focus(window); chord("ctrl", "r"); wait_title("Recover a copy")
    session.capture("context-08-recorded-location-copy-chooser.png")
    recovered = root / "Recovered legacy version.txt"
    chord("ctrl", "a"); session.run(["wtype", str(recovered)])
    before = len(events()); keys("Return"); wait_event(before)
    assert recovered.read_text() == protected and returned_saved.read_text() == protected
    assert (nested / "protected.txt").read_text() == unrelated
    for path in session.output.glob("files*.log"):
        log = path.read_text()
        assert not any(marker in log for marker in ("panicked", "CRITICAL", "Gtk-WARNING")), log
    return {"status": "passed", "scheme": scheme, "scale": scale, "filesystems": filesystems,
            "checks": [
                "changed copied work is explicitly archived through native Undo",
                "containing-folder Cut/Paste carries archived bytes across actual filesystems",
                "moved archive retains its source filename in Quick Look",
                "unrelated file move and Undo preserve both unrelated and archived bytes",
                "Recovery location after unrelated Undo is captured for visual verification",
                "Recovery copy after unrelated Undo is undoable without consuming the archive",
                "authentic v15 path-map input upgrades after process shutdown and restart",
                "legacy unprotected shared archive expires without removing protected saved bytes",
                "legacy recorded location and source-named preview are rendered",
                "Keep both returns archived bytes while preserving recreated source work",
                "returned legacy archive stays previewable with a qualified location hint",
                "legacy Recovery copy chooser and real copy preserve both versions",
                "GTK logs contain no warnings, criticals or panics",
            ], "windows": windows()}
