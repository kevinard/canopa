"""Local-volume acceptance through native GIO and the private desktop UI."""
import json
import sys
import time


def verify(session, config, fixture, scheme, scale, binary, process):
    media = session.runtime / "volumes"

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((window for window in windows() if window.get("title") == value), None)

    def wait_title(value):
        window = session.wait_for(lambda: title(value), value)
        time.sleep(.3)
        return window

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name])
            time.sleep(.25)

    def ctrl(key):
        session.run(["wtype", "-M", "ctrl", "-k", key, "-m", "ctrl"])
        time.sleep(.3)

    def device_action(remove=False):
        ctrl("l"); keys("Tab", "Tab", "Tab", "space"); time.sleep(.3)
        keys("space", *(["Down"] * 7))
        if remove:
            keys("Tab")
        keys("space")

    def events():
        return (media / "events").read_text().splitlines()

    def notices():
        path = session.output / "notifications.json"
        return json.loads(path.read_text()) if path.exists() else []

    browser = wait_title("Downloads — Canopa Files")
    pointer = session.build_pointer()
    time.sleep(.5)
    session.capture("devices-01-discovery.png")
    width, height = browser["layout"]["window_size"]
    session.run([str(pointer), str(round(75 * 1600 / width)), str(round(485 * 1000 / height))])
    wait_title("Fixture USB — Canopa Files")
    keys("Down", "space"); wait_title("Drive notes.txt — Quick Look")
    session.capture("devices-02-preview.png")
    keys("Escape")
    session.run([str(pointer), str(round(140 * 1600 / width)), str(round(485 * 1000 / height))])
    session.wait_for(lambda: "unmount requested without force" in (media / "events").read_text(), "explicit safe unmount")
    time.sleep(.4); session.capture("devices-03-unmounted.png")
    ctrl("l"); session.run(["wtype", str(fixture)]); keys("Return")
    wait_title("Downloads — Canopa Files")
    session.run([str(pointer), str(round(75 * 1600 / width)), str(round(485 * 1000 / height))])
    session.wait_for(lambda: "mount requested" in (media / "events").read_text().splitlines(), "explicit mount of an unmounted volume")
    wait_title("Fixture USB — Canopa Files")
    session.capture("devices-04-mounted-again.png")
    session.run(["niri", "msg", "action", "set-column-width", "360"])
    time.sleep(.4)
    assert title("Fixture USB — Canopa Files")["layout"]["window_size"][0] == 360
    ctrl("l"); session.run(["wtype", str(fixture)]); keys("Return")
    wait_title("Downloads — Canopa Files")
    ctrl("l"); keys("Tab", "Tab", "Tab", "space"); time.sleep(.3)
    session.capture("devices-05-narrow-folder-menu.png")
    keys("space"); time.sleep(.3)
    session.capture("devices-06-narrow-locations.png")
    (media / "unplugged").touch()
    (media / "Fixture USB").rename(media / "held-payload")
    session.wait_for(lambda: "device disconnected" in (media / "events").read_text(), "private backend disconnect")
    time.sleep(.4); session.capture("devices-07-live-disconnect.png")
    (media / "held-payload").rename(media / "Fixture USB")
    (media / "unplugged").unlink()
    session.wait_for(lambda: "device reconnected" in (media / "events").read_text(), "private backend reconnect")
    time.sleep(.4); session.capture("devices-08-live-reconnect.png")
    keys(*(["Down"] * 7), "Return")
    wait_title("Fixture USB — Canopa Files")
    keys("Down", "space"); preview = wait_title("Drive notes.txt — Quick Look")
    assert preview["is_floating"] and preview["layout"]["window_size"][0] >= 600
    session.capture("devices-09-narrow-preview.png")
    assert (media / "Fixture USB/Drive notes.txt").read_text() == "A real file on the private fixture volume.\n"
    keys("Escape")
    started, release = session.runtime / "action-started", session.runtime / "release-action"
    helper = session.runtime / "device-wait.py"
    helper.write_text(f'from pathlib import Path\nimport time\nPath({str(started)!r}).touch()\nwhile not Path({str(release)!r}).exists(): time.sleep(.02)\n')
    command = f'{sys.executable} "{helper}"'
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\nfiles {{ action "device-wait" name="Device wait" command={json.dumps(command)} shortcut="F7" {{ selection min=1; }}; }}\n')
    time.sleep(1.2)
    keys("F7"); wait_title("Operations — Canopa Files")
    session.wait_for(started.exists, "actual action is running before removal")
    keys("Escape")
    before = events().count("unmount requested without force")
    device_action(remove=True); time.sleep(.5)
    session.capture("devices-10-active-operation.png")
    assert events().count("unmount requested without force") == before, "an active Files job must prevent removal"
    release.touch()
    session.wait_for(lambda: any(event["title"] == "Run Device wait" and event["body"].startswith("Completed")
                                for event in notices()), "action completed before removal retry")
    device_action(remove=True)
    session.wait_for(lambda: events().count("unmount requested without force") == before + 1, "removal succeeds after own jobs finish")
    ctrl("l"); session.run(["wtype", str(fixture)]); keys("Return"); wait_title("Downloads — Canopa Files")
    (media / "fail-mount").touch()
    before = events().count("mount requested")
    device_action()
    session.wait_for(lambda: events().count("mount requested") == before + 1, "backend mount refusal")
    time.sleep(.4); session.capture("devices-11-mount-error.png")
    assert title("Downloads — Canopa Files"), "failed mount must retain the current location"
    (media / "fail-mount").unlink()
    device_action(); wait_title("Fixture USB — Canopa Files")
    session.capture("devices-12-mount-retry.png")
    before = events().count("unmount completed")
    device_action(remove=True)
    session.wait_for(lambda: events().count("unmount completed") == before + 1, "unmount before delayed mount")
    ctrl("l"); session.run(["wtype", str(fixture)]); keys("Return"); wait_title("Downloads — Canopa Files")
    (media / "hold-mount").touch()
    before = events().count("mount requested")
    device_action()
    session.wait_for(lambda: events().count("mount requested") == before + 1, "backend mount stays pending")
    ctrl("l"); session.run(["wtype", str(fixture / "A-project")]); keys("Return"); wait_title("A-project — Canopa Files")
    before = events().count("mount completed")
    (media / "release-mount").touch()
    session.wait_for(lambda: events().count("mount completed") == before + 1, "delayed mount finishes after navigation")
    time.sleep(.5)
    assert title("A-project — Canopa Files"), "a late mount must not replace newer navigation"
    session.capture("devices-13-newer-navigation.png")

    before = events().count("unmount completed")
    device_action(remove=True)
    session.wait_for(lambda: events().count("unmount completed") == before + 1, "unmount before closed-origin mount")
    (media / "release-mount").unlink()
    before = events().count("mount requested")
    device_action()
    session.wait_for(lambda: events().count("mount requested") == before + 1, "mount is pending before all windows close")
    for window in windows():
        if window.get("app_id") == "org.canopa.Files":
            session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
    session.wait_for(lambda: not windows(), "all Files windows close during mount")
    time.sleep(.8)
    assert process.poll() is None, "a pending device operation must hold the application"
    session.run([str(binary), str(fixture)])
    reopened = wait_title("Downloads — Canopa Files")
    assert reopened["pid"] == process.pid, "reopening must reach the original process"
    before = events().count("mount completed")
    (media / "release-mount").touch()
    session.wait_for(lambda: events().count("mount completed") == before + 1, "pending mount finishes after reopening")
    time.sleep(.5)
    assert title("Downloads — Canopa Files") and not title("Fixture USB — Canopa Files")
    session.capture("devices-14-closed-origin.png")

    # Clipboard source and destination are outside the fixture volume. This
    # isolates the application queue's removal barrier from filesystem failure.
    ctrl("l"); session.run(["wtype", str(fixture / "A-project")]); keys("Return"); wait_title("A-project — Canopa Files")
    keys("Home"); ctrl("c")
    session.wait_for(lambda: "text/uri-list" in session.run(["wl-paste", "--list-types"]), "native clipboard has the selected source")
    assert session.run(["wl-paste", "--type", "text/uri-list"]).strip() == (fixture / "A-project/notes.rs").as_uri()
    target = fixture / "Queued after removal"
    target.mkdir()
    ctrl("l"); session.run(["wtype", str(target)]); keys("Return"); wait_title("Queued after removal — Canopa Files")
    queued = session.runtime / "queued-action-started"
    helper = session.runtime / "queued-action.py"
    helper.write_text(f'from pathlib import Path\nPath({str(queued)!r}).touch()\n')
    command = f'{sys.executable} "{helper}"'
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\nfiles {{ action "queued-device" name="Queued device action" command={json.dumps(command)} shortcut="F8" {{ selection min=0; }}; }}\n')
    time.sleep(1.2)
    (media / "hold-unmount").touch()
    before = events().count("unmount requested without force")
    device_action(remove=True)
    session.wait_for(lambda: events().count("unmount requested without force") == before + 1, "backend unmount stays pending")
    session.capture("devices-15a-browser-before-paste.png")
    ctrl("v"); keys("F8")
    time.sleep(.5)
    assert not (target / "notes.rs").exists() and not queued.exists(), "file jobs and actions must wait during removal"
    session.capture("devices-15-removal-barrier.png")
    (media / "release-unmount").touch()
    session.wait_for(lambda: (target / "notes.rs").exists() and queued.exists(), "queued transfer and action resume after removal")
    assert (target / "notes.rs").read_bytes() == (fixture / "A-project/notes.rs").read_bytes()
    session.wait_for(lambda: {"Run Queued device action", "Copy 1 item to Queued after removal"}.issubset(
        {event["title"] for event in notices() if event["body"].startswith("Completed")}), "both resumed jobs report completion")
    assert not any("UNEXPECTED" in event for event in events())
    time.sleep(.2)
    session.capture("devices-16-queued-work-completed.png")
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "checks": ["mounted GIO volume discovery and real-file browsing", "explicit asynchronous unmount without force", "unmounted local volume remains available for explicit mount and navigation", "live disconnect and reconnect while narrow Locations is open", "keyboard device navigation at 360 pixels and independent floating preview", "active Files action prevents unmount until completion", "mount failure keeps the location and is retryable", "late mount preserves newer navigation", "pending mount survives zero windows without reopening a closed origin", "new transfers and actions wait during removal and resume afterward"], "windows": windows()}
