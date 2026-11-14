"""Native review of seeded Rename claims; real crash boundaries are core-tested."""
import json
import os
from pathlib import Path
import sqlite3
import time


def setup(session, fixture, config):
    root = fixture / "Claim review"
    assert root.is_relative_to(session.runtime / "home")
    root.mkdir()
    (root / "Recovered").mkdir()
    config.write_text(config.read_text() + 'files { shortcut "recovery" "<Alt>r"; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Claim review"
    history = session.runtime / "state/canopa-files/operations.sqlite3"
    binary = process.args[0]
    checks = []
    claims = []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "claim review focus")
        time.sleep(.15)

    def key(name, modifier=None):
        command = ["wtype"]
        if modifier:
            command += ["-M", modifier]
        command += ["-k", name]
        if modifier:
            command += ["-m", modifier]
        session.run(command)
        time.sleep(.2)

    def read(query, values=()):
        with sqlite3.connect(history.as_uri() + "?mode=ro", uri=True) as db:
            return db.execute(query, values).fetchall()

    def stop(owner):
        # Only this fixture's idle owner; no operation is being interrupted here.
        session.processes.remove(owner)
        try:
            owner.terminate()
            owner.wait(timeout=10)
        finally:
            if owner.poll() is None:
                session.processes.append(owner)
        session.wait_for(lambda: all(w.get("pid") != owner.pid for w in windows()), "private claim owner surfaces retire")

    def restart(label, interrupted=True):
        owner = session.start([str(binary), str(root)], label)
        current = session.wait_for(lambda: title(root.name), "restarted claim Browser")
        assert current["pid"] == owner.pid
        if not interrupted:
            focus(current)
            key("r", "alt")
        recovery = session.wait_for(lambda: title("Recovery"), "startup claim review")
        return owner, current, recovery

    def identity(path):
        data = path.lstat()
        return {"device": data.st_dev, "inode": data.st_ino}

    stop(process)
    for directory in (False, True):
        name = "folder" if directory else "file"
        source = root / f"source-{name}"
        target = root / f"renamed-{name}"
        receipt = root / f".canopa-files-stage-native-{name}"
        receipt.mkdir(mode=0o700)
        payload = root / (receipt.name + "-entry") if directory else receipt / "folder"
        if directory:
            payload.mkdir()
            (payload / "child").write_bytes(b"Claimed readonly child\n")
            payload.chmod(0o555)
        else:
            payload.write_bytes(b"Claimed file bytes\n")
        source.write_bytes(b"Newer source occupant\n")
        target.write_bytes(b"Newer destination occupant\n")
        expected = identity(payload)
        with sqlite3.connect(history) as db:
            assert db.execute("PRAGMA user_version").fetchone() == (60,)
            assert db.execute("SELECT count(*) FROM pending").fetchone() == (0,)
            operation = db.execute("INSERT INTO operations(label) VALUES(?)", (f"rename source-{name} to renamed-{name}",)).lastrowid
            pending = {"direction": "forward", "id": operation, "ordinal": 0,
                       "change": {"kind": "rename", "original": str(source), "current": str(target),
                                  "identity": expected, "retained": None},
                       "stage": {"directory": str(receipt), "identity": identity(receipt),
                                 "parent_identity": identity(root), "parent_access": {"kind": "existing"},
                                 "payload_identity": None, "sibling_payload": directory}}
            db.execute("INSERT INTO pending(singleton,body) VALUES(1,?)", (json.dumps(pending),))
        process, browser, recovery = restart(f"files-claim-{name}.log")
        focus(recovery)
        session.run(["niri", "msg", "action", "set-window-width", "360"])
        session.wait_for(lambda: title("Recovery")["layout"]["window_size"][0] == 360, "actual narrow claim review")
        key("Home")
        # Recover a copy, then Keep in Recovery (Space previews the row; there
        # is no Quick Look button). Disabled controls are omitted from the
        # actual focus order; activate the second live control.
        for _ in range(2):
            key("Tab")
        session.capture(f"claim-{name}-before-keep-narrow.png")
        key("Return")
        session.wait_for(lambda: read("SELECT count(*) FROM pending") == [(0,)], "reviewed journal retired")
        session.wait_for(lambda: len(read("SELECT id FROM garbage")) == len(claims) + 1, "protected claim owner saved")
        records = [json.loads(row[0]) for row in read("SELECT body FROM garbage ORDER BY id")]
        record = records[-1]
        assert record["payload"] and record["retained"]["recovery"]
        assert record["retained"]["stage"]["directory"] == str(receipt)
        assert read("SELECT count(*) FROM changes WHERE operation=?", (operation,)) == [(0,)]
        assert identity(payload) == expected and receipt.is_dir()
        assert source.read_bytes() == b"Newer source occupant\n"
        assert target.read_bytes() == b"Newer destination occupant\n"
        if directory:
            assert payload.stat().st_mode & 0o777 == 0o555
        claims.append({"payload": str(payload), "receipt": str(receipt), "identity": expected,
                       "operation": operation, "directory": directory})
        checks.append(f"narrow native {name} Keep transfers protected ownership without moving data, overwriting either occupant or fabricating Rename/Undo")
        stop(process)

    process, browser, recovery = restart("files-claim-kept-restarted.log", interrupted=False)
    assert read("SELECT count(*) FROM pending") == [(0,)]
    focus(recovery)
    key("End")  # newest archive is the readonly directory
    session.capture("claim-kept-after-restart.png")
    key("Return")
    dialog = session.wait_for(lambda: title("Recover a copy"), "kept directory copy-out draft")
    focus(dialog)
    destination = str(root / "Recovered")
    for start in range(0, len(destination), 40):
        session.run(["wtype", destination[start:start + 40]])
    key("Return")
    output = root / "Recovered/renamed-folder/child"
    session.wait_for(output.exists, "actual copied kept directory child")
    # File publication precedes destination-dialog retirement. Observe the
    # application's own return to Recovery before asking to focus a Browser.
    session.wait_for(lambda: title("Recover a copy") is None, "claim copy destination retires")
    session.wait_for(lambda: any(w["id"] == recovery["id"] and w["is_focused"] for w in windows()), "claim copy returns focus to Recovery")
    (session.output / "claim-copy-retired-windows.json").write_text(json.dumps(windows(), indent=2) + "\n")
    assert output.read_bytes() == b"Claimed readonly child\n"
    for claim in claims:
        assert identity(Path(claim["payload"])) == claim["identity"]
        assert Path(claim["receipt"]).is_dir()
    checks.append("restarted native Recovery copies the kept readonly directory and preserves both archived payloads")
    focus(browser)
    (session.output / "claim-browser-focused-windows.json").write_text(json.dumps(windows(), indent=2) + "\n")
    session.run(["wtype", "-M", "ctrl", "-M", "shift", "-k", "n", "-m", "shift", "-m", "ctrl"])
    dialog = session.wait_for(lambda: title("New folder"), "ordinary folder creation after claim review")
    focus(dialog)
    session.run(["wtype", "after-review"])
    key("Return")
    session.wait_for(lambda: (root / "after-review").is_dir(), "ordinary work unblocked after review")
    assert read("SELECT count(*) FROM pending") == [(0,)]
    for claim in claims:
        assert read("SELECT count(*) FROM changes WHERE operation=?", (claim["operation"],)) == [(0,)]
    checks.append("ordinary native Create Folder proceeds after explicit claim review; original Rename steps remain unconfirmed")
    (session.output / "claim-review-facts.json").write_text(json.dumps(claims, indent=2) + "\n")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "Seeded post-interruption journals exercise native review/copy-out; actual SIGKILL claims and failure atomicity are verified by core tests."}
