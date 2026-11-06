"""Real Iced file commands and native clipboard peers in the private desktop."""
import json
import os
import signal
import subprocess
import time
from contextlib import contextmanager
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Clipboard fixtures"
    assert root.is_relative_to(session.runtime / "home")
    root.mkdir()
    (root / "original.txt").write_bytes(b"Original clipboard bytes\n")
    (root / "move.txt").write_bytes(b"Move keeps these bytes\n")
    (root / "delayed.txt").write_bytes(b"Delayed clipboard bytes\n")
    (root / "Destination").mkdir()
    (root / "Captured destination").mkdir()
    (root / "Later folder").mkdir()


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Clipboard fixtures"

    def chord(key, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", key]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.2)

    def uri(path):
        return "file://" + quote_from_bytes(os.fsencode(path), safe="/")

    def clipboard(mime="x-special/gnome-copied-files"):
        args = ["wl-paste", "--no-newline"] + (["--type", mime] if mime else [])
        result = subprocess.run(args,
                                env=session.env, capture_output=True, timeout=3)
        return result.stdout if result.returncode == 0 else None

    @contextmanager
    def external(payload, label, observe=True):
        with (session.output / f"{label}.log").open("w") as output:
            peer = subprocess.Popen(["wl-copy", "--foreground", "--type", "text/uri-list", payload],
                                    env=session.env, stdout=output, stderr=output, start_new_session=True)
        try:
            if observe:
                session.wait_for(lambda: clipboard("text/uri-list") == payload.encode(), "external file selection")
            yield peer
        finally:
            if peer.poll() is None:
                os.killpg(peer.pid, signal.SIGCONT)
                os.killpg(peer.pid, signal.SIGKILL)
            peer.wait(timeout=5)

    def navigate(path):
        chord("l", "ctrl")
        session.run(["wtype", uri(path)])
        chord("Return")
        directory = path if path.is_dir() else path.parent
        session.wait_for(lambda: any(w.get("title") == f"{directory.name} — Canopa Files"
                                    for w in json.loads(session.run(["niri", "msg", "--json", "windows"]))),
                         "clipboard folder navigation")
        time.sleep(.3)

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "clipboard Browser focus")

    def question():
        return next((w for w in windows() if w.get("title") == "File conflict — Canopa Files"), None)

    def notices(prefix):
        events = session.output / "notifications.json"
        if not events.exists():
            return []
        records = json.loads(events.read_text())
        if prefix == "Undo":
            return [e for e in records if e["body"].split(" ·", 1)[0].endswith(" reversed")]
        return [e for e in records if e["title"].startswith(prefix)]

    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"]
                                for w in json.loads(session.run(["niri", "msg", "--json", "windows"]))),
                     "clipboard Browser focus")
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    time.sleep(.3)
    chord("l", "ctrl")
    session.run(["wtype", uri(root / "original.txt")])
    session.capture("clipboard-location-input.png")
    chord("Return")
    session.wait_for(lambda: any(w.get("title") == "Clipboard fixtures — Canopa Files"
                                for w in json.loads(session.run(["niri", "msg", "--json", "windows"]))),
                     "clipboard source navigation")
    time.sleep(.3)
    session.capture("clipboard-copy-narrow.png")
    chord("c", "ctrl")
    expected = ("copy\n" + uri(root / "original.txt") + "\n").encode()
    session.wait_for(lambda: clipboard() == expected, "Iced Copy available to an independent native client")
    assert (root / "original.txt").read_bytes() == b"Original clipboard bytes\n"
    checks = ["Ctrl+C in a narrow Iced Browser publishes the selected real file to an independent native client"]
    destination = root / "Destination/original.txt"
    navigate(destination.parent)
    chord("v", "ctrl")
    session.wait_for(destination.exists, "actual Iced Paste copy")
    assert destination.read_bytes() == b"Original clipboard bytes\n"
    assert (root / "original.txt").read_bytes() == b"Original clipboard bytes\n"
    events = session.output / "notifications.json"
    session.wait_for(lambda: events.exists() and any(e["title"].startswith("Copy")
                                                   for e in json.loads(events.read_text())), "copy completion notification")
    session.capture("clipboard-pasted-copy.png")
    chord("z", "ctrl")
    session.wait_for(lambda: not destination.exists(), "shared Undo of clipboard copy")
    session.wait_for(lambda: notices("Undo"), "Undo completion")
    assert (root / "original.txt").read_bytes() == b"Original clipboard bytes\n"
    checks.append("Ctrl+V copies actual bytes to the captured folder, sends completion and exposes the copy through shared Ctrl+Z")
    navigate(root)
    destination.write_bytes(b"Existing destination stays intact\n")
    navigate(destination.parent)
    chord("v", "ctrl")
    dialog = session.wait_for(question, "Iced file conflict")
    assert dialog["is_floating"]
    session.wait_for(lambda: any(w["id"] == dialog["id"] and w["is_focused"] for w in windows()), "default Cancel focus")
    session.capture("clipboard-file-conflict.png")
    chord("Return")
    session.wait_for(lambda: question() is None, "default Cancel closes the conflict")
    session.wait_for(lambda: len(notices("Copy")) >= 2, "cancelled copy completion")
    assert destination.read_bytes() == b"Existing destination stays intact\n"
    assert (root / "original.txt").read_bytes() == b"Original clipboard bytes\n"
    checks.append("file conflicts float above a narrow Browser and default Cancel preserves both actual versions")
    chord("v", "ctrl")
    session.wait_for(question, "second Iced file conflict")
    chord("Tab")
    chord("Tab")
    chord("Return")
    session.wait_for(lambda: question() is None, "Keep both closes the conflict")
    kept = session.wait_for(lambda: next((p for p in destination.parent.glob("original*.txt") if p != destination), None), "new Keep both destination")
    session.wait_for(lambda: len(notices("Copy")) >= 3, "Keep both completion")
    assert kept.read_bytes() == b"Original clipboard bytes\n"
    assert destination.read_bytes() == b"Existing destination stays intact\n"
    session.capture("clipboard-keep-both.png")
    chord("z", "ctrl")
    session.wait_for(lambda: not kept.exists(), "Undo only the Keep both result")
    session.wait_for(lambda: len(notices("Undo")) >= 2, "Keep both Undo completion")
    assert destination.read_bytes() == b"Existing destination stays intact\n"
    checks.append("Keep both creates a separate actual file and shared Undo preserves the existing destination")
    moving = root / "move.txt"
    moved = destination.parent / moving.name
    navigate(moving)
    chord("x", "ctrl")
    cut = ("cut\n" + uri(moving) + "\n").encode()
    session.wait_for(lambda: clipboard() == cut, "Iced Cut publication")
    navigate(moved.parent)
    chord("v", "ctrl")
    session.wait_for(lambda: moved.exists() and not moving.exists(), "actual Iced Cut/Paste move")
    session.wait_for(lambda: notices("Move"), "move completion")
    assert moved.read_bytes() == b"Move keeps these bytes\n"
    session.wait_for(lambda: clipboard() is None, "completed Move releases its original Cut selection")
    chord("z", "ctrl")
    session.wait_for(lambda: moving.exists() and not moved.exists(), "Undo clipboard move")
    session.wait_for(lambda: len(notices("Undo")) >= 3, "move Undo completion")
    assert moving.read_bytes() == b"Move keeps these bytes\n"
    checks.append("Cut/Paste moves the real file, clears only the consumed Cut and supports shared Undo")
    navigate(root)
    moved.write_bytes(b"Previous move destination\n")
    navigate(moving)
    chord("x", "ctrl")
    session.wait_for(lambda: clipboard() == cut, "second Cut publication")
    navigate(moved.parent)
    chord("v", "ctrl")
    session.wait_for(question, "Move Replace conflict")
    foreign = uri(root / "original.txt") + "\r\n"
    with external(foreign, "newer-owner"):
        chord("Tab")
        chord("Return")
        session.wait_for(lambda: not moving.exists() and moved.read_bytes() == b"Move keeps these bytes\n", "actual Move Replace")
        session.wait_for(lambda: len(notices("Move")) >= 2, "Move Replace completion")
        assert clipboard("text/uri-list") == foreign.encode()
        chord("z", "ctrl")
        session.wait_for(lambda: moving.exists() and moved.read_bytes() == b"Previous move destination\n", "Undo Move Replace")
        session.wait_for(lambda: len(notices("Undo")) >= 4, "Move Replace Undo completion")
        assert clipboard("text/uri-list") == foreign.encode()
    checks.append("Move Replace and Undo preserve both versions and a newer external clipboard owner")

    delayed = uri(root / "delayed.txt") + "\r\n"
    captured = root / "Captured destination/delayed.txt"
    later = root / "Later folder"
    with external(delayed, "delayed-owner") as peer:
        navigate(captured.parent)
        os.kill(peer.pid, signal.SIGSTOP)
        try:
            chord("v", "ctrl")
            navigate(later)
        finally:
            os.kill(peer.pid, signal.SIGCONT)
        session.wait_for(captured.exists, "Paste into the originally captured folder")
        session.wait_for(lambda: len(notices("Copy")) >= 4, "delayed Paste completion")
        assert captured.read_bytes() == b"Delayed clipboard bytes\n"
        assert not (later / "delayed.txt").exists()
        assert any(w["id"] == browser["id"] and w.get("title") == "Later folder — Canopa Files" for w in windows())
        session.capture("clipboard-captured-destination.png")
    checks.append("a delayed external read pastes into the captured destination while later navigation stays unchanged")

    navigate(root / "original.txt")
    chord("c", "ctrl")
    session.wait_for(lambda: clipboard() == expected, "file Copy before text editing")
    chord("l", "ctrl")
    chord("a", "ctrl")
    draft = "/a text selection without a file command"
    session.run(["wtype", draft])
    chord("a", "ctrl")
    chord("c", "ctrl")
    session.wait_for(lambda: clipboard(None) == draft.encode(), "Iced text-field Copy")
    assert clipboard() is None
    chord("Escape")
    chord("F10", "shift")
    session.capture("clipboard-context-menu.png")
    chord("Down")
    chord("Down")
    chord("Return")
    session.wait_for(lambda: clipboard() == expected, "Copy through the real context menu")
    checks.append("text fields retain their text clipboard and the context-menu Copy publishes the selected file")

    chord("n", "ctrl")
    other = session.wait_for(lambda: next((w for w in windows() if w.get("app_id") == "org.canopa.Files"
                                         and w["id"] != browser["id"]), None), "independent Browser for background attention")
    attention_before = len(notices("Operation needs a decision"))
    with external(foreign, "background-conflict-owner") as peer:
        for attempt in range(2):
            focus(browser)
            navigate(destination.parent)
            completed_before = len(notices("Copy"))
            os.kill(peer.pid, signal.SIGSTOP)
            try:
                chord("v", "ctrl")
                focus(other)
            finally:
                os.kill(peer.pid, signal.SIGCONT)
            count = attention_before + attempt + 1
            session.wait_for(lambda: len(notices("Operation needs a decision")) >= count,
                             "background transfer attention notification")
            assert question() is None, "a background question took focus"
            assert any(w["id"] == other["id"] and w["is_focused"] for w in windows())
            time.sleep(1)
            assert len(notices("Operation needs a decision")) == count, "the same question notified twice"
            assert destination.read_bytes() == b"Existing destination stays intact\n"
            assert (root / "original.txt").read_bytes() == b"Original clipboard bytes\n"
            if attempt == 0:
                session.capture("clipboard-background-question.png")
            focus(browser)
            session.wait_for(question, "foreground source opens the pending question")
            chord("Escape")
            session.wait_for(lambda: question() is None, "background conflict Cancel")
            session.wait_for(lambda: len(notices("Copy")) > completed_before, "background conflict completion")
    focus(other)
    chord("w", "ctrl")
    session.wait_for(lambda: not any(w["id"] == other["id"] for w in windows()), "close independent Browser")
    focus(browser)
    navigate(root / "original.txt")
    chord("c", "ctrl")
    session.wait_for(lambda: clipboard() == expected, "restore owned file Copy")
    checks.append("each background transfer question notifies once without taking focus, and cancelling preserves both files")

    session.run(["niri", "msg", "action", "close-window", "--id", str(browser["id"])])
    session.wait_for(lambda: not any(w.get("app_id") == "org.canopa.Files" for w in windows()), "every Browser closes")
    time.sleep(1)
    assert process.poll() is None
    assert clipboard() == expected
    # Cancel retains exact unfinished transfer work until explicit Abandon. It
    # is another application owner, even after this clipboard selection is lost.
    # The no-operation clipboard-lifetime case verifies natural process exit.
    with external(delayed, "release-background-owner"):
        assert clipboard("text/uri-list") == delayed.encode()
        time.sleep(1)
        assert process.poll() is None
    checks.append("file ownership survives closing every Browser; canceled transfers still retain the process after clipboard supersession")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks}
