"""One real Iced Downloads session: previews, tiled moves, Undo and image action."""
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time
from urllib.parse import quote_from_bytes

from PIL import Image


def workflow(session, fixture, browser, config, scheme, scale):
    checks = []
    notifications = session.output / "notifications.json"
    wire = session.output / "files.log"

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((w for w in windows() if w["title"] == value), None)

    def wait_title(value):
        window = session.wait_for(lambda: next((w for w in windows() if w["title"] == value and w["is_focused"]), None), value)
        time.sleep(.25)
        return window

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "Downloads workflow focus")
        time.sleep(.2)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name])
            time.sleep(.2)

    def chord(modifiers, key):
        session.run(["wtype", *[part for mod in modifiers for part in ("-M", mod)], "-k", key,
                     *[part for mod in reversed(modifiers) for part in ("-m", mod)]])
        time.sleep(.25)

    def navigate(window, path):
        focus(window)
        chord(["ctrl"], "l")
        session.run(["wtype", "file://" + quote_from_bytes(os.fsencode(path), safe="/")])
        keys("Return")
        return wait_title(f"{path.name} — Canopa Files")

    def width(window, value):
        focus(window)
        session.run(["niri", "msg", "action", "set-window-width", str(value)])
        session.wait_for(lambda: next(w for w in windows() if w["id"] == window["id"])["layout"]["window_size"][0] == value,
                         f"Downloads width {value}")
        time.sleep(.3)

    def events():
        return json.loads(notifications.read_text()) if notifications.exists() else []

    def completed(before, count, undo=False):
        def delivered():
            for e in events()[before:]:
                first = e["body"].split(" ·", 1)[0]
                if undo and first == f"{count} reversed":
                    return e
                if not undo and first == f"{count} completed" and "0 failed" in e["body"] and "0 remaining" in e["body"]:
                    return e
            return None
        return session.wait_for(delivered, "saved Downloads Undo" if undo else "saved Downloads Move")

    def check(message):
        checks.append(message)
        (session.output / "downloads-progress.json").write_text(json.dumps(checks, indent=2) + "\n")
        (session.output / f"downloads-checkpoint-{len(checks):02}.json").write_text(json.dumps(
            {"checks": checks, "windows": windows(), "notices": events(), "namespace": inventory(fixture)}, indent=2) + "\n")

    def close_preview(preview, owner):
        focus(preview)
        keys("Escape")
        session.wait_for(lambda: all(w["id"] != preview["id"] for w in windows()), "exact Quick Look retirement")
        session.wait_for(lambda: any(w["id"] == owner["id"] and w["is_focused"] for w in windows()), "Quick Look restores its originating browser")

    def image_visible(name):
        def painted():
            frame = session.capture(name).convert("RGB")
            colors = frame.getcolors(frame.width * frame.height)
            return sum(count for count, color in colors
                       if all(abs(a - b) < 5 for a, b in zip(color, (24, 160, 120)))) > 500
        session.wait_for(painted, "actual Downloads image pixels")

    media = Path(session.env["CANOPA_FIXTURE_VOLUMES"])
    assert media.is_relative_to(session.runtime)
    session.wait_for(lambda: (media / "events").exists() and "private provider initialized" in (media / "events").read_text(), "private Downloads volume provider")
    source = browser
    destination = fixture / "A-project"
    selected = [fixture / "B-notes.txt", fixture / "C-photo.png"]
    originals = {path.name: path.read_bytes() for path in selected}
    destination_note = (destination / "notes.rs").read_bytes()
    (session.output / "downloads-original-note.txt").write_bytes(originals[selected[0].name])
    (session.output / "downloads-original-photo.png").write_bytes(originals[selected[1].name])
    wide = next(w for w in windows() if w["id"] == source["id"])
    assert wide["layout"]["window_size"][0] >= 960, wide
    session.capture("downloads-00-wide-columns.png")
    check("the workflow begins in a wide default-column browser before adapting to a narrow tile")
    width(source, 360)
    keys("Home", "Down", "space")
    preview = wait_title("Quick Look: B-notes.txt — Canopa Files")
    assert preview["is_floating"] and preview["layout"]["window_size"][0] >= 600, preview
    session.capture("downloads-01-narrow-text.png")
    check("default columns in a 360px browser open a separate comfortably sized floating text preview")
    keys("Down")
    preview = wait_title("Quick Look: C-photo.png — Canopa Files")
    image_visible("downloads-02-following-image.png")
    check("navigation from the focused preview selects the next source file and paints its actual image")

    chord(["ctrl"], "n")
    target = session.wait_for(lambda: next((w for w in windows() if w["id"] != source["id"] and w["title"] == "Downloads — Canopa Files"), None), "second Downloads browser")
    navigate(target, destination)
    width(target, 640)
    keys("Home")
    assert title("Quick Look: C-photo.png — Canopa Files")["id"] == preview["id"]
    assert title("Quick Look: C-photo.png — Canopa Files")["is_floating"]
    session.capture("downloads-03-independent-tiles.png")
    check("a second tiled browser navigates independently without claiming the source browser's floating preview")
    close_preview(preview, source)
    check("closing the preview returns focus to its exact source browser")
    keys("space")
    background_preview = wait_title("Quick Look: C-photo.png — Canopa Files")
    focus(target)
    keys("Home")  # fresh target input must not authorize a background focus steal
    session.run(["niri", "msg", "action", "close-window", "--id", str(background_preview["id"])])
    session.wait_for(lambda: all(w["id"] != background_preview["id"] for w in windows()), "background Quick Look retirement")
    assert any(w["id"] == target["id"] and w["is_focused"] for w in windows()), "closing a background preview stole focus from the destination"
    check("closing an inactive preview after fresh destination input leaves focus in that destination browser")
    focus(source)

    keys("Home", "Down")
    chord(["shift"], "Down")
    for value in (640, 360):
        width(source, value)
        assert title("Downloads — Canopa Files")["id"] == source["id"]
    chord(["ctrl"], "x")
    def clipboard_ready():
        try:
            payload = session.run(["wl-paste", "--type", "x-special/gnome-copied-files"]).strip().splitlines()
            return payload if payload[0] == "cut" and set(payload[1:]) == {p.as_uri() for p in selected} else None
        except (subprocess.CalledProcessError, IndexError):
            return None
    payload = session.wait_for(clipboard_ready, "exact two-file native Cut payload")
    (session.output / "downloads-cut-payload.txt").write_text("\n".join(payload) + "\n")
    check("widening and narrowing preserve the two-file selection and Cut exports precisely those native file URIs")
    focus(target)
    before = len(events())
    chord(["ctrl"], "v")
    moved = completed(before, 2)
    assert "2 items" in moved["title"]
    assert all(not p.exists() for p in selected)
    assert all((destination / name).read_bytes() == data for name, data in originals.items())
    assert (destination / "notes.rs").read_bytes() == destination_note
    session.capture("downloads-04-moved.png")
    check("Paste moves both selected files into the second window, saves its result and preserves existing destination contents")
    before = len(events())
    chord(["ctrl"], "z")
    completed(before, 2, undo=True)
    assert all(p.read_bytes() == originals[p.name] for p in selected)
    assert all(not (destination / p.name).exists() for p in selected)
    assert (destination / "notes.rs").read_bytes() == destination_note
    check("shared Undo from the destination restores both source files and leaves its original file intact")
    keys("Home", "space")
    destination_preview = wait_title("Quick Look: notes.rs — Canopa Files")
    assert destination_preview["is_floating"]
    session.capture("downloads-04a-destination-after-undo.png")
    close_preview(destination_preview, target)
    focus(source)
    session.run(["wtype", "C-photo"])
    keys("space")
    preview = wait_title("Quick Look: C-photo.png — Canopa Files")
    image_visible("downloads-05-undo-restored.png")
    close_preview(preview, source)
    check("both browsers retain normal keyboard selection and their own Quick Look after the mistaken move is undone")

    # A pointer move and keyboard move belong to the same ordinary workflow.
    # Both fitting tiles must be revealed before absolute coordinates are used.
    focus(target)
    focus(source)
    session.run(["niri", "msg", "action", "center-visible-columns"])
    tiled = sorted([w for w in windows() if not w["is_floating"]], key=lambda w: w["layout"]["pos_in_scrolling_layout"])
    assert {w["id"] for w in tiled} == {source["id"], target["id"]}
    logical = next(o["logical"] for o in json.loads(session.run(["niri", "msg", "--json", "outputs"])).values() if o.get("logical"))
    total = sum(w["layout"]["tile_size"][0] for w in tiled)
    assert total <= logical["width"] + 1
    def left(window):
        index = next(i for i, w in enumerate(tiled) if w["id"] == window["id"])
        return (logical["width"] - total) / 2 + sum(w["layout"]["tile_size"][0] for w in tiled[:index]) + tiled[index]["layout"]["window_offset_in_tile"][0]
    source_width = next(w["layout"]["window_size"][0] for w in tiled if w["id"] == source["id"])
    target_width = next(w["layout"]["window_size"][0] for w in tiled if w["id"] == target["id"])
    frame = session.capture("downloads-05a-drag-selection.png")
    x = round((left(source) + source_width - 40) * scale)
    background = frame.getpixel((x, round(500 * scale)))
    runs, start = [], None
    for y in range(round(130 * scale), round(430 * scale) + 1):
        selected_band = y < round(430 * scale) and max(abs(a - b) for a, b in zip(frame.getpixel((x, y)), background)) > 8
        if selected_band and start is None:
            start = y
        elif not selected_band and start is not None:
            if y - start >= round(20 * scale):
                runs.append((start, y))
            start = None
    assert len(runs) == 1, f"expected one actual painted image selection: {runs}"
    source_y = sum(runs[0]) / (2 * scale)
    first = (str(round((left(source) + source_width - 40) * 1600 / logical["width"])), str(round(source_y * 1000 / logical["height"])))
    last = (str(round((left(target) + target_width - 80) * 1600 / logical["width"])), str(round(350 * 1000 / logical["height"])))
    (session.output / "downloads-drag-geometry.json").write_text(json.dumps(
        {"windows": tiled, "output": logical, "source": first, "target": last, "selection_band": runs[0]}, indent=2) + "\n")
    pointer = session.build_pointer("drag")
    ready, release = session.runtime / "downloads-drag.ready", session.runtime / "downloads-drag.release"
    before_namespace, before_notices, offset = inventory(fixture), len(events()), len(wire.read_text())
    helper = session.start([str(pointer), *first, *last, str(ready), str(release)], "downloads-drag-pointer.log")
    session.wait_for(ready.exists, "held Downloads image drag")
    sources = re.findall(r"-> wl_data_device#\d+\.start_drag\(wl_data_source#(\d+)", wire.read_text()[offset:])
    assert sources, "the visible image did not start a native drag"
    source_id = sources[-1]
    def move_selected():
        actions = re.findall(rf"wl_data_source#{source_id}\.action\((\d+)\)", wire.read_text()[offset:])
        return actions and actions[-1] == "2"
    session.wait_for(move_selected, "prepared Downloads native Move")
    assert inventory(fixture) == before_namespace and len(events()) == before_notices, "hover admitted Downloads file work"
    session.capture("downloads-05b-image-drag-held.png")
    release.touch()
    assert helper.wait(timeout=10) == 0
    session.processes.remove(helper)
    completed(before_notices, 1)
    assert not selected[1].exists() and (destination / selected[1].name).read_bytes() == originals[selected[1].name]
    assert selected[0].read_bytes() == originals[selected[0].name]
    before = len(events())
    focus(target)
    chord(["ctrl"], "z")
    completed(before, 1, undo=True)
    assert all(p.read_bytes() == originals[p.name] for p in selected)
    assert not (destination / selected[1].name).exists()
    check("a real held image drag moves only after release, preserves the note and reverses through the same destination-window Undo")
    focus(source)
    session.run(["wtype", "C-photo"])
    time.sleep(.3)

    # Create the command through the actual Iced editor, not a configuration stub.
    result = session.runtime / "thumbnail-action.json"
    output = fixture / "Thumbnail output"
    action_helper = session.runtime / "thumbnail-action.py"
    action_helper.write_text(
        'import json, os, sys\nfrom pathlib import Path\nfrom PIL import Image\n'
        f'output=Path({str(output)!r}); output.mkdir(exist_ok=True)\n'
        'files=[Path(value) for value in sys.argv[1:-1]]\ncreated=[]\n'
        'for path in files:\n'
        '    with Image.open(path) as image:\n'
        '        image.thumbnail((32,32))\n'
        '        target=output/(path.stem+"-thumb.png")\n'
        '        image.save(target); created.append(str(target))\n'
        f'Path({str(result)!r}).write_text(json.dumps(dict(files=[str(path) for path in files], directory=sys.argv[-1], cwd=os.getcwd(), outputs=created)))\n'
        'print("Created thumbnail from the selected image.")\n')
    (session.output / "downloads-thumbnail-helper.py").write_text(action_helper.read_text())
    chord(["ctrl", "shift"], "a")
    editor = wait_title("Custom Actions — Canopa Files")
    keys("Tab", "space")  # empty editor: Add action
    session.capture("downloads-editor-01-added.png")
    # Selecting the new draft replaces the focus identities. Start from the
    # new group's unfocused state: chooser, Add, Delete, Name.
    keys("Tab", "Tab", "Tab", "Tab")
    def replace(value):
        chord(["ctrl"], "a")
        session.run(["wtype", value])
    replace("Create thumbnails")
    session.capture("downloads-editor-02-name.png")
    keys("Tab")
    replace("image-x-generic-symbolic")
    keys("Tab")
    replace(f'{shlex.quote(sys.executable)} {shlex.quote(str(action_helper))} %F %d')
    keys("Tab")
    replace("*.png")
    session.capture("downloads-editor-03-command.png")
    keys(*(["Tab"] * 7))  # Files, Folders, Min, Max, Shortcut, Record, Toolbar
    keys("space")
    chord(["shift"], "Tab")
    keys("space")  # Record shortcut
    chord(["ctrl", "alt"], "i")
    chord(["ctrl"], "s")
    session.wait_for(lambda: "Create thumbnails" in config.read_text() and "thumbnail-action.py" in config.read_text(), "graphical image action saved")
    saved = config.read_text()
    assert f'color-scheme "{scheme}"' in saved and '<Control><Alt>i' in saved
    assert re.search(r'toolbar\s*=\s*#true', saved), saved
    assert len(re.findall(r'^\s*action\s+', saved, re.MULTILINE)) == 1, saved
    (session.output / "downloads-saved-config.kdl").write_text(saved)
    session.capture("downloads-06-action-editor.png")
    chord(["ctrl"], "w")
    session.wait_for(lambda: all(w["id"] != editor["id"] for w in windows()), "exact saved action editor retirement")
    focus(source)
    check("the Iced editor creates and saves a file-only image action, icon, toolbar choice and recorded shortcut without losing shared appearance")
    before = len(events())
    chord(["ctrl", "alt"], "i")
    session.wait_for(lambda: any(e["title"] == "Create thumbnails" and e["body"] == "Finished" for e in events()[before:]), "actual image action completion")
    focus(source)
    chord(["ctrl"], "j")
    operations = wait_title("Operations — Canopa Files")
    def report():
        try:
            return json.loads(result.read_text())
        except (OSError, json.JSONDecodeError):
            return None
    invocation = session.wait_for(report, "actual thumbnail subprocess result")
    thumbnail = output / "C-photo-thumb.png"
    assert invocation == {"files": [str(selected[1])], "directory": str(fixture), "cwd": str(fixture), "outputs": [str(thumbnail)]}, invocation
    (session.output / "downloads-action-invocation.json").write_text(json.dumps(invocation, indent=2) + "\n")
    with Image.open(thumbnail) as image:
        assert image.size == (32, 24) and image.convert("RGB").getpixel((16, 12)) == (24, 160, 120)
    assert all(p.read_bytes() == originals[p.name] for p in selected)
    check("the configured shortcut runs a real image resize with the exact selected path and working directory, producing a 32x24 thumbnail without changing originals")
    width(operations, 360)
    # The only command is last: reverse traversal reaches Open folder then Output
    # independently of the preceding completed file-operation controls.
    chord(["shift"], "Tab")
    chord(["shift"], "Tab")
    before_output = session.capture("downloads-07a-output-closed.png")
    keys("Return")
    session.wait_for(lambda: session.capture("downloads-07b-output-open.png").tobytes() != before_output.tobytes(), "action output paints before its controls are traversed")
    keys("Tab", "Tab")  # Open folder, Copy all output (one short output page)
    keys("Return")
    action_output = session.run(["wl-paste", "--no-newline"])
    assert "Created thumbnail from the selected image." in action_output, action_output
    (session.output / "downloads-action-output.txt").write_text(action_output)
    session.capture("downloads-07-action-result.png")
    check("360px Operations exposes the finished action and its actual captured output through keyboard controls")
    chord(["ctrl"], "w")
    session.wait_for(lambda: all(w["id"] != operations["id"] for w in windows()), "Operations view closes without changing completed output")
    navigate(source, output)
    width(source, 360)
    keys("Home", "Left")
    wait_title("Downloads — Canopa Files")
    keys("Right")
    wait_title("Thumbnail output — Canopa Files")
    keys("space")
    preview = wait_title("Quick Look: C-photo-thumb.png — Canopa Files")
    assert preview["is_floating"] and preview["layout"]["window_size"][0] >= 600
    image_visible("downloads-08-thumbnail-preview.png")
    assert title("A-project — Canopa Files")["id"] == target["id"]
    assert (destination / "notes.rs").read_bytes() == destination_note
    for log in session.output.glob("files*.log"):
        assert not any(marker in log.read_text() for marker in ("panicked", "CRITICAL")), log
    check("Left/Right retain parent navigation after the new output folder appears, its thumbnail opens in source-bound floating Quick Look, and the second browser retains its independent destination")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "invocation": invocation, "windows": windows(), "scope": "full Downloads workflow through Iced; remaining product, accessibility, device and release gates stay open"}


def inventory(root):
    return {os.fsencode(p.relative_to(root)).hex():
            {"kind": "directory", "inode": p.stat().st_ino} if p.is_dir() else
            {"kind": "file", "inode": p.stat().st_ino, "bytes": p.stat().st_size,
             "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
            for p in root.rglob("*")}


def verify(session, fixture, browser, config, scheme, scale):
    try:
        return workflow(session, fixture, browser, config, scheme, scale)
    finally:
        (session.output / "downloads-final-files.json").write_text(json.dumps(inventory(fixture), indent=2) + "\n")
