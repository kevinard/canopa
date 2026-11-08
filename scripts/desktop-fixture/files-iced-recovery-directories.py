"""Actual directory Recovery and exact continuation through the Iced frontend."""
import json
import os
from pathlib import Path
import sqlite3
import threading
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Directory Recovery"
    assert root.is_relative_to(session.runtime / "home")
    (root / "Source").mkdir(parents=True)
    (root / "Target").mkdir()
    (root / "Source/Saved report.txt").write_bytes(b"Incoming replacement\n")
    (root / "Target/Saved report.txt").write_bytes(b"Previous saved file\n")
    config.write_text(config.read_text() + 'files { shortcut "recovery" "<Alt>r"; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Directory Recovery"
    current = root / "Target/Saved report.txt"
    notifications = session.output / "notifications.json"
    checks = []
    note = "b" + "long" * 32 + ".txt"
    copy_name = note[:-4] + " (2).txt"
    source_bytes = b"Recovered directory document\n"
    newer_bytes = b"Newer output document\n"

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "directory Recovery focus")
        time.sleep(.15)

    def key(name):
        session.run(["wtype", "-k", name])
        time.sleep(.2)

    def chord(name, modifier="ctrl"):
        session.run(["wtype", "-M", modifier, "-k", name, "-m", modifier])
        time.sleep(.25)

    def uri(path):
        return "file://" + quote_from_bytes(os.fsencode(path), safe="/")

    def type_text(value, submit=False):
        session.run(["wtype", "-d", "5", value])
        if submit:
            key("Return")

    def navigate(path):
        focus(browser)
        chord("l")
        type_text(uri(path), submit=True)
        time.sleep(.5)

    def events():
        return json.loads(notifications.read_text()) if notifications.exists() else []

    def notice(before, predicate):
        return session.wait_for(lambda: next((e for e in events()[before:] if e["app"] == "Canopa Files" and predicate(e)), None), "this Recovery result notification")

    def request_receipts():
        # Observe only private durable scheduling receipts, never execution
        # payloads. Running=1 and Finished=5 are the stored RequestPhase values.
        # A cancelled question followed by Continue must create a successor;
        # matching window titles and Tab counts cannot establish its identity.
        history = session.runtime / "state/canopa-files/operations.sqlite3"
        connection = sqlite3.connect(history.as_uri() + "?mode=ro", uri=True)
        try:
            return connection.execute("SELECT id,revision,phase FROM requests WHERE id NOT IN (SELECT request FROM source_leases) ORDER BY id").fetchall()
        finally:
            connection.close()

    def fresh_operations():
        previous = title("Operations")
        if previous:
            focus(previous)
            chord("w")
            session.wait_for(lambda: all(w["id"] != previous["id"] for w in windows()), "old Operations surface retires")
        focus(browser)
        chord("j")
        window = session.wait_for(lambda: title("Operations"), "fresh Operations window")
        focus(window)
        return window

    def fifth_control(capture=None):
        # Idle: New browser, Undo, Recovery, Details, Continue.
        # Waiting: New browser, Recovery, Details, Cancel, Review.
        for _ in range(5):
            key("Tab")
        if capture:
            session.capture(capture)
        key("Return")

    navigate(root / "Source/Saved report.txt")
    chord("c")
    navigate(root / "Target")
    chord("v")
    conflict = session.wait_for(lambda: title("File conflict"), "native replacement question")
    focus(conflict)
    key("Tab")  # explicit Replace, following Cancel
    key("Return")
    session.wait_for(lambda: current.read_bytes() == b"Incoming replacement\n" and title("File conflict") is None, "initial replacement committed")
    saved = session.wait_for(lambda: next((root / "Target").glob(".canopa-files-stage-*/previous"), None), "retained previous version")
    held = root / "Held original.txt"
    saved.rename(held)
    saved.mkdir()
    (saved / "a-padding.txt").write_bytes(b"a" * (16 * 1024 * 1024))
    (saved / note).write_bytes(source_bytes)
    (saved / "nested").mkdir()
    (saved / "nested/c.txt").write_bytes(b"Nested recovered document\n")
    (saved / "nested").chmod(0o555)
    saved.chmod(0o555)
    checks.append("real replacement retains the old file and an unexpected saved directory remains recoverable")

    chord("r", "alt")
    recovery = session.wait_for(lambda: title("Recovery"), "Iced Recovery")
    focus(recovery)
    key("F5")
    time.sleep(.5)
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    session.wait_for(lambda: title("Recovery")["layout"]["window_size"][0] == 340, "actual narrow Recovery geometry")
    key("Home")
    session.capture("recovery-directory-source.png")
    key("Return")
    dialog = session.wait_for(lambda: title("Recover a copy"), "captured directory Recovery draft")
    focus(dialog)
    key("Tab")  # optional full path, leaving the folder draft intact
    recovered = root / "Recovered directory"
    type_text(uri(recovered))
    arrival = []

    def independent_output():
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            try:
                with (recovered / note).open("xb") as output:
                    output.write(newer_bytes)
                arrival.append("created")
                return
            except (FileNotFoundError, PermissionError):
                time.sleep(.001)
            except Exception as error:
                arrival.append(str(error))
                return
        arrival.append("No writable copy interval for independent output")

    thread = threading.Thread(target=independent_output, daemon=True)
    thread.start()
    key("Return")
    question = session.wait_for(lambda: title("Recovery copy needs a decision"), "actual Recovery child collision")
    thread.join(timeout=1)
    assert arrival == ["created"], arrival
    assert question["is_floating"]
    focus(question)
    session.capture("recovery-directory-child-question.png")
    before = len(events())
    key("Return")  # the safe default is Cancel
    notice(before, lambda e: e["title"] == "Recover Saved report.txt" and "cancelled" in e["body"])
    session.wait_for(lambda: all(w["id"] not in (dialog["id"], question["id"]) for w in windows()), "cancelled copy retires its draft and question")
    assert (recovered / "a-padding.txt").stat().st_size == 16 * 1024 * 1024
    assert (recovered / note).read_bytes() == newer_bytes
    assert not (recovered / "nested").exists()
    assert recovered.stat().st_mode & 0o777 == 0o555
    checks.append("Return cancels a child conflict while preserving completed bytes, newer output and read-only modes")

    held_output = root / "Held output"
    recovered.rename(held_output)
    recovered.mkdir()
    (recovered / "unrelated.txt").write_bytes(b"Unrelated replacement folder\n")
    operations = fresh_operations()
    before = len(events())
    fifth_control()
    refusal = notice(before, lambda e: e["title"] == "Recover Saved report.txt" and ("changed" in e["body"].lower() or "original" in e["body"].lower()))
    assert list(recovered.iterdir()) == [recovered / "unrelated.txt"], refusal
    assert (held_output / note).read_bytes() == newer_bytes
    session.capture("recovery-directory-continuation-refused.png")
    unrelated = root / "Unrelated output"
    recovered.rename(unrelated)
    held_output.rename(recovered)
    operations = fresh_operations()
    fifth_control()
    question = session.wait_for(lambda: title("Recovery copy needs a decision"), "exact continuation returns to its original child collision")
    assert not (recovered / "nested").exists()
    assert (recovered / note).read_bytes() == newer_bytes
    checks.append("Continue refuses a replaced output root and retains the exact plan for the original directory")

    # The resumed request belongs to Operations. Retire all windows while its
    # decision waits, then activate the same process and review it explicitly.
    before_close = len(events())
    known_outcomes = {e["body"] for e in events()[:before_close] if e["title"] == "Recover Saved report.txt"}
    waiting_receipts = request_receipts()
    running = [row for row in waiting_receipts if row[2] == 1]
    assert len(running) == 1, waiting_receipts
    waiting_request = running[0]

    def still_unanswered():
        assert request_receipts() == waiting_receipts, (waiting_receipts, request_receipts())
        outcomes = [e for e in events()[before_close:] if e["title"] == "Recover Saved report.txt"
                    and e["body"] not in known_outcomes and "handed to another request" not in e["body"]]
        assert not outcomes, outcomes
        assert not (recovered / "nested").exists()
        assert (recovered / note).read_bytes() == newer_bytes

    # Close the identified source even when the compositor keeps keyboard
    # focus on the pending decision in the other window.
    session.run(["niri", "msg", "action", "close-window", "--id", str(recovery["id"])])
    session.wait_for(lambda: title("Recovery") is None, "Recovery source window retires")
    focus(browser)
    chord("w")
    session.wait_for(lambda: all(w["id"] != browser["id"] for w in windows()), "last Browser retires")
    focus(operations)
    chord("w")
    session.wait_for(lambda: not windows(), "all native windows close while Recovery waits")
    assert process.poll() is None
    still_unanswered()
    session.run([process.args[0], str(root)])
    browser = session.wait_for(lambda: title("Directory Recovery"), "same host opens another Browser")
    assert browser["pid"] == process.pid
    operations = fresh_operations()
    still_unanswered()
    fifth_control("recovery-directory-before-review.png")
    question = session.wait_for(lambda: title("Recovery copy needs a decision"), "Review restores the unanswered Recovery decision")
    still_unanswered()
    focus(question)
    session.capture("recovery-directory-reopened-question.png")
    before = len(events())
    key("Tab")  # Keep both is the first offered action after Cancel
    key("Return")
    notice(before, lambda e: e["title"] == "Recover Saved report.txt" and "0 remaining" in e["body"] and "cancelled" not in e["body"])
    session.wait_for(lambda: all(w["id"] != question["id"] for w in windows()), "answered Recovery surface retires before Browser focus")
    session.wait_for(lambda: (recovered / "nested/c.txt").exists(), "exact remaining Recovery copy completed")
    assert (recovered / copy_name).read_bytes() == source_bytes
    assert (recovered / note).read_bytes() == newer_bytes
    assert recovered.stat().st_mode & 0o777 == 0o555
    assert (recovered / "nested").stat().st_mode & 0o777 == 0o555
    finished_receipts = request_receipts()
    assert [row[0] for row in finished_receipts] == [row[0] for row in waiting_receipts], finished_receipts
    assert next(row for row in finished_receipts if row[0] == waiting_request[0])[2] == 5, finished_receipts
    intervening = events()[before_close:]
    assert not [e for e in intervening if e["title"] == "Recover Saved report.txt"
                and "cancelled" in e["body"] and e["body"] not in known_outcomes
                and "handed to another request" not in e["body"]], intervening
    (session.output / "windowless-decision.json").write_text(json.dumps({"same_process": process.pid,
        "waiting_request": waiting_request, "unchanged_receipts_until_explicit_answer": True,
        "no_successor_request": True, "finished_receipts": finished_receipts,
        "outcome_events": intervening}, indent=2) + "\n")
    checks.append("all windows can close without answering Recovery; activation reuses the owner and Review explicitly keeps both")

    focus(browser)
    chord("l")
    location = uri(recovered / copy_name)
    session.start(["wl-copy", "--foreground", "--type", "text/plain", location], "recovered-location-clipboard.log")
    session.wait_for(lambda: session.run(["wl-paste", "--no-newline"]) == location, "private recovered path clipboard")
    chord("v")
    key("Return")
    session.wait_for(lambda: title("Recovered directory"), "browse the recovered file's folder")
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    session.wait_for(lambda: next(w for w in windows() if w["id"] == browser["id"])["layout"]["window_size"][0] == 340, "actual narrow recovered-folder Browser")
    key("space")
    preview = session.wait_for(lambda: title(f"Quick Look: {copy_name}"), "recovered directory child preview")
    assert preview["is_floating"]
    outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
    logical = next(o["logical"] for o in outputs.values() if o.get("logical"))
    width = min(760, logical["width"] - 350)
    assert width >= 320
    focus(preview)
    session.run(["niri", "msg", "action", "set-window-width", str(width)])
    session.wait_for(lambda: next(w for w in windows() if w["id"] == preview["id"])["layout"]["window_size"][0] == width, "floating preview fits beside the narrow Browser")
    session.run(["niri", "msg", "action", "move-floating-window", "--id", str(preview["id"]), "-x", "350", "-y", "0"])
    time.sleep(.2)
    session.capture("recovery-directory-narrow-preview.png")
    key("Escape")
    focus(browser)
    before = len(events())
    chord("z")
    notice(before, lambda e: "reversed" in e["body"])
    session.wait_for(lambda: not (recovered / copy_name).exists() and not (recovered / "nested").exists(), "Undo reverses only the continuation")
    assert (recovered / "a-padding.txt").exists()
    before = len(events())
    chord("z")
    undo = session.wait_for(lambda: title("Undo needs a decision"), "older Recovery prefix reviews the nonempty output root")
    focus(undo)
    key("Tab")  # explicit Keep item preserves the independently created child
    key("Return")
    notice(before, lambda e: "reversed" in e["body"])
    assert list(recovered.iterdir()) == [recovered / note]
    assert (recovered / note).read_bytes() == newer_bytes
    assert (saved / note).read_bytes() == source_bytes
    assert (saved / "nested/c.txt").read_bytes() == b"Nested recovered document\n"
    assert (unrelated / "unrelated.txt").read_bytes() == b"Unrelated replacement folder\n"
    assert held.read_bytes() == b"Previous saved file\n"
    assert current.read_bytes() == b"Incoming replacement\n"
    assert all(p.stat().st_mode & 0o777 == 0o555 for p in (saved, saved / "nested", recovered))
    session.capture("recovery-directory-undo-preserves-newer-work.png")
    checks.append("separate Undo entries reverse continuation and prefix while keeping newer output and all source versions")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "Iced directory Recovery, output-root refusal, exact continuation, all-window decision lifetime and Undo; reconciliation, retry failures, expiry and full product acceptance remain open"}
