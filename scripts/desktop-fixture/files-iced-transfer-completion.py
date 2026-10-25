"""Completed Copy/Move survive real result-save death without filesystem replay."""
import hashlib
import json
import os
import signal
import sqlite3
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Transfer completion"
    assert root.is_relative_to(session.runtime / "home")
    for action in ("Copy", "Move"):
        bundle = root / f"{action} Input" / "bundle"
        (bundle / "nested").mkdir(parents=True)
        (bundle / "nested/note.txt").write_bytes(b"Original selected bytes\n")
        (root / f"{action} Target").mkdir()
    config.write_text(config.read_text() + 'files { shortcut "operations" "<Alt>o"; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Transfer completion"
    history = session.runtime / "state/canopa-files/operations.sqlite3"
    binary = process.args[0]
    checks = []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((window for window in windows()
                     if window.get("title") == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"]
                                    for item in windows()), "transfer-completion focus")

    def key(name, modifier=None):
        args = ["wtype"] + (["-M", modifier] if modifier else []) + ["-k", name]
        if modifier:
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.2)

    def navigate(path):
        focus(browser)
        key("l", "ctrl")
        session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(path), safe="/")])
        key("Return")
        session.wait_for(lambda: title(path.name), "transfer-completion Browser location")
        time.sleep(.3)

    def read(query, values=()):
        with sqlite3.connect(history.as_uri() + "?mode=ro", uri=True) as database:
            return database.execute(query, values).fetchall()

    for action in ("Copy", "Move"):
        source = root / f"{action} Input" / "bundle"
        destination = root / f"{action} Target"
        target = destination / "bundle/nested/note.txt"
        with sqlite3.connect(history) as database:
            database.execute("CREATE TRIGGER refuse_transfer_result BEFORE UPDATE OF phase ON requests "
                             "WHEN OLD.phase=1 AND NEW.phase<>1 BEGIN "
                             "SELECT RAISE(ABORT,'private transfer result-save refusal'); END")
        navigate(source.parent)
        key("a", "ctrl")
        key("c" if action == "Copy" else "x", "ctrl")
        navigate(destination)
        key("v", "ctrl")
        execution = session.wait_for(lambda: next(iter(read(
            "SELECT e.request,e.operation,e.payload_sha256,t.result FROM request_executions e "
            "JOIN requests r ON r.id=e.request JOIN request_transfer_ends t ON t.request=e.request "
            "WHERE r.phase=1 AND t.result IS NOT NULL AND NOT EXISTS(SELECT 1 FROM pending)")), None),
            f"{action} end saved before refused application result")
        request, operation, digest, receipt = execution
        recorded = json.loads(receipt)
        assert recorded["kind"] == action
        assert target.read_bytes() == b"Original selected bytes\n"
        assert hashlib.sha256(read("SELECT body FROM requests WHERE id=?", (request,))[0][0]).digest() == digest
        confirmed = read("SELECT ordinal,body FROM changes WHERE operation=? ORDER BY ordinal", (operation,))
        assert confirmed
        session.processes.remove(process)
        try:
            process.kill()
            session.wait_for(lambda: process.poll() is not None, f"actual {action} result-save death")
        finally:
            if process.poll() is None:
                session.processes.append(process)
        assert process.returncode == -signal.SIGKILL
        session.wait_for(lambda: all(window.get("pid") != process.pid for window in windows()),
                         "killed transfer owner surfaces retire")
        target.write_bytes(b"Newer completed target after death\n")
        identity = (target.stat().st_dev, target.stat().st_ino)
        source.mkdir(exist_ok=True)
        (source / "new-source.txt").write_bytes(b"New source work after death\n")
        with sqlite3.connect(history) as database:
            database.execute("DROP TRIGGER refuse_transfer_result")
        process = session.start([str(binary), str(destination)], f"files-{action.lower()}-restarted.log")
        browser = session.wait_for(lambda: title(destination.name), f"restarted {action} Browser")
        assert browser["pid"] == process.pid
        session.wait_for(lambda: read("SELECT phase FROM requests WHERE id=?", (request,)) == [(5,)],
                         f"exact recorded {action} result restores")
        result = json.loads(read("SELECT body FROM requests WHERE id=?", (request,))[0][0])["Completed"]
        assert result["transfer"]["operation"] == operation
        assert result["transfer"]["kind"] == action and result["transfer"]["completed"] > 0
        for field in ("operation", "kind", "destination", "completed", "bytes", "skipped", "destinations"):
            assert result["transfer"][field] == recorded[field], field
        assert "recorded filesystem result restored" in result["detail"]
        focus(browser)
        key("o", "alt")
        operations = session.wait_for(lambda: title("Operations"), "restored transfer Operations")
        focus(operations)
        session.run(["niri", "msg", "action", "set-window-width", "360"])
        session.wait_for(lambda: title("Operations")["layout"]["window_size"][0] == 360,
                         "narrow completed transfer result")
        session.capture(f"transfer-completion-{action.lower()}-finished.png")
        key("w", "ctrl")
        session.wait_for(lambda: title("Operations") is None, "completed Operations closes")
        assert target.read_bytes() == b"Newer completed target after death\n"
        assert (target.stat().st_dev, target.stat().st_ino) == identity
        assert (source / "new-source.txt").read_bytes() == b"New source work after death\n"
        assert read("SELECT ordinal,body FROM changes WHERE operation=? ORDER BY ordinal", (operation,)) == confirmed
        assert read("SELECT operation,payload_sha256 FROM request_executions WHERE request=?", (request,)) == [(operation, digest)]
        checks.append(f"native {action}, real SIGKILL and restart preserve newer target/source bytes, inode, exact report and original Undo steps")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "completed Copy/Move result restoration; unfinished plans remain review-only after restart"}
