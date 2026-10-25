"""Actual Iced device-row drops against the private native GIO provider."""
import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Device drops"
    assert root.is_relative_to(session.runtime / "home")
    (root / "Source").mkdir(parents=True)
    (root / "Device destination").mkdir()
    (root / "Source/drag.txt").write_bytes(b"Device drag source bytes\n")
    config.write_text(config.read_text() + 'files { sidebar "always"; '
                      'places { place "home"; place "downloads"; }; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Device drops"
    source = root / "Source/drag.txt"
    local = root / "Device destination"
    media = Path(session.env["CANOPA_FIXTURE_VOLUMES"])
    payload = media / "Fixture USB"
    assert media.is_relative_to(session.runtime)
    original = source.read_bytes()
    drive_note = (payload / "Drive notes.txt").read_bytes()
    wire = session.output / "files.log"
    pointer = session.build_pointer("drag")
    checks, geometry = [], []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w["title"] == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "device-drop Browser focus")

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
        window = session.wait_for(lambda: title(directory.name), "device-drop location")
        time.sleep(.3)
        return window

    def events():
        return (media / "events").read_text().splitlines()

    def notices():
        p = session.output / "notifications.json"
        return json.loads(p.read_text()) if p.exists() else []

    def receipts():
        # Scheduling metadata only, never an execution payload or authority.
        p = session.runtime / "state/canopa-files/operations.sqlite3"
        connection = sqlite3.connect(p.as_uri() + "?mode=ro", uri=True)
        try:
            return connection.execute("SELECT id,revision,phase FROM requests WHERE id NOT IN (SELECT request FROM source_leases) ORDER BY id").fetchall()
        finally:
            connection.close()

    def inventory():
        result = {}
        for label, directory in [("home", root), ("drive", payload)]:
            for p in directory.rglob("*"):
                key = label + "/" + os.fsencode(p.relative_to(directory)).hex()
                result[key] = {"kind": "directory", "inode": p.stat().st_ino} if p.is_dir() else {
                    "kind": "file", "inode": p.stat().st_ino, "bytes": p.stat().st_size,
                    "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
        return result

    def checkpoint(name):
        (session.output / (name + ".json")).write_text(json.dumps({"checks": checks,
            "windows": windows(), "namespace": inventory(), "receipts": receipts(),
            "notices": notices(), "provider_events": events()}, indent=2) + "\n")

    def check(message):
        checks.append(message)
        checkpoint(f"device-drop-checkpoint-{len(checks):02}")

    def saved(before, kind):
        return session.wait_for(lambda: next((n for n in notices()[before:]
            if n["body"].split(" ·", 1)[0] == ("1 reversed" if kind == "Undo" else "1 completed")
            and (kind == "Undo" or (n["title"].startswith(kind) and "0 failed" in n["body"] and "0 remaining" in n["body"]))), None),
            "saved device-drop " + kind)

    session.wait_for(lambda: "private provider initialized" in events(), "private device provider")
    focus(browser)
    browser = navigate(source)
    chord("2", "ctrl")
    session.run(["niri", "msg", "action", "set-column-width", "40%"])
    chord("n", "ctrl")
    session.wait_for(lambda: len(windows()) == 2, "two device-drop browsers")
    target = next(w for w in windows() if w["id"] != browser["id"])
    session.run(["niri", "msg", "action", "set-column-width", "60%"])
    navigate(local)
    chord("2", "ctrl")
    focus(browser)
    session.run(["niri", "msg", "action", "move-column-to-first"])

    def align():
        focus(target)
        focus(browser)
        session.run(["niri", "msg", "action", "center-visible-columns"])
        time.sleep(.2)

    def begin(name, action="2", target_x=75, target_y=228):
        align()
        # Move/Undo can restore a file into a refreshed, unselected source view.
        # Select it through ordinary browsing input before each new gesture.
        chord("Home")
        actual = sorted(windows(), key=lambda w: w["layout"]["pos_in_scrolling_layout"])
        output = next(o["logical"] for o in json.loads(session.run(["niri", "msg", "--json", "outputs"])).values() if o.get("logical"))
        total = sum(w["layout"]["tile_size"][0] for w in actual)
        assert len(actual) == 2 and total <= output["width"] + 1
        assert target_x > 160 or next(w["layout"]["window_size"][0] for w in actual if w["id"] == target["id"]) >= 560

        def point(window, x, y):
            index = next(i for i, w in enumerate(actual) if w["id"] == window["id"])
            layout = actual[index]["layout"]
            left = (output["width"] - total) / 2 + sum(w["layout"]["tile_size"][0] for w in actual[:index])
            dx, dy = layout["window_offset_in_tile"]
            return str(round((left + dx + x) * 1600 / output["width"])), str(round((dy + y) * 1000 / output["height"]))

        # The source toolbar can reflow. Find its one selected row's painted band.
        frame = session.capture(name + "-before.png")
        first_x = int(point(browser, 260, 350)[0]) * frame.width // 1600
        background = frame.getpixel((first_x, round(350 * frame.height / output["height"])))
        runs, start = [], None
        for y in range(round(100 * scale), round(280 * scale) + 1):
            selected = y < round(280 * scale) and max(abs(a - b) for a, b in zip(frame.getpixel((first_x, y)), background)) > 8
            if selected and start is None:
                start = y
            elif not selected and start is not None:
                if y - start >= round(20 * scale):
                    runs.append((start, y))
                start = None
        assert len(runs) == 1, f"expected one painted source selection: {runs}"
        source_y = sum(runs[0]) / (2 * scale)
        first, last = point(browser, 260, source_y), point(target, target_x, target_y)
        geometry.append({"name": name, "windows": actual, "output": output,
                         "source_band": runs[0], "source": first, "target": last})
        (session.output / "device-drop-geometry.json").write_text(json.dumps(geometry, indent=2) + "\n")
        before, notice_count, wire_start = inventory(), len(notices()), len(wire.read_text())
        ready, release = session.runtime / (name + ".ready"), session.runtime / (name + ".release")
        helper = session.start([str(pointer), *first, *last, str(ready), str(release)], name + "-pointer.log")
        session.wait_for(ready.exists, "held device-row drag")
        starts = re.findall(r"-> wl_data_device#\d+\.start_drag\(wl_data_source#(\d+)", wire.read_text()[wire_start:])
        assert starts, "visible source did not start a native device drag"
        source_id = starts[-1]

        def negotiated(expected):
            actions = re.findall(rf"wl_data_source#{source_id}\.action\((\d+)\)", wire.read_text()[wire_start:])
            return actions and actions[-1] == expected

        session.wait_for(lambda: negotiated(action), "negotiated device-row action " + action)
        assert inventory() == before and len(notices()) == notice_count, "hover admitted device file work"
        session.capture(name + "-held.png")
        return {"helper": helper, "release": release, "source": source_id,
                "wire_start": len(wire.read_text()), "rows": receipts(),
                "notices": notice_count, "namespace": before, "negotiated": negotiated}

    def release(held):
        held["release"].touch()
        assert held["helper"].wait(timeout=10) == 0
        session.processes.remove(held["helper"])
        source_id = held["source"]

        def settled():
            current = wire.read_text()[held["wire_start"]:]
            ended = any(f"wl_data_source#{source_id}.{event}()" in current for event in ["cancelled", "dnd_finished"])
            destroy = current.rfind(f"-> wl_data_source#{source_id}.destroy()")
            acknowledged = destroy >= 0 and f"wl_display#1.delete_id({source_id})" in current[destroy:]
            new_rows = [row for row in receipts() if row[0] not in {old[0] for old in held["rows"]}]
            return (ended or acknowledged) and all(row[2] >= 2 for row in new_rows)

        session.wait_for(settled, "device gesture ends and admitted work is terminal")
        time.sleep(.3)

    def successful(name, *, copy=False, target_x=75, target_y=228):
        held = begin(name, target_x=target_x, target_y=target_y)
        modifier = None
        if copy:
            focus(target)
            modifier = session.start(["wtype", "-M", "ctrl", "-s", "60000", "-m", "ctrl"], name + "-modifier.log")
            session.wait_for(lambda: held["negotiated"]("1"), "Ctrl chooses device Copy")
            session.capture(name + "-copy-held.png")
        release(held)
        if modifier:
            session.retire(modifier)
        saved(held["notices"], "Copy" if copy else "Move")
        assert (payload / source.name).read_bytes() == original
        assert source.read_bytes() == original if copy else not source.exists()
        assert (payload / "Drive notes.txt").read_bytes() == drive_note
        before = len(notices())
        focus(target)
        chord("z", "ctrl")
        saved(before, "Undo")
        assert source.read_bytes() == original and not (payload / source.name).exists()
        assert (payload / "Drive notes.txt").read_bytes() == drive_note
        assert not list(local.iterdir())
        checkpoint(name + "-after-undo")

    def refused(held):
        release(held)
        assert inventory() == held["namespace"], "refused device drop changed files"
        assert receipts() == held["rows"], "refused device drop admitted an operation"
        assert len(notices()) == held["notices"], "refused device drop generated attention"

    def device_command(*, mounted, remove=False):
        focus(target)
        chord("Menu")
        chord("End")
        chord("Up")
        chord("Return")
        chord("End")
        if mounted and not remove:
            chord("Up")
        chord("Return")

    try:
        assert source.stat().st_dev == payload.stat().st_dev
        successful("device-drop-01-mounted")
        assert title(local.name)["id"] == target["id"], "device-row drop navigated the target browser"
        check("the mounted device Open row accepts default same-filesystem Move and shared Undo without navigating the destination browser or changing existing drive contents")

        successful("device-drop-02-copy", copy=True)
        check("Ctrl selects Copy onto the mounted device, preserves the source and reverses through shared Undo")

        provider_before = events()
        held = begin("device-drop-03-removal-control", action="0", target_y=266)
        refused(held)
        assert events() == provider_before, "dropping on the removal control dispatched a device request"
        check("the separate removal control rejects a drop without unmounting, changing files or admitting work")

        held = begin("device-drop-04-replaced-device")
        inode = payload.stat().st_ino
        (media / "unplugged").touch()
        session.wait_for(lambda: "device disconnected" in events(), "held target device disconnect")
        session.wait_for(lambda: held["negotiated"]("0"), "disconnected device withdraws the prepared action")
        (media / "replacement-device").touch()
        (media / "unplugged").unlink()
        session.wait_for(lambda: "device reconnected" in events(), "replacement device observation")
        time.sleep(.7)
        assert payload.stat().st_ino == inode
        assert held["negotiated"]("0"), "a fresh device row recaptured the held drag's original storage authority"
        session.capture("device-drop-04-replacement-held.png")
        refused(held)
        check("disconnecting and replacing a held device withdraws the action and cannot recapture authority even at the same root pathname and directory inode")

        successful("device-drop-05-reviewed-device")
        check("a fresh drag can use the reviewed replacement device and shared Undo restores its original source")

        before = len(notices())
        device_command(mounted=True, remove=True)
        session.wait_for(lambda: any(n["body"] == "Device unmounted." for n in notices()[before:]), "explicit native unmount")
        assert "unmount requested without force" in events()
        time.sleep(.4)
        provider_before = events()
        held = begin("device-drop-06-unmounted", action="0")
        refused(held)
        assert events() == provider_before and "mount requested" not in events(), "a drop silently mounted the device"
        check("the unmounted Open row rejects a fresh drop without implicit mounting, filesystem mutation or operation admission")

        before = len(notices())
        device_command(mounted=False)
        session.wait_for(lambda: any(n["body"] == "Device mounted." for n in notices()[before:]), "explicit native mount")
        session.wait_for(lambda: title(payload.name), "explicit mount opens its live device root")
        navigate(local)
        successful("device-drop-07-remounted")
        check("explicit Open mounts the device through GIO; the same browser then accepts a fresh device-row transfer and shared Undo")

        focus(target)
        navigate(payload)
        session.run(["niri", "msg", "action", "set-window-width", "340"])
        session.wait_for(lambda: next(w for w in windows() if w["id"] == target["id"])["layout"]["window_size"][0] == 340, "narrow mounted-device browser")
        successful("device-drop-08-narrow-root", target_x=220, target_y=350)
        assert title(payload.name)["id"] == target["id"]
        check("the mounted root remains a usable drop destination in a 340px tile without a sidebar, with the same transfer and Undo behavior")
        assert process.poll() is None and not any("UNEXPECTED forced" in event for event in events())
        for log in session.output.glob("files*.log"):
            assert not any(marker in log.read_text() for marker in ("panicked", "CRITICAL")), log
        session.capture("device-drop-09-complete.png")
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
                "scope": "private native provider device drops and mounted-root tiling; physical devices and complete product acceptance remain separate"}
    finally:
        checkpoint("device-drop-final")
