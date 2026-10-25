"""Selection-time clipboard placement through real native input and GIO events."""
import json
import time


def verify(session, config, fixture, scheme, scale, binary, process, cross_destination):
    media = session.runtime / "volumes"
    volume = media / "Fixture USB"
    held = media / "Original device"
    source = volume / "Drive notes.txt"
    original = source.read_bytes()
    assert cross_destination is not None
    destination = cross_destination / "Clipboard destination"
    destination.mkdir()
    filesystems = {"source": volume.stat().st_dev, "destination": destination.stat().st_dev}
    assert filesystems["source"] != filesystems["destination"]

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
            time.sleep(.25)

    def ctrl(key):
        session.run(["wtype", "-M", "ctrl", "-k", key, "-m", "ctrl"])
        time.sleep(.3)

    def navigate(path, heading):
        ctrl("l")
        session.run(["wtype", str(path)])
        keys("Return")
        wait_title(heading + " — Canopa Files")

    def notices():
        path = session.output / "notifications.json"
        return json.loads(path.read_text()) if path.exists() else []

    def events():
        return (media / "events").read_text().splitlines()

    wait_title("Downloads — Canopa Files")
    navigate(source, "Fixture USB")
    ctrl("x")
    session.wait_for(lambda: session.run(["wl-paste", "--type", "text/uri-list"]).strip() == source.as_uri(),
                     "original device selection is on the native clipboard")
    clipboard = session.run(["wl-paste", "--type", "x-special/gnome-copied-files"])
    assert clipboard.splitlines() == ["cut", source.as_uri()]
    session.capture("clipboard-device-01-selected.png")
    navigate(destination, destination.name)

    # Public provider removal/reconnection, with separate real directory trees.
    # No actual block device, mount or user folder is changed.
    before = events().count("device disconnected")
    (media / "unplugged").touch()
    session.wait_for(lambda: events().count("device disconnected") == before + 1, "original media disconnected")
    volume.rename(held)
    volume.mkdir()
    replacement = b"Different bytes on replacement media.\n"
    source.write_bytes(replacement)
    (media / "replacement-device").touch()
    before = events().count("device reconnected")
    (media / "unplugged").unlink()
    session.wait_for(lambda: events().count("device reconnected") == before + 1, "replacement media reconnected")
    time.sleep(1.2)
    inventory = session.run(["gio", "mount", "--list", "--detail"])
    (session.output / "clipboard-replacement-inventory.txt").write_text(inventory)
    assert "canopa-private-replacement" in inventory
    seen = len(notices())
    ctrl("v")
    session.wait_for(lambda: any(n["title"] == "Move 1 item to Clipboard destination" for n in notices()[seen:]),
                     "Paste reports its outcome after source media replacement")
    ctrl("j")
    wait_title("Operations — Canopa Files")
    session.capture("clipboard-device-02-replacement-refused.png")
    output = destination / source.name
    (session.output / "clipboard-placement-observation.json").write_text(json.dumps({
        "clipboard": clipboard,
        "destination_exists": output.exists(),
        "destination_text": output.read_text() if output.exists() else None,
        "replacement_source_exists": source.exists(),
        "replacement_source_text": source.read_text() if source.exists() else None,
        "original_source_text": (held / source.name).read_text(),
    }, indent=2))
    assert not (destination / source.name).exists(), "Cut/Paste moved a replacement device's file instead of the selected source"
    assert source.read_bytes() == replacement
    assert (held / source.name).read_bytes() == original
    checks = ["Cut/Paste refuses source media replaced after the selection was captured",
              "original and replacement source bytes remain intact",
              "Cut/Copy destination is on a different actual filesystem from the selected source"]

    def retry(label):
        seen = len(notices())
        ctrl("j")
        wait_title("Operations — Canopa Files")
        session.run(["wtype", "-M", "alt", "-k", "r", "-m", "alt"])
        session.wait_for(lambda: any(label in n["title"] for n in notices()[seen:]), "exact failed clipboard request reports its retry")
        time.sleep(.3)

    retry("Move 1 item to Clipboard destination")
    assert not output.exists() and source.read_bytes() == replacement
    assert (held / source.name).read_bytes() == original
    session.capture("clipboard-device-03-replacement-retry-refused.png")
    checks.append("Retry retains the Cut selection's original media requirement")

    foreign = media / "Replacement device"

    def disconnect():
        before = events().count("device disconnected")
        (media / "unplugged").touch()
        session.wait_for(lambda: events().count("device disconnected") == before + 1, "clipboard fixture media disconnect")

    def reconnect():
        before = events().count("device reconnected")
        (media / "unplugged").unlink()
        session.wait_for(lambda: events().count("device reconnected") == before + 1, "clipboard fixture media reconnect")
        time.sleep(1.2)

    def original_returns():
        disconnect()
        volume.rename(foreign)
        held.rename(volume)
        (media / "replacement-device").unlink()
        reconnect()

    def replacement_returns():
        disconnect()
        volume.rename(held)
        foreign.rename(volume)
        (media / "replacement-device").touch()
        reconnect()

    original_returns()
    retry("Move 1 item to Clipboard destination")
    assert output.read_bytes() == original and not source.exists()
    assert (foreign / source.name).read_bytes() == replacement
    session.capture("clipboard-device-04-original-retry-completed.png")
    checks.append("explicit Retry moves the selected bytes after the original identified device returns")
    ctrl("z")
    session.wait_for(lambda: source.exists() and not output.exists(), "Undo returns the original selected source")
    assert source.read_bytes() == original and (foreign / source.name).read_bytes() == replacement
    checks.append("Undo returns the moved original without touching replacement media")
    keys("Escape")

    alias = fixture / "Clipboard alias"
    alias.symlink_to(volume, target_is_directory=True)
    copy_target = cross_destination / "Clipboard copy destination"
    copy_target.mkdir()
    navigate(alias / source.name, alias.name)
    ctrl("c")
    session.wait_for(lambda: session.run(["wl-paste", "--type", "text/uri-list"]).strip() == (alias / source.name).as_uri(),
                     "Copy captures the source's displayed alias")
    navigate(copy_target, copy_target.name)
    replacement_returns()
    seen = len(notices())
    ctrl("v")
    session.wait_for(lambda: any(n["title"] == "Copy 1 item to Clipboard copy destination" for n in notices()[seen:]), "Copy reports replacement media refusal")
    assert not (copy_target / source.name).exists()
    assert source.read_bytes() == replacement and (held / source.name).read_bytes() == original
    ctrl("j")
    wait_title("Operations — Canopa Files")
    session.capture("clipboard-device-05-copy-replacement-refused.png")
    checks.append("Copy through a browsing alias also retains selection-time source media")
    original_returns()
    retry("Copy 1 item to Clipboard copy destination")
    assert (copy_target / source.name).read_bytes() == original and source.read_bytes() == original
    ctrl("z")
    session.wait_for(lambda: not (copy_target / source.name).exists(), "Undo removes only the copied output")
    assert source.read_bytes() == original
    checks.append("Copy Retry and separate Undo preserve the returned original source")
    keys("Escape")

    # A fresh selection captures the current epoch before testing alias identity.
    navigate(alias / source.name, alias.name)
    ctrl("c")
    navigate(copy_target, copy_target.name)
    other = fixture / "Unrelated alias target"
    other.mkdir()
    (other / source.name).write_bytes(b"A different file behind a retargeted alias.\n")
    alias.unlink()
    alias.symlink_to(other, target_is_directory=True)
    seen = len(notices())
    ctrl("v")
    session.wait_for(lambda: any(n["title"] == "Copy 1 item to Clipboard copy destination" for n in notices()[seen:]), "retargeted source alias reports refusal")
    assert not (copy_target / source.name).exists()
    assert source.read_bytes() == original
    assert (other / source.name).read_bytes() == b"A different file behind a retargeted alias.\n"
    ctrl("j")
    wait_title("Operations — Canopa Files")
    session.capture("clipboard-device-06-retargeted-source-refused.png")
    checks.append("a retargeted source browsing alias cannot redirect an already captured Copy")
    alias.unlink()
    alias.symlink_to(volume, target_is_directory=True)
    retry("Copy 1 item to Clipboard copy destination")
    assert (copy_target / source.name).read_bytes() == original
    ctrl("z")
    session.wait_for(lambda: not (copy_target / source.name).exists(), "Undo after source alias return")
    checks.append("restoring the original alias allows exact Retry and separate Undo")
    keys("Escape")

    # Source requirements must not freeze a destination the user chooses later.
    local = fixture / "B-notes.txt"
    local_bytes = local.read_bytes()
    navigate(local, "Downloads")
    ctrl("c")
    replacement_returns()
    navigate(volume, "Fixture USB")
    seen = len(notices())
    ctrl("v")
    session.wait_for(lambda: any(n["title"] == "Copy 1 item to Fixture USB" and n["body"].startswith("Completed") for n in notices()[seen:]),
                     "freshly reviewed destination can use the currently mounted media")
    assert (volume / local.name).read_bytes() == local_bytes and local.read_bytes() == local_bytes
    assert source.read_bytes() == replacement and (held / source.name).read_bytes() == original
    session.run(["niri", "msg", "action", "set-column-width", "360"])
    time.sleep(.4)
    ctrl("j")
    wait_title("Operations — Canopa Files")
    session.run(["niri", "msg", "action", "set-column-width", "360"])
    time.sleep(.4)
    assert title("Operations — Canopa Files")["layout"]["window_size"][0] == 360
    session.capture("clipboard-device-07-independent-destination.png")
    checks.append("source capture leaves a later deliberately chosen destination independent at 360 pixels")
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "checks": checks, "filesystems": filesystems, "windows": windows()}
