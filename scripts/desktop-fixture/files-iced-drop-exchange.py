"""Real GTK/Files exchange, clipboard coexistence and windowless receipts."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
from urllib.parse import quote_from_bytes
from PIL import ImageChops


def setup(session, fixture, config):
    root = fixture / "Native exchange"
    assert root.is_relative_to(session.runtime / "home")
    for name in ["Source", "Incoming", "Destination", "Waiting destination", "Clipboard destination", "Text clipboard folder"]:
        (root / name).mkdir(parents=True)
    raw = os.fsdecode(b"foreign \xff #.txt")
    (root / "Incoming" / raw).write_bytes(b"Foreign native bytes\n")
    (root / "Waiting destination" / raw).write_bytes(b"Previous destination bytes\n")
    (root / "Source" / os.fsdecode(b"a outgoing \xff #.txt")).write_bytes(b"Outgoing native bytes\n")
    (root / "Source/clipboard.txt").write_bytes(b"Owned clipboard bytes\n")
    config.write_text(config.read_text() + 'files { shortcut "operations" "<Alt>o"; }\n')


def workflow(session, fixture, browser, scheme, scale, process):
    root = fixture / "Native exchange"
    original = root / "Incoming" / os.fsdecode(b"foreign \xff #.txt")
    outgoing = root / "Source" / os.fsdecode(b"a outgoing \xff #.txt")
    clip = root / "Source/clipboard.txt"
    destination = root / "Destination"
    waiting = root / "Waiting destination"
    text_folder = root / "Text clipboard folder"
    expected_in, expected_out = b"Foreign native bytes\n", b"Outgoing native bytes\n"
    checks, geometries = [], []
    pointer = session.build_pointer("drag")
    files_wire = session.output / "files.log"
    binary = Path(process.args[0])

    def read(path):
        return json.loads(path.read_text()) if path.exists() else {}

    def check(message):
        checks.append(message)
        (session.output / "exchange-progress.json").write_text(json.dumps(checks, indent=2) + "\n")
        (session.output / f"exchange-checkpoint-{len(checks):02}.json").write_text(json.dumps(
            {"checks": checks, "notices": notices(), "gtk": read(report), "windows": windows(), "namespace": namespace()}, indent=2) + "\n")

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def files():
        return [w for w in windows() if w.get("app_id") == "org.canopa.Files"]

    def title(name):
        return next((w for w in files() if w.get("title") == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "exchange focus")

    def chord(key, *modifiers):
        command = ["wtype"]
        for modifier in modifiers:
            command += ["-M", modifier]
        command += ["-k", key]
        for modifier in reversed(modifiers):
            command += ["-m", modifier]
        session.run(command)
        time.sleep(.2)

    def navigate(path):
        chord("l", "ctrl")
        session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(path), safe="/")])
        chord("Return")
        directory = path if path.is_dir() else path.parent
        result = session.wait_for(lambda: title(directory.name), "exchange location")
        time.sleep(.3)
        return result

    def notices():
        return read(session.output / "notifications.json") or []

    def completed(kind):
        if kind == "Undo":
            return [e for e in notices() if e["body"].split(" ·", 1)[0].endswith(" reversed")]
        return [e for e in notices() if e["title"].startswith(kind)
                and e["body"].split(" ·", 1)[0].endswith(" completed")
                and "0 failed" in e["body"] and "0 remaining" in e["body"]]

    def namespace():
        return {os.fsencode(p.relative_to(root)).hex():
                ("directory", p.stat().st_ino) if p.is_dir() else
                ("file", p.stat().st_ino, hashlib.sha256(p.read_bytes()).hexdigest())
                for p in root.rglob("*")}

    def text_owner(name):
        owner = session.start(["wl-copy", "--foreground", "--type", "text/plain;charset=utf-8", str(text_folder)], name + ".log")
        attempts = []
        def ready():
            try:
                value = session.run(["wl-paste", "--no-newline", "--type", "text/plain;charset=utf-8"])
            except subprocess.CalledProcessError as error:
                attempts.append({"returncode": error.returncode, "stderr": error.stderr.decode(errors="replace") if isinstance(error.stderr, bytes) else error.stderr})
                value = None
            (session.output / f"{name}-acquisition.json").write_text(json.dumps({"attempts": attempts, "ready": value == str(text_folder)}, indent=2) + "\n")
            return value == str(text_folder)
        session.wait_for(ready, "external text clipboard")
        return owner

    def start_peer(name, action="copy"):
        report, finish = session.output / f"{name}-gtk.json", session.runtime / f"{name}.finish"
        output = root / f"{name} output"
        previous = session.env.get("WAYLAND_DEBUG")
        session.env["WAYLAND_DEBUG"] = "client"
        try:
            peer = session.start(["/usr/bin/python3", str(Path(__file__).with_name("files-dnd-transport-peer.py")),
                                  str(root), str(original), str(output), str(report), str(finish), action], name + "-gtk.log")
        finally:
            if previous is None:
                session.env.pop("WAYLAND_DEBUG")
            else:
                session.env["WAYLAND_DEBUG"] = previous
        session.wait_for(lambda: read(report).get("ready") and any(w.get("title") == "GTK DnD transport" for w in windows()), "independent GTK exchange peer")
        window = next(w for w in windows() if w.get("title") == "GTK DnD transport")
        return peer, window, report, finish, output

    def arrange(left, right):
        for window in [right, left]:
            focus(window)
            session.run(["niri", "msg", "action", "set-column-width", "50%"])
        session.run(["niri", "msg", "action", "move-column-to-first"])
        session.run(["niri", "msg", "action", "center-visible-columns"])
        time.sleep(.3)

    def begin(name, left, right, *, from_files):
        arrange(left, right)
        focus(left)
        actual = sorted(windows(), key=lambda w: w["layout"]["pos_in_scrolling_layout"])
        assert len(actual) == 2
        logical = next(o["logical"] for o in json.loads(session.run(["niri", "msg", "--json", "outputs"])).values() if o.get("logical"))
        total = sum(w["layout"]["tile_size"][0] for w in actual)
        assert total <= logical["width"] + 1
        def point(window, x, y):
            index = next(i for i, w in enumerate(actual) if w["id"] == window["id"])
            layout = actual[index]["layout"]
            left = (logical["width"] - total) / 2 + sum(w["layout"]["tile_size"][0] for w in actual[:index])
            dx, dy = layout["window_offset_in_tile"]
            return str(round((left + dx + x) * 1600 / logical["width"])), str(round((dy + y) * 1000 / logical["height"]))
        first, last = point(left, 100, 144 if from_files else 200), point(right, 100, 350)
        geometries.append({"name": name, "output": logical, "windows": actual, "source": first, "target": last})
        (session.output / "exchange-geometry.json").write_text(json.dumps(geometries, indent=2) + "\n")
        before, notice_count = namespace(), len(notices())
        offset = len(files_wire.read_text())
        ready, release = session.runtime / f"{name}.ready", session.runtime / f"{name}.release"
        helper = session.start([str(pointer), *first, *last, str(ready), str(release)], name + "-pointer.log")
        session.wait_for(ready.exists, "held external drag")
        # Native action negotiation proves source preparation completed before release.
        source_wire = files_wire if from_files else session.output / "exchange-gtk.log"
        action = "1" if from_files and name == "external-copy" else "2"
        def accepted():
            wire = source_wire.read_text()
            if from_files:
                wire = wire[offset:]
            starts = list(re.finditer(r"-> wl_data_device#\d+\.start_drag\(wl_data_source#(\d+)", wire))
            if not starts:
                return None
            actions = re.findall(rf"wl_data_source#{starts[-1][1]}\.action\((\d+)\)", wire[starts[-1].end():])
            return starts[-1][1] if actions and actions[-1] == action else None
        source_id = session.wait_for(accepted, "actual foreign native action")
        assert namespace() == before and len(notices()) == notice_count, "hover admitted file work"
        session.capture(name + "-held.png")
        return helper, release, source_id, offset, before, notice_count

    def release(held):
        helper, marker = held[:2]
        marker.touch()
        helper.wait(timeout=10)
        assert helper.returncode == 0
        session.processes.remove(helper)
        time.sleep(.3)

    def undo(window, predicate):
        before = len(completed("Undo"))
        focus(window)
        chord("z", "ctrl")
        session.wait_for(predicate, "actual external drop Undo")
        session.wait_for(lambda: len(completed("Undo")) > before, "saved external drop Undo")

    def operation_facts(name):
        focus(browser)
        chord("o", "alt")
        operations = session.wait_for(lambda: title("Operations"), "native operation overview for cancellation")
        focus(operations)
        session.run(["niri", "msg", "action", "set-column-width", "100%"])
        session.run(["niri", "msg", "action", "center-column"])
        time.sleep(.3)
        logical = next(o["logical"] for o in json.loads(session.run(["niri", "msg", "--json", "outputs"])).values() if o.get("logical"))
        frame = session.capture(name)
        sx, sy = frame.width / logical["width"], frame.height / logical["height"]
        # Exclude title/focus chrome and inspect the actual operation facts. A
        # cancelled hover admits no job and must not add a persistent error row.
        facts = frame.crop((round(12 * sx), round(110 * sy), round((logical["width"] - 24) * sx), round(335 * sy)))
        chord("Escape")
        session.wait_for(lambda: title("Operations") is None, "close cancellation overview")
        return facts

    focus(browser)
    browser = navigate(destination)
    chord("2", "ctrl")
    external_text = text_owner("exchange-text-owner")
    peer, gtk, report, finish, peer_output = start_peer("exchange")

    # A foreign hover payload is read again after Drop, then enters the normal queue.
    before = len(completed("Move"))
    held = begin("foreign-move", gtk, browser, from_files=False)
    release(held)
    moved = destination / original.name
    session.wait_for(lambda: moved.exists() and not original.exists(), "foreign raw-name Move")
    session.wait_for(lambda: len(completed("Move")) > before, "saved foreign Move")
    session.wait_for(lambda: read(report)["sources"][-1]["finished"], "GTK receipt after Files commit")
    assert moved.read_bytes() == expected_in
    wire = files_wire.read_text()[held[3]:]
    drop = wire.index(".drop()")
    reads = list(re.finditer(r'-> wl_data_offer#(\d+)\.receive\("text/uri-list"', wire))
    assert any(a[1] == b[1] and a.start() < drop < b.start() for a in reads for b in reads), "Files did not re-read the exact foreign offer after Drop"
    undo(browser, lambda: original.exists() and original.read_bytes() == expected_in and not moved.exists())
    check("foreign raw-byte URI hover is confirmed again after Drop, moved by Files, acknowledged to GTK and reversed through shared Undo")

    before_cancel_facts = operation_facts("foreign-cancel-before-operations.png")
    held = begin("foreign-cancel", gtk, browser, from_files=False)
    finish.with_suffix(".cancel").touch()
    session.wait_for(lambda: read(report).get("cancel_requested") and read(report).get("cancel_active") == 1, "GTK cancels its original source")
    gtk_wire = session.output / "exchange-gtk.log"
    def destroyed():
        wire = gtk_wire.read_text()
        marker = f"-> wl_data_source#{held[2]}.destroy()"
        offset = wire.rfind(marker)
        return offset >= 0 and f"wl_display#1.delete_id({held[2]})" in wire[offset + len(marker):]
    session.wait_for(destroyed, "acknowledged cancelled GTK source destruction")
    release(held)
    hover_wire = files_wire.read_text()[held[3]:]
    hover_offer = re.findall(r'-> wl_data_offer#(\d+)\.receive\("text/uri-list"', hover_wire)[0]
    session.wait_for(lambda: f"-> wl_data_offer#{hover_offer}.destroy()" in files_wire.read_text()[held[3]:], "Files retires the cancelled foreign offer")
    retired_wire = files_wire.read_text()[held[3]:]
    if ".drop()" in retired_wire:
        assert len(re.findall(rf'-> wl_data_offer#{hover_offer}\.receive\("text/uri-list"', retired_wire)) >= 2, "phantom Drop was not freshly confirmed"
    assert namespace() == held[4] and len(notices()) == held[5], "cancelled foreign hover admitted a filesystem operation"
    assert original.read_bytes() == expected_in and not moved.exists()
    after_cancel_facts = operation_facts("foreign-cancel-after-operations.png")
    difference = ImageChops.difference(before_cancel_facts, after_cancel_facts)
    if difference.getbbox():
        difference.save(session.output / "foreign-cancel-operations-difference.png")
        raise AssertionError("A cancelled foreign drag changed the operation overview without admitting file work")
    check("cancelled GTK source cannot turn prefetched hover bytes or a phantom Drop into a Files mutation or persistent operation error")

    focus(browser)
    chord("l", "ctrl")
    chord("v", "ctrl")
    chord("Return")
    browser = session.wait_for(lambda: title(text_folder.name), "Iced text clipboard after native drops")
    assert session.run(["wl-paste", "--no-newline", "--type", "text/plain;charset=utf-8"]) == str(text_folder)
    check("foreign text clipboard remains readable by Iced's location editor after completed and cancelled native drags")

    focus(browser)
    navigate(clip)
    chord("c", "ctrl")
    expected_clip = "copy\nfile://" + quote_from_bytes(os.fsencode(clip), safe="/") + "\n"
    # The old owner exits on handoff; reap it before a liveness-scanning wait.
    assert external_text.wait(timeout=10) == 0
    session.processes.remove(external_text)
    session.wait_for(lambda: session.run(["wl-paste", "--no-newline", "--type", "x-special/gnome-copied-files"]) == expected_clip, "owned Files clipboard")
    browser = navigate(outgoing)
    chord("2", "ctrl")
    held = begin("external-copy", browser, gtk, from_files=True)
    release(held)
    session.wait_for(lambda: read(report)["drops"] and read(report)["drops"][-1]["received"], "independent GTK Copy")
    assert (peer_output / outgoing.name).read_bytes() == expected_out and outgoing.read_bytes() == expected_out
    assert not read(report)["drops"][-1]["finished"]
    finish.touch()
    session.wait_for(lambda: read(report)["drops"][-1]["finished"] and f"wl_data_source#{held[2]}.dnd_finished()" in files_wire.read_text()[held[3]:], "held native Copy receipt")
    check("actual Iced source exports raw-byte URIs to GTK Copy and retains the source until delayed native completion")

    assert session.run(["wl-paste", "--no-newline", "--type", "x-special/gnome-copied-files"]) == expected_clip
    focus(browser)
    browser = navigate(root / "Clipboard destination")
    before = len(completed("Copy"))
    chord("v", "ctrl")
    pasted = root / "Clipboard destination/clipboard.txt"
    session.wait_for(lambda: pasted.exists() and len(completed("Copy")) > before, "original owned clipboard after outgoing drag")
    assert pasted.read_bytes() == clip.read_bytes() and not (pasted.parent / outgoing.name).exists()
    undo(browser, lambda: not pasted.exists() and clip.read_bytes() == b"Owned clipboard bytes\n")
    check("dragging another row never replaces the owned file clipboard or its original selection")

    # A foreign receipt waits through a conflict, all Files windows closing and reactivation.
    focus(browser)
    browser = navigate(waiting)
    chord("2", "ctrl")
    held = begin("foreign-windowless", gtk, browser, from_files=False)
    release(held)
    question = session.wait_for(lambda: title("File conflict"), "foreign Move conflict")
    assert original.read_bytes() == expected_in and (waiting / original.name).read_bytes() == b"Previous destination bytes\n"
    assert not read(report)["sources"][-1]["finished"]
    session.capture("foreign-windowless-question.png")
    session.run(["niri", "msg", "action", "close-window", "--id", str(browser["id"])])
    operations = session.wait_for(lambda: title("Operations"), "background drop Operations")
    session.wait_for(lambda: title("File conflict") is None, "closing source window defers foreign drop question")
    focus(operations)
    chord("Escape")
    session.wait_for(lambda: not files(), "every Files window closes during accepted foreign drop")
    time.sleep(.5)
    assert process.poll() is None and not read(report)["sources"][-1]["finished"] and not read(report)["sources"][-1]["cancelled"]
    assert not any(e["title"].startswith("Move") and "Waiting destination" in e["title"] for e in notices()), "closing windows cancelled or completed the retained drop"
    assert original.read_bytes() == expected_in and (waiting / original.name).read_bytes() == b"Previous destination bytes\n"
    session.capture("foreign-windowless-pending.png")
    check("an accepted foreign drop and unanswered conflict survive every Files window closing without implicit permission")

    session.run([str(binary), str(waiting)])
    browser = session.wait_for(lambda: title(waiting.name), "same-owner foreign drop reactivation")
    assert browser["pid"] == process.pid
    focus(browser)
    chord("o", "alt")
    operations = session.wait_for(lambda: title("Operations"), "reopened shared drop queue")
    focus(operations)
    session.capture("foreign-windowless-before-review.png")
    # A newly opened Operations has no focused control: New browser, Recovery,
    # this transfer's Details, Cancel, then Review.
    for _ in range(5):
        chord("Tab")
    session.capture("foreign-windowless-review-control.png")
    chord("Return")
    question = session.wait_for(lambda: title("File conflict"), "same retained foreign drop question")
    focus(question)
    session.capture("foreign-windowless-resumed.png")
    before = len(completed("Move"))
    chord("Tab")
    chord("Return")
    session.wait_for(lambda: not original.exists() and (waiting / original.name).read_bytes() == expected_in, "explicit Replace resumes original foreign drop")
    session.wait_for(lambda: len(completed("Move")) > before and read(report)["sources"][-1]["finished"], "native receipt follows actual background completion")
    focus(operations)
    chord("Escape")
    undo(browser, lambda: original.exists() and original.read_bytes() == expected_in and (waiting / original.name).read_bytes() == b"Previous destination bytes\n")
    check("reactivation keeps the same backend and native receipt; explicit Replace and shared Undo preserve both actual versions")

    # Remove clipboard ownership so it cannot mask the outgoing drag lifetime.
    text_owner("exchange-final-text-owner")
    session.retire(peer)
    peer, gtk, report, finish, peer_output = start_peer("final", "move")
    focus(browser)
    browser = navigate(outgoing)
    chord("2", "ctrl")
    held = begin("external-windowless-move", browser, gtk, from_files=True)
    release(held)
    session.wait_for(lambda: read(report)["drops"] and read(report)["drops"][-1]["received"], "GTK commits external Move before receipt")
    assert not outgoing.exists() and (peer_output / outgoing.name).read_bytes() == expected_out
    for window in files():
        session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
    session.wait_for(lambda: not files(), "outgoing origin closes before native completion")
    time.sleep(.5)
    assert process.poll() is None and not read(report)["drops"][-1]["finished"]
    session.capture("external-windowless-move-held.png")
    finish.touch()
    assert process.wait(timeout=15) == 0
    session.processes.remove(process)
    session.wait_for(lambda: read(report)["drops"][-1]["finished"], "GTK completes the held Move receipt")
    assert f"wl_data_source#{held[2]}.dnd_finished()" in files_wire.read_text()[held[3]:]
    assert not outgoing.exists() and (peer_output / outgoing.name).read_bytes() == expected_out and not files()
    check("outgoing native Move completes with zero Files windows, performs no second deletion and releases the ordinary owner afterward")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "actual independent GTK exchange, clipboard coexistence and windowless receipt completion; full drag/drop and product acceptance remains separate"}


def verify(session, fixture, browser, scheme, scale, process):
    try:
        return workflow(session, fixture, browser, scheme, scale, process)
    finally:
        root = fixture / "Native exchange"
        inventory = [{"path_bytes": os.fsencode(p.relative_to(root)).hex(), "bytes": p.stat().st_size,
                      "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
                     for p in sorted(root.rglob("*")) if p.is_file()]
        (session.output / "exchange-filesystem.json").write_text(json.dumps(inventory, indent=2) + "\n")
