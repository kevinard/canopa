"""Review ordinary requests without replay, including actual result-save death."""
import hashlib
import json
import os
import signal
import sqlite3
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Request review"
    assert root.is_relative_to(session.runtime / "home")
    root.mkdir()
    (root / "source.txt").write_bytes(b"Original selected bytes\n")
    (root / "existing.txt").write_bytes(b"Unrelated existing bytes\n")
    config.write_text(config.read_text() + 'files { shortcut "operations" "<Alt>o"; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Request review"
    source = root / "source.txt"
    existing = root / "existing.txt"
    target = root / "committed.txt"
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
                                    for item in windows()), "request-review focus")

    def key(name, modifier=None):
        command = ["wtype"]
        if modifier:
            command += ["-M", modifier]
        command += ["-k", name]
        if modifier:
            command += ["-m", modifier]
        session.run(command)
        time.sleep(.2)

    def navigate(path):
        focus(browser)
        key("l", "ctrl")
        session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(path), safe="/")])
        key("Return")
        session.wait_for(lambda: title(root.name), "request-review Browser location")
        time.sleep(.3)

    def rename(name):
        key("F2")
        draft = session.wait_for(lambda: title("Rename"), "native Rename draft")
        focus(draft)
        session.run(["wtype", name])
        key("Return")
        session.wait_for(lambda: title("Rename") is None, "Rename durably admitted")

    def read(query, values=()):
        with sqlite3.connect(history.as_uri() + "?mode=ro", uri=True) as database:
            return database.execute(query, values).fetchall()

    def operations():
        focus(browser)
        key("o", "alt")
        window = session.wait_for(lambda: title("Operations"), "native request review")
        focus(window)
        session.run(["niri", "msg", "action", "set-window-width", "360"])
        session.wait_for(lambda: title("Operations")["layout"]["window_size"][0] == 360,
                         "actual narrow request review")
        return window

    def dismiss(request, has_undo, capture):
        current = operations()
        # New Browser, optional shared Undo, Recovery, then this newest row's
        # Dismiss request. No synthetic click or direct scheduling write.
        for _ in range(4 if has_undo else 3):
            key("Tab")
        session.capture(capture)
        key("Return")
        session.wait_for(lambda: read("SELECT phase FROM requests WHERE id=?", (request,)) == [(5,)],
                         "review dismissal saved")
        body = json.loads(read("SELECT body FROM requests WHERE id=?", (request,))[0][0])
        assert "dismissed after review" in body["Completed"]["detail"]
        focus(current)
        key("w", "ctrl")
        session.wait_for(lambda: title("Operations") is None, "review window closes")

    navigate(source)
    rename(existing.name)
    failed = session.wait_for(lambda: next((row[0] for row in read(
        "SELECT id FROM requests WHERE phase=2 ORDER BY id DESC")), None), "failed colliding Rename")
    assert source.read_bytes() == b"Original selected bytes\n"
    assert existing.read_bytes() == b"Unrelated existing bytes\n"
    dismiss(failed, False, "dismissal-failed-narrow.png")
    assert source.read_bytes() == b"Original selected bytes\n"
    assert existing.read_bytes() == b"Unrelated existing bytes\n"
    checks.append("native narrow Dismiss request settles a failed Rename without changing either file")

    with sqlite3.connect(history) as database:
        database.execute("CREATE TRIGGER refuse_result BEFORE UPDATE OF phase ON requests "
                         "WHEN OLD.phase=1 AND NEW.phase<>1 BEGIN "
                         "SELECT RAISE(ABORT,'private native result-save refusal'); END")
    navigate(source)
    rename(target.name)
    session.wait_for(target.exists, "actual Rename commits before result loss")
    execution = session.wait_for(lambda: next(iter(read(
        "SELECT e.request,e.operation,e.payload_sha256 FROM request_executions e "
        "JOIN requests r ON r.id=e.request "
        "JOIN request_mutation_ends m ON m.request=e.request WHERE r.phase=1 AND m.ended=1 "
        "AND EXISTS(SELECT 1 FROM changes c WHERE c.operation=e.operation) "
        "AND NOT EXISTS(SELECT 1 FROM pending)")), None), "confirmed execution and refused result save")
    request, operation, digest = execution
    original_body = read("SELECT body FROM requests WHERE id=?", (request,))[0][0]
    assert hashlib.sha256(original_body).digest() == digest
    # This exit is the stimulus, so it must not remain a required live service
    # in Session.wait_for. Re-enrol it for cleanup if the expected exit fails.
    session.processes.remove(process)
    try:
        process.kill()
        session.wait_for(lambda: process.poll() is not None, "actual result-save process death")
    finally:
        if process.poll() is None:
            session.processes.append(process)
    assert process.returncode == -signal.SIGKILL
    session.wait_for(lambda: all(window.get("pid") != process.pid for window in windows()),
                     "killed request owner surfaces retire")
    newer = b"Newer edits after the actual process died\n"
    target.write_bytes(newer)
    identity = (target.stat().st_dev, target.stat().st_ino)
    with sqlite3.connect(history) as database:
        database.execute("DROP TRIGGER refuse_result")
    (session.output / "dismissal-crash.json").write_text(json.dumps({
        "pid": process.pid, "returncode": process.returncode, "request": request,
        "operation": operation, "payload_sha256": digest.hex(), "identity": identity,
    }, indent=2) + "\n")
    process = session.start([str(binary), str(root)], "files-dismissal-restarted.log")
    browser = session.wait_for(lambda: title(root.name), "restarted request-review Browser")
    assert browser["pid"] == process.pid
    assert not source.exists() and target.read_bytes() == newer
    session.wait_for(lambda: read("SELECT phase FROM requests WHERE id=?", (request,)) == [(5,)],
                     "exact recorded completion restores its result")
    body = json.loads(read("SELECT body FROM requests WHERE id=?", (request,))[0][0])
    assert "recorded result was restored" in body["Completed"]["detail"]
    current = operations()
    session.capture("completion-restarted-finished-narrow.png")
    focus(current)
    key("w", "ctrl")
    session.wait_for(lambda: title("Operations") is None, "completed Operations window closes")
    assert target.read_bytes() == newer and not source.exists()
    assert (target.stat().st_dev, target.stat().st_ino) == identity
    assert read("SELECT operation,payload_sha256 FROM request_executions WHERE request=?", (request,)) == [(operation, digest)]
    assert read("SELECT count(*) FROM changes WHERE operation=?", (operation,)) == [(1,)]
    checks.append("real SIGKILL after commit/result-save refusal restores exact completion without replay; newer bytes, inode and execution evidence survive")

    focus(browser)
    key("z", "ctrl")
    session.wait_for(lambda: source.exists() and not target.exists(), "original Rename Undo after completion restoration")
    assert source.read_bytes() == newer
    assert (source.stat().st_dev, source.stat().st_ino) == identity
    assert existing.read_bytes() == b"Unrelated existing bytes\n"
    session.capture("dismissal-preserved-undo.png")
    checks.append("the original committed Rename remains independently undoable and returns all post-crash edits")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "Iced failed-request dismissal and exact ordinary-mutation completion after actual commit/result-save death; no filesystem replay or complete durable continuation claim"}
