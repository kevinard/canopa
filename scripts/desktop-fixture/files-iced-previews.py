"""Real Iced transient ownership, text/source, image and PDF preview acceptance."""
import json
import os
import subprocess
import time


def verify(session, fixture, browser, launched, scheme, scale):
    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((item for item in windows() if item["title"] == value), None)

    def wait_title(value):
        result = session.wait_for(lambda: title(value), value, timeout=30)
        time.sleep(.3)
        return result

    def preview(name):
        return wait_title(f"Quick Look: {name} — Canopa Files")

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "preview workflow focus")
        time.sleep(.25)

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
        time.sleep(.25)

    def navigate(location):
        chord("l")
        session.run(["wtype", str(location)])
        keys("Return")
        return wait_title("Downloads — Canopa Files")

    def actual_launch(path):
        def recorded():
            try:
                return [os.fsencode(item) for item in json.loads(launched.read_text())] == [os.fsencode(path)]
            except (FileNotFoundError, json.JSONDecodeError):
                return False
        session.wait_for(recorded, f"preview action launches {path}")

    outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
    logical = next(item["logical"] for item in outputs.values() if item.get("logical"))
    pointer = session.build_pointer()

    def position(window):
        width, height = window["layout"]["window_size"]
        x = max(0, (logical["width"] - width) // 2)
        y = max(0, (logical["height"] - height) // 2)
        session.run(["niri", "msg", "action", "move-floating-window", "--id", str(window["id"]), "-x", str(x), "-y", str(y)])
        time.sleep(.3)
        return x, y, width, height

    def click(x, y):
        session.run([str(pointer), str(round(x / logical["width"] * 1600)), str(round(y / logical["height"] * 1000))])
        time.sleep(.25)

    def pixels(name, expected):
        def rendered():
            frame = session.capture(name)
            color = frame.getpixel((frame.width // 2, frame.height // 2))
            return all(abs(a-b) <= 12 for a,b in zip(color, expected))
        session.wait_for(rendered, f"actual preview pixels {name}", timeout=30)

    def absent(id):
        session.wait_for(lambda: all(item["id"] != id for item in windows()), f"preview {id} closes")

    focus(browser)
    session.run(["niri", "msg", "action", "set-window-width", "360"])
    time.sleep(.4)
    navigate(fixture / "B-notes.txt")
    keys("space")
    floating = preview("B-notes.txt")
    assert floating["is_floating"], floating
    initial_size = floating["layout"]["window_size"]
    assert abs(initial_size[0] - 760) <= 4, floating
    assert 560 <= initial_size[1] <= min(logical["height"], 640), floating
    assert title("Downloads — Canopa Files")["layout"]["window_size"][0] == 360
    x, y, width, height = position(floating)
    focus(floating)
    # Text copy inspects the actual editor contents, not just its window title.
    click(x + 60, y + 110)
    chord("a")
    chord("c")
    session.wait_for(lambda: session.run(["wl-paste", "--no-newline"]).rstrip("\n") == (fixture / "B-notes.txt").read_text().rstrip("\n"), "actual text preview clipboard")
    session.run(["wtype", "UNAUTHORIZED EDIT"])
    chord("a")
    chord("c")
    assert session.run(["wl-paste", "--no-newline"]).rstrip("\n") == (fixture / "B-notes.txt").read_text().rstrip("\n")
    session.capture("preview-01-floating-readonly-text.png")
    # Tab leaves text selection and focuses Open. Space activates that action.
    keys("Tab")
    launched.unlink(missing_ok=True)
    keys("space")
    actual_launch(fixture / "B-notes.txt")
    assert preview("B-notes.txt")["id"] == floating["id"]
    keys("Escape")
    absent(floating["id"])
    session.wait_for(lambda: any(item["id"] == browser["id"] and item["is_focused"] for item in windows()), "parent regains focus after Escape")
    launched.unlink(missing_ok=True)
    keys("Return")
    actual_launch(fixture / "B-notes.txt")

    source = fixture / "Source café.rs"
    code = '// café 日本語\r\nfn main() {\r\n    let café = "éclair";\r\n    let count = 42;\r\n    /* first line\r\n       second line */\r\n}\r\n'
    source.write_bytes(code.encode())
    navigate(source)
    keys("space")
    syntax_preview = preview(source.name)
    x, y, width, height = position(syntax_preview)
    palettes = {
        "dark": ((166, 227, 161), (249, 226, 175), (167, 186, 182)),
        "light": ((50, 106, 31), (133, 89, 0), (82, 103, 94)),
    }

    def syntax_pixels(name, color_scheme):
        def rendered():
            frame = session.capture(name)
            area = frame.crop((round((x + 8) * scale), round((y + 80) * scale),
                               round((x + width - 8) * scale), round((y + min(height - 40, 400)) * scale)))
            return all(sum(all(abs(a - b) <= 12 for a, b in zip(pixel, color))
                           for pixel in area.get_flattened_data()) >= 4
                       for color in palettes[color_scheme])
        session.wait_for(rendered, "actual syntax string, number and comment colors")

    syntax_pixels("preview-syntax.png", scheme)
    focus(syntax_preview)
    click(x + 60, y + 110)
    chord("a")
    chord("c")
    copied = subprocess.check_output(["wl-paste", "--no-newline"], env=session.env, timeout=10)
    (session.output / "preview-source-clipboard.bin").write_bytes(copied)
    (session.output / "preview-source-original.bin").write_bytes(source.read_bytes())
    # The native editor copies text with LF separators; source preparation
    # retains the original endings. Compare raw UTF-8 bytes explicitly so the
    # subprocess text wrapper cannot hide Unicode or newline mistakes.
    assert copied == code.replace("\r\n", "\n").encode(), repr(copied)
    assert source.read_bytes() == code.encode()
    config = session.runtime / "config/canopa/config.kdl"
    original_config = config.read_text()
    other_scheme = "light" if scheme == "dark" else "dark"
    config.write_text(original_config.replace(f'color-scheme "{scheme}"', f'color-scheme "{other_scheme}"'))
    syntax_pixels("preview-syntax-other-theme.png", other_scheme)
    config.write_text(original_config)
    syntax_pixels("preview-syntax-original-theme.png", scheme)
    focus(browser)
    navigate(fixture / "B-notes.txt")
    assert preview("B-notes.txt")["id"] == syntax_preview["id"]
    focus(syntax_preview)

    def plain_text():
        click(x + 60, y + 110)
        chord("a")
        chord("c")
        return subprocess.check_output(["wl-paste", "--no-newline"], env=session.env, timeout=10) == (fixture / "B-notes.txt").read_bytes()

    session.wait_for(plain_text, "actual plain text after the source selection changes")

    def plain_pixels():
        frame = session.capture("preview-syntax-plain-return.png")
        area = frame.crop((round((x + 8) * scale), round((y + 80) * scale),
                           round((x + width - 8) * scale), round((y + min(height - 40, 400)) * scale)))
        return not any(all(abs(a - b) <= 12 for a, b in zip(pixel, color))
                       for pixel in area.get_flattened_data() for color in palettes[scheme][:2])

    session.wait_for(plain_pixels, "plain selection clears previous source colors")
    focus(syntax_preview)
    keys("Escape")
    absent(syntax_preview["id"])
    focus(browser)

    keys("space")
    floating = preview("B-notes.txt")
    assert floating["is_floating"]
    position(floating)
    focus(floating)
    keys("Down")
    assert preview("C-photo.png")["id"] == floating["id"]
    pixels("preview-02-image-follow.png", (24, 160, 120))
    # A point near the fitted image's right edge must leave the image when
    # Zoom out activates, then return when Fit activates. Check rendered extent.
    def image_edge(name, inside):
        def rendered():
            frame = session.capture(name)
            color = frame.getpixel((round((logical["width"] / 2 + 300) * scale), frame.height // 2))
            matches = all(abs(a-b) <= 12 for a,b in zip(color, (24, 160, 120)))
            return matches == inside
        session.wait_for(rendered, f"actual image extent {name}")
    keys("Tab", "Tab", "space")
    assert preview("C-photo.png")["id"] == floating["id"]
    image_edge("preview-02b-image-zoom-out.png", False)
    keys("Tab", "space")
    image_edge("preview-02c-image-fit-restored.png", True)
    keys("Escape")
    absent(floating["id"])
    session.wait_for(lambda: any(item["id"] == browser["id"] and item["is_focused"] for item in windows()), "Escape returns to source")
    keys("space")
    floating = preview("C-photo.png")
    focus(floating)
    keys("space")
    absent(floating["id"])
    session.wait_for(lambda: any(item["id"] == browser["id"] and item["is_focused"] for item in windows()), "default Space returns to source")
    keys("space")
    floating = preview("C-photo.png")
    position(floating)
    focus(floating)
    keys("Down")
    assert preview("D-document.pdf")["id"] == floating["id"]
    pixels("preview-03-pdf-first-page.png", (255, 0, 0))
    keys("Page_Down")
    pixels("preview-04-pdf-second-page.png", (0, 255, 0))
    keys("Page_Up")
    pixels("preview-05-pdf-page-return.png", (255, 0, 0))

    focus(browser)
    chord("n")
    other = session.wait_for(lambda: next((item for item in windows() if item["title"] == "Downloads — Canopa Files" and item["id"] != browser["id"]), None), "second ordinary browser")
    focus(other)
    session.run(["niri", "msg", "action", "set-window-width", "640"])
    session.run(["wtype", "B-notes"])
    time.sleep(.4)
    assert preview("D-document.pdf")["id"] == floating["id"]
    pixels("preview-06-other-window-does-not-retarget.png", (255, 0, 0))

    focus(browser)
    chord("f")
    session.run(["wtype", "B-notes"])
    wait_title("Search: B-notes — Downloads — Canopa Files")
    keys("Down")
    assert preview("B-notes.txt")["id"] == floating["id"]
    session.capture("preview-07-search-source.png")
    keys("Escape")
    wait_title("Downloads — Canopa Files")
    preview("D-document.pdf")

    chord("t")
    wait_title("Downloads — Canopa Files")
    wait_title("Quick Look — Canopa Files")
    session.run(["wtype", "B-notes"])
    preview("B-notes.txt")
    chord("Tab", shift=True)
    preview("D-document.pdf")
    chord("Tab")
    preview("B-notes.txt")
    chord("w")
    preview("D-document.pdf")
    pixels("preview-08-source-tabs.png", (255, 0, 0))
    chord("w")
    absent(floating["id"])
    assert any(item["id"] == other["id"] for item in windows())
    focus(other)
    launched.unlink(missing_ok=True)
    keys("Return")
    actual_launch(fixture / "B-notes.txt")
    session.capture("preview-09-owner-close.png")
    log = (session.output / "files.log").read_text()
    assert not any(marker in log for marker in ["panicked", "CRITICAL", "invalid object", "Could not attach transient"]), log
    return {"status": "passed", "scope": "Iced transient text/source/image/PDF previews; media and full GUI migration remain required", "scheme": scheme, "scale": scale,
        "initial_preview_size": initial_size, "checks": [
            "Quick Look opens as an actual floating transient with no app-specific Niri rule",
            "a 360px browser retains its width beside a 760px preview with bounded initial height",
            "actual text editor selection copies the prepared file contents",
            "source previews render prepared syntax colors and copy exact Unicode text with native LF separators",
            "live palette changes restyle the same source preview and plain-text selection clears lexical colors",
            "typing cannot edit the read-only text preview",
            "Tab leaves text for Open and Space launches the exact selected file",
            "Escape restores source focus, selection and actual launch target",
            "Down from default preview focus browses its source and renders actual image pixels",
            "Tab and Space activate Zoom out and change the rendered image extent",
            "Fit restores the actual fitted image extent",
            "default-focus Space closes to the same source browser",
            "PDF paging renders distinct actual first/second pages and returns",
            "another browser's selection does not retarget the existing preview",
            "search selection uses the same source preview and leaving search restores it",
            "source tabs independently retarget and restore preview selection",
            "closing the owner destroys its preview while another browser remains usable",
        ], "windows": windows()}
