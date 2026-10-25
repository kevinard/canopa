"""Native drag authority across actual destination and source replacement."""
import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Stale drops"
    assert root.is_relative_to(session.runtime / "home")
    (root / "Source").mkdir(parents=True)
    (root / "Destination").mkdir()
    (root / "Source/a.txt").write_bytes(b"Original held source bytes\n")
    config.write_text(config.read_text() + 'files { sidebar "never"; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Stale drops"
    source, destination = root / "Source/a.txt", root / "Destination"
    original = b"Original held source bytes\n"
    wire = session.output / "files.log"
    pointer = session.build_pointer("drag")
    checks, geometry = [], []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w["title"] == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "stale-drag Browser focus")

    def chord(key, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", key]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.2)

    def navigate(path):
        chord("l", "ctrl")
        session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(path), safe="/")])
        chord("Return")
        directory = path if path.is_dir() else path.parent
        result = session.wait_for(lambda: title(directory.name), "stale-drag location")
        time.sleep(.3)
        return result

    def notices():
        p = session.output / "notifications.json"
        return json.loads(p.read_text()) if p.exists() else []

    def receipts():
        # Read only scheduling identity/revision/phase; never execution payloads.
        path = session.runtime / "state/canopa-files/operations.sqlite3"
        connection = sqlite3.connect(path.as_uri() + "?mode=ro", uri=True)
        try:
            return connection.execute("SELECT id,revision,phase FROM requests WHERE id NOT IN (SELECT request FROM source_leases) ORDER BY id").fetchall()
        finally:
            connection.close()

    def inventory():
        return {os.fsencode(p.relative_to(root)).hex():
                {"kind": "directory", "inode": p.stat().st_ino} if p.is_dir() else
                {"kind": "file", "inode": p.stat().st_ino, "bytes": p.stat().st_size,
                 "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
                for p in root.rglob("*")}

    def checkpoint(name):
        (session.output / (name + ".json")).write_text(json.dumps(
            {"checks": checks, "namespace": inventory(), "receipts": receipts(),
             "notices": notices(), "windows": windows()}, indent=2) + "\n")

    focus(browser)
    browser = navigate(source)
    chord("2", "ctrl")
    session.run(["niri", "msg", "action", "set-column-width", "50%"])
    chord("n", "ctrl")
    session.wait_for(lambda: len(windows()) == 2, "two stale-drag browsers")
    target = next(w for w in windows() if w["id"] != browser["id"])
    session.run(["niri", "msg", "action", "set-column-width", "50%"])
    navigate(destination)
    chord("2", "ctrl")
    focus(browser)
    session.run(["niri", "msg", "action", "move-column-to-first"])

    def begin(name):
        focus(target)
        focus(browser)
        session.run(["niri", "msg", "action", "center-visible-columns"])
        actual = sorted(windows(), key=lambda w: w["layout"]["pos_in_scrolling_layout"])
        output = next(o["logical"] for o in json.loads(session.run(["niri", "msg", "--json", "outputs"])).values() if o.get("logical"))
        total = sum(w["layout"]["tile_size"][0] for w in actual)
        assert len(actual) == 2 and total <= output["width"] + 1
        def point(window, x, y):
            index = next(i for i, w in enumerate(actual) if w["id"] == window["id"])
            layout = actual[index]["layout"]
            left = (output["width"] - total) / 2 + sum(w["layout"]["tile_size"][0] for w in actual[:index])
            dx, dy = layout["window_offset_in_tile"]
            return str(round((left + dx + x) * 1600 / output["width"])), str(round((dy + y) * 1000 / output["height"]))
        first, last = point(browser, 100, 144), point(target, 100, 350)
        geometry.append({"name": name, "windows": actual, "output": output, "source": first, "target": last})
        (session.output / "stale-geometry.json").write_text(json.dumps(geometry, indent=2) + "\n")
        before, count, start = inventory(), len(notices()), len(wire.read_text())
        ready, release = session.runtime / (name + ".ready"), session.runtime / (name + ".release")
        helper = session.start([str(pointer), *first, *last, str(ready), str(release)], name + "-pointer.log")
        session.wait_for(ready.exists, "held original drag")
        starts = re.findall(r"-> wl_data_device#\d+\.start_drag\(wl_data_source#(\d+)", wire.read_text()[start:])
        assert starts, "visible source did not start a native drag"
        source_id = starts[-1]
        def move_selected():
            actions = re.findall(rf"wl_data_source#{source_id}\.action\((\d+)\)", wire.read_text()[start:])
            return actions and actions[-1] == "2"
        session.wait_for(move_selected, "prepared native Move before replacement")
        offers = re.findall(r'-> wl_data_offer#(\d+)\.accept\(\d+, "text/uri-list"\)', wire.read_text()[start:])
        assert offers
        assert inventory() == before and len(notices()) == count, "hover admitted filesystem work"
        session.capture(name + "-original-held.png")
        # Wire object IDs can be reused after leaving the source window. Start
        # receipt observation after this exact prepared target's capture, so an
        # earlier offer lifetime cannot satisfy retirement for the current one.
        return helper, release, source_id, offers[-1], len(wire.read_text()), {row[0] for row in receipts()}

    def release(held):
        helper, path, source_id, offer_id, offset, before = held
        path.touch()
        assert helper.wait(timeout=10) == 0
        session.processes.remove(helper)
        def settled():
            rows = [row for row in receipts() if row[0] not in before]
            current = wire.read_text()[offset:]
            retired = f"-> wl_data_offer#{offer_id}.destroy()" in current
            ended = any(f"wl_data_source#{source_id}.{event}()" in current
                        for event in ["cancelled", "dnd_finished"])
            destroy = current.rfind(f"-> wl_data_source#{source_id}.destroy()")
            acknowledged = destroy >= 0 and f"wl_display#1.delete_id({source_id})" in current[destroy:]
            return retired and (ended or acknowledged) and all(row[2] >= 2 for row in rows)
        session.wait_for(settled, "original native receipt retired and admitted work terminal")
        time.sleep(.3)

    try:
        held = begin("stale-01-replaced-destination")
        old_inode = destination.stat().st_ino
        destination.rename(root / "Original destination")
        destination.mkdir()
        (destination / "newer.txt").write_bytes(b"New destination contents\n")
        assert old_inode != destination.stat().st_ino
        focus(target)
        chord("F5")
        time.sleep(1)
        session.capture("stale-01-replacement-held.png")
        checkpoint("stale-01-before-release")
        release(held)
        checkpoint("stale-01-after-release")
        assert source.exists() and source.read_bytes() == original, "held drag consumed its source after its prepared destination was replaced"
        assert not (destination / source.name).exists() and not (root / "Original destination" / source.name).exists(), "held drag wrote into a replacement or renamed destination"
        assert (destination / "newer.txt").read_bytes() == b"New destination contents\n"
        checks.append("replacing a held destination and refreshing the actual target view cannot recapture authority or move the original source")
        checkpoint("stale-01-refused")

        def undo(expected):
            before = len(notices())
            focus(target)
            chord("z", "ctrl")
            session.wait_for(lambda: source.exists() and source.read_bytes() == expected
                             and not (destination / source.name).exists(), "shared Undo restores the reviewed source")
            session.wait_for(lambda: any(n["body"].split(" ·", 1)[0].endswith(" reversed")
                                         for n in notices()[before:]), "saved native Undo result")

        held = begin("stale-02-reviewed-destination")
        release(held)
        assert not source.exists() and (destination / source.name).read_bytes() == original
        undo(original)
        checks.append("a fresh drag can use the reviewed replacement destination and shared Undo restores the source")
        checkpoint("stale-02-reviewed")

        held = begin("stale-03-content-arrival")
        (destination / "arrival.txt").write_bytes(b"Unrelated arrival during hover\n")
        focus(target)
        chord("F5")
        time.sleep(1)
        session.capture("stale-03-arrival-held.png")
        release(held)
        assert not source.exists() and (destination / source.name).read_bytes() == original
        undo(original)
        assert (destination / "arrival.txt").read_bytes() == b"Unrelated arrival during hover\n"
        assert (destination / "newer.txt").read_bytes() == b"New destination contents\n"
        checks.append("ordinary contents arriving in the original directory during refresh still permit Move and Undo without changing those arrivals")
        checkpoint("stale-03-arrival")

        held = begin("stale-04-source-replacement")
        retained = root / "Original selected.txt"
        source.rename(retained)
        newer = b"Newer source pathname contents\n"
        source.write_bytes(newer)
        focus(browser)
        chord("F5")
        time.sleep(1)
        session.capture("stale-04-new-source-held.png")
        release(held)
        assert source.read_bytes() == newer and retained.read_bytes() == original
        assert not (destination / source.name).exists()
        checks.append("replacing the original selected file during a held gesture preserves both versions and admits no successful Move")
        checkpoint("stale-04-source-refused")

        held = begin("stale-05-reviewed-source")
        release(held)
        assert not source.exists() and (destination / source.name).read_bytes() == newer
        undo(newer)
        assert retained.read_bytes() == original
        checks.append("a fresh drag captures the newly reviewed source and shares Undo while the old selected bytes remain untouched")
        checkpoint("stale-05-source-reviewed")
        session.capture("stale-06-complete.png")
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
                "scope": "held destination/source replacement, ordinary contents refresh and fresh reviewed gestures; device/delayed/full-product gates remain open"}
    finally:
        checkpoint("stale-final")
