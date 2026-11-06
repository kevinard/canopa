"""File clipboard ownership with no unfinished operation owning the process."""
import json
import os
import signal
import subprocess
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Clipboard lifetime"
    root.mkdir()
    (root / "original.txt").write_bytes(b"Original clipboard lifetime bytes\n")
    (root / "replacement.txt").write_bytes(b"Foreign clipboard lifetime bytes\n")


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Clipboard lifetime"

    def uri(path):
        return "file://" + quote_from_bytes(os.fsencode(path), safe="/")

    def chord(key, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", key]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.2)

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def clipboard(mime="x-special/gnome-copied-files"):
        result = subprocess.run(["wl-paste", "--no-newline", "--type", mime],
                                env=session.env, capture_output=True, timeout=3)
        return result.stdout if result.returncode == 0 else None

    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()),
                     "clipboard lifetime Browser focus")
    chord("l", "ctrl")
    session.run(["wtype", uri(root / "original.txt")])
    chord("Return")
    session.wait_for(lambda: any(w["id"] == browser["id"] and w.get("title") == "Clipboard lifetime — Canopa Files"
                                for w in windows()), "clipboard lifetime source selection")
    time.sleep(.3)
    chord("c", "ctrl")
    expected = ("copy\n" + uri(root / "original.txt") + "\n").encode()
    session.wait_for(lambda: clipboard() == expected, "owned file Copy read by an independent client")
    session.run(["niri", "msg", "action", "close-window", "--id", str(browser["id"])])
    session.wait_for(lambda: not any(w.get("app_id") == "org.canopa.Files" for w in windows()),
                     "clipboard lifetime Browser closed")
    time.sleep(1)
    assert process.poll() is None
    assert clipboard() == expected
    payload = uri(root / "replacement.txt") + "\r\n"
    with (session.output / "clipboard-lifetime-foreign.log").open("w") as output:
        peer = subprocess.Popen(["wl-copy", "--foreground", "--type", "text/uri-list", payload],
                                env=session.env, stdout=output, stderr=output, start_new_session=True)
    try:
        assert process.wait(timeout=10) == 0
        assert clipboard("text/uri-list") == payload.encode()
    finally:
        if peer.poll() is None:
            os.killpg(peer.pid, signal.SIGKILL)
        peer.wait(timeout=5)
    assert (root / "original.txt").read_bytes() == b"Original clipboard lifetime bytes\n"
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": [
        "file Copy remains available to an independent client after every Browser closes",
        "with no unfinished operations, clipboard supersession releases the background process within ten seconds",
        "foreign clipboard ownership and source bytes survive Files exit",
    ]}
