"""Directory returns, protected Recovery, partial Merge and remaining work."""
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
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "directory return workflow focus")
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
    root = fixture / "Directory returns"; root.mkdir()
    original = root / "project"; original.mkdir()
    (original / "incoming.txt").write_text("Original project work.\n")
    navigate(root); session.run(["wtype", "project"]); keys("F2")
    wait_title("Rename"); chord("ctrl", "a"); session.run(["wtype", "moved"])
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    moved = root / "moved"
    assert not original.exists() and (moved / "incoming.txt").read_text() == "Original project work.\n"

    original.mkdir(); (original / "keep.txt").write_text("Newer destination work.\n")
    source = root / "destination-backup.txt"; source.write_text("Copied version.\n")
    navigate(root); session.run(["wtype", source.name]); chord("ctrl", "c")
    navigate(original); before = len(events()); chord("ctrl", "v"); wait_event(before, "Completed")
    copied = original / source.name
    assert copied.read_text() == "Copied version.\n"
    copied.write_text("Protected newer destination version.\n")
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); chord("alt", "m"); wait_event(before, "Completed")
    assert not copied.exists()

    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    session.capture("directories-01-return-conflict.png")
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    returned = root / "project (2)"
    assert not moved.exists() and (returned / "incoming.txt").read_text() == "Original project work.\n"
    assert (original / "keep.txt").read_text() == "Newer destination work.\n"

    session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
    recovery = wait_title("Recovery — Canopa Files")
    focus(recovery); keys("F5"); time.sleep(.5); keys("Up", "space")
    wait_title("destination-backup.txt — Quick Look"); time.sleep(.4)
    session.capture("directories-02-protected-destination-preview.png"); keys("Escape")
    focus(recovery); chord("ctrl", "r"); wait_title("Recover a copy")
    recovered = fixture / "Recovered directory destination.txt"
    chord("ctrl", "a"); session.run(["wtype", str(recovered)])
    before = len(events()); keys("Return")
    session.wait_for(recovered.exists, "protected directory destination copy")
    wait_event(before, "Completed"); wait_title("Operations — Canopa Files")
    assert recovered.read_text() == "Protected newer destination version.\n"
    assert (original / "keep.txt").read_text() == "Newer destination work.\n"
    assert (returned / "incoming.txt").read_text() == "Original project work.\n"

    merging = root / "merge-project"; merging.mkdir()
    (merging / "a.txt").write_text("First incoming child.\n")
    (merging / "b.txt").write_text("Second incoming child.\n")
    navigate(root); session.run(["wtype", "merge-project"]); keys("F2")
    wait_title("Rename"); chord("ctrl", "a"); session.run(["wtype", "merge-moved"])
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    merging.mkdir(); (merging / "b.txt").write_text("Newer conflicting child.\n")
    (merging / "newer.txt").write_text("Newer destination child.\n")
    destination_identity = (merging.stat().st_dev, merging.stat().st_ino)
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    session.capture("directories-03-merge-review.png")
    before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
    wait_title("Undo conflict — Canopa Files")
    session.capture("directories-04-child-conflict.png")
    before = len(events()); chord("alt", "s"); wait_event(before, "Partly undone")
    merge_source = root / "merge-moved"
    assert (merging / "a.txt").read_text() == "First incoming child.\n"
    assert not (merge_source / "a.txt").exists()
    assert (merge_source / "b.txt").read_text() == "Second incoming child.\n"
    assert (merging / "b.txt").read_text() == "Newer conflicting child.\n"
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    assert not merge_source.exists()
    assert (merging.stat().st_dev, merging.stat().st_ino) == destination_identity
    assert (merging / "a.txt").read_text() == "First incoming child.\n"
    assert (merging / "b.txt").read_text() == "Newer conflicting child.\n"
    assert (merging / "b (2).txt").read_text() == "Second incoming child.\n"
    assert (merging / "newer.txt").read_text() == "Newer destination child.\n"
    navigate(merging); keys("Home", "Down", "space")
    wait_title("b (2).txt — Quick Look"); time.sleep(.4)
    session.capture("directories-05-merged-child-preview.png"); keys("Escape")

    remainder = root / "remainder-project"; remainder.mkdir()
    (remainder / "item.txt").write_text("Incoming remainder child.\n")
    navigate(root); session.run(["wtype", "remainder-project"]); keys("F2")
    wait_title("Rename"); chord("ctrl", "a"); session.run(["wtype", "remainder-moved"])
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    remainder.mkdir(); (remainder / "item.txt").write_text("Newer remainder child.\n")
    remaining_source = root / "remainder-moved"
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
    wait_title("Undo conflict — Canopa Files")
    (remaining_source / "new.txt").write_text("New work arriving during Merge.\n")
    keys("Return"); wait_title("Items remain after Merge — Canopa Files")
    session.capture("directories-06-remaining-items-review.png")
    before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    assert (remaining_source / "new.txt").read_text() == "New work arriving during Merge.\n"
    focus(browser); chord("ctrl", "z"); wait_title("Items remain after Merge — Canopa Files")
    before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
    focus(browser); chord("ctrl", "z"); wait_title("Items remain after Merge — Canopa Files")
    (remaining_source / "late.txt").write_text("Work added during review.\n")
    before = len(events()); chord("alt", "k"); wait_event(before, "Failed")
    focus(browser); chord("ctrl", "z"); wait_title("Items remain after Merge — Canopa Files")
    before = len(events()); chord("alt", "k"); event = wait_event(before, "Completed")
    assert "0 changes undone" in event["body"] and "1 item kept" in event["body"], event
    assert (remaining_source / "new.txt").read_text() == "New work arriving during Merge.\n"
    assert (remaining_source / "late.txt").read_text() == "Work added during review.\n"
    assert (remainder / "item.txt").read_text() == "Newer remainder child.\n"
    assert (remainder / "item (2).txt").read_text() == "Incoming remainder child.\n"
    navigate(remaining_source); session.run(["wtype", "new"]); keys("space")
    wait_title("new.txt — Quick Look"); time.sleep(.4)
    session.capture("directories-07-kept-remainder-preview.png"); keys("Escape")

    additions = root / "additions-project"; additions.mkdir()
    (additions / "a.txt").write_text("Returned before the additional pass.\n")
    (additions / "z.txt").write_text("Last recorded child.\n")
    navigate(root); session.run(["wtype", additions.name]); keys("F2")
    wait_title("Rename"); chord("ctrl", "a"); session.run(["wtype", "additions-moved"])
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    additions.mkdir(); (additions / "z.txt").write_text("Newer destination z.\n")
    additions_source = root / "additions-moved"
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
    wait_title("Undo conflict — Canopa Files")
    assert (additions / "a.txt").read_text() == "Returned before the additional pass.\n"
    (additions_source / "a.txt").write_text("Additional work using a returned child's name.\n")
    (additions_source / "later.txt").write_text("Disjoint additional work.\n")
    keys("Return"); wait_title("Items remain after Merge — Canopa Files")
    session.capture("directories-08-merge-additional-items.png")
    (additions_source / "late.txt").write_text("Arrived while reviewing the additional pass.\n")
    before = len(events()); chord("alt", "m"); wait_event(before, "Failed")
    assert (additions_source / "a.txt").read_text() == "Additional work using a returned child's name.\n"
    focus(browser); chord("ctrl", "z"); wait_title("Items remain after Merge — Canopa Files")
    before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
    wait_title("Undo conflict — Canopa Files")
    session.capture("directories-09-additional-child-conflict.png")
    before = len(events()); chord("alt", "s"); wait_event(before, "Partly undone")
    assert (additions_source / "a.txt").read_text() == "Additional work using a returned child's name.\n"
    assert (additions / "later.txt").read_text() == "Disjoint additional work.\n"
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    assert not additions_source.exists()
    assert (additions / "a.txt").read_text() == "Returned before the additional pass.\n"
    assert (additions / "a (2).txt").read_text() == "Additional work using a returned child's name.\n"
    assert (additions / "z.txt").read_text() == "Newer destination z.\n"
    assert (additions / "z (2).txt").read_text() == "Last recorded child.\n"
    assert (additions / "late.txt").read_text() == "Arrived while reviewing the additional pass.\n"
    navigate(additions); keys("Home", "space")
    wait_title("a (2).txt — Quick Look"); time.sleep(.4)
    session.capture("directories-10-additional-child-preview.png"); keys("Escape")

    blocked = root / "blocked-project"; blocked.mkdir()
    (blocked / "a.txt").write_text("Returned before the destination moved.\n")
    (blocked / "z.txt").write_text("Unreturned blocked child.\n")
    navigate(root); session.run(["wtype", blocked.name]); keys("F2")
    wait_title("Rename"); chord("ctrl", "a"); session.run(["wtype", "blocked-moved"])
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    blocked.mkdir(); (blocked / "z.txt").write_text("Newer blocked destination z.\n")
    blocked_source = root / "blocked-moved"
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
    wait_title("Undo conflict — Canopa Files")
    before = len(events()); chord("alt", "s"); wait_event(before, "Partly undone")
    held_destination = root / "blocked destination elsewhere"; blocked.rename(held_destination)
    blocked.mkdir(); (blocked / "foreign.txt").write_text("Foreign destination work.\n")
    focus(browser); chord("ctrl", "z"); wait_title("Merge cannot continue — Canopa Files")
    session.capture("directories-11-replaced-merge-destination.png")
    before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    focus(browser); chord("ctrl", "z"); wait_title("Merge cannot continue — Canopa Files")
    before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
    focus(browser); chord("ctrl", "z"); wait_title("Merge cannot continue — Canopa Files")
    (blocked_source / "late.txt").write_text("New source work during root review.\n")
    before = len(events()); chord("alt", "f"); wait_event(before, "Failed")
    focus(browser); chord("ctrl", "z"); wait_title("Merge cannot continue — Canopa Files")
    before = len(events()); chord("alt", "f"); event = wait_event(before, "Completed")
    assert "0 changes undone" in event["body"] and "1 item not returned" in event["body"], event
    assert (blocked_source / "z.txt").read_text() == "Unreturned blocked child.\n"
    assert (blocked_source / "late.txt").read_text() == "New source work during root review.\n"
    assert (blocked / "foreign.txt").read_text() == "Foreign destination work.\n"
    assert (held_destination / "a.txt").read_text() == "Returned before the destination moved.\n"
    navigate(blocked_source); session.run(["wtype", "z"]); keys("space")
    wait_title("z.txt — Quick Look"); time.sleep(.4)
    session.capture("directories-12-finished-merge-source-preview.png"); keys("Escape")

    missing = root / "missing-project"; missing.mkdir()
    (missing / "a.txt").write_text("Returned before source disappearance.\n")
    (missing / "z.txt").write_text("Unfinished missing-source child.\n")
    navigate(root); session.run(["wtype", missing.name]); keys("F2")
    wait_title("Rename"); chord("ctrl", "a"); session.run(["wtype", "missing-moved"])
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    missing.mkdir(); (missing / "z.txt").write_text("Newer missing-source destination z.\n")
    missing_source = root / "missing-moved"
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
    wait_title("Undo conflict — Canopa Files")
    before = len(events()); chord("alt", "s"); wait_event(before, "Partly undone")
    held_source = root / "missing source elsewhere"; missing_source.rename(held_source)
    focus(browser); chord("ctrl", "z"); wait_title("Merge cannot continue — Canopa Files")
    session.capture("directories-13-missing-merge-source.png")
    before = len(events()); keys("Return"); wait_event(before, "Partly undone")
    focus(browser); chord("ctrl", "z"); wait_title("Merge cannot continue — Canopa Files")
    held_source.rename(missing_source)
    before = len(events()); chord("alt", "f"); wait_event(before, "Failed")
    focus(browser); chord("ctrl", "z"); wait_title("Undo conflict — Canopa Files")
    before = len(events()); keys("Return"); wait_event(before, "Completed")
    assert not missing_source.exists()
    assert (missing / "a.txt").read_text() == "Returned before source disappearance.\n"
    assert not (missing / "a (2).txt").exists()
    assert (missing / "z.txt").read_text() == "Newer missing-source destination z.\n"
    assert (missing / "z (2).txt").read_text() == "Unfinished missing-source child.\n"
    navigate(missing); keys("Home", "Down", "space")
    wait_title("z (2).txt — Quick Look"); time.sleep(.4)
    session.capture("directories-14-retried-merge-preview.png"); keys("Escape")

    saved_source = root / "Saved Merge source"; saved_source.mkdir()
    saved_target = root / "Saved Merge target"; saved_target.mkdir()
    for early in [False, True]:
        name = "early-saved.txt" if early else "late-saved.txt"
        source = saved_source / name; current = saved_target / name
        incoming = f"Original moved file for {name}.\n"
        previous = f"Saved previous destination for {name}.\n"
        source.write_text(incoming); current.write_text(previous)
        navigate(saved_source); session.run(["wtype", name]); chord("ctrl", "x")
        navigate(saved_target); chord("ctrl", "v"); wait_title("File conflict — Canopa Files")
        before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
        saved = next(path for path in saved_target.glob(".canopa-files-stage-*/previous") if path.read_text() == previous)
        held = root / f"Held saved version {name}"
        original_elsewhere = root / f"Original moved file {name}"; current.rename(original_elsewhere)
        current.mkdir(); (current / "a.txt").write_text(f"First returned child for {name}.\n")
        (current / "z.txt").write_text(f"Last returned child for {name}.\n")
        source.mkdir(); (source / "z.txt").write_text(f"Newer destination z for {name}.\n")
        if early:
            saved.rename(held)
        focus(browser); chord("ctrl", "z"); wait_title("Moved item changed — Canopa Files")
        chord("alt", "r")
        if early:
            wait_title("Saved version unavailable — Canopa Files")
            session.capture("directories-18-source-only-before-merge.png")
            chord("alt", "r")
        wait_title("Undo conflict — Canopa Files")
        before = len(events()); chord("alt", "m"); wait_event(before, "Waiting for your decision")
        wait_title("Undo conflict — Canopa Files")
        if not early:
            saved.rename(held)
        before = len(events()); keys("Return")
        if not early:
            wait_title("Saved version unavailable during Merge — Canopa Files")
            session.capture("directories-15-unavailable-merge-saved.png")
            before = len(events()); keys("Return"); wait_event(before, "Partly undone")
            assert current.is_dir() and not any(current.iterdir())
            focus(browser); chord("ctrl", "z"); wait_title("Saved version unavailable during Merge — Canopa Files")
            before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
            focus(browser); chord("ctrl", "z"); wait_title("Saved version unavailable during Merge — Canopa Files")
            (source / "later.txt").write_text("Work added during saved review.\n")
            before = len(events()); chord("alt", "c"); wait_event(before, "Failed")
            assert current.is_dir()
            focus(browser); chord("ctrl", "z"); wait_title("Saved version unavailable during Merge — Canopa Files")
            before = len(events()); chord("alt", "c")
        event = wait_event(before, "Partly undone")
        assert "1 saved version not restored" in event["body"], event
        assert not current.exists()
        assert (source / "a.txt").read_text() == f"First returned child for {name}.\n"
        assert (source / "z.txt").read_text() == f"Newer destination z for {name}.\n"
        assert (source / "z (2).txt").read_text() == f"Last returned child for {name}.\n"
        if early:
            navigate(source); keys("Home", "space"); wait_title("a.txt — Quick Look"); time.sleep(.4)
            session.capture("directories-19-source-only-merged-preview.png"); keys("Escape")
        else:
            focus(browser); chord("ctrl", "z"); wait_title("Saved version unavailable — Canopa Files")
            session.capture("directories-16-pending-merge-saved-restoration.png")
            before = len(events()); keys("Escape"); wait_event(before, "Cancelled")
            assert (source / "later.txt").read_text() == "Work added during saved review.\n"
        held.rename(saved)
        focus(browser); chord("ctrl", "z"); wait_title("Saved version changed — Canopa Files")
        before = len(events()); chord("alt", "r"); wait_event(before, "Completed")
        assert current.read_text() == previous and original_elsewhere.read_text() == incoming
        assert (source / "a.txt").read_text() == f"First returned child for {name}.\n"
        if not early:
            navigate(saved_target); session.run(["wtype", name]); keys("space")
            wait_title(f"{name} — Quick Look"); time.sleep(.4)
            session.capture("directories-17-restored-merge-saved-preview.png"); keys("Escape")
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "Gtk-WARNING"]), log
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": [
        "normal Rename moves the original project directory",
        "normal Copy and changed-copy Undo protect newer destination data",
        "directory return shows the original-location conflict",
        "Return defaults to Keep both and returns the incoming folder under a new name",
        "newer destination files stay in their original folder",
        "protected newer destination data remains available in Recovery",
        "Quick Look previews the protected destination version after the return",
        "Recovery copies out the exact protected newer bytes",
        "recovering data preserves both directory versions",
        "directory Undo offers an explicit Merge choice",
        "Merge asks separately about a colliding child file",
        "Skip retains the unfinished child after returning its disjoint sibling",
        "retry reviews only the unfinished child and keeps both file versions",
        "Merge preserves the identity of the newer destination folder",
        "completed children remain returned and newer destination work stays intact",
        "the owned empty incoming root is removed after its last child returns",
        "Quick Look previews the child returned under a new name",
        "new source work receives a separate remaining-items review",
        "Return defaults to Skip for the remaining source folder",
        "Escape preserves the pending remaining-items decision",
        "changes to the remaining folder refuse stale Keep",
        "fresh Keep leaves every remaining source file in place",
        "Keep reports zero reversed changes and one kept item",
        "keeping remaining work preserves returned and newer destination children",
        "Quick Look previews source work after its folder is kept",
        "additional items require an explicit Merge remaining items choice",
        "a stale additional-items Merge answer preserves the source work",
        "Merge remaining items asks separately about a reused child filename",
        "Skip keeps the colliding additional child while returning disjoint additions",
        "retry returns only the unfinished additional child under a new name",
        "additional passes preserve returned children and newer destination work",
        "Quick Look previews the additional child returned under a new name",
        "a partial Merge reviews a replaced destination before returning its children",
        "Return safely skips the lost-folder review",
        "Escape preserves the unfinished Merge branch",
        "source changes refuse a stale Finish this Merge answer",
        "Finish leaves unreturned source work, returned children and replacement files intact",
        "Finish reports zero undone changes and one item not returned",
        "Quick Look previews unreturned work after finishing a blocked Merge",
        "a partial Merge reviews a missing incoming folder",
        "the missing-folder review can be skipped for later retry",
        "reappearance of the incoming folder invalidates a stale Finish answer",
        "retry returns only the unfinished child after the original folder reappears",
        "Quick Look previews the child returned after missing-folder recovery",
        "a saved version lost during child return receives a separate Merge review",
        "Return safely skips the unavailable Merge saved-version review",
        "Escape preserves the incomplete saved-version decision",
        "new returned-folder work refuses a stale Continue answer",
        "Continue removes only the owned empty incoming folder",
        "Continue keeps saved restoration pending as a separate Undo step",
        "later saved restoration preserves returned and newer child data",
        "Quick Look previews the saved file restored after Merge",
        "source-only return can begin a directory Merge while the saved version is unavailable",
        "source-only Merge preserves newer destination children",
        "source-only Merge retains one saved-restoration remainder",
        "a reappearing saved version can be restored after source-only Merge",
        "Quick Look previews contents returned by source-only Merge",
    ], "windows": windows()}
