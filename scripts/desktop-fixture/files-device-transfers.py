"""Real native transfers across public storage-provider loss and reconnection."""
import hashlib
import json
import subprocess
import time


def verify(session, config, fixture, scheme, scale, case):
    media = session.runtime / "volumes"
    volume = media / "Fixture USB"
    (media / "disconnect-unmount").touch()
    measurements = {}

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

    def navigate(path):
        ctrl("l"); session.run(["wtype", str(path)]); keys("Return")
        wait_title(f"{path.name} — Canopa Files")

    def events():
        return (media / "events").read_text().splitlines()

    def notices():
        path = session.output / "notifications.json"
        return json.loads(path.read_text()) if path.exists() else []

    def copy_sources(paths):
        session.start(["wl-copy", "--foreground", "--type", "text/uri-list",
                       "".join(f"{path.as_uri()}\r\n" for path in paths)], f"{case}-clipboard.log")
        def ready():
            try:
                return session.run(["wl-paste", "--type", "text/uri-list"]).strip().splitlines() == [path.as_uri() for path in paths]
            except subprocess.CalledProcessError:
                return False
        session.wait_for(ready, "exact external file clipboard sources")

    def remount():
        (media / "unplugged").unlink()
        session.wait_for(lambda: "device reconnected" in events(), "backend reconnect")
        ctrl("l"); keys("Tab", "Tab", "Tab", "space"); time.sleep(.3)
        keys("space", *(["Down"] * 7), "space")
        session.wait_for(lambda: "mount completed" in events(), "explicit mount after reconnect")
        wait_title("Fixture USB — Canopa Files")

    def digest(path):
        with path.open("rb") as stream:
            return hashlib.file_digest(stream, "sha256").digest()

    wait_title("Downloads — Canopa Files")
    if case == "inflight":
        a, b, c = (fixture / name for name in ("a-completed", "b-interrupted", "c-waiting"))
        a.write_bytes(b"Completed before device loss.\n")
        with b.open("wb") as stream:
            for _ in range(96):
                stream.write(b"Canopa transfer bytes.\n" * (1024 * 1024 // 23))
        c.write_bytes(b"Waiting after device loss.\n")
        copy_sources([a, b, c]); navigate(volume)
        before = len(notices())
        session.run(["wtype", "-M", "ctrl", "-k", "v", "-m", "ctrl"])

        def active_copy():
            return next((path for path in volume.glob(".canopa-files-stage-*/folder")
                         if path.is_file() and path.stat().st_size > 64 * 1024), None)

        partial = session.wait_for(active_copy, "actual bytes written before device loss", timeout=40)
        (media / "unplugged").touch()
        session.wait_for(lambda: "device disconnected" in events(), "backend disconnect during copy")
        session.wait_for(lambda: any(notice["title"] == "Copy 3 items to Fixture USB" for notice in notices()[before:]), "active transfer reports device loss", timeout=50)
        assert not (volume / b.name).exists(), "active copy committed into the unavailable backing folder"
        assert not (volume / c.name).exists(), "batch continued into the unavailable backing folder"
        assert (volume / a.name).read_bytes() == a.read_bytes()
        first_inode = (volume / a.name).stat().st_ino
        partial_size = partial.stat().st_size
        assert 0 < partial_size < b.stat().st_size
        time.sleep(.4)
        assert partial.stat().st_size == partial_size, "copy kept writing after its paused outcome"
        assert all(path.exists() for path in (a, b, c))
        ctrl("j"); wait_title("Operations — Canopa Files")
        session.capture("device-transfer-01-paused.png")
        assert any(notice["body"].startswith("Paused") for notice in notices()[before:])
        keys("Escape"); remount(); ctrl("j"); wait_title("Operations — Canopa Files")
        before = len(notices())
        session.run(["wtype", "-M", "alt", "-k", "c", "-m", "alt"])
        session.wait_for(lambda: any(notice["body"].startswith("Completed") for notice in notices()[before:]), "explicit continuation after remount", timeout=60)
        assert digest(volume / b.name) == digest(b)
        assert (volume / c.name).read_bytes() == c.read_bytes()
        assert (volume / a.name).stat().st_ino == first_inode
        assert not partial.exists()
        time.sleep(.4)  # Completion notification precedes the next GTK frame.
        session.capture("device-transfer-02-resumed.png")
        before = len(notices()); started = time.monotonic(); ctrl("z")
        session.wait_for(lambda: any(notice["body"].startswith("Completed") for notice in notices()[before:]), "Undo continued portion", timeout=60)
        measurements["continued_undo_seconds"] = round(time.monotonic() - started, 3)
        assert (volume / a.name).exists() and not (volume / b.name).exists() and not (volume / c.name).exists()
        before = len(notices()); ctrl("z")
        session.wait_for(lambda: any(notice["body"].startswith("Completed") for notice in notices()[before:]), "Undo completed prefix")
        assert not (volume / a.name).exists()
        checks = ["actual active copy stops on provider loss before commit", "completed prefix and all source files survive", "explicit continuation after remount uses exact remaining targets", "continued portion and completed prefix are separately undoable"]
    else:
        local = fixture / "Local source"
        missing = volume / "Unavailable source"
        local.write_bytes(b"Independent local source.\n")
        missing.write_bytes(b"Source on the original fixture device.\n")
        destination = fixture / "Batch destination"
        destination.mkdir()
        copy_sources([local, missing]); navigate(destination)
        (media / "unplugged").touch()
        session.wait_for(lambda: "device disconnected" in events(), "one source device disappears")
        before = len(notices()); ctrl("v")
        session.wait_for(lambda: any(notice["title"] == "Copy 2 items to Batch destination" for notice in notices()[before:]), "source batch reports its outcome")
        assert (destination / local.name).read_bytes() == local.read_bytes(), "one unavailable source blocked the independent local file"
        assert not (destination / missing.name).exists(), "copy read from an unavailable source's backing folder"
        local_inode = (destination / local.name).stat().st_ino
        ctrl("j"); wait_title("Operations — Canopa Files")
        session.capture("device-sources-01-isolated.png")
        assert any(notice["body"].startswith("Completed with errors") for notice in notices()[before:])
        keys("Escape"); remount(); ctrl("j"); wait_title("Operations — Canopa Files")
        before = len(notices())
        session.run(["wtype", "-M", "alt", "-k", "r", "-m", "alt"])
        session.wait_for(lambda: any(notice["body"].startswith("Completed") for notice in notices()[before:]), "retry only unavailable source")
        assert (destination / missing.name).read_bytes() == missing.read_bytes()
        assert (destination / local.name).stat().st_ino == local_inode
        time.sleep(.4)
        session.capture("device-sources-02-retried.png")
        checks = ["one unavailable source leaves independent files running", "source backing folder is never copied", "Retry copies only the failed source after original-device remount"]
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks, "measurements": measurements, "windows": windows()}
