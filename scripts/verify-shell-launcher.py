#!/usr/bin/env python3
"""Exercise launcher input, file opening, clipboard and extensions in private Niri.

Uses validate-desktop.py's isolated Weston/Niri/bus fixture. No installed settings
or live desktop processes are changed. Requires wtype, wl-clipboard and qalc.
"""
import argparse
import importlib.util
import json
import os
import shutil
import struct
import subprocess
from pathlib import Path
import sys
import tempfile
import time
import zlib

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(desktop)


def verify(binary, files_binary, output, session, scale):
    runtime = session.runtime
    binary_dir = binary.parent
    # Bundled providers and the clipboard watcher re-execute the running Shell.
    # Keep that path valid even if a concurrent Cargo build replaces its output.
    binary = Path(shutil.copy2(binary, runtime / "canopa-shell"))
    files = runtime / "search-root"
    files.mkdir()
    target = files / "needle-file 'quoted'.txt"
    target.write_text("launcher fixture\n")
    folder = files / "needle-folder"
    folder.mkdir()
    system_data = runtime / "system-data"
    system_data.mkdir()
    (system_data / "mime").symlink_to("/usr/share/mime", target_is_directory=True)
    (system_data / "icons").symlink_to("/usr/share/icons", target_is_directory=True)
    session.env["XDG_DATA_DIRS"] = str(system_data)
    applications = runtime / "data/applications"
    applications.mkdir(parents=True)
    opened = runtime / "opened.json"
    opener = runtime / "opener.py"
    opener.write_text(f"import json,sys\nfrom pathlib import Path\nPath({str(opened)!r}).write_text(json.dumps(sys.argv[1:]))\n")
    (applications / "canopa-launcher-fixture.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=Unique Launcher Fixture\n"
        f"Exec=python3 {opener} %u\nMimeType=text/plain;inode/directory;x-scheme-handler/http;x-scheme-handler/https;\n")
    config_dir = runtime / "config"
    config_dir.mkdir(exist_ok=True)
    (config_dir / "mimeapps.list").write_text(
        "[Default Applications]\ntext/plain=canopa-launcher-fixture.desktop;\n"
        "inode/directory=canopa-launcher-fixture.desktop;\n"
        "x-scheme-handler/http=canopa-launcher-fixture.desktop;\n"
        "x-scheme-handler/https=canopa-launcher-fixture.desktop;\n")
    preferences = config_dir / "canopa/launcher.json"
    preferences.parent.mkdir(parents=True)
    preferences.write_text(json.dumps({"version":1,"extensions":{},"web_template":"https://example.test/search?q={query}","searches":[{"keyword":"docs","template":"https://docs.example.test/?q={query}"}],"shortcuts":[]}))
    preferences.chmod(0o600)
    extension = runtime / "extension.py"
    extension.write_text("""import json,sys,time
q=json.loads(sys.stdin.readline())
if q['query']=='slow': time.sleep(1.5)
print(json.dumps({'protocol_version':1,'results':[{'id':'value','label':q['query'],
 'description':'Fixture extension','actions':[{'label':'Copy','action':{'type':'copy','text':q['query']}}]}]}))
""")
    config = config_dir / "shell.kdl"
    config.write_text(f'''menus {{ menu "Applications" show-icons=#true prompt="Search apps and files" {{
 source "launcher" {{ root {json.dumps(str(files))};
  extension "Fixture" keyword="p" {{ argv "python3" {json.dumps(str(extension))}; }}
  extension "Missing" keyword="broken" {{ argv "/canopa-nonexistent-fixture"; }}
 }}
}} }}
bar "main" {{ edge "left"; modules {{ start "launcher"; }}
 panel {{ width 650; height 430; animation {{ open-ms 0; close-ms 0; }} }}
}}
modules {{ module "launcher" {{ type "button"; label "Launcher"; icon "L";
 actions {{ click {{ toggle-menu "Applications"; }} }}
}} }}
''')
    (output / "fixture.kdl").write_text(config.read_text())
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n', scale=scale)
    session.env["PATH"] = str(binary_dir) + os.pathsep + str(files_binary.parent) + os.pathsep + session.env["PATH"]
    files_process = session.start([str(files_binary), "--daemon", "--defaults"], "files.log")
    session.wait_for(lambda: "true" in session.run(["gdbus", "call", "--session", "--dest", "org.freedesktop.DBus", "--object-path", "/org/freedesktop/DBus", "--method", "org.freedesktop.DBus.NameHasOwner", "org.freedesktop.FileManager1"]), "Files reveal service", timeout=45)
    session.start(["wtype", "-s", "600000"], "keyboard.log")
    process = session.start([str(binary), "--config", str(config)], "shell.log")
    session.wait_for_layers(1)
    session.wait_for(lambda: (runtime / "canopa-shell.sock").is_socket(), "shell IPC")
    session.wait_for(lambda: "configuration reloaded" in (output / "shell.log").read_text(), "initial configuration observation")

    def open_query(query, verify_input=True):
        session.run([str(binary), "--open-menu", "Applications"])
        session.wait_for_layers(2)
        # Layer mapping precedes Iced's keyboard-focus operation under load.
        time.sleep(.8)
        session.run(["wtype", query])
        time.sleep(.4)
        if verify_input:
            session.run(["wtype", "-M", "ctrl", "a", "c", "-m", "ctrl"])
            clipboard(query)

    def clipboard(expected):
        def matches():
            try:
                return session.run(["wl-paste", "--type", "text/plain;charset=utf-8", "--no-newline"]) == expected
            except subprocess.CalledProcessError:
                return False
        session.wait_for(matches, "clipboard value")

    def close():
        session.run([str(binary), "--toggle-menu", "Applications"])
        session.wait_for_layers(1)

    for query, expected in [
        ("emoji", "😀"),
        ("emoji woman technologist", "👩‍💻"),
        ("emoji check mark", "✔️"),
        ("symbol em dash", "—"),
    ]:
        open_query(query)
        if query == "emoji woman technologist":
            session.run(["grim", str(output / "emoji.png")])
        session.run(["wtype", "-k", "Return"])
        session.wait_for_layers(1)
        clipboard(expected)

    open_query("= 12*(8+3)")
    session.run(["grim", str(output / "calculator.png")])
    session.run(["wtype", "-k", "Return"])
    session.wait_for_layers(1)
    clipboard("132")

    for query, expected in [("10 cm to m", "0.1 m"), ("= 32 F to C", "0 °C"),
                            ("1 GiB to MiB", "1024 MiB"), ("36 km/h in m/s", "10 m/s"),
                            ("sqrt(144)", "12"), ("200+10%", "220"), ("sqrt(144) m to cm", "1200 cm")]:
        open_query(query)
        if "32 F" in query:
            session.run(["grim", str(output / "unit-conversion.png")])
        session.run(["wtype", "-k", "Return"])
        session.wait_for_layers(1)
        clipboard(expected)

    open_query("= 1 m to kg")
    session.run(["wtype", "-k", "Return"])
    assert len(session.layers()) == 2, "incompatible units must not produce an actionable result"
    session.run(["wtype", "-M", "ctrl", "a", "-m", "ctrl", "1 kg to g"])
    time.sleep(.4)
    session.run(["wtype", "-k", "Return"])
    session.wait_for_layers(1)
    clipboard("1000 g")

    open_query("files needle-file")
    session.run(["grim", str(output / "files.png")])
    session.run(["wtype", "-M", "ctrl", "-k", "Return", "-m", "ctrl"])
    session.wait_for_layers(1)
    clipboard(str(target))

    for query, expected in [("files needle-file", str(target)), ("files needle-folder", str(folder)), ("Unique Launcher Fixture", None)]:
        opened.unlink(missing_ok=True)
        open_query(query)
        session.run(["wtype", "-k", "Return"])
        session.wait_for_layers(1)
        session.wait_for(opened.exists, f"opening {query}")
        arguments = json.loads(opened.read_text())
        if expected is not None:
            from urllib.parse import unquote, urlparse
            actual = arguments[0]
            if actual.startswith("file:"):
                actual = unquote(urlparse(actual).path)
            assert actual == expected, arguments

    # Open with remains in this Panel Menu and delivers the original file.
    opened.unlink(missing_ok=True)
    open_query("files needle-file")
    session.run(["wtype", "-M", "ctrl", "4", "-m", "ctrl"])
    time.sleep(.5)
    session.run(["wtype", "Unique Launcher Fixture"])
    time.sleep(.4)
    session.run(["grim", str(output / "open-with.png")])
    session.run(["wtype", "-k", "Return"])
    session.wait_for_layers(1)
    session.wait_for(opened.exists, "Open with file handoff")
    from urllib.parse import unquote, urlparse
    chosen = json.loads(opened.read_text())[0]
    assert unquote(urlparse(chosen).path) == str(target) if chosen.startswith("file:") else chosen == str(target)

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def files_windows():
        return [window for window in windows() if window["app_id"] == "org.canopa.Files"]

    open_query("files needle-file")
    session.run(["wtype", "-M", "ctrl", "3", "-m", "ctrl"])
    session.wait_for_layers(1)
    session.wait_for(lambda: len(files_windows()) == 1, "reveal in Files")
    browser = files_windows()[0]
    time.sleep(.5)
    open_query("windows search-root")
    session.run(["grim", str(output / "windows.png")])
    session.run(["wtype", "-k", "Return"])
    session.wait_for_layers(1)
    session.wait_for(lambda: any(window["id"] == browser["id"] and window["is_focused"] for window in windows()), "focused existing window")
    assert len(files_windows()) == 1, "window activation must focus the existing browser"

    open_query("files needle-file")
    session.run(["wtype", "-M", "ctrl", "5", "-m", "ctrl"])
    session.wait_for_layers(1)
    session.wait_for(lambda: len(files_windows()) == 3, "source-owned Quick Look")
    session.wait_for(lambda: any("Quick Look" in window["title"] for window in files_windows()), "preview title")
    session.run(["grim", str(output / "quick-look.png")])
    preview = next(window for window in files_windows() if "Quick Look" in window["title"])
    session.run(["niri", "msg", "action", "focus-window", "--id", str(preview["id"])])
    session.run(["wtype", "-k", "Escape"])
    session.wait_for(lambda: len(files_windows()) == 2, "Quick Look dismissal")
    session.wait_for(lambda: any(window["is_focused"] and window["id"] != browser["id"] for window in files_windows()), "focus returned to preview source")

    history = runtime / "state/canopa/shell/launcher-history.json"
    session.wait_for(history.exists, "private selection history")
    saved_history = history.read_text()
    assert "app:canopa-launcher-fixture.desktop" in saved_history
    assert "file:" in saved_history and "sqrt" not in saved_history and "200+10%" not in saved_history
    assert history.stat().st_mode & 0o777 == 0o600
    session.retire(process)
    session.wait_for_layers(0)
    process = session.start([str(binary), "--config", str(config)], "shell-restarted.log")
    session.wait_for_layers(1)
    session.wait_for(lambda: "configuration reloaded" in (output / "shell-restarted.log").read_text(), "restarted configuration observation")
    open_query("settings clear launcher history")
    session.run(["grim", str(output / "clear-history.png")])
    session.run(["wtype", "-k", "Return"])
    session.wait_for_layers(1)
    session.wait_for(lambda: json.loads(history.read_text())["entries"] == [], "durable history clear after restart")

    open_query("settings volume")
    session.run(["grim", str(output / "controls.png")])
    session.run(["wtype", "-k", "Return"])
    session.wait_for_layers(2)
    time.sleep(.3)
    session.run(["grim", str(output / "audio-panel.png")])
    session.run([str(binary), "--toggle-menu", "Applications"])
    session.wait_for_layers(2)
    close()

    open_query("settings keyboard")
    session.run(["wtype", "-k", "Return"])
    session.wait_for_layers(1)
    session.wait_for(lambda: any(window["app_id"] == "org.canopa.Settings" for window in windows()), "input settings page")
    time.sleep(.5)
    session.run(["grim", str(output / "settings-input.png")])

    # Browser requests terminate at the fixture handler; these checks never use the network.
    for query, expected in [("web hello & goodbye", "https://example.test/search?q=hello%20%26%20goodbye"),
                            ("docs panel actions", "https://docs.example.test/?q=panel%20actions"),
                            ("https://example.test/a?x=1&y=two", "https://example.test/a?x=1&y=two")]:
        opened.unlink(missing_ok=True)
        open_query(query)
        session.run(["grim",str(output / "web-search.png")])
        session.run(["wtype","-k","Return"])
        session.wait_for_layers(1)
        session.wait_for(opened.exists,"browser URL handoff")
        assert json.loads(opened.read_text()) == [expected], opened.read_text()
    for query, expected in [("timestamp 0","1970-01-01T00:00:00Z"),("date tomorrow",None),("time in Tokyo",None)]:
        open_query(query)
        session.run(["wtype","-k","Return"])
        session.wait_for_layers(1)
        value=session.run(["wl-paste","--type","text/plain;charset=utf-8","--no-newline"])
        if expected is not None:
            assert value == expected, value
        elif query == "date tomorrow":
            import datetime
            assert value == str(datetime.date.today()+datetime.timedelta(days=1)),value
        else:
            assert "JST (+09:00)" in value,value

    open_query("files needle-file")
    session.run(["wtype","-k","Tab"])
    time.sleep(.4)
    session.run(["wtype","pin"])
    time.sleep(.2)
    session.run(["grim",str(output / "search-actions.png")])
    session.run(["wtype","-k","Return"])
    session.wait_for(lambda: any(s["pinned"] for s in json.loads(preferences.read_text())["shortcuts"]),"saved pin")
    assert len(session.layers()) == 2
    close()
    open_query("files needle-file")
    session.run(["wtype","-k","Tab"])
    time.sleep(.4)
    session.run(["wtype","alias"])
    time.sleep(.2)
    session.run(["wtype","-k","Return"])
    time.sleep(.3)
    session.run(["wtype","work"])
    time.sleep(.2)
    session.run(["grim",str(output / "alias.png")])
    session.run(["wtype","-k","Return"])
    session.wait_for(lambda: any(s["alias"] == "work" and s["pinned"] for s in json.loads(preferences.read_text())["shortcuts"]),"saved alias with pin preserved")
    close()
    opened.unlink(missing_ok=True)
    open_query("work")
    session.run(["wtype","-k","Return"])
    session.wait_for_layers(1)
    session.wait_for(opened.exists,"alias original file handoff")
    actual=json.loads(opened.read_text())[0]
    assert unquote(urlparse(actual).path) == str(target) if actual.startswith("file:") else actual == str(target)

    # Change the owner preferences exactly as Settings does; Shell must apply them live.
    values=json.loads(preferences.read_text())
    values["extensions"]["builtin:web"]={"enabled":False,"keyword":"web"}
    temporary=preferences.with_suffix(".new")
    temporary.write_text(json.dumps(values));temporary.chmod(0o600);temporary.replace(preferences)
    time.sleep(1.5)
    opened.unlink(missing_ok=True)
    open_query("web disabled fixture")
    session.run(["wtype","-k","Return"])
    time.sleep(.3)
    assert len(session.layers()) == 2 and not opened.exists(),"disabled extension still acted"
    close()
    values["extensions"]["builtin:web"]={"enabled":True,"keyword":"search"}
    temporary.write_text(json.dumps(values));temporary.chmod(0o600);temporary.replace(preferences)
    time.sleep(1.5)
    open_query("search restored")
    session.run(["wtype","-k","Return"])
    session.wait_for_layers(1)
    session.wait_for(opened.exists,"renamed extension keyword")
    assert json.loads(opened.read_text()) == ["https://example.test/search?q=restored"]

    settings_before=sum(w["app_id"] == "org.canopa.Settings" for w in windows())
    open_query("settings launcher extensions")
    session.run(["wtype","-k","Return"])
    session.wait_for_layers(1)
    session.wait_for(lambda: sum(w["app_id"] == "org.canopa.Settings" for w in windows()) > settings_before,"Launcher settings page")
    time.sleep(.7)
    session.run(["grim",str(output / "settings-launcher.png")])

    # Exercise the Settings frontend, including its keyboard focus and Save path.
    def settings_launcher_focus():
        session.run(["wtype","-M","ctrl","2","-m","ctrl"])
        time.sleep(.2)
        session.run(["wtype","-M","ctrl","5","-m","ctrl"])
        time.sleep(.3)
    settings_launcher_focus()
    # Date and time precedes Web and adds an enable control and keyword field.
    session.run(["wtype", *(["-k", "Tab"] * 6), "-M","ctrl","a","-m","ctrl","find","-M","ctrl","s","-m","ctrl"])
    session.wait_for(lambda: json.loads(preferences.read_text())["extensions"]["builtin:web"]["keyword"] == "find","Settings keyboard keyword save")
    assert any(s["alias"] == "work" and s["pinned"] for s in json.loads(preferences.read_text())["shortcuts"])
    time.sleep(1.5)
    opened.unlink(missing_ok=True)
    open_query("find settings saved")
    session.run(["wtype","-k","Return"])
    session.wait_for_layers(1)
    session.wait_for(opened.exists,"Settings keyword applied to launcher")
    assert json.loads(opened.read_text()) == ["https://example.test/search?q=settings%20saved"]
    settings_launcher_focus()
    session.run(["wtype", *(["-k", "Tab"] * 5), "-k","space","-M","ctrl","s","-m","ctrl"])
    session.wait_for(lambda: not json.loads(preferences.read_text())["extensions"]["builtin:web"]["enabled"],"Settings keyboard extension toggle save")
    time.sleep(1.5)
    opened.unlink(missing_ok=True)
    open_query("find disabled through settings")
    session.run(["wtype","-k","Return"])
    time.sleep(.3)
    assert len(session.layers()) == 2 and not opened.exists()
    close()

    open_query("p extension value")
    session.run(["grim", str(output / "extension.png")])
    session.run(["wtype", "-k", "Return"])
    session.wait_for_layers(1)
    clipboard("extension value")

    open_query("p slow")
    session.run(["wtype", "-M", "ctrl", "a", "-m", "ctrl", "p fresh"])
    time.sleep(1.7)
    session.run(["wtype", "-k", "Return"])
    session.wait_for_layers(1)
    clipboard("fresh")

    open_query("broken query")
    session.run(["grim", str(output / "unavailable-extension.png")])
    session.run(["wtype", "-M", "ctrl", "a", "-m", "ctrl", "= 2+2"])
    time.sleep(.4)
    session.run(["wtype", "-k", "Return"])
    session.wait_for_layers(1)
    clipboard("4")

    catalog=runtime / "state/canopa/shell/launcher-extensions.json"
    session.wait_for(lambda: any(e["id"] == "custom:Missing" and "unavailable" in e["status"] for e in json.loads(catalog.read_text())["extensions"]),"safe extension failure discovery")
    observed=json.loads(catalog.read_text())
    assert any(e["id"] == "custom:Fixture" and "Copy" in e["actions"] for e in observed["extensions"])
    assert "extension value" not in catalog.read_text() and "p fresh" not in catalog.read_text()
    (output / "extension-catalog.json").write_text(catalog.read_text())

    open_query("files missing")
    session.run(["wtype", "-k", "Escape"])
    assert len(session.layers()) == 2, "first Escape must clear the query"
    session.run(["wtype", "-k", "Escape"])
    session.wait_for_layers(1)
    open_query("p slow")
    close()
    time.sleep(1.7)
    assert len(session.layers()) == 1, "late extension response reopened launcher"
    clip_file = runtime / "state/canopa/shell/clipboard.json"

    def saved_clipboard():
        return json.loads(clip_file.read_text()) if clip_file.exists() else {}

    def copy_external(text, *flags):
        subprocess.run(
            ["wl-copy", "--type", "text/plain;charset=utf-8", *flags],
            input=text, text=True, env=session.env, check=True, timeout=5,
        )

    def select():
        session.run(["wtype", "-k", "Return"])

    # The launcher helpers skip their input clipboard check while testing history.
    open_query("clip", verify_input=False)
    select()
    session.wait_for(lambda: saved_clipboard().get("recording"), "opt-in clipboard recording")
    close()
    time.sleep(.5)
    body = "launcher invoice marker\n  second line\n"
    copy_external(body)
    time.sleep(.35)
    copy_external("temporary selection")
    open_query("clip invoice marker", verify_input=False)
    select()
    session.wait_for_layers(1)
    clipboard(body)

    copy_external("password-hint-test-value", "--sensitive")
    open_query("clip password-hint-test-value", verify_input=False)
    select()
    time.sleep(.2)
    assert len(session.layers()) == 2, "sensitive offer entered history"
    close()

    open_query("clip invoice marker", verify_input=False)
    session.run(["wtype", "-k", "Tab"])
    time.sleep(.3)
    session.run(["wtype", "save as snippet"])
    select()
    time.sleep(.3)
    session.run(["wtype", "Invoice footer"])
    select()
    session.wait_for(lambda: bool(saved_clipboard().get("snippets")), "saved history snippet")
    assert saved_clipboard()["snippets"][0]["text"] == body
    close()

    open_query("clip clear history", verify_input=False)
    select()
    time.sleep(.3)
    close()
    open_query("clip invoice marker", verify_input=False)
    select()
    time.sleep(.2)
    assert len(session.layers()) == 2, "cleared copy remained searchable"
    close()

    open_query("clip turn off", verify_input=False)
    select()
    session.wait_for(lambda: saved_clipboard().get("recording") is False, "stop clipboard recording")
    close()
    copy_external("saved deliberately while recording was off\n")
    open_query("snip new", verify_input=False)
    select()
    time.sleep(.4)
    session.run(["wtype", "Manual snippet"])
    select()
    session.wait_for(lambda: len(saved_clipboard().get("snippets", [])) == 2, "explicit snippet while history off")
    assert saved_clipboard()["snippets"][0]["text"] == "saved deliberately while recording was off\n"
    close()
    def fixture_png(width, height):
        def chunk(kind, data):
            return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
        pixels = (b"\x00" + bytes((240, 45, 85, 255)) * width) * height
        return (b"\x89PNG\r\n\x1a\n"
                + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
                + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))

    def copy_image(image, *flags):
        subprocess.run(["wl-copy", "--type", "image/png", *flags], input=image,
                       env=session.env, check=True, timeout=5)

    def no_history_match(query):
        open_query(query, verify_input=False)
        select()
        time.sleep(.2)
        assert len(session.layers()) == 2, f"unexpected history result for {query}"
        close()

    settings_window = max(w["id"] for w in windows() if w["app_id"] == "org.canopa.Settings")
    extension_catalog = clip_file.with_name("launcher-extensions.json")

    def focus_clipboard_settings():
        session.run(["niri", "msg", "action", "focus-window", "--id", str(settings_window)])
        settings_launcher_focus()
        extensions = len(json.loads(extension_catalog.read_text())["extensions"])
        searches = len(json.loads(preferences.read_text())["searches"])
        # Two page controls precede the extension cards. The default URL,
        # custom searches and the add button follow the cards.
        tabs = 2 * extensions + 3 * searches + 5
        session.run(["wtype", *(["-k", "Tab"] * tabs)])

    focus_clipboard_settings()
    session.run(["wtype", "-k", "space", "-k", "Tab", "-k", "space", "-k", "Tab",
                 "-M", "ctrl", "a", "-m", "ctrl", "3", "-k", "Tab", "-k", "Return"])
    session.wait_for(lambda: all(saved_clipboard().get(key) == value for key, value in
                                 {"recording": True, "images": True, "max_entries": 3}.items()),
                     "save clipboard preferences in Settings")
    assert len(saved_clipboard()["snippets"]) == 2
    session.run(["grim", str(output / "settings-clipboard.png")])
    time.sleep(1.5)

    for marker in ["expired", "second", "third", "fourth"]:
        copy_external(f"settings-limit-{marker}")
        time.sleep(.4)
    no_history_match("clip settings-limit-expired")
    open_query("clip settings-limit-fourth", verify_input=False)
    select()
    session.wait_for_layers(1)
    clipboard("settings-limit-fourth")

    copy_image(fixture_png(53, 17), "--sensitive")
    no_history_match("clip PNG 53")
    image = fixture_png(48, 16)
    copy_image(image)
    time.sleep(.5)
    copy_external("text retained alongside image")
    open_query("clip PNG 48", verify_input=False)
    session.run(["grim", str(output / "clipboard-image.png")])
    select()
    session.wait_for_layers(1)
    assert subprocess.check_output(["wl-paste", "--type", "image/png", "--no-newline"],
                                   env=session.env, timeout=5) == image

    focus_clipboard_settings()
    session.run(["wtype", "-k", "Tab", "-k", "space", "-k", "Tab", "-k", "Tab", "-k", "Return"])
    session.wait_for(lambda: saved_clipboard().get("images") is False, "disable only image history")
    assert saved_clipboard()["recording"] is True
    time.sleep(1.5)
    no_history_match("clip PNG 48")
    open_query("clip text retained alongside image", verify_input=False)
    select()
    session.wait_for_layers(1)
    clipboard("text retained alongside image")
    session.run(["wl-copy", "--clear"])

    session.retire(process)
    session.wait_for_layers(0)
    process = session.start([str(binary), "--config", str(config)], "shell-snippets-restart.log")
    session.wait_for_layers(1)
    session.wait_for(lambda: "configuration reloaded" in (output / "shell-snippets-restart.log").read_text(), "snippet restart configuration")
    open_query("snip Invoice footer", verify_input=False)
    session.run(["grim", str(output / "clipboard-snippet.png")])
    select()
    session.wait_for_layers(1)
    clipboard(body)
    assert saved_clipboard()["max_entries"] == 3
    assert "history" not in saved_clipboard()
    no_history_match("clip text retained alongside image")
    session.retire(process)
    session.wait_for_layers(0)
    desktop.write_json(output / "result.json", {"status": "passed", "scale": scale,
        "checks": ["emoji suggestions and exact multi-codepoint clipboard copy",
                   "named symbol clipboard copy", "calculator clipboard after close", "length conversion", "temperature conversion",
                   "binary data conversion", "speed conversion", "incompatible units recovery",
                   "percentages", "math functions", "functions with conversion",
                   "file copy path", "file opening", "Open with chooser", "reveal parent",
                   "window focus", "Quick Look ownership and dismissal", "private history persistence and clear", "existing audio panel", "input Settings page", "explicit web search and URL opening", "custom search keywords",
                   "date and time extension", "searchable action menu", "saved pins and aliases",
                   "live extension enable and keyword preferences", "Launcher Settings page", "Settings keyboard keyword and enable saves",
                   "folder opening", "application launch", "extension copy", "superseded query",
                   "provider failure isolation", "Escape clear/close", "close during query",
                   "external clipboard capture and recopy", "skip sensitive offers",
                   "pin as snippet and clear recent history", "create snippet with recording off",
                   "Settings keyboard clipboard preferences and history limit",
                   "skip sensitive images", "image thumbnail and exact PNG recopy",
                   "disable image history while retaining text", "snippet copy after Shell restart"]})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/canopa-shell")
    parser.add_argument("--files-binary", type=Path, default=ROOT / "target/debug/canopa-files")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scale", type=float, default=1.0)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="canopa-launcher-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(args.binary.resolve(), args.files_binary.resolve(), output, session, args.scale)
        except Exception:
            try:
                session.run(["grim", str(output / "failure.png")])
                history = session.runtime / "state/canopa/shell/launcher-history.json"
                if history.exists():
                    (output / "history-on-failure.json").write_text(history.read_text())
            except Exception:
                pass
            raise
        finally:
            session.close()
    print(f"PASS: launcher native checks at scale {args.scale}")


if __name__ == "__main__":
    main()
