"""Clipboard-owned retained bytes through actual native history expiry."""
import json
import os
import signal
import subprocess
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Source lease fixtures"
    assert root.is_relative_to(session.runtime / "home")
    for name in ("Incoming", "Destination", "History", "Paste"):
        (root / name).mkdir(parents=True)
    (root / "Incoming/note.txt").write_bytes(b"Current destination bytes\n")
    (root / "Destination/note.txt").write_bytes(b"Clipboard retained bytes\n")


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Source lease fixtures"
    old = b"Clipboard retained bytes\n"
    current = b"Current destination bytes\n"
    checks = []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def chord(key, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", key]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.08)

    def uri(path):
        return "file://" + quote_from_bytes(os.fsencode(path), safe="/")

    def clipboard(mime="x-special/gnome-copied-files"):
        result = subprocess.run(["wl-paste", "--no-newline", "--type", mime],
                                env=session.env, capture_output=True, timeout=3)
        return result.stdout if result.returncode == 0 else None

    def focus():
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "source lease Browser focus")

    def navigate(path):
        focus()
        chord("l", "ctrl")
        session.run(["wtype", uri(path)])
        chord("Return")
        directory = path if path.is_dir() else path.parent
        session.wait_for(lambda: title(directory.name), "source lease folder navigation")
        time.sleep(.25)

    def notifications():
        path = session.output / "notifications.json"
        return json.loads(path.read_text()) if path.exists() else []

    def create(index):
        name = f"History {index:03}"
        chord("n", "ctrl", "shift")
        prompt = session.wait_for(lambda: title("New folder"), "history-expiry folder prompt")
        session.wait_for(lambda: any(w["id"] == prompt["id"] and w["is_focused"] for w in windows()), "history-expiry name focus")
        session.run(["wtype", name])
        chord("Return")
        session.wait_for(lambda: (root / "History" / name).is_dir() and title("New folder") is None, "actual history-expiry folder creation")
        session.wait_for(lambda: any(e["title"] == f"create folder {name}" for e in notifications()), "history-expiry completion notification")
        time.sleep(.1)

    focus()
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    navigate(root / "Incoming/note.txt")
    chord("c", "ctrl")
    session.wait_for(lambda: clipboard() == ("copy\n" + uri(root / "Incoming/note.txt") + "\n").encode(), "initial Copy")
    navigate(root / "Destination")
    chord("v", "ctrl")
    conflict = session.wait_for(lambda: title("File conflict"), "actual replacement question")
    session.wait_for(lambda: any(w["id"] == conflict["id"] and w["is_focused"] for w in windows()), "replacement Cancel focus")
    chord("Tab")
    chord("Return")
    session.wait_for(lambda: title("File conflict") is None and (root / "Destination/note.txt").read_bytes() == current, "actual destination replacement")
    candidates = [path for path in (root / "Destination").rglob("*")
                  if not path.is_symlink() and path.is_file() and path.read_bytes() == old]
    assert len(candidates) == 1, candidates
    saved = candidates[0]
    (session.output / "retained-path.json").write_text(json.dumps({"path": str(saved)}))
    navigate(saved)
    chord("c", "ctrl")
    expected = ("copy\n" + uri(saved) + "\n").encode()
    session.wait_for(lambda: clipboard() == expected, "Copy of actual retained bytes")
    checks.append("a narrow Iced Browser publishes a real retained version through the native file clipboard")
    chord("F2")
    rename = session.wait_for(lambda: title("Rename"), "Rename of reserved retained bytes")
    session.wait_for(lambda: any(w["id"] == rename["id"] and w["is_focused"] for w in windows()), "reserved Rename name focus")
    session.run(["wtype", "Attempted reserved rename"])
    chord("Return")
    session.wait_for(lambda: title("Rename") is None, "reserved Rename admission")
    session.wait_for(lambda: any(e["title"].lower().startswith("rename ") for e in notifications()), "reserved Rename completion")
    assert saved.exists(), "Rename consumed the clipboard-owned retained version"
    assert saved.read_bytes() == old and not (saved.parent / "Attempted reserved rename").exists()
    assert clipboard() == expected
    checks.append("Rename reports refusal while a clipboard lease needs the retained version and preserves its bytes and selection")
    navigate(root / "History")
    for index in range(100):
        create(index)
    assert saved.read_bytes() == old, "owned Copy lost retained bytes when its original Undo entry expired"
    assert clipboard() == expected
    session.capture("source-lease-after-expiry.png")
    checks.append("100 actual native folder operations expire the old Undo entry while owned Copy preserves its bytes")
    navigate(root / "Paste")
    chord("v", "ctrl")
    copied = root / "Paste" / saved.name
    session.wait_for(copied.exists, "Paste after history expiry")
    session.wait_for(lambda: sum(e["title"].lower().startswith("copy") for e in notifications()) >= 2, "Paste completion")
    assert copied.read_bytes() == old and saved.read_bytes() == old
    checks.append("Paste after expiry copies the exact retained bytes through durable request admission")
    with (session.output / "superseding-owner.log").open("w") as log:
        peer = subprocess.Popen(["wl-copy", "--foreground", "--type", "text/plain", "New clipboard owner"], env=session.env, stdout=log, stderr=log, start_new_session=True)
    try:
        session.wait_for(lambda: clipboard("text/plain") == b"New clipboard owner", "external clipboard ownership")
        navigate(root / "History")
        # Native clipboard observation does not acknowledge Iced's ownership
        # event or the metadata worker's queued release. Exercise bounded public
        # expiry opportunities while those independent event streams settle.
        for index in range(100, 110):
            create(index)
            if not saved.exists():
                break
        assert not saved.exists(), "released expired retained bytes were not cleaned up"
        assert copied.read_bytes() == old
        assert (root / "Destination/note.txt").read_bytes() == current
        assert (root / "Incoming/note.txt").read_bytes() == current
        assert len(list((root / "History").iterdir())) == index + 1
        assert clipboard("text/plain") == b"New clipboard owner"
        session.capture("source-lease-released.png")
    finally:
        if peer.poll() is None:
            os.killpg(peer.pid, signal.SIGKILL)
        peer.wait(timeout=5)
    checks.append("superseding ownership releases expired bytes while preserving the pasted copy, current files and foreign clipboard")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks}
