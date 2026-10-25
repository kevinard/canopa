"""Explicit hidden-folder hierarchy and parent selection at tiled widths."""
import json
import time


def verify(session, fixture, browser, scheme, scale):

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((window for window in windows() if window["title"] == value), None)

    def wait_title(value):
        window = session.wait_for(lambda: title(value), value)
        session.wait_for(lambda: title(value) and title(value)["is_focused"], f"focus of {value}")
        time.sleep(.25)
        return window

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "hidden-folder navigation focus")
        time.sleep(.25)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name]); time.sleep(.25)

    def chord(modifier, key):
        session.run(["wtype", "-M", modifier, "-k", key, "-m", modifier]); time.sleep(.3)

    def navigate(location):
        focus(browser); chord("ctrl", "l")
        session.run(["wtype", str(location)]); keys("Return")
        wait_title(f"{location.name} — Canopa Files")

    # Keep the real pointer away from the toolbar while resizing, so a hover
    # tooltip cannot cover the selected hidden row in the narrow screenshot.
    pointer = session.build_pointer()
    session.run([str(pointer), "1500", "900", "move"])
    root = fixture / "Hidden navigation"
    active = root / ".active"; deep = active / "deep"
    deep.mkdir(parents=True); (root / ".other").mkdir(); (root / "visible").mkdir()
    (deep / "notes.txt").write_text("Selection survives hidden-folder parent navigation.\n")
    navigate(deep); session.run(["wtype", "notes.txt"]); time.sleep(.8)
    session.capture("hidden-01-wide-ancestors.png")
    keys("space"); wait_title("notes.txt — Quick Look"); keys("Escape")
    focus(browser); session.run(["niri", "msg", "action", "set-window-width", "360"]); time.sleep(.8)
    assert title("deep — Canopa Files")["layout"]["window_size"][0] == 360
    keys("Left"); wait_title(".active — Canopa Files")
    keys("Left"); wait_title("Hidden navigation — Canopa Files")
    session.capture("hidden-02-narrow-parent.png")
    # A hidden sibling must stay absent; failed type-to-select keeps the actual
    # child just left selected, so Right returns to .active rather than .other.
    session.run(["wtype", ".other"]); keys("Right"); wait_title(".active — Canopa Files")
    keys("Right"); wait_title("deep — Canopa Files")
    keys("space"); wait_title("notes.txt — Quick Look"); keys("Escape")
    focus(browser); session.run(["niri", "msg", "action", "set-window-width", "1280"]); time.sleep(.8)
    session.capture("hidden-03-wide-restored.png")
    keys("Left"); wait_title(".active — Canopa Files")
    keys("Left"); wait_title("Hidden navigation — Canopa Files")
    session.run(["wtype", ".other"]); keys("Right"); wait_title(".active — Canopa Files")
    keys("Left"); wait_title("Hidden navigation — Canopa Files")
    # Explicitly hiding hidden files clears the navigation exception. Rendered
    # inspection must show only visible/, with both dot folders absent.
    chord("ctrl", "h"); time.sleep(.5); chord("ctrl", "h"); time.sleep(.8)
    session.capture("hidden-04-explicit-hide.png")
    session.run(["wtype", "visible"]); keys("Right"); wait_title("visible — Canopa Files")
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "Gtk-WARNING"]), log
    return {"status":"passed", "scheme":scheme, "scale":scale, "checks":[
        "explicit hidden location opens without enabling hidden siblings",
        "wide hierarchy rendered with its actual hidden ancestor child",
        "source selection opens Quick Look before resizing",
        "browser tile narrows to 360 logical pixels",
        "Left reaches the parent at narrow width",
        "parent retains the hidden child just left for Right navigation",
        "hidden sibling type-to-select cannot replace that selection",
        "returning to the deep folder preserves its file selection and preview",
        "widening restores the same hierarchy and current location",
        "wide parent navigation follows the same hidden-child selection",
        "explicit Hide hidden clears temporary visibility in rendered listing",
        "ordinary visible folder navigation remains available",
    ], "windows":windows()}
