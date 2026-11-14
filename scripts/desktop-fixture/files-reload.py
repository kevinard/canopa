"""Live configuration acceptance through the private desktop's real UI."""
import json
import sys
import time


def verify(session, config, fixture, scheme, scale):
    checks = []

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((window for window in windows() if window.get("title") == value), None)

    def wait_title(value):
        window = session.wait_for(lambda: title(value), value)
        time.sleep(.3)
        return window

    def chord(mods, key):
        session.run(["wtype", *sum((["-M", mod] for mod in mods), []), "-k", key,
                     *sum((["-m", mod] for mod in reversed(mods)), [])])
        time.sleep(.3)

    def ctrl(key):
        chord(["ctrl"], key)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name])
            time.sleep(.25)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "reload workflow focus")
        time.sleep(.3)

    def replace(path, text):
        temporary = path.with_name(path.name + ".new")
        temporary.write_text(text)
        temporary.replace(path)

    def settled():
        # Two observations plus worker/UI delivery; pixel predicates below wait
        # for actual repaint. Negative checks also cross that debounce interval.
        time.sleep(1.2)

    def surface(name, expected):
        def rendered():
            frame = session.capture(name).convert("RGB")
            # List view stripes alternate rows; an unstriped row shows the surface.
            column = [frame.getpixel((frame.width - 35, y)) for y in range(frame.height // 2, frame.height - 80, 3)]
            return any(all(abs(a - b) < 8 for a, b in zip(color, expected)) for color in column)
        session.wait_for(rendered, name)

    def pdf_color(name, expected):
        def rendered():
            frame = session.capture(name).convert("RGB")
            return frame.getpixel((frame.width // 2, frame.height // 2)) == expected
        session.wait_for(rendered, f"rendered PDF {name}")

    def pdf_green(name):
        pdf_color(name, (0, 255, 0))

    opposite = "light" if scheme == "dark" else "dark"
    expected = (251, 252, 248) if opposite == "light" else (32, 45, 49)
    base = f'schema-version 1\nappearance {{ color-scheme "{opposite}"; }}\n'
    browser = wait_title("Downloads — Canopa Files")
    keys("Home", "Down", "Down", "Down", "space")
    preview = wait_title("Quick Look: D-document.pdf — Canopa Files")
    pdf_color("reload-00-pdf-first.png", (255, 0, 0))
    keys("Page_Down")
    pdf_green("reload-01-pdf-second.png")
    replace(config, base)
    # ADR0171: the floating Quick Look dims the browser behind it, so this
    # probe checks the repainted scheme's brightness instead of its exact color.
    def repainted():
        frame = session.capture("reload-02-appearance.png").convert("RGB")
        color = frame.getpixel((frame.width - 35, frame.height - 80))
        return min(color) > 200 if opposite == "light" else max(color) < 70
    session.wait_for(repainted, "reload-02-appearance.png")
    assert title("Quick Look: D-document.pdf — Canopa Files")["id"] == preview["id"]
    pdf_green("reload-03-pdf-preserved.png")
    assert title("Quick Look: D-document.pdf — Canopa Files")["is_focused"]
    checks += ["atomic appearance reload repaints existing windows", "Quick Look keeps its window, focus, source and PDF page"]
    keys("Escape")

    target = fixture / "Reload target"
    target.mkdir()
    ctrl("l"); session.run(["wtype", str(target)])
    replace(config, base + '// unrelated source edit\nshell {}\n')
    settled()
    session.capture("reload-04-location-draft.png")
    keys("Return"); wait_title("Reload target — Canopa Files")
    checks.append("source-only changes preserve in-progress location input and focus")
    ctrl("l"); session.run(["wtype", str(fixture)]); keys("Return")
    wait_title("Downloads — Canopa Files"); keys("c")

    result = session.runtime / "reload-action.json"
    helper = session.runtime / "reload-action.py"
    helper.write_text(f'import json, sys\nfrom pathlib import Path\nPath({str(result)!r}).write_text(json.dumps(sys.argv[1:]))\n')
    command = f'{sys.executable} "{helper}" %F'

    def files(mode="list", width=320, name="Reload action"):
        return (f'files {{ view mode="{mode}" column-width={width}; shortcut "search" "F6";\n'
                f'action "reload" name={json.dumps(name)} command={json.dumps(command)} shortcut="F7" {{ selection min=1; }}\n}}\n')

    live = base + files()
    ctrl("l"); session.run(["wtype", str(target)])
    replace(config, live); settled()
    session.capture("reload-04b-files-location-draft.png")
    keys("Return"); wait_title("Reload target — Canopa Files")
    ctrl("l"); session.run(["wtype", str(fixture)]); keys("Return")
    # Returning to a parent deliberately selects the child folder just left.
    wait_title("Downloads — Canopa Files"); keys("c")
    checks.append("effective Files changes preserve in-progress location input and focus")
    # Running an action reports through notifications; Operations stays closed.
    keys("F7")
    session.wait_for(result.exists, "automatically installed custom action")
    assert json.loads(result.read_text()) == [str(fixture / "C-photo.png")]
    focus(browser); keys("space")
    wait_title("Quick Look: C-photo.png — Canopa Files"); keys("Escape")
    session.capture("reload-05-list-selection.png")
    checks.append("Files view, shortcuts and custom actions install together while preserving selection")
    ctrl("n")
    other = session.wait_for(lambda: next((w for w in windows() if w["id"] != browser["id"] and w.get("title") == "Downloads — Canopa Files"), None), "second reload browser")
    focus(other)
    replace(config, live.replace('"F6"', '"F8"')); settled()
    keys("F8"); session.run(["wtype", "B-notes"]); settled(); keys("Down", "space")
    wait_title("Quick Look: B-notes.txt — Canopa Files"); keys("Escape", "Escape")
    ctrl("w"); session.wait_for(lambda: all(w["id"] != other["id"] for w in windows()), "second reload browser closed")
    focus(browser)
    keys("F8"); session.run(["wtype", "C-photo"]); settled(); keys("Down", "space")
    wait_title("Quick Look: C-photo.png — Canopa Files"); keys("Escape", "Escape")
    replace(config, live); settled()
    checks.append("external shortcut changes reach both existing browser windows")

    replace(config, f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\nfiles {{ shortcut "copy" "Up"; }}\n')
    settled(); surface("reload-06-invalid-candidate.png", expected)
    result.unlink(); keys("F7")
    session.wait_for(result.exists, "last good action survives rejected candidate")
    assert json.loads(result.read_text()) == [str(fixture / "C-photo.png")]
    focus(browser)
    checks.append("invalid Files settings retain the complete appearance and action candidate")

    theme = config.with_name("live-theme.kdl")
    themed = f'schema-version 1\nappearance {{ color-scheme "{opposite}"; theme "live-theme.kdl"; }}\n' + files()
    replace(config, themed); settled(); surface("reload-07-missing-theme.png", expected)
    theme.write_text('palette { surface "#304050"; }\n')
    surface("reload-08-created-theme.png", (48, 64, 80))
    replace(theme, 'palette { surface "#405060"; }\n')
    surface("reload-09-edited-theme.png", (64, 80, 96))
    replace(theme, 'private-theme-value "never-display-this-value"\n')
    settled(); surface("reload-10-invalid-theme.png", (64, 80, 96))
    replace(theme, 'palette { surface "#304050"; }\n')
    surface("reload-11-theme-repaired.png", (48, 64, 80))
    checks.append("missing theme creation, atomic dependency edits, rejection and repair repaint automatically")

    ctrl("comma"); wait_title("Preferences — Canopa Files")
    # Views: four pages, three views, four sort keys, two switches, narrower, width.
    keys(*(["Tab"] * 15)); ctrl("a"); session.run(["wtype", "400"])
    changed = themed.replace('column-width=320', 'column-width=360')
    replace(config, changed); settled()
    session.capture("reload-12-preferences-draft.png")
    ctrl("s"); settled()
    assert config.read_text() == changed, "stale preferences must not overwrite the source"
    session.capture("reload-13-preferences-refused.png")
    ctrl("r"); settled(); session.capture("reload-14-preferences-discard.png")
    keys("Tab", "Return"); settled()
    session.capture("reload-15-preferences-reloaded.png")
    # ADR0169: an unchanged reloaded draft has nothing to save; a submitted
    # edit made after explicit Reload saves over the reloaded source.
    assert "column-width=360" in config.read_text()
    chord(["alt"], "1"); settled()
    keys(*(["Tab"] * 15)); ctrl("a"); session.run(["wtype", "380"])
    ctrl("s"); session.wait_for(lambda: "column-width=380" in config.read_text(), "explicitly reloaded preferences can save")
    keys("Escape"); session.wait_for(lambda: title("Preferences — Canopa Files") is None, "preferences closed after reload")
    checks.append("external reload preserves uncommitted preference fields and stale Save refuses; explicit Reload accepts the source")

    focus(browser); chord(["ctrl", "shift"], "a"); wait_title("Custom Actions — Canopa Files"); settled()
    keys("Tab", "Tab", "Tab", "Tab")  # action chooser, Add, Delete, Name
    ctrl("a"); session.run(["wtype", "Unsaved action draft"])
    changed = themed.replace('Reload action', 'External action')
    replace(config, changed); settled()
    session.capture("reload-16-action-draft.png")
    ctrl("s"); settled()
    assert config.read_text() == changed, "stale custom actions must not overwrite the source"
    session.capture("reload-17-action-refused.png")
    ctrl("r"); settled(); session.capture("reload-18-action-discard.png")
    keys("Tab", "Return"); settled(); session.capture("reload-19-action-reloaded.png")
    ctrl("s"); session.wait_for(lambda: config.read_text() != changed, "explicitly reloaded actions can save")
    assert "External action" in config.read_text() and "Unsaved action draft" not in config.read_text()
    checks.append("external reload keeps the custom action draft; stale Save refuses and Ctrl+R restores the current catalog")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks, "windows": windows()}
