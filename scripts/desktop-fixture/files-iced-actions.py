"""Actual Iced action editing and command invocation in an isolated desktop.

All commands and output belong to the private session; no user paths or bus.
This slice does not establish durable restart continuation or icon acceptance.
"""
import json
import os
import re
from pathlib import Path
import shlex
import sys
import time
from urllib.parse import quote_from_bytes


def setup(session, fixture, config):
    recorder = session.runtime / "record-action.py"
    recorder.write_text(
        "import json, os, sys\nfrom pathlib import Path\n"
        f"p = Path({str(session.runtime / 'action-result.json')!r})\n"
        "p.write_text(json.dumps({'paths': [os.fsencode(p).hex() for p in sys.argv[1:]], 'cwd': os.fsencode(os.getcwd()).hex()}))\n"
        "sys.stdout.write('action stdout\\n' + 'o' * 70000)\n"
        "sys.stderr.write('action stderr\\n' + 'e' * 70000)\n"
    )
    config.write_text(config.read_text() +
        'files {\n toolbar "back" "action:inspect" "location" "search" "quick-look";\n'
        ' action "inspect" name="Inspect" command="/bin/true %F" shortcut="<Control><Alt>u" toolbar=#true {\n'
        '  selection min=1 files=#true directories=#false;\n }\n}\n')
    # Preserve an unrelated owner and comment during graphical saves.
    config.write_text(config.read_text() + '// Preserve this comment\nnotifications { }\n')


def verify(session, fixture, browser, config, scheme, scale):
    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(name):
        return next((window for window in windows() if window.get("title") == f"{name} — Canopa Files"), None)

    def wait_title(name):
        result = session.wait_for(lambda: title(name), name)
        time.sleep(.35)
        return result

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "window focus")
        time.sleep(.25)

    def key(name, repeat=1):
        session.run(["wtype", *[arg for _ in range(repeat) for arg in ("-k", name)]])
        time.sleep(.15)

    def chord(name, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", name]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.25)

    def replace(value):
        chord("a", "ctrl")
        session.run(["wtype", value])

    pointer = session.build_pointer()
    def click(x, y, button=None):
        # Virtual pointer coordinates are normalized to its declared 1600x1000.
        outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
        logical = next(o["logical"] for o in outputs.values() if o.get("logical"))
        args = [str(pointer), str(round(x * 1600 / logical["width"])), str(round(y * 1000 / logical["height"]))]
        if button:
            args.append(button)
        session.run(args)
        time.sleep(.25)

    def navigate(path):
        chord("l", "ctrl")
        session.run(["wtype", str(path)])
        key("Return")
        wait_title("Downloads")

    result_path = session.runtime / "action-result.json"
    def expect_run(path):
        def actual():
            try:
                return json.loads(result_path.read_text()) == {"paths": [os.fsencode(path).hex()], "cwd": os.fsencode(fixture).hex()}
            except (FileNotFoundError, json.JSONDecodeError):
                return False
        session.wait_for(actual, "captured command paths and working directory")

    focus(browser)
    session.capture("actions-01-toolbar-wide.png")
    chord("a", "ctrl", "shift")
    editor = wait_title("Custom Actions")
    # Traverse both buttons and native inputs. No pointer coordinates for form
    # fields: this verifies their actual keyboard focus and scroll reveal.
    key("Tab", 4)  # chooser, Add, Delete, Name
    replace("Record selection")
    key("Tab", 2)  # Icon, Command
    command = f"{shlex.quote(sys.executable)} {shlex.quote(str(session.runtime / 'record-action.py'))} %F"
    replace(command)
    key("Tab", 7)  # patterns, files, folders, min, max, shortcut, Record
    session.capture("actions-02-editor-record-focus.png")
    key("space")
    chord("i", "ctrl", "alt")
    chord("s", "ctrl")
    session.wait_for(lambda: 'Record selection' in config.read_text() and 'record-action.py' in config.read_text(), "graphical save")
    assert re.search(r'\bname\s*=\s*"Record selection"', config.read_text()), 'rapid Tab/Ctrl+A did not replace the Name field'
    assert '// Preserve this comment' in config.read_text() and 'notifications' in config.read_text()
    assert '<Control><Alt>i' in config.read_text() and '<Control><Alt>u' not in config.read_text()
    session.capture("actions-03-editor-saved.png")
    chord("w", "ctrl")
    session.wait_for(lambda: title("Custom Actions") is None, "saved editor closes")
    focus(browser)
    session.run(["wtype", "A-project"])
    time.sleep(.3)
    chord("i", "ctrl", "alt")
    assert not result_path.exists(), "file-only action ran for a folder"
    session.capture("actions-03b-inapplicable-folder.png")
    # Raw Unix bytes survive the location editor, selection, custom shortcut,
    # worker and final process argv without interpretation as shell code.
    raw = os.fsencode(fixture) + b"/raw-\xff $literal #%.txt"
    with open(raw, "wb") as stream:
        stream.write(b"A private action fixture.\n")
    navigate("file://" + quote_from_bytes(raw, safe="/"))
    chord("i", "ctrl", "alt")
    expect_run(raw)
    notifications = session.output / "notifications.json"
    session.wait_for(lambda: notifications.exists() and any(e["title"] == "Record selection" and e["body"] == "Finished" for e in json.loads(notifications.read_text())), "actual completion notification")
    chord("j", "ctrl")
    operations = wait_title("Operations")
    session.capture("actions-04-operations-finished.png")
    # Test the visible output control and read-only native text selection.
    session.run(["niri", "msg", "action", "set-window-width", "1280"])
    time.sleep(.4)
    session.capture("actions-05-operations-output-control.png")
    key("Tab", 3)  # New browser, Recovery, Show output
    session.capture("actions-focus-01-output-button.png")
    before_output = session.capture("actions-focus-02-before-output.png")
    key("Return")
    started = time.monotonic()
    def output_painted():
        frame = session.capture("actions-focus-02-after-return.png")
        return frame.crop((0, round(260 * scale), round(320 * scale), round(450 * scale))).tobytes() != before_output.crop((0, round(260 * scale), round(320 * scale), round(450 * scale))).tobytes()
    session.wait_for(output_painted, "command output paints", timeout=30)
    (session.output / "output-paint-seconds.json").write_text(json.dumps(time.monotonic() - started))
    key("Tab", 3)  # Open folder, Next page, Copy all output.
    session.capture("actions-focus-copy-all.png")
    key("Return")
    captured = session.run(["wl-paste", "--no-newline"])
    assert 'action stdout' in captured and 'action stderr' in captured
    assert captured.count('[Output was truncated]') == 2
    assert len(captured) < 132000
    key("Tab")  # Native read-only page text.
    chord("a", "ctrl")
    chord("c", "ctrl")
    page = session.run(["wl-paste", "--no-newline"])
    assert page == captured[:8192], (len(page), len(captured))
    session.run(["wtype", "this must not edit command output"])
    chord("a", "ctrl")
    chord("c", "ctrl")
    assert session.run(["wl-paste", "--no-newline"]) == page
    chord("Tab", "shift")  # Copy all.
    chord("Tab", "shift")  # Next page.
    key("space")
    key("Tab", 2)  # Copy all, page text.
    chord("a", "ctrl")
    chord("c", "ctrl")
    actual_page = session.run(["wl-paste", "--no-newline"])
    (session.output / "output-pages.json").write_text(json.dumps({"all_bytes": len(captured), "first_bytes": len(page), "second_bytes": len(actual_page), "second_matches": actual_page == captured[8192:16384]}))
    assert actual_page == captured[8192:16384], (len(actual_page), actual_page[:40])
    session.run(["niri", "msg", "action", "set-window-width", "360"])
    time.sleep(.4)
    session.capture("actions-05b-output-narrow.png")
    chord("w", "ctrl")
    focus(browser)
    session.run(["niri", "msg", "action", "set-window-width", "360"])
    time.sleep(.5)
    session.capture("actions-06-toolbar-narrow.png")
    result_path.unlink()
    # Narrow strip keeps explicit custom action ordering: Back then this action.
    click(115, 108)
    expect_run(raw)
    before_menu = session.capture("actions-06a-before-menu.png")
    key("Menu")
    menu_frame = session.capture("actions-07-file-menu.png")
    position = (round(280 * scale), menu_frame.height - round(90 * scale))
    expected, actual = before_menu.getpixel(position), menu_frame.getpixel(position)
    assert max(abs(a-b) for a,b in zip(expected, actual)) <= 3, (expected, actual)
    result_path.unlink()
    key("Down", 4)  # Open, Quick Look, Copy, Cut, configured action.
    session.capture("actions-07b-configured-action-focus.png")
    key("Return")
    expect_run(raw)
    # Dirty reload keeps drafts until the user explicitly chooses discard.
    chord("a", "ctrl", "shift")
    editor = wait_title("Custom Actions")
    key("Tab", 4)
    replace("Unsaved draft")
    chord("r", "ctrl")
    session.capture("actions-08-discard-default.png")
    key("Return")  # Keep editing is the default.
    assert 'Unsaved draft' not in config.read_text()
    chord("s", "ctrl")
    session.wait_for(lambda: 'Unsaved draft' in config.read_text(), "kept draft saves")
    chord("w", "ctrl")
    session.wait_for(lambda: title("Custom Actions") is None, "editor close")
    focus(browser)
    # Replace the saved command externally; the existing browser must pick up
    # the new catalog without resetting its raw selected path.
    long_script = session.runtime / "long-action.py"
    pids = session.runtime / "action-pids.json"
    long_script.write_text(
        "import json, os, subprocess, sys, time\nfrom pathlib import Path\n"
        "child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])\n"
        f"Path({str(pids)!r}).write_text(json.dumps([os.getpid(), child.pid]))\n"
        "child.wait()\n")
    old_command = command
    long_command = f"{shlex.quote(sys.executable)} {shlex.quote(str(long_script))} %F"
    text = config.read_text()
    assert old_command in text
    config.write_text(text.replace(old_command, long_command))
    time.sleep(.9)
    chord("i", "ctrl", "alt")
    session.wait_for(pids.exists, "long action starts")
    owned = json.loads(pids.read_text())
    chord("w", "ctrl")  # Last Browser closes; active command keeps the process.
    session.wait_for(lambda: title("Downloads") is None, "source Browser closes")
    operations = wait_title("Operations")
    focus(operations)
    session.capture("actions-09-running-without-browser.png")
    key("Tab", 3)  # New browser, Recovery, latest command Cancel.
    key("Return")
    session.wait_for(lambda: any(e["title"] == "Unsaved draft" and e["body"] == "Cancelled" for e in json.loads(notifications.read_text())), "cancellation completion notification")
    def stopped(pid):
        try:
            return Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()[0] == "Z"
        except FileNotFoundError:
            return True
    session.wait_for(lambda: all(stopped(pid) for pid in owned), "command and child process group stopped")
    session.capture("actions-10-cancelled.png")
    chord("n", "ctrl")
    session.wait_for(lambda: title("home"), "new Browser after source closure")
    focus(operations)
    session.capture("actions-11-settled-after-reopen.png")
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "checks": ["configured toolbar order and narrow adaptation", "keyboard button/field traversal and scroll reveal", "graphical action save preserves other owners", "shortcut recording changes the accelerator", "file-only action refuses folder selection", "raw process argv and captured working folder", "desktop completion notification", "shared Operations summary and read-only paged/copied bounded output", "narrow toolbar invocation", "keyboard context-menu invocation", "dirty reload defaults to Keep editing", "live action catalog reload preserves selection", "command outlives its last Browser", "process-group cancellation stops child", "new Browser after source closure"],
            "remaining": ["theme icons", "condition matrix", "durable restart lifetime", "full configuration and focus matrix"]}
