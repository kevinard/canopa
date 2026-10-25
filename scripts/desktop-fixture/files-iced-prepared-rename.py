"""Rename must keep the selected object while the name draft is open."""
import json
import os
from pathlib import Path
import sqlite3
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Prepared Rename"
    assert root.is_relative_to(session.runtime / "home")
    root.mkdir()
    (root / "selected.txt").write_bytes(b"Original selected object\n")
    config.write_text(config.read_text() + 'files { shortcut "operations" "<Alt>o"; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Prepared Rename"
    source = root / "selected.txt"
    parked = root / "original-parked.txt"
    target = root / "renamed.txt"

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def key(name, modifier=None):
        command = ["wtype"]
        if modifier:
            command += ["-M", modifier]
        command += ["-k", name]
        if modifier:
            command += ["-m", modifier]
        session.run(command)
        time.sleep(.2)

    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    key("l", "ctrl")
    session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(source), safe="/")])
    key("Return")
    session.wait_for(lambda: title(root.name), "prepared Rename Browser")
    time.sleep(.3)
    key("F2")
    draft = session.wait_for(lambda: title("Rename"), "prepared Rename draft")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(draft["id"])])
    session.run(["wtype", target.name])
    identity = (source.stat().st_dev, source.stat().st_ino)
    source.rename(parked)
    source.write_bytes(b"Replacement never selected\n")
    replacement = (source.stat().st_dev, source.stat().st_ino)
    assert replacement != identity
    # Allow ordinary watcher refresh; authority must survive independently of
    # whether the renderer has accepted a fresh directory listing.
    time.sleep(1)
    session.capture("prepared-rename-before-submit.png")
    key("Return")
    time.sleep(1)
    session.capture("prepared-rename-after-submit.png")
    observed = {
        "original_identity": identity, "replacement_identity": replacement,
        "source_exists": source.exists(), "target_exists": target.exists(),
        "parked_bytes": parked.read_bytes().decode(),
        "source_bytes": source.read_bytes().decode() if source.exists() else None,
        "target_bytes": target.read_bytes().decode() if target.exists() else None,
        "draft_visible": title("Rename") is not None,
    }
    (session.output / "prepared-rename-observation.json").write_text(json.dumps(observed, indent=2) + "\n")
    assert source.exists(), f"Rename adopted an unselected replacement: {observed}"
    assert source.read_bytes() == b"Replacement never selected\n"
    assert (source.stat().st_dev, source.stat().st_ino) == replacement
    assert not target.exists(), observed
    assert (parked.stat().st_dev, parked.stat().st_ino) == identity
    assert parked.read_bytes() == b"Original selected object\n"
    assert title("Rename") is not None, "Refused preparation must remain correctable or cancellable"
    replacement_parked = root / "replacement-preserved.txt"
    source.rename(replacement_parked)
    parked.rename(source)
    key("Return")
    session.wait_for(lambda: target.exists() and title("Rename") is None,
                     "same prepared draft accepts its returned original")
    assert target.read_bytes() == b"Original selected object\n"
    assert (target.stat().st_dev, target.stat().st_ino) == identity
    assert replacement_parked.read_bytes() == b"Replacement never selected\n"
    assert (replacement_parked.stat().st_dev, replacement_parked.stat().st_ino) == replacement
    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    key("z", "ctrl")
    session.wait_for(lambda: source.exists() and not target.exists(), "prepared Rename shared Undo")
    assert (source.stat().st_dev, source.stat().st_ino) == identity
    assert source.read_bytes() == b"Original selected object\n"
    assert replacement_parked.read_bytes() == b"Replacement never selected\n"
    session.capture("prepared-rename-original-returned-and-undone.png")
    checks = ["a real F2 draft refuses a different entry installed before submission, preserving both objects",
              "returning the original accepts the same prepared draft without adopting the replacement",
              "shared Undo returns the original entry and preserves the independent replacement"]

    media = Path(session.env["CANOPA_FIXTURE_VOLUMES"])
    assert media.is_relative_to(session.runtime)
    device = media / "Fixture USB"
    device_source = device / "prepared-device.txt"
    device_target = device / "device-renamed.txt"
    device_source.write_bytes(b"Original device selection\n")
    device_identity = (device_source.stat().st_dev, device_source.stat().st_ino)
    history = session.runtime / "state/canopa-files/operations.sqlite3"

    def device_draft():
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        key("l", "ctrl")
        session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(device_source), safe="/")])
        key("Return")
        session.wait_for(lambda: title(device.name), "prepared device Browser")
        time.sleep(.3)
        key("F2")
        draft = session.wait_for(lambda: title("Rename"), "prepared device Rename draft")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(draft["id"])])
        session.run(["wtype", device_target.name])

    def terminal_phase():
        with sqlite3.connect(history.as_uri() + "?mode=ro", uri=True) as database:
            phase = database.execute("SELECT phase FROM requests ORDER BY id DESC LIMIT 1").fetchone()
        return phase[0] if phase and phase[0] >= 2 else None

    device_draft()
    (media / "unplugged").touch()
    session.wait_for(lambda: "device disconnected" in (media / "events").read_text(), "draft device disconnect")
    (media / "replacement-device").touch()
    (media / "unplugged").unlink()
    session.wait_for(lambda: "device reconnected" in (media / "events").read_text(), "draft replacement device")
    time.sleep(.7)
    assert (device_source.stat().st_dev, device_source.stat().st_ino) == device_identity
    key("Return")
    session.wait_for(lambda: title("Rename") is None, "device Rename admitted with original requirements")
    phase = session.wait_for(terminal_phase, "prepared device terminal result")
    observed = {"phase": phase, "source_exists": device_source.exists(),
                "target_exists": device_target.exists(), "original_identity": device_identity}
    (session.output / "prepared-rename-device-observation.json").write_text(json.dumps(observed, indent=2) + "\n")
    assert phase == 2, f"An open F2 draft adopted replacement storage: {observed}"
    assert device_source.read_bytes() == b"Original device selection\n" and not device_target.exists()
    assert (device_source.stat().st_dev, device_source.stat().st_ino) == device_identity
    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    key("o", "alt")
    review = session.wait_for(lambda: title("Operations"), "prepared device failure review")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(review["id"])])
    session.capture("prepared-rename-replacement-device-refused.png")
    key("w", "ctrl")
    session.wait_for(lambda: title("Operations") is None, "device failure review closes")
    checks.append("the same open F2 draft refuses a private replacement device at the same pathname, directory and source inode")

    device_draft()
    key("Return")
    session.wait_for(lambda: device_target.exists() and title("Rename") is None,
                     "fresh deliberate device preparation")
    assert (device_target.stat().st_dev, device_target.stat().st_ino) == device_identity
    assert device_target.read_bytes() == b"Original device selection\n"
    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    key("z", "ctrl")
    session.wait_for(lambda: device_source.exists() and not device_target.exists(), "reviewed device Rename Undo")
    assert (device_source.stat().st_dev, device_source.stat().st_ino) == device_identity
    checks.append("a fresh deliberate F2 selection uses the reviewed replacement device and preserves real shared Undo")
    session.capture("prepared-rename-reviewed-device-undone.png")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "Rename draft object/parent/device authority; private provider only, not physical hardware, atomic mutation or restart execution"}
