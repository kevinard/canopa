"""Actual Trash/Restore in a private Iced desktop; never select host Trash."""
import json
import os
from pathlib import Path
import subprocess
import time
from urllib.parse import quote_from_bytes, unquote_to_bytes


def setup(session, fixture, config):
    root = fixture / "Trash fixtures"
    assert root.is_relative_to(session.runtime / "home")
    root.mkdir()
    (root / "A-trash.txt").write_bytes(b"Trash and restore original bytes\n")
    with open(os.fsencode(root) + b"/B-raw-\xff.rs", "wb") as stream:
        stream.write(b"fn main() { /* original raw name */ }\n")
    (root / "Elsewhere").mkdir()
    (root / "Copies").mkdir()
    os.mkdir(os.fsencode(root) + b"/else-\xfe")
    config.write_text(config.read_text() + 'files { shortcut "show-trash" "<Alt>t"; }\n')


def verify(session, fixture, browser, scheme, scale, process):
    root = fixture / "Trash fixtures"
    source = root / "A-trash.txt"
    data = Path(session.env["XDG_DATA_HOME"])
    assert data.resolve().is_relative_to(session.runtime.resolve()), "Trash data must be private"
    files = data / "Trash/files"
    info = data / "Trash/info"
    checks = []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        try:
            session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "native window focus")
        except RuntimeError:
            print(json.dumps({"focus_requested": window["id"], "windows": windows()}), flush=True)
            raise
        time.sleep(.15)

    def key(name):
        session.run(["wtype", "-k", name])
        time.sleep(.2)

    def chord(name, shift=False, alt=False):
        modifier = "alt" if alt else "ctrl"
        args = ["wtype", "-M", modifier]
        if shift:
            args += ["-M", "shift"]
        args += ["-k", name]
        if shift:
            args += ["-m", "shift"]
        session.run(args + ["-m", modifier])
        time.sleep(.25)

    def navigate(path):
        focus(browser)
        location = "file://" + quote_from_bytes(os.fsencode(path), safe="/")
        session.run(["wtype", "-M", "ctrl", "-k", "l", "-m", "ctrl", location, "-k", "Return"])
        time.sleep(.5)

    def stored():
        entries = list(files.iterdir()) if files.exists() else []
        return entries[0] if len(entries) == 1 else None

    def metadata(payload):
        return info / (payload.name + ".trashinfo")

    def copied_text(mime=None):
        try:
            return session.run(["wl-paste", "--no-newline"] + (["--type", mime] if mime else []))
        except subprocess.CalledProcessError:
            return None

    def destination_closed(dialog):
        # The title can change before the native window is actually retired.
        session.wait_for(lambda: all(w["id"] != dialog["id"] for w in windows()), "accepted native destination dialog retires")
        session.wait_for(lambda: any(w["id"] == trash["id"] and w["is_focused"] for w in windows()), "destination returns focus to its Trash owner")
        time.sleep(.15)

    navigate(source)
    key("Delete")
    session.wait_for(lambda: not source.exists() and stored(), "private stored payload after Delete")
    payload = stored()
    assert payload.read_bytes() == b"Trash and restore original bytes\n"
    original = next(line[5:] for line in metadata(payload).read_bytes().splitlines() if line.startswith(b"Path="))
    assert unquote_to_bytes(original) == os.fsencode(source)
    chord("z")
    session.wait_for(lambda: source.exists() and not payload.exists(), "actual Undo returns the trashed file")
    assert source.read_bytes() == b"Trash and restore original bytes\n"
    checks.append("Delete preserves the private payload, byte-exact original metadata and shared Undo")

    navigate(source)
    key("Delete")
    session.wait_for(lambda: not source.exists() and stored(), "second recoverable Trash operation")
    payload = stored()
    chord("t", alt=True)
    trash = session.wait_for(lambda: title("Trash"), "actual Iced Trash window")
    focus(trash)
    chord("1")  # Main Trash is required before every native restore.
    time.sleep(.5)
    key("Home")
    key("space")
    preview = session.wait_for(lambda: title("Quick Look: A-trash.txt"), "Trash source-bound floating preview")
    assert preview["is_floating"]
    outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
    logical = next(item["logical"] for item in outputs.values() if item.get("logical"))
    width, height = preview["layout"]["window_size"]
    x, y = max(0, (logical["width"] - width) // 2), max(0, (logical["height"] - height) // 2)
    session.run(["niri", "msg", "action", "move-floating-window", "--id", str(preview["id"]), "-x", str(x), "-y", str(y)])
    pointer = session.build_pointer()

    def preview_text(expected):
        focus(preview)
        session.run([str(pointer), str(round((x + 60) / logical["width"] * 1600)), str(round((y + 160) / logical["height"] * 1000))])
        chord("a")
        chord("c")
        return copied_text() == expected

    session.wait_for(lambda: preview_text("Trash and restore original bytes\n"), "actual stored preview text")
    payload.write_bytes(b"Updated retained preview bytes\n")
    session.wait_for(lambda: preview_text("Updated retained preview bytes\n"), "Trash preview refreshes changed payload contents")
    session.capture("trash-preview-current-content.png")
    navigate(fixture / "B-notes.txt")
    assert title("Quick Look: A-trash.txt"), "another Browser stole Trash preview ownership"
    focus(preview)
    key("Escape")
    focus(trash)
    chord("r")
    session.wait_for(lambda: source.exists() and not payload.exists(), "Restore to the original raw location")
    assert source.read_bytes() == b"Updated retained preview bytes\n"
    time.sleep(.4)
    chord("z")
    session.wait_for(lambda: not source.exists() and stored(), "Undo Restore returns the payload to Trash")
    checks.append("Trash preview reads current stored contents, keeps its source and Restore shares Undo")

    payload = stored()
    source.write_bytes(b"Preserve newer original occupant\n")
    notifications = session.output / "notifications.json"
    before = len(json.loads(notifications.read_text())) if notifications.exists() else 0
    key("Home")
    chord("r")
    refusal = session.wait_for(lambda: next((event for event in json.loads(notifications.read_text())[before:]
        if event["title"] == "Restore A-trash.txt" and "exist" in event["body"].lower()), None)
        if notifications.exists() else None, "this Restore's occupied-destination refusal")
    assert refusal["app"] == "Canopa Files"
    assert source.read_bytes() == b"Preserve newer original occupant\n"
    assert payload.read_bytes() == b"Updated retained preview bytes\n"
    session.capture("trash-occupied-refusal.png")
    chord("r", shift=True)
    dialog = session.wait_for(lambda: title("Restore to folder"), "captured Restore destination dialog")
    assert dialog["is_floating"]
    focus(dialog)
    session.run(["wtype", "-d", "5", str(root)])
    key("Tab")
    key("Return")
    chooser = session.wait_for(lambda: title("Choose a folder"), "Restore folder browser")
    assert chooser["id"] == dialog["id"]
    focus(chooser)
    # Use this folder is focused on open, followed by Home, Up and the first subfolder.
    for _ in range(3):
        key("Tab")
    key("Return")
    key("Return")
    session.wait_for(lambda: title("Restore to folder"), "folder browser returns to the draft")
    target = root / "Copies/A-trash.txt"
    assert not target.exists() and payload.exists(), "choosing a folder must never restore the item"
    key("Tab")
    key("Tab")
    key("Return")
    session.wait_for(lambda: target.exists() and not payload.exists(), "chosen Restore destination")
    destination_closed(dialog)
    assert target.read_bytes() == b"Updated retained preview bytes\n"
    assert source.read_bytes() == b"Preserve newer original occupant\n"
    checks.append("the folder browser supports keyboard navigation and stages a destination before an explicit Restore")

    raw = os.fsencode(root) + b"/B-raw-\xff.rs"
    navigate(raw)
    key("Delete")
    session.wait_for(lambda: not os.path.exists(raw) and stored(), "raw filename is stored")
    payload = stored()
    focus(trash)
    key("Home")
    chord("r", shift=True)
    dialog = session.wait_for(lambda: title("Restore to folder"), "raw selected item destination")
    focus(dialog)
    raw_folder = os.fsencode(root) + b"/else-\xfe"
    session.run(["wtype", "file://" + quote_from_bytes(raw_folder, safe="/"), "-k", "Return"])
    raw_target = raw_folder + b"/B-raw-\xff.rs"
    session.wait_for(lambda: os.path.exists(raw_target) and not payload.exists(), "raw original basename at raw destination")
    destination_closed(dialog)
    with open(raw_target, "rb") as stream:
        assert stream.read() == b"fn main() { /* original raw name */ }\n"
    time.sleep(.4)
    focus(trash)
    chord("z")
    session.wait_for(lambda: not os.path.exists(raw_target) and stored(), "Undo raw Restore")
    checks.append("raw filename bytes and a raw folder URI survive Restore-to and Undo")

    payload = stored()
    metadata(payload).unlink()
    time.sleep(.5)
    key("Home")
    chord("c")
    session.wait_for(lambda: copied_text("text/uri-list") is not None and payload.as_uri() in copied_text("text/uri-list"), "orphan publishes its actual stored URI")
    navigate(root / "Copies")
    chord("v")
    recovered = root / "Copies" / payload.name
    session.wait_for(recovered.exists, "Copy recovers orphan contents")
    assert recovered.read_bytes() == payload.read_bytes() == b"fn main() { /* original raw name */ }\n"
    checks.append("missing metadata leaves the payload copyable through the native file clipboard")

    incoming = []
    for number in range(120):
        path = root / f"C-many-{number:03}.txt"
        path.write_text(f"Catalog item {number}\n")
        incoming.append(str(path))
    last = root / "zz-last.txt"
    last.write_text("Last visible item\n")
    session.run(["gio", "trash", *incoming, str(last)])
    focus(trash)
    time.sleep(.6)
    session.run(["niri", "msg", "action", "set-window-width", "340"])
    time.sleep(.4)
    key("End")
    key("space")
    floating = session.wait_for(lambda: title("Quick Look: zz-last.txt"), "external GIO items refresh into the bounded Trash list")
    assert floating["is_floating"]
    assert title("Trash")["layout"]["window_size"][0] == 340
    session.capture("trash-narrow-last-item.png")
    focus(trash)
    chord("w")
    session.wait_for(lambda: title("Trash") is None and title("Quick Look: zz-last.txt") is None, "closing Trash closes its own preview")
    assert process.poll() is None
    checks.append("external GIO changes refresh, End reaches a long list in a narrow tile, and Trash closes its preview")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "Iced Main Trash, Restore-to, Undo, orphan Copy and source-bound previews; Recovery and broader migration remain open"}
