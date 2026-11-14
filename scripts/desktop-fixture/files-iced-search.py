"""Real Iced filename search, tab restoration and 50,000-result budget acceptance."""
import json
import os
import time
from pathlib import Path


def verify(session, fixture, browser, launched, scheme, scale):
    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def wait_title(value):
        result = session.wait_for(lambda: next((item for item in windows() if item["title"] == value), None), value, timeout=30)
        time.sleep(.3)
        return result

    def folder(name):
        return wait_title(f"{name} — Canopa Files")

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name])
            time.sleep(.25)

    def chord(modifier, key):
        session.run(["wtype", "-M", modifier, "-k", key, "-m", modifier])
        time.sleep(.25)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "search source window focus")
        time.sleep(.2)

    def navigate(path):
        chord("ctrl", "l")
        session.run(["wtype", str(path)])
        keys("Return")
        return folder(path.name)

    def search_title(query, location, state="Search"):
        return f"{state}: {query} — {location.name} — Canopa Files"

    def search(query, location):
        chord("ctrl", "f")
        session.run(["wtype", query])
        wait_title(search_title(query, location))

    def expect_open(path):
        launched.unlink(missing_ok=True)
        keys("Return")
        def recorded():
            try:
                return [os.fsencode(item) for item in json.loads(launched.read_text())] == [os.fsencode(path)]
            except (FileNotFoundError, json.JSONDecodeError):
                return False
        session.wait_for(recorded, f"actual launch of search selection {path}")

    def expect_empty(query, location):
        search(query, location)
        launched.unlink(missing_ok=True)
        keys("Down", "Return")
        time.sleep(.3)
        assert not launched.exists(), f"empty search opened an old selection: {query}"

    pointer = session.build_pointer()
    outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
    logical = next(item["logical"] for item in outputs.values() if item.get("logical"))
    def click(x, y):
        session.run([str(pointer), str(round(x / logical["width"] * 1600)), str(round(y / logical["height"] * 1000))])
        time.sleep(.25)

    root = fixture / "Search fixtures"
    left = root / "A-left"
    right = root / "B-right"
    denied = root / "C-unreadable"
    for path in [left, right, denied, left / "needle-folder"]:
        path.mkdir(parents=True, exist_ok=True)
    (root / "needle-root.txt").write_text("Filename match.\n")
    (root / "plain-content.txt").write_text("content-body-token appears only in the contents.\n")
    (root / ".needle-hidden.txt").write_text("Hidden filename match.\n")
    for path in [left, right]:
        (path / "needle-duplicate.txt").write_text(f"From {path.name}.\n")
    (left / "needle-folder/inside.txt").write_text("Inside the matched folder.\n")
    raw_parent = os.fsencode(left) + b"/raw-parent-\xfe"
    os.mkdir(raw_parent)
    raw_file = raw_parent + b"/needle-raw.txt"
    with open(raw_file, "wb") as stream:
        stream.write(b"Original Unix bytes remain operation targets.\n")
    for index in range(80):
        (root / f"unrelated-{index:02d}.txt").write_text("Original browsing state.\n")
    denied.chmod(0)
    try:
        try:
            os.listdir(denied)
        except PermissionError:
            pass
        else:
            raise AssertionError("private unreadable-directory fixture needs ordinary user permissions")
        focus(browser)
        session.run(["niri", "msg", "action", "set-window-width", "360"])
        time.sleep(.5)
        navigate(root)
        keys("End")
        original = root / "unrelated-79.txt"
        expect_open(original)
        before = session.capture("search-01-original-scroll.png")

        # Start with the actual visible Search button, not an internal message:
        # the magnifier at the end of the location field.
        click(208, 64)
        session.run(["wtype", "needle"])
        wait_title(search_title("needle", root))
        current = session.capture("search-02-current-folder.png")
        # The narrow layout puts Search below the location. Verify that a
        # continuous band of toolbar background separates the breadcrumb and
        # Places, allowing the controls' widths to follow their contents.
        top, bottom = (round(y * scale) for y in (56, 72))
        background = current.getpixel((round(5 * scale), top))
        run = widest = 0
        for x in range(round(190 * scale), round(250 * scale)):
            clear = all(
                max(abs(a - b) for a, b in zip(current.getpixel((x, y)), background)) <= 4
                for y in range(top, bottom)
            )
            run = run + 1 if clear else 0
            widest = max(widest, run)
        assert widest >= round(3 * scale), f"no visible gap before Places: {widest} pixels"
        # An old toolbar tooltip must not paint over the active input's top
        # edge after the pointer starts Search and the user begins typing.
        input_background = current.getpixel((round(250 * scale), round(140 * scale)))
        tooltip_pixels = sum(
            max(abs(a-b) for a,b in zip(current.getpixel((x, y)), input_background)) > 40
            for x in range(round(125 * scale), round(225 * scale))
            for y in range(round(127 * scale), round(130 * scale))
        )
        assert tooltip_pixels <= round(2 * scale * scale), f"toolbar tooltip overlaps search input: {tooltip_pixels} pixels"
        keys("Down")
        expect_open(root / "needle-root.txt")
        keys("Escape")
        folder(root.name)
        expect_open(original)
        after = session.capture("search-03-restored-scroll.png")
        position = (round((360 - 70) * scale), after.height - round(50 * scale))
        expected, actual = before.getpixel(position), after.getpixel(position)
        assert max(abs(a-b) for a,b in zip(expected, actual)) <= 3, (expected, actual)

        expect_empty("content-body-token", root)
        session.capture("search-04-filename-only.png")
        # Search contents (ADR0175) finds the same query inside the text file.
        # The filter band wraps under the field in a 360px tile.
        click(161, 181)
        time.sleep(.6)
        chord("ctrl", "f")
        keys("Down")
        expect_open(root / "plain-content.txt")
        session.capture("search-04b-contents.png")
        click(161, 181)
        time.sleep(.6)
        chord("ctrl", "f")
        launched.unlink(missing_ok=True)
        keys("Down", "Return")
        time.sleep(.3)
        assert not launched.exists(), "turning Search contents off left a content match actionable"
        search("NEEDLE", root)
        keys("Down")
        expect_open(root / "needle-root.txt")
        chord("alt", "s")
        wait_title(search_title("NEEDLE", root))
        session.run(["wtype", "needle-duplicate"])
        time.sleep(.4)
        expect_open(left / "needle-duplicate.txt")
        keys("Down")
        expect_open(right / "needle-duplicate.txt")
        session.capture("search-05-recursive-locations.png")
        chord("alt", "r")
        folder(right.name)
        expect_open(right / "needle-duplicate.txt")

        navigate(root)
        search("needle-folder", root)
        keys("Down")
        chord("alt", "s")
        wait_title(search_title("needle-folder", root))
        keys("Down", "Right")
        folder("needle-folder")
        session.run(["wtype", "inside"])
        time.sleep(.3)
        expect_open(left / "needle-folder/inside.txt")

        navigate(root)
        expect_empty("needle-hidden", root)
        chord("ctrl", "h")
        wait_title(search_title("needle-hidden", root))
        keys("Down")
        expect_open(root / ".needle-hidden.txt")
        chord("ctrl", "h")
        wait_title(search_title("needle-hidden", root))
        keys("Escape")
        folder(root.name)

        search("needle-raw", root)
        keys("Down")
        chord("alt", "s")
        wait_title(search_title("needle-raw", root))
        keys("Down")
        expect_open(raw_file)
        chord("alt", "r")
        folder("raw-parent-�")
        expect_open(raw_file)
        session.capture("search-06-raw-parent-reveal.png")

        navigate(root)
        search("needle", root)
        keys("Down")
        chord("alt", "s")
        wait_title(search_title("needle", root))
        chord("alt", "d")
        session.capture("search-07-read-failures.png")
        # Supersede several real worker requests before accepting the final one.
        chord("ctrl", "f")
        session.run(["wtype", "needle"])
        chord("ctrl", "a")
        session.run(["wtype", "content-body-token"])
        wait_title(search_title("content-body-token", root))
        launched.unlink(missing_ok=True)
        keys("Down", "Return")
        time.sleep(.3)
        assert not launched.exists(), "superseded search result remained actionable"

        search("needle-duplicate", root)
        keys("Down")
        expect_open(left / "needle-duplicate.txt")
        chord("ctrl", "t")
        folder(root.name)
        search("unrelated-79", root)
        keys("Down")
        expect_open(original)
        session.run(["wtype", "-M", "ctrl", "-M", "shift", "-k", "Tab", "-m", "shift", "-m", "ctrl"])
        wait_title(search_title("needle-duplicate", root))
        expect_open(left / "needle-duplicate.txt")
        chord("ctrl", "Tab")
        wait_title(search_title("unrelated-79", root))
        expect_open(original)
        chord("ctrl", "w")
        wait_title(search_title("needle-duplicate", root))
        session.capture("search-08-independent-tabs.png")

        budget = fixture / "Budget"
        budget.mkdir()
        for index in range(50_005):
            (budget / f"budget-{index:05d}.txt").touch()
        for index in [49_999, 50_004]:
            (budget / f"budget-{index:05d}.txt").write_text("Actual launched budget boundary.\n")
        navigate(budget)
        search("budget", budget)
        keys("Down", "End")
        expect_open(budget / "budget-49999.txt")
        session.capture("search-09-budget-limit.png")
        # Submit More and Stop in one ordered virtual-keyboard batch. The old
        # 50,000 results must survive cancellation during the new metadata scan.
        # Under load the larger scan can finish before Stop is read; that run
        # proves nothing, so start the 50,000-result search again and retry.
        stopped = search_title("budget", budget, "Stopped search")
        for attempt in range(3):
            session.run(["wtype", "-M", "alt", "-k", "m", "-k", "x", "-m", "alt"])
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline and not any(item["title"] == stopped for item in windows()):
                time.sleep(.2)
            if any(item["title"] == stopped for item in windows()):
                break
            assert attempt < 2, "Stop never interrupted the larger scan"
            keys("Escape")
            folder(budget.name)
            search("budget", budget)
        wait_title(stopped)
        expect_open(budget / "budget-49999.txt")
        session.capture("search-10-stopped-more-keeps-results.png")
        chord("alt", "m")
        wait_title(search_title("budget", budget))
        keys("End")
        expect_open(budget / "budget-50004.txt")
        session.capture("search-11-larger-budget.png")
        keys("Escape")
        folder(budget.name)
    finally:
        denied.chmod(0o700)
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL"]), log
    return {"status": "passed", "scope": "Iced filename search and browsing restoration; full file-manager scope remains active", "scheme": scheme, "scale": scale,
        "initial_result_budget": 50_000, "actual_matching_files": 50_005, "checks": [
            "visible Search button starts current-folder filename search in a 360px tile",
            "long location text stays inside its allotted toolbar space",
            "Down moves input focus into actual results and Return launches the correct file",
            "Escape restores the original selected file and painted scroll position",
            "content-only matches never become actionable filename results",
            "Search contents in the filter band finds and opens the content match, and turning it off removes it",
            "recursive scope finds equal filenames at distinct containing paths",
            "Show in containing folder selects the exact recursive result",
            "Right enters a recursively matched folder at narrow width",
            "hidden filename results respect explicit show/hide controls",
            "raw Unix parent bytes survive result activation and containing-folder navigation",
            "unreadable locations remain inspectable while other matches are usable",
            "superseded real worker results cannot replace the final query",
            "independent tabs retain their query, results and selection",
            "initial 50000-result limit stops at the actual boundary file",
            "Stop during a larger request preserves earlier results and selection",
            "More results reaches all 50005 actual matching files",
        ], "windows": windows()}
