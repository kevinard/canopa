"""Exact failed work remains retryable after the last browser closes."""
import hashlib
import json
import os
import subprocess
import time


def verify(session, fixture, browser, scheme, scale, binary, process, *, keep_open=False):
    source = fixture / "Unfinished request"
    source.mkdir()
    large = source / "a-completed.bin"
    with large.open("wb") as output:
        for _ in range(16):
            output.write(b"Canopa completed prefix.\n" * 43690)
    blocked = source / "z-retry.txt"
    os.mkfifo(blocked)
    destination = fixture / "Job destination"
    destination.mkdir()
    completed = destination / source.name / large.name
    retried = destination / source.name / blocked.name

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((w for w in windows() if w.get("title") == value), None)

    def wait_title(value):
        result = session.wait_for(lambda: title(value), value)
        time.sleep(.3)
        return result

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name])
            time.sleep(.2)

    def ctrl(key):
        session.run(["wtype", "-M", "ctrl", "-k", key, "-m", "ctrl"])
        time.sleep(.25)

    def notices():
        path = session.output / "notifications.json"
        return json.loads(path.read_text()) if path.exists() else []

    def digest(path):
        with path.open("rb") as stream:
            return hashlib.file_digest(stream, "sha256").hexdigest()

    if keep_open:
        ctrl("n")
        session.wait_for(lambda: any(w.get("app_id") == "org.canopa.Files" and w["id"] != browser["id"] and w["is_focused"] for w in windows()),
                         "control keeps the original browser while copying from a second window")
    session.start(["wl-copy", "--foreground", "--type", "text/uri-list", source.as_uri() + "\r\n"], "job-lifetime-clipboard.log")
    def clipboard_ready():
        try:
            return session.run(["wl-paste", "--type", "text/uri-list"]).strip() == source.as_uri()
        except subprocess.CalledProcessError:
            return False

    session.wait_for(clipboard_ready,
                     "external clipboard owns the exact source without holding Files alive")
    ctrl("l")
    session.run(["wtype", str(destination)])
    keys("Return")
    wait_title("Job destination — Canopa Files")
    ctrl("v")

    def copying():
        for stage in destination.rglob(".canopa-files-stage-*/folder"):
            try:
                if stage.is_file() and stage.stat().st_size > 64 * 1024:
                    return stage
            except FileNotFoundError:
                pass
        return None

    session.wait_for(copying, "real copy bytes exist before closing all browsers", timeout=40)
    for window in windows():
        if window.get("app_id") == "org.canopa.Files" and not (keep_open and window["id"] == browser["id"]):
            session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
    session.wait_for(lambda: not any(w.get("app_id") == "org.canopa.Files" and not (keep_open and w["id"] == browser["id"]) for w in windows()),
                     "originating Files windows close during the transfer")
    assert process.poll() is None, "active copy must retain the application lifetime"
    session.wait_for(lambda: any(n["title"] == "Copy 1 item to Job destination" and n["body"].startswith("Completed with errors") for n in notices()),
                     "windowless copy reports its failed child", timeout=60)
    assert completed.exists() and digest(completed) == digest(large)
    assert not retried.exists()
    inode = completed.stat().st_ino
    # The session guard treats an exited managed process as unexpected. This
    # boundary deliberately observes natural exit, and restores cleanup ownership
    # if the process stays alive or another fixture assertion fails.
    if keep_open:
        time.sleep(.8)
        assert process.poll() is None
    else:
        session.processes.remove(process)
        try:
            session.wait_for(lambda: process.poll() is not None, "application settles after its last job and notification")
        finally:
            if process.poll() is None:
                session.processes.append(process)
        assert process.returncode == 0

    # Repair only the unsupported source. Retry must preserve the completed prefix.
    blocked.unlink()
    blocked.write_bytes(b"Repaired after the original process exited.\n")
    if keep_open:
        session.run([str(binary), str(destination)])
        expected_pid = process.pid
    else:
        restarted = session.start([str(binary), str(destination)], "files-restarted.log")
        expected_pid = restarted.pid
    reopened = wait_title("Job destination — Canopa Files")
    assert reopened["pid"] == expected_pid
    assert (reopened["pid"] == process.pid) == keep_open
    ctrl("j")
    operations = wait_title("Operations — Canopa Files")
    assert operations["is_focused"]
    session.capture("job-lifetime-01-reopened-operations.png")
    (session.output / "job-lifetime-observation.json").write_text(json.dumps({
        "original_pid": process.pid, "original_exit": process.returncode,
        "reopened_pid": reopened["pid"], "same_process_control": keep_open,
        "operations_focused": operations["is_focused"], "completed_inode": inode,
        "completed_sha256": digest(completed), "source_sha256": digest(large),
        "failed_destination_exists_before_retry": retried.exists(),
        "repaired_source_text": blocked.read_text(),
        "notifications": notices(),
    }, indent=2))
    session.run(["wtype", "-M", "alt", "-k", "r", "-m", "alt"])
    session.wait_for(retried.exists, "reopened Operations retries the exact failed child")
    assert retried.read_bytes() == blocked.read_bytes()
    assert completed.stat().st_ino == inode and digest(completed) == digest(large)
    time.sleep(.4)
    session.capture("job-lifetime-02-exact-retry.png")
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "checks": ["active copy outlives its originating browser" if keep_open else "active copy outlives every browser",
                       "same-process Operations retains the exact failed request" if keep_open else "natural exit preserves the exact failed request",
                       "reopened Operations retries only the repaired child and preserves completed output"], "windows": windows()}
