"""Device loss must never redirect a queued operation into the backing folder."""
import json
import time


def verify(session, config, fixture, scheme, scale, binary, process, identityless=False):
    media = session.runtime / "volumes"
    volume = media / "Fixture USB"
    alias = fixture / "Volume alias"
    alias.symlink_to(volume, target_is_directory=True)

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((window for window in windows() if window.get("title") == value), None)

    def wait_title(value):
        result = session.wait_for(lambda: title(value), value)
        time.sleep(.3)
        return result

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name]); time.sleep(.25)

    def ctrl(key):
        session.run(["wtype", "-M", "ctrl", "-k", key, "-m", "ctrl"]); time.sleep(.3)

    def device_action(remove=False):
        ctrl("l"); keys("Tab", "Tab", "Tab", "space"); time.sleep(.3)
        keys("space", *(["Down"] * 7))
        if remove: keys("Tab")
        keys("space")

    def events():
        return (media / "events").read_text().splitlines()

    def notices():
        path = session.output / "notifications.json"
        return json.loads(path.read_text()) if path.exists() else []

    wait_title("Downloads — Canopa Files")
    ctrl("l"); session.run(["wtype", str(fixture / "A-project")]); keys("Return")
    wait_title("A-project — Canopa Files"); keys("Home"); ctrl("c")
    session.wait_for(lambda: "text/uri-list" in session.run(["wl-paste", "--list-types"]), "selected source is on the native clipboard")
    assert session.run(["wl-paste", "--type", "text/uri-list"]).strip() == (fixture / "A-project/notes.rs").as_uri()
    ctrl("l"); session.run(["wtype", str(alias)]); keys("Return"); wait_title("Volume alias — Canopa Files")
    (media / "hold-unmount").touch()
    ctrl("l"); keys("Tab", "Tab", "Tab", "space"); time.sleep(.3)
    keys("space", *(["Down"] * 7), "Tab", "space")
    session.wait_for(lambda: "unmount requested without force" in events(), "unmount stays pending")
    ctrl("v"); time.sleep(.4)
    assert not (volume / "notes.rs").exists(), "paste must wait during removal"
    session.capture("device-safety-01-queued.png")
    (media / "release-unmount").touch()
    session.wait_for(lambda: "unmount completed" in events(), "private backend unmounted")
    session.wait_for(lambda: any(notice["title"] == "Copy 1 item to Volume alias" for notice in notices()), "queued copy reports its outcome")
    session.capture("device-safety-02-result.png")
    assert not (volume / "notes.rs").exists(), "queued copy wrote into the unmounted volume's underlying folder through an alias"
    assert (fixture / "A-project/notes.rs").read_text() == 'fn main() { println!("Hello Canopa"); }\n'

    (media / "replacement-device").touch()
    device_action(); wait_title("Fixture USB — Canopa Files")
    seen = len(notices())
    ctrl("j"); wait_title("Operations — Canopa Files")
    session.run(["wtype", "-M", "alt", "-k", "r", "-m", "alt"])
    session.wait_for(lambda: any("Copy 1 item to Volume alias" in notice["title"] for notice in notices()[seen:]), "retry with replacement media reports an outcome")
    assert not (volume / "notes.rs").exists(), "Retry accepted different media at the original mount point"
    keys("Escape")
    if identityless:
        # The platform cannot identify this media across mounts. Reusing Retry
        # is unsafe; a freshly reviewed request remains available.
        seen = len(notices())
        ctrl("v")
        session.wait_for(lambda: any(notice["title"] == "Copy 1 item to Fixture USB" and notice["body"].startswith("Completed") for notice in notices()[seen:]), "new request after reviewing unidentified media")
        assert (volume / "notes.rs").read_bytes() == (fixture / "A-project/notes.rs").read_bytes()
        time.sleep(.2); session.capture("device-safety-identityless.png")
        return {"status": "passed", "scheme": scheme, "scale": scale,
                "checks": ["Retry cannot authorize remounted media without a stable device identity", "a freshly reviewed copy to the mounted device succeeds"], "windows": windows()}
    before = events().count("unmount completed")
    device_action(remove=True)
    session.wait_for(lambda: events().count("unmount completed") == before + 1, "replacement media unmounted")
    (media / "replacement-device").unlink()
    device_action(); wait_title("Fixture USB — Canopa Files")
    seen = len(notices())
    ctrl("j"); wait_title("Operations — Canopa Files")
    session.run(["wtype", "-M", "alt", "-k", "r", "-m", "alt"])
    session.wait_for(lambda: any("Copy 1 item to Volume alias" in notice["title"] and notice["body"].startswith("Completed") for notice in notices()[seen:]), "explicit retry after remount")
    assert (volume / "notes.rs").read_bytes() == (fixture / "A-project/notes.rs").read_bytes()
    time.sleep(.2); session.capture("device-safety-03-retried.png")
    keys("Escape")
    ctrl("l"); session.run(["wtype", str(fixture / "B-notes.txt")]); keys("Return"); wait_title("Downloads — Canopa Files")
    ctrl("c")
    session.wait_for(lambda: session.run(["wl-paste", "--type", "text/uri-list"]).strip() == (fixture / "B-notes.txt").as_uri(), "second clipboard source selected")
    ctrl("l"); session.run(["wtype", str(alias)]); keys("Return"); wait_title("Volume alias — Canopa Files")
    (media / "release-unmount").unlink()
    before = events().count("unmount requested without force")
    device_action(remove=True)
    session.wait_for(lambda: events().count("unmount requested without force") == before + 1, "second unmount stays pending")
    seen = len(notices())
    ctrl("v"); time.sleep(.4)
    other = fixture / "Other destination"
    other.mkdir()
    alias.unlink(); alias.symlink_to(other, target_is_directory=True)
    (media / "release-unmount").touch()
    session.wait_for(lambda: any(notice["title"] == "Copy 1 item to Volume alias" for notice in notices()[seen:]), "retargeted alias request reports its outcome")
    assert not (other / "B-notes.txt").exists(), "retargeting the alias redirected an already queued copy into a different folder"
    assert not (volume / "B-notes.txt").exists()
    session.capture("device-safety-03b-retarget-refused.png")
    alias.unlink(); alias.symlink_to(volume, target_is_directory=True)
    device_action(); wait_title("Fixture USB — Canopa Files")
    before = events().count("unmount completed")
    device_action(remove=True)
    session.wait_for(lambda: events().count("unmount completed") == before + 1, "unmounted before process restart")
    (media / "unmounted").touch()
    # Release the native file clipboard so the process has no clipboard hold.
    session.start(["wl-copy", "--foreground", "--type", "text/plain", "ordinary fixture text"], "clipboard-replacement.log")
    session.wait_for(lambda: "text/plain" in session.run(["wl-paste", "--list-types"]), "file clipboard ownership released")
    for window in windows():
        if window.get("app_id") == "org.canopa.Files":
            session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
    session.processes.remove(process)
    session.wait_for(lambda: process.poll() is not None, "clean exit while fixture volume is unmounted")
    assert process.returncode == 0
    process = session.start([str(binary), str(alias)], "files-restarted.log")
    wait_title("Volume alias — Canopa Files")
    session.run(["wtype", "-M", "ctrl", "-M", "shift", "-k", "n", "-m", "shift", "-m", "ctrl"])
    wait_title("New folder"); ctrl("a"); session.run(["wtype", "Must stay absent"]); keys("Return")
    session.wait_for(lambda: any(notice["title"] == "Create folder" for notice in notices()), "folder request reports its outcome after restart")
    assert not (volume / "Must stay absent").exists(), "restart forgot the unavailable volume and created a folder in its backing directory"
    ctrl("j"); wait_title("Operations — Canopa Files"); session.capture("device-safety-04-restarted.png")
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "checks": ["queued copy through a symlink cannot write into an unmounted volume's backing folder", "Retry refuses different media at the same mount point", "explicit Retry after the original device returns succeeds", "retargeting the browsing alias cannot redirect queued work", "restart retains protection for known unavailable roots through aliases"], "windows": windows()}
