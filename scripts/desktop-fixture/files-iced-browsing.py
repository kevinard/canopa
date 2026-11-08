"""Narrow/wide native acceptance for the first Iced browsing migration slice.

This deliberately does not claim preview, transfer or other feature parity.
"""
import json
import ctypes
import os
import struct
import time
from urllib.parse import quote_from_bytes


def verify(session, fixture, browser, launched, scheme, scale):
    wide_width = browser["layout"]["window_size"][0]
    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((item for item in windows() if item["title"] == f"{name} — Canopa Files"), None)

    def wait_title(name):
        result = session.wait_for(lambda: title(name), f"Iced folder {name}")
        time.sleep(.4)
        return result

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "Iced browser focus")
        time.sleep(.2)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name])
            time.sleep(.25)

    def chord(key, shift=False):
        command = ["wtype", "-M", "ctrl"]
        if shift:
            command += ["-M", "shift"]
        command += ["-k", key]
        if shift:
            command += ["-m", "shift"]
        session.run(command + ["-m", "ctrl"])
        time.sleep(.3)

    def navigate(location, name=None):
        chord("l")
        session.run(["wtype", str(location)])
        keys("Return")
        return wait_title(name or location.name)

    def expect_open(path):
        launched.unlink(missing_ok=True)
        keys("Return")
        def recorded():
            try:
                return [os.fsencode(item) for item in json.loads(launched.read_text())] == [os.fsencode(path)]
            except (FileNotFoundError, json.JSONDecodeError):
                return False
        session.wait_for(recorded, f"actual GIO launch of {path}")

    pointer = session.build_pointer()
    session.run([str(pointer), "1500", "900", "move"])
    focus(browser)
    session.capture("iced-01-wide.png")
    chord("l")
    session.run(["wtype", "/uncommitted-location"])
    keys("Escape")
    keys("End")
    expect_open(fixture / "document-69.txt")
    # Wait for the actual target to open before taking the reference frame.
    # The key events can reach Files before the compositor paints the selection.
    end_frame = session.capture("iced-02-end-revealed.png")
    session.run(["niri", "msg", "action", "set-window-width", "360"])
    session.wait_for(lambda: (window := title("Downloads")) and window["layout"]["window_size"][0] == 360,
                     "narrow Browser geometry")
    time.sleep(.25)
    narrow_frame = session.capture("iced-03-narrow-keeps-selection.png")
    # The selected final row must remain painted at the same vertical offset.
    # Checking its fill away from text avoids accepting an offscreen selection.
    expected = end_frame.getpixel((end_frame.width - round(70 * scale), end_frame.height - round(50 * scale)))
    actual = narrow_frame.getpixel((round((360 - 70) * scale), narrow_frame.height - round(50 * scale)))
    assert max(abs(a - b) for a, b in zip(expected, actual)) <= 3, (expected, actual)
    expect_open(fixture / "document-69.txt")

    root = fixture / "Hidden navigation"
    active = root / ".active"
    deep = active / "deep"
    deep.mkdir(parents=True)
    (root / ".other").mkdir()
    (root / "visible").mkdir()
    (deep / "notes.txt").write_text("Iced selection remains attached to this tab.\n")
    navigate(deep)
    session.run(["wtype", "notes"])
    time.sleep(.4)
    expect_open(deep / "notes.txt")
    keys("Left")
    wait_title(".active")
    keys("Left")
    wait_title("Hidden navigation")
    session.capture("iced-04-hidden-parent.png")
    session.run(["wtype", ".other"])
    keys("Right")
    wait_title(".active")
    keys("Right")
    wait_title("deep")
    expect_open(deep / "notes.txt")
    session.run(["niri", "msg", "action", "set-window-width", str(wide_width)])
    time.sleep(.7)
    session.capture("iced-05-wide-restored.png")
    expect_open(deep / "notes.txt")

    chord("t")
    navigate(fixture)
    session.run(["wtype", "A-project"])
    keys("Right")
    wait_title("A-project")
    chord("Tab", shift=True)
    wait_title("deep")
    expect_open(deep / "notes.txt")
    chord("Tab")
    wait_title("A-project")
    chord("w")
    wait_title("deep")
    expect_open(deep / "notes.txt")

    # A raw Unix name must survive the location editor and actual desktop launch.
    raw = os.fsencode(fixture) + b"/raw-\xff #%.txt"
    with open(raw, "wb") as stream:
        stream.write(b"Raw filename fixture.\n")
    uri = "file://" + quote_from_bytes(raw, safe="/")
    navigate(uri, "Downloads")
    expect_open(raw)
    raw_frame = session.capture("iced-06-raw-file-uri.png")
    raw_fill = raw_frame.getpixel((raw_frame.width - round(70 * scale), raw_frame.height - round(50 * scale)))
    assert max(abs(a - b) for a, b in zip(expected, raw_fill)) <= 3, (expected, raw_fill)

    chord("n")
    second = session.wait_for(lambda: next((item for item in windows() if item["id"] != browser["id"]), None), "second Iced browser")
    focus(second)
    navigate(fixture / "A-project")
    focus(browser)
    expect_open(raw)
    outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
    logical_width = next(item["logical"]["width"] for item in outputs.values() if item.get("logical"))
    tile_width = logical_width // 2
    session.run(["niri", "msg", "action", "set-window-width", str(tile_width)])
    focus(second)
    session.run(["niri", "msg", "action", "set-window-width", str(tile_width)])
    focus(browser)
    session.run(["niri", "msg", "action", "center-visible-columns"])
    time.sleep(.7)
    tiles = session.capture("iced-07-two-tiles.png")
    desktop_color = narrow_frame.getpixel((narrow_frame.width - 10, narrow_frame.height // 2))
    for numerator in [1, 3]:
        painted = tiles.getpixel((tiles.width * numerator // 4, tiles.height // 2))
        assert max(abs(a - b) for a, b in zip(desktop_color, painted)) >= 10, (desktop_color, painted)
    assert len(windows()) == 2
    assert all(item["layout"]["window_size"][0] == tile_width for item in windows())
    assert all(item["app_id"] == "org.canopa.Files" and not item["is_floating"] for item in windows())
    # Observe actual directory opens: narrow browsing must not enumerate hidden
    # ancestor columns. A subsequent wide view proves this watch is effective.
    focus(second)
    chord("w")
    focus(browser)
    large = fixture / "Large parent"
    child = large / "child"
    child.mkdir(parents=True)
    for index in range(5000):
        (large / f"sibling-{index:05d}.txt").touch()
    (child / "current.txt").write_text("Current folder remains usable.\n")
    libc = ctypes.CDLL(None, use_errno=True)
    libc.inotify_add_watch.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_uint32]
    watch = libc.inotify_init1(os.O_NONBLOCK | os.O_CLOEXEC)
    assert watch >= 0, os.strerror(ctypes.get_errno())
    try:
        assert libc.inotify_add_watch(watch, os.fsencode(large), 0x20) >= 0
        def parent_opened():
            found = False
            while True:
                try:
                    data = os.read(watch, 65536)
                except BlockingIOError:
                    return found
                offset = 0
                while offset < len(data):
                    _, mask, _, length = struct.unpack_from("iIII", data, offset)
                    found |= bool(mask & 0x20 and length == 0)
                    offset += 16 + length
        session.run(["niri", "msg", "action", "set-window-width", "360"])
        time.sleep(.4)
        navigate(child)
        session.run(["wtype", "current"])
        time.sleep(.3)
        expect_open(child / "current.txt")
        assert not parent_opened(), "narrow browser enumerated a hidden ancestor column"
        started = time.monotonic()
        session.run(["niri", "msg", "action", "set-window-width", str(wide_width)])
        session.wait_for(parent_opened, "visible large ancestor snapshot")
        expect_open(child / "current.txt")
        loading_seconds = round(time.monotonic() - started, 3)
        session.capture("iced-08-large-ancestor.png")
    finally:
        os.close(watch)
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL"]), log
    return {"status": "passed", "scope": "Iced browsing migration only; full feature parity remains required", "scheme": scheme, "scale": scale, "large_ancestor": {"sibling_files": 5000, "hidden_parent_opens": 0, "visible_parent_open_observed": True, "launch_after_widen_seconds": loading_seconds}, "checks": [
        "ordinary Iced windows use org.canopa.Files",
        "Escape dismisses location editing and restores browsing keyboard input",
        "End reveals the actual selected file and GIO opens it",
        "360px tile keeps location, selection, visible scroll position and launch target",
        "Left reaches hidden parents and Right returns through the child just left",
        "hidden siblings cannot replace the explicit navigation selection",
        "widening preserves selected file and surrounding hierarchy",
        "tabs navigate independently and restore their selected launch target",
        "closing one tab restores the remaining tab",
        "raw Unix filename survives percent-encoded location and actual GIO argv",
        "two independent browser windows use half the available output width",
        "narrow browsing does not open the hidden ancestor containing 5000 files",
        "widening loads that ancestor while the current selected file remains usable",
    ], "windows": windows()}
