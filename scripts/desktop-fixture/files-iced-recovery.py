"""Recovery copies through the native frontend, using private retained bytes."""
import json
import os
from pathlib import Path
import subprocess
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    root = fixture / "Recovery fixtures"
    assert root.is_relative_to(session.runtime / "home")
    for name in ("Source", "Destination", "Recovered", "Raw source", "Raw destination"):
        (root / name).mkdir(parents=True)
    (root / "Source/report.txt").write_bytes(b"Incoming current bytes\n")
    (root / "Destination/report.txt").write_bytes(b"Retained previous bytes\n")
    for folder, content in ((b"Raw source", b"Incoming raw bytes\n"), (b"Raw destination", b"Retained raw bytes\n")):
        with open(os.fsencode(root) + b"/" + folder + b"/raw-\xff.rs", "wb") as stream:
            stream.write(content)
    os.mkdir(os.fsencode(root) + b"/Recovered/raw-folder-\xfe")
    config.write_text(config.read_text() + 'files { shortcut "recovery" "<Alt>r"; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Recovery fixtures"
    checks = []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "native Recovery focus")
        time.sleep(.15)

    def key(name):
        session.run(["wtype", "-k", name])
        time.sleep(.2)

    def chord(name, modifier="ctrl"):
        session.run(["wtype", "-M", modifier, "-k", name, "-m", modifier])
        time.sleep(.25)

    def uri(path):
        return "file://" + quote_from_bytes(os.fsencode(path), safe="/")

    def type_text(value, submit=False):
        # Keep every driver invocation within Session.run's bounded deadline,
        # including long percent-encoded URIs under software rendering.
        for start in range(0, len(value), 40):
            session.run(["wtype", value[start:start + 40]])
        if submit:
            key("Return")

    def navigate(path):
        focus(browser)
        chord("l")
        type_text(uri(path), submit=True)
        time.sleep(.5)

    def replace(source, destination, expected):
        navigate(source)
        chord("c")
        navigate(destination)
        chord("v")
        question = session.wait_for(lambda: title("File conflict"), "actual replacement question")
        focus(question)
        key("Tab")  # explicit Replace after the shared Cancel default
        key("Return")
        target = os.fsencode(destination) + b"/" + os.path.basename(os.fsencode(source))
        session.wait_for(lambda: Path(os.fsdecode(target)).read_bytes() == expected and title("File conflict") is None, "actual replacement commit")
        time.sleep(.4)

    def payload(content):
        return next((p for p in root.rglob("*") if p.is_file() and p.read_bytes() == content), None)

    def draft():
        focus(recovery)
        key("Return")
        dialog = session.wait_for(lambda: title("Recover a copy"), "captured Recovery destination draft")
        assert dialog["is_floating"]
        focus(dialog)
        return dialog

    def retired(dialog):
        session.wait_for(lambda: all(w["id"] != dialog["id"] for w in windows()), "native Recovery destination retires")
        session.wait_for(lambda: any(w["id"] == recovery["id"] and w["is_focused"] for w in windows()), "Recovery destination returns its own parent")
        time.sleep(.2)

    def selected_preview(name):
        focus(recovery)
        key("Home")
        key("space")
        preview = session.wait_for(lambda: next((w for w in windows() if w.get("title", "").startswith("Quick Look:")), None), "source-bound Recovery preview")
        if preview["title"] != f"Quick Look: {name} — Canopa Files":
            focus(recovery)
            key("End")
            preview = session.wait_for(lambda: title(f"Quick Look: {name}"), "selected retained filename")
        return preview

    replace(root / "Source/report.txt", root / "Destination", b"Incoming current bytes\n")
    stored = session.wait_for(lambda: payload(b"Retained previous bytes\n"), "owned saved previous version")
    chord("r", "alt")
    recovery = session.wait_for(lambda: title("Recovery"), "actual Iced Recovery window")
    preview = selected_preview("report.txt")
    assert preview["is_floating"]
    outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
    logical = next(o["logical"] for o in outputs.values() if o.get("logical"))
    width, height = preview["layout"]["window_size"]
    x, y = max(0, (logical["width"] - width) // 2), max(0, (logical["height"] - height) // 2)
    session.run(["niri", "msg", "action", "move-floating-window", "--id", str(preview["id"]), "-x", str(x), "-y", str(y)])
    pointer = session.build_pointer()

    def preview_text(expected):
        focus(preview)
        session.run([str(pointer), str(round((x + 60) / logical["width"] * 1600)), str(round((y + 160) / logical["height"] * 1000))])
        chord("a")
        chord("c")
        try:
            return session.run(["wl-paste", "--no-newline"]) == expected
        except subprocess.CalledProcessError:
            return False

    session.wait_for(lambda: preview_text("Retained previous bytes\n"), "preview reads the retained version")
    stored.write_bytes(b"Edited retained version\n")
    session.wait_for(lambda: preview_text("Edited retained version\n"), "metadata observation refreshes the actual preview")
    session.capture("recovery-current-preview.png")
    navigate(fixture / "B-notes.txt")
    assert title("Quick Look: report.txt"), "another Browser stole Recovery preview ownership"
    focus(preview)
    key("Escape")
    dialog = draft()
    type_text(str(root / "Recovered"), submit=True)
    target = root / "Recovered/report.txt"
    session.wait_for(target.exists, "Recovery copy at selected destination")
    retired(dialog)
    assert target.read_bytes() == stored.read_bytes() == b"Edited retained version\n"
    assert (root / "Destination/report.txt").read_bytes() == b"Incoming current bytes\n"
    chord("z")
    session.wait_for(lambda: not target.exists(), "Recovery copy has actual shared Undo")
    checks.append("source-bound floating preview observes saved edits; Recover keeps the retained and current versions and shares Undo")

    target.write_bytes(b"Preserve occupied output\n")
    notices = session.output / "notifications.json"
    before = len(json.loads(notices.read_text())) if notices.exists() else 0
    dialog = draft()
    type_text(str(root / "Recovered"), submit=True)
    session.wait_for(lambda: any(e["app"] == "Canopa Files" and "exist" in e["body"].lower() for e in json.loads(notices.read_text())[before:]) if notices.exists() else False, "this Recovery request refuses an occupied root")
    assert title("Recover a copy")["id"] == dialog["id"], "refusal discarded the destination draft"
    assert target.read_bytes() == b"Preserve occupied output\n"
    assert stored.read_bytes() == b"Edited retained version\n"
    session.capture("recovery-destination-refused.png")
    focus(dialog)
    key("Tab")
    type_text("renamed.txt", submit=True)
    renamed = root / "Recovered/renamed.txt"
    session.wait_for(renamed.exists, "the preserved draft accepts another filename in its chosen folder")
    retired(dialog)
    assert renamed.read_bytes() == b"Edited retained version\n"
    chord("z")
    session.wait_for(lambda: not renamed.exists(), "Undo renamed Recovery copy")
    checks.append("occupied output stays intact, refusal keeps the same draft and chosen folder, and a new name succeeds")

    raw = os.fsencode(root) + b"/Raw source/raw-\xff.rs"
    replace(raw, root / "Raw destination", b"Incoming raw bytes\n")
    preview = selected_preview("raw-\ufffd.rs")
    focus(preview)
    key("Escape")
    dialog = draft()
    raw_folder = os.fsencode(root) + b"/Recovered/raw-folder-\xfe"
    type_text(uri(raw_folder), submit=True)
    raw_target = raw_folder + b"/raw-\xff.rs"
    session.wait_for(lambda: os.path.exists(raw_target), "original raw filename at raw folder URI")
    retired(dialog)
    assert Path(os.fsdecode(raw_target)).read_bytes() == b"Retained raw bytes\n"
    chord("z")
    session.wait_for(lambda: not os.path.exists(raw_target), "Undo raw Recovery copy")
    dialog = draft()
    key("Tab")
    full_target = raw_folder + b"/explicit-\xfd.rs"
    type_text(uri(full_target), submit=True)
    session.wait_for(lambda: os.path.exists(full_target), "explicit full raw destination URI")
    retired(dialog)
    assert Path(os.fsdecode(full_target)).read_bytes() == b"Retained raw bytes\n"
    chord("z")
    session.wait_for(lambda: not os.path.exists(full_target), "Undo full URI Recovery copy")
    checks.append("original raw filename, raw folder URI and full raw destination URI preserve bytes through Recovery and Undo")

    focus(recovery)
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    key("End")
    key("space")
    floating = session.wait_for(lambda: next((w for w in windows() if w.get("title", "").startswith("Quick Look:")), None), "narrow Recovery preview")
    assert floating["is_floating"]
    session.capture("recovery-narrow-floating-preview.png")
    focus(recovery)
    chord("w")
    session.wait_for(lambda: title("Recovery") is None and all(w["id"] != floating["id"] for w in windows()), "closing Recovery retires its own preview")
    assert process.poll() is None
    checks.append("Recovery adapts to a narrow tile and closes its source-owned floating preview")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "Iced retained-file Recovery, previews, destination refusal, raw destinations and Undo; directory continuation, reconciliation and full product acceptance remain open"}
