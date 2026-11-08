"""Inspect actual partial transfers through Iced Operations in a private desktop."""
import json
import os
import re
import stat
import time
from urllib.parse import quote_from_bytes
from PIL import ImageChops


def setup(session, fixture, config):
    root = fixture / "Transfer fixtures"
    assert root.is_relative_to(session.runtime / "home")
    source = root / "Source"
    destination = root / "Destination"
    source.mkdir(parents=True)
    destination.mkdir()
    (source / "a-completed.txt").write_bytes(b"Confirmed transfer bytes\n")
    os.mkfifo(source / "b-failed")
    (source / "c-conflict.txt").write_bytes(b"Incoming conflict bytes\n")
    (source / "d-remaining.txt").write_bytes(b"Unstarted transfer bytes\n")
    (destination / "c-conflict.txt").write_bytes(b"Existing destination bytes\n")
    large = root / "Large source"
    large.mkdir()
    (root / "Large destination").mkdir()
    for index in range(1, 26):
        (large / f"item-{index:02}.txt").write_bytes(b"Paging bytes\n")
    config.write_text(config.read_text() + 'files { shortcut "operations" "<Alt>o"; }\n')


def verify(session, fixture, browser, scheme, scale, process, *, focus_only=False, delay_focus=False, parent_closure=False, compact_only=False):
    root = fixture / "Transfer fixtures"
    source = root / "Source"
    destination = root / "Destination"

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

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
        uri = "file://" + quote_from_bytes(os.fsencode(path), safe="/")
        session.run(["wtype", "-d", "5", uri])
        chord("Return")
        session.wait_for(lambda: title(path.name), "transfer folder navigation")
        time.sleep(.3)

    def compact_inspector(inspector):
        session.wait_for(lambda: title("Transfer details")["is_focused"], "inspector focus before compact resize")
        chord("Tab")
        chord("Tab")
        session.run(["niri", "msg", "action", "set-window-width", "320"])
        session.run(["niri", "msg", "action", "set-window-height", "240"])
        compact = session.wait_for(lambda: next((w for w in windows() if w["id"] == inspector
                                  and w["layout"]["window_size"][0] <= 330
                                  and w["layout"]["window_size"][1] <= 300), None), "minimum inspector configuration")
        layout = compact["layout"]
        cx, cy = (sum(values) for values in zip(layout["tile_pos_in_workspace_view"], layout["window_offset_in_tile"]))
        cw, ch = layout["window_size"]
        box = tuple(round(value * scale) for value in (cx + 8, cy + 50, cx + cw - 8, cy + ch - 8))
        chord("Return")
        assert title("Transfer details") and title("Transfer details")["id"] == inspector, "compact resize changed section activation into Close"
        chord("Home")
        top = session.capture("transfer-minimum-start.png").crop(box)
        chord("End")
        def scrolled():
            current = title("Transfer details")
            assert current and current["id"] == inspector
            frame = session.capture("transfer-minimum-end.png").crop(box)
            return ImageChops.difference(top, frame).getbbox()
        session.wait_for(scrolled, "minimum inspector exposes scrollable recorded facts", timeout=3)
        # Failed retains focus. Skipped, Remaining and Sources keep their order.
        for _ in range(3):
            chord("Tab")
        session.capture("transfer-minimum-source-control.png")
        chord("Return")
        assert title("Transfer details")["id"] == inspector
        session.capture("transfer-minimum-source-section.png")
        session.run(["niri", "msg", "action", "set-window-height", "560"])
        session.wait_for(lambda: title("Transfer details")["layout"]["window_size"][1] >= 550, "wide-height inspector configuration")
        chord("Return")
        assert title("Transfer details") and title("Transfer details")["id"] == inspector, "normal-height resize changed section activation into Close"
        session.capture("transfer-wide-source-control.png")
        session.run(["niri", "msg", "action", "set-window-height", "240"])
        session.wait_for(lambda: title("Transfer details")["layout"]["window_size"][1] <= 300, "restored compact inspector configuration")
        chord("Return")
        assert title("Transfer details") and title("Transfer details")["id"] == inspector, "return to compact height changed section activation into Close"
        session.capture("transfer-minimum-restored-control.png")
        chord("Tab")
        chord("Return")
        session.wait_for(lambda: all(w["id"] != inspector for w in windows()), "minimum inspector Close remains keyboard-accessible")

    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "transfer Browser focus")
    navigate(source)
    chord("a", "ctrl")
    chord("c", "ctrl")
    navigate(destination)
    chord("v", "ctrl")
    session.wait_for(lambda: title("File conflict"), "partial transfer conflict")
    assert (destination / "a-completed.txt").read_bytes() == b"Confirmed transfer bytes\n"
    assert not (destination / "b-failed").exists()
    assert not (destination / "d-remaining.txt").exists()
    session.run(["niri", "msg", "action", "close-window", "--id", str(browser["id"])])
    operations = session.wait_for(lambda: title("Operations"), "background transfer Operations")
    session.wait_for(lambda: title("File conflict") is None, "source closure defers its question")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(operations["id"])])
    session.run(["niri", "msg", "action", "set-window-width", "360"])
    time.sleep(.3)
    session.capture("transfer-progress-waiting.png")
    chord("d", "alt")
    details = session.wait_for(lambda: title("Transfer details"), "Iced transfer progress inspector")
    assert details["is_floating"]
    session.capture("transfer-live-details.png")
    assert process.poll() is None
    if parent_closure:
        session.run(["niri", "msg", "action", "focus-window", "--id", str(operations["id"])])
        session.wait_for(lambda: title("Operations")["is_focused"], "Operations focus for new Browser")
        chord("n", "ctrl")
        other = session.wait_for(lambda: next((w for w in windows() if w.get("app_id") == "org.canopa.Files"
                                             and w["id"] not in [operations["id"], details["id"]]), None), "independent Browser for parent closure")
        session.wait_for(lambda: any(w["id"] == other["id"] and w["is_focused"] for w in windows()), "independent Browser focus")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(operations["id"])])
        session.wait_for(lambda: title("Operations")["is_focused"], "Operations focus before Review")
        # New browser, Recovery, Details, Cancel, Review.
        for _ in range(5):
            chord("Tab")
        chord("Return")
        question = session.wait_for(lambda: title("File conflict"), "Operations-owned pending question")
        session.wait_for(lambda: title("File conflict")["is_focused"], "pending question focus")
        assert title("Transfer details")["id"] == details["id"]
        session.capture("transfer-parent-before-close.png")
        session.run(["niri", "msg", "action", "close-window", "--id", str(operations["id"])])
        session.wait_for(lambda: all(w["id"] != operations["id"] for w in windows()), "Operations parent closes")
        session.wait_for(lambda: all(w["id"] not in [question["id"], details["id"]] for w in windows()),
                         "closing Operations retires both of its auxiliary windows", timeout=3)
        session.capture("transfer-parent-after-close.png")
        notices = session.output / "notifications.json"
        assert not any(e["title"] == "Copy 4 items to Destination" for e in json.loads(notices.read_text())), "parent closure answered the waiting transfer"
        assert (destination / "a-completed.txt").read_bytes() == b"Confirmed transfer bytes\n"
        assert (destination / "c-conflict.txt").read_bytes() == b"Existing destination bytes\n"
        assert not (destination / "d-remaining.txt").exists()
        assert process.poll() is None
        session.run(["niri", "msg", "action", "focus-window", "--id", str(other["id"])])
        session.wait_for(lambda: any(w["id"] == other["id"] and w["is_focused"] for w in windows()), "surviving Browser focus")
        chord("o", "alt")
        reopened = session.wait_for(lambda: title("Operations"), "reopened shared Operations")
        assert reopened["id"] != operations["id"]
        session.wait_for(lambda: title("Operations")["is_focused"], "reopened Operations focus")
        chord("d", "alt")
        inspector = session.wait_for(lambda: title("Transfer details"), "reopened waiting transfer details")
        assert inspector["id"] != details["id"]
        session.wait_for(lambda: title("Transfer details")["is_focused"], "reopened inspector focus")
        session.capture("transfer-parent-reopened-details.png")
        chord("Escape")
        session.wait_for(lambda: title("Transfer details") is None, "close reopened details")
        session.wait_for(lambda: title("Operations")["is_focused"], "Operations focus before retained Review")
        for _ in range(5):
            chord("Tab")
        chord("Return")
        retained = session.wait_for(lambda: title("File conflict"), "same pending conflict remains reviewable")
        assert retained["id"] != question["id"]
        session.wait_for(lambda: title("File conflict")["is_focused"], "retained question focus")
        session.capture("transfer-parent-retained-question.png")
        chord("Return")
        session.wait_for(lambda: title("File conflict") is None, "explicit cancellation of remaining work")
        session.wait_for(lambda: any(e["title"] == "Copy 4 items to Destination" for e in json.loads(notices.read_text())), "explicit cancellation completion notice")
        session.wait_for(lambda: title("Operations")["is_focused"], "Operations focus before Undo")
        chord("z", "ctrl")
        session.wait_for(lambda: not (destination / "a-completed.txt").exists(), "Undo confirmed prefix after parent closure")
        assert stat.S_ISFIFO((source / "b-failed").lstat().st_mode)
        assert (source / "c-conflict.txt").read_bytes() == b"Incoming conflict bytes\n"
        assert (source / "d-remaining.txt").read_bytes() == b"Unstarted transfer bytes\n"
        assert (destination / "c-conflict.txt").read_bytes() == b"Existing destination bytes\n"
        assert not (destination / "d-remaining.txt").exists()
        return {"status": "passed", "scheme": scheme, "scale": scale,
                "checks": ["closing Operations retires its inspector and question while preserving the unanswered transfer",
                           "reopened Operations reviews the retained question and only explicit cancellation stops work before shared Undo"]}
    if focus_only:
        session.run(["niri", "msg", "action", "focus-window", "--id", str(operations["id"])])
        session.wait_for(lambda: any(w["id"] == operations["id"] and w["is_focused"] for w in windows()), "inspector parent focus")
        chord("n", "ctrl")
        other = session.wait_for(lambda: next((w for w in windows() if w.get("app_id") == "org.canopa.Files"
                                             and w["id"] not in [operations["id"], details["id"]]), None), "third window for focus return")
        session.wait_for(lambda: any(w["id"] == other["id"] and w["is_focused"] for w in windows()), "independent Browser focus")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(details["id"])])
        session.wait_for(lambda: any(w["id"] == details["id"] and w["is_focused"] for w in windows()), "inspector focus before Escape")
        if delay_focus:
            wire = (session.output / "files.log").read_text()
            top = re.findall(r'xdg_toplevel#(\d+)\.set_title\("Transfer details — Canopa Files"\)', wire)[-1]
            shell = re.findall(r'xdg_surface#(\d+)\.get_toplevel\(new id xdg_toplevel#' + top + r'\)', wire)[-1]
            surface = re.findall(r'get_xdg_surface\(new id xdg_surface#' + shell + r', wl_surface#(\d+)\)', wire)[-1]
            (session.runtime / "focus-delay-source").write_text(surface + "\n")
        session.capture("transfer-focus-before-close.png")
        chord("Escape")
        session.wait_for(lambda: all(w["id"] != details["id"] for w in windows()), "inspector native surface closes")
        if delay_focus:
            assert "[files-focus-expiry-fixture]" in (session.output / "files.log").read_text(), "native reply delay was not exercised"
            session.wait_for(lambda: any(w["id"] == other["id"] and w["is_focused"] for w in windows()),
                             "expired reply leaves the independent Browser focused", timeout=3)
        else:
            session.wait_for(lambda: any(w["id"] == operations["id"] and w["is_focused"] for w in windows()),
                             "inspector focus returns to its Operations parent", timeout=3)
        session.capture("transfer-focus-after-close.png")
        return {"status": "passed", "scheme": scheme, "scale": scale,
                "checks": ["an overdue native focus reply cannot activate Operations and releases its closing source"
                           if delay_focus else "closing a floating inspector restores its Operations parent after another Browser was focused"]}
    chord("Escape")
    session.wait_for(lambda: title("Transfer details") is None, "close only the live inspector")
    session.capture("transfer-review-pending.png")
    # New browser, Recovery, this transfer's Details, Cancel, then Review.
    # Disabled history and command paging buttons add no keyboard stops.
    for _ in range(5):
        chord("Tab")
    session.capture("transfer-review-control.png")
    chord("Return")
    question = session.wait_for(lambda: title("File conflict"), "review the retained transfer question")
    session.wait_for(lambda: any(w["id"] == question["id"] and w["is_focused"] for w in windows()), "reviewed conflict focus")
    chord("Return")
    session.wait_for(lambda: title("File conflict") is None, "cancel remaining transfer work")
    notices = session.output / "notifications.json"
    session.wait_for(lambda: notices.exists() and any(e["title"].startswith("Copy")
                     and "b-failed" in e["body"] and "only regular files" in e["body"]
                     for e in json.loads(notices.read_text())),
                     "actual stopped-result notification")
    assert (destination / "a-completed.txt").read_bytes() == b"Confirmed transfer bytes\n"
    assert (destination / "c-conflict.txt").read_bytes() == b"Existing destination bytes\n"
    assert not (destination / "d-remaining.txt").exists()
    chord("d", "alt")
    stopped = session.wait_for(lambda: title("Transfer details"), "inspect the stopped transfer")
    session.capture("transfer-failed-details.png")
    if compact_only:
        compact_inspector(stopped["id"])
        return {"status": "passed", "scheme": scheme, "scale": scale,
                "checks": ["stopped-result facts remain visible and keyboard-scrollable in the minimum inspector"]}
    # Close, completed destinations, failed, skipped, then remaining work.
    for _ in range(4):
        chord("Tab")
    chord("Return")
    assert title("Transfer details") is not None, "recorded-result sections are not keyboard-accessible"
    session.capture("transfer-remaining-details.png")
    chord("Escape")
    session.wait_for(lambda: title("Transfer details") is None, "close recorded details")
    chord("z", "ctrl")
    session.wait_for(lambda: not (destination / "a-completed.txt").exists(), "Undo only the confirmed copy")
    session.wait_for(lambda: any(e["body"].startswith("1 reversed") for e in json.loads(notices.read_text())), "partial-copy Undo completion")
    assert stat.S_ISFIFO((source / "b-failed").lstat().st_mode)
    assert (source / "a-completed.txt").read_bytes() == b"Confirmed transfer bytes\n"
    assert (source / "c-conflict.txt").read_bytes() == b"Incoming conflict bytes\n"
    assert (source / "d-remaining.txt").read_bytes() == b"Unstarted transfer bytes\n"
    assert (destination / "c-conflict.txt").read_bytes() == b"Existing destination bytes\n"
    chord("d", "alt")
    original_details = session.wait_for(lambda: title("Transfer details"), "review the original recorded result after Undo")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(operations["id"])])
    chord("n", "ctrl")
    other = session.wait_for(lambda: next((w for w in windows() if w.get("app_id") == "org.canopa.Files"
                                         and w["id"] not in [operations["id"], original_details["id"]]), None), "new Browser for a later transfer")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(other["id"])])
    navigate(root / "Large source")
    chord("a", "ctrl")
    chord("c", "ctrl")
    navigate(root / "Large destination")
    chord("v", "ctrl")
    session.wait_for(lambda: all((root / "Large destination" / f"item-{index:02}.txt").exists()
                                for index in range(1, 26)), "actual twenty-five-file copy")
    session.wait_for(lambda: any(e["title"].startswith("Copy 25 items") and "25 completed" in e["body"]
                                for e in json.loads(notices.read_text())), "large copy completion")
    assert all((root / "Large destination" / f"item-{index:02}.txt").read_bytes() == b"Paging bytes\n"
               for index in range(1, 26))
    assert title("Transfer details")["id"] == original_details["id"]
    session.run(["niri", "msg", "action", "focus-window", "--id", str(original_details["id"])])
    session.wait_for(lambda: any(w["id"] == original_details["id"] and w["is_focused"] for w in windows()), "original inspector focus")
    session.capture("transfer-original-result-binding.png")
    chord("Escape")
    session.wait_for(lambda: title("Transfer details") is None, "close the original result inspector")
    session.wait_for(lambda: any(w["id"] == operations["id"] and w["is_focused"] for w in windows()), "inspector returns focus to Operations")
    chord("d", "alt")
    newest = session.wait_for(lambda: title("Transfer details"), "inspect the newer completed transfer")
    session.wait_for(lambda: any(w["id"] == newest["id"] and w["is_focused"] for w in windows()), "completed inspector focus")
    # Compositor focus precedes the CSD title redraw. Compare only the report
    # viewport, excluding title bars, controls and other windows.
    layout = newest["layout"]
    position = layout["tile_pos_in_workspace_view"]
    offset = layout["window_offset_in_tile"]
    width, height = layout["window_size"]
    x, y = (position[i] + offset[i] for i in range(2))
    report_box = tuple(round(value * scale) for value in (x + 20, y + 80, x + width - 20, y + height - 230))

    def report_frame(name):
        return session.capture(name).convert("RGB").crop(report_box)

    def wait_report(name, reference, *, changed):
        def ready():
            current = title("Transfer details")
            assert current is not None and current["id"] == newest["id"], "report inspector closed or changed during keyboard navigation"
            frame = report_frame(name)
            different = ImageChops.difference(reference, frame).getbbox() is not None
            return frame if different == changed else None
        return session.wait_for(ready, f"rendered report response: {name}", timeout=3)

    top = report_frame("transfer-result-page-1.png")
    for forward, backward, label in [("Down", "Up", "line"), ("Next", "Prior", "viewport")]:
        chord(forward)
        wait_report(f"transfer-result-{label}-down.png", top, changed=True)
        chord(backward)
        wait_report(f"transfer-result-{label}-up.png", top, changed=False)
    chord("End")
    wait_report("transfer-result-page-1-end.png", top, changed=True)
    chord("Home")
    wait_report("transfer-result-page-1-home.png", top, changed=False)
    # Close, five result sections, then Next (Previous is disabled on page one).
    for _ in range(6):
        chord("Tab")
    chord("Return")
    assert title("Transfer details") is not None, "result paging is not keyboard-accessible"
    wait_report("transfer-result-page-2.png", top, changed=True)
    # At the end, focus moves to enabled Previous. On returning to the start it
    # moves to Next, so repeated Enter never disappears into a disabled control.
    chord("Return")
    wait_report("transfer-result-page-1-returned.png", top, changed=False)
    chord("Return")
    wait_report("transfer-result-page-2-before-section.png", top, changed=True)
    chord("Tab")
    chord("Tab")
    chord("Return")
    wait_report("transfer-result-section-reset.png", top, changed=False)
    # Put the newer job on page two, choose the original stopped transfer, then
    # explicitly select the newer one again. One native inspector must survive
    # both selections, with the newer result reset to its first page.
    # The chosen Completed section retains focus; five stops reach Next.
    for _ in range(5):
        chord("Tab")
    chord("Return")
    page_two = wait_report("transfer-result-before-job-switch.png", top, changed=True)
    session.run(["niri", "msg", "action", "focus-window", "--id", str(operations["id"])])
    session.wait_for(lambda: title("Operations")["is_focused"], "Operations focus for older job selection")
    # Starting unfocused: New Browser, shared Undo, Recovery, newer Details/Open
    # folder, then the original transfer's Details (Undo has no folder action).
    for _ in range(6):
        chord("Tab")
    chord("Return")
    session.wait_for(lambda: title("Transfer details")["is_focused"], "explicit older-job inspection")
    assert title("Transfer details")["id"] == newest["id"], "choosing another job replaced the native inspector"
    wait_report("transfer-switched-old-job.png", page_two, changed=True)
    session.run(["niri", "msg", "action", "focus-window", "--id", str(operations["id"])])
    session.wait_for(lambda: title("Operations")["is_focused"], "Operations focus for newer job selection")
    chord("d", "alt")
    session.wait_for(lambda: title("Transfer details")["is_focused"], "explicit newer-job inspection")
    assert title("Transfer details")["id"] == newest["id"]
    wait_report("transfer-switched-new-job-reset.png", top, changed=False)
    compact_inspector(newest["id"])
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "checks": ["a partial transfer survives source closure and exposes live progress in a floating inspector beside narrow Operations",
                       "cancelling records its completed, failed and remaining work for keyboard inspection without touching unstarted files",
                       "shared Undo reverses the confirmed prefix while failures, unstarted sources and the existing conflict stay untouched",
                       "the original inspector stays bound while a newer transfer completes, and its twenty-five-file result is keyboard-scrolled and paged in both directions with section reset",
                       "explicit job changes reuse one inspector and reset result pagination",
                       "recorded facts remain visible and keyboard-scrollable at the inspector minimum size"]}
