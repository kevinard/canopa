"""Real interrupted Undo, salvage and explicit reconciliation through Iced."""
import hashlib
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import sys
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Interrupted Recovery"
    assert root.is_relative_to(session.runtime / "home")
    (root / "Source").mkdir(parents=True)
    (root / "Target").mkdir()
    line = b"Source data preserved for Recovery.\n"
    (root / "Source/report.txt").write_bytes(b"Preserved from interrupted Undo.\n" + line * (16 * 1024 * 1024 // len(line)))
    config.write_text(config.read_text() + 'files { shortcut "recovery" "<Alt>r"; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Interrupted Recovery"
    source = root / "Source/report.txt"
    target = root / "Target"
    original = target / "report.txt"
    rescued = root / "Rescued work"

    def rescued_bytes():
        return [digest(path) for path in sorted(rescued.rglob("*")) if path.is_file()]
    newer = b"Newer occupant after interruption.\n"
    binary = Path(process.args[0])
    notifications = session.output / "notifications.json"
    checks = []

    def digest(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()

    expected = digest(source)

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "interrupted Recovery focus")
        time.sleep(.15)

    def key(name):
        session.run(["wtype", "-k", name])
        time.sleep(.2)

    def chord(name, modifier="ctrl"):
        session.run(["wtype", "-M", modifier, "-k", name, "-m", modifier])
        time.sleep(.25)

    def type_path(path):
        value = "file://" + quote_from_bytes(os.fsencode(path), safe="/")
        for start in range(0, len(value), 40):
            session.run(["wtype", value[start:start + 40]])

    def navigate(path):
        focus(browser)
        chord("l")
        type_path(path)
        key("Return")
        time.sleep(.5)

    def events():
        return json.loads(notifications.read_text()) if notifications.exists() else []

    def notice(before, predicate):
        return session.wait_for(lambda: next((e for e in events()[before:] if e["app"] == "Canopa Files" and predicate(e)), None), "this interrupted-work result", timeout=40)

    observed = {}

    def same_journal(expected, step):
        current = pending()
        if current != expected:
            (session.output / "journal-mismatch.json").write_text(json.dumps(
                {"step": step, "expected": observed.get(expected), "current": observed.get(current)}, indent=2) + "\n")
        return current == expected

    def pending():
        # Observe the private journal's identity without interpreting or writing
        # its execution payload. Copy-out must leave this primary intent intact.
        history = session.runtime / "state/canopa-files/operations.sqlite3"
        connection = sqlite3.connect(history.as_uri() + "?mode=ro", uri=True)
        try:
            row = connection.execute("SELECT body FROM pending WHERE singleton=1").fetchone()
            if row is None:
                return None
            identity = hashlib.sha256(row[0].encode()).hexdigest()
            observed.setdefault(identity, row[0])  # Failure evidence only.
            return identity
        finally:
            connection.close()

    def restart(log):
        child = session.start([str(binary), str(target)], log)
        window = session.wait_for(lambda: title("Target"), "restarted Browser")
        assert window["pid"] == child.pid
        recovery = session.wait_for(lambda: title("Recovery"), "startup presents interrupted Recovery")
        return child, window, recovery

    def reconcile(recovery):
        focus(recovery)
        key("Home")  # selects the payload and clears widget focus
        # Recover, Keep in Recovery, Refresh, Reconcile; the one-page More is disabled.
        for _ in range(4):
            key("Tab")
        session.capture("interrupted-before-reconcile.png")
        before = len(events())
        key("Return")
        return notice(before, lambda e: e["title"] == "Retry interrupted operation")

    navigate(source)
    chord("c")
    navigate(target)
    before = len(events())
    chord("v")
    notice(before, lambda e: e["title"].startswith("Copy ") and "1 completed" in e["body"])
    assert digest(original) == expected
    evidence = session.output / "interrupted-process.json"
    watcher = session.start([sys.executable, str(Path(__file__).with_name("files-interrupted.py")), str(process.pid), str(target), str(evidence)], "interruption-watcher.log")
    # These expected exits are the stimulus. Keep exact live children eligible
    # for cleanup if the quarantine boundary is not reached.
    session.processes.remove(process)
    session.processes.remove(watcher)
    try:
        chord("z")
        session.wait_for(lambda: process.poll() is not None and watcher.poll() is not None, "actual process termination during Undo", timeout=50)
    finally:
        for child in (process, watcher):
            if child.poll() is None:
                session.processes.append(child)
    assert watcher.returncode == 0 and process.returncode == -signal.SIGKILL
    session.wait_for(lambda: all(w.get("pid") != process.pid for w in windows()), "crashed Files surfaces retire before restart")
    quarantine = Path(json.loads(evidence.read_text())["quarantine"])
    assert quarantine.is_relative_to(target) and digest(quarantine) == expected
    original.write_bytes(newer)
    journal = pending()
    assert journal is not None
    process, browser, recovery = restart("files-interrupted-reopened.log")
    assert pending() == journal and original.read_bytes() == newer
    assert digest(quarantine) == expected
    checks.append("real native Undo is interrupted at quarantine; restart presents Recovery without replaying the original journal")

    focus(recovery)
    chord("w")
    session.wait_for(lambda: all(w["id"] != recovery["id"] for w in windows()), "dismissed startup Recovery retires")
    focus(browser)
    before = len(events())
    chord("z")
    notice(before, lambda e: e["title"] == "Undo" and "Recovery" in e["body"])
    assert title("Recovery") is None, "a normal queue update reopened dismissed startup attention"
    assert pending() == journal and original.read_bytes() == newer and digest(quarantine) == expected
    chord("r", "alt")
    recovery = session.wait_for(lambda: title("Recovery"), "explicitly reopened Recovery")
    checks.append("dismissing startup attention is respected while ordinary mutations stay blocked by the unresolved journal")

    focus(recovery)
    session.capture("interrupted-recovery-catalog.png")
    # ADR0142: Recovery exposes the interrupted Undo's whole work folder, which
    # holds the quarantined report, rather than the bare file.
    key("Home")
    key("space")
    preview = session.wait_for(lambda: title("Quick Look: Working files"), "work-folder preview of quarantined bytes")
    assert preview["is_floating"]
    session.capture("interrupted-quarantine-preview.png")
    key("Escape")
    session.wait_for(lambda: all(w["id"] != preview["id"] for w in windows()), "native preview retires")
    focus(recovery)
    key("Home")
    key("Return")
    draft = session.wait_for(lambda: title("Recover a copy"), "quarantine copy destination")
    focus(draft)
    key("Tab")
    type_path(rescued)
    before = len(events())
    key("Return")
    notice(before, lambda e: e["title"].startswith("Recover ") and "0 failed" in e["body"])
    session.wait_for(lambda: all(w["id"] != draft["id"] for w in windows()), "accepted copy draft retires")
    assert rescued_bytes() == [expected] and digest(quarantine) == expected
    assert original.read_bytes() == newer and pending() == journal
    checks.append("Recovery previews the interrupted work folder; its non-consuming copy-out holds the exact quarantined bytes, is separately journaled and leaves the original interruption intact")

    session.processes.remove(process)
    try:
        for window in windows():
            if window.get("pid") == process.pid:
                session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
        session.wait_for(lambda: process.poll() is not None, "orderly exit before the second actual restart")
    finally:
        if process.poll() is None:
            session.processes.append(process)
    assert process.returncode == 0
    process, browser, recovery = restart("files-interrupted-second-restart.log")
    assert pending() == journal and digest(quarantine) == expected
    assert rescued_bytes() == [expected] and original.read_bytes() == newer
    focus(recovery)
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    session.wait_for(lambda: title("Recovery")["layout"]["window_size"][0] == 340, "actual narrow interrupted Recovery")
    session.capture("interrupted-restarted-narrow.png")
    refusal = reconcile(recovery)
    assert "rechecked" not in refusal["body"].lower(), refusal
    assert original.read_bytes() == newer and digest(quarantine) == expected
    # ADR0147: the copy already left its name, so retrying records the Undo and
    # keeps the taken copy as preserved work for Recovery. It never moves the
    # copy back over, or next to, the newer occupant.
    assert pending() is not None, "refused reconciliation retired the interrupted work"
    assert "errno" not in refusal["body"].lower() and "os error" not in refusal["body"].lower(), refusal
    checks.append("copy-out and a second restart preserve the original journal; explicit reconciliation leaves the occupied original name and the preserved copy untouched")

    focus(recovery)
    key("Home")
    # Recover a copy, then Keep in Recovery: the second live control.
    for _ in range(2):
        key("Tab")
    session.capture("interrupted-before-keep.png")
    key("Return")
    session.wait_for(lambda: pending() is None, "Keep in Recovery retires the interrupted work")
    assert original.read_bytes() == newer and digest(quarantine) == expected
    session.capture("interrupted-kept.png")
    focus(browser)
    before = len(events())
    chord("z")
    notice(before, lambda e: "reversed" in e["body"])
    assert not rescued.exists() and original.read_bytes() == newer
    assert digest(quarantine) == expected and digest(source) == expected
    checks.append("Keep in Recovery retires the work without moving the preserved copy; the salvage keeps its own Undo")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "Iced interrupted Undo inspection, original-name preview, non-consuming salvage, two restarts and explicit reconciliation; general restart execution and full product acceptance remain open"}
