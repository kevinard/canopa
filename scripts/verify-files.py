#!/usr/bin/env python3
"""Verify Files browsing, Quick Look, transfers and Trash in a private Niri desktop.

All files and preferences belong to a temporary HOME. No installed desktop
services can activate on its private bus. Retains screenshots and summary.json.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import resource
import shutil
import signal
import subprocess
import sys
import tempfile
import time

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def pdf(path):
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 240 180] /Contents 4 0 R >>",
        b"<< /Length 26 >>\nstream\n1 0 0 rg 0 0 240 180 re f\nendstream",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 240 180] /Contents 6 0 R >>",
        b"<< /Length 26 >>\nstream\n0 1 0 rg 0 0 240 180 re f\nendstream",
    ]
    document = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for index, data in enumerate(objects, 1):
        offsets.append(len(document))
        document.extend(f"{index} 0 obj\n".encode() + data + b"\nendobj\n")
    xref = len(document)
    document.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        document.extend(f"{offset:010} 00000 n \n".encode())
    document.extend(f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    path.write_bytes(document)


def verify(binary, session, scheme, scale, reload_only=False, desktop_entry=None, cross_destination=None, hidden_navigation_only=False, downloads_only=False, *, iced_browsing_only=False, iced_search_only=False, iced_preview_only=False, iced_actions_only=False, iced_interface_only=False, iced_views_only=False, iced_preferences_only=False, iced_preferences_case="full", iced_views_case="full", iced_operations_only=False, iced_operations_case="full", iced_media_only=False, iced_media_formats_only=False):
    output = session.output
    fixture = session.runtime / "home/Downloads"
    fixture.mkdir(parents=True)
    # Setup callbacks must see the same private HOME as the eventual application.
    session.env["HOME"] = str(fixture.parent)
    (fixture / "A-project").mkdir()
    (fixture / "A-project/notes.rs").write_text('fn main() { println!("Hello Canopa"); }\n')
    (fixture / "B-notes.txt").write_text("A native preview of a real file.\n")
    Image.new("RGBA", (96, 72), (24, 160, 120, 255)).save(fixture / "C-photo.png")
    pdf(fixture / "D-document.pdf")
    for i in range(70):
        (fixture / f"document-{i:02d}.txt").write_text(f"Document {i}\n")
    config = session.runtime / "config/canopa/config.kdl"
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n')
    if iced_search_only:
        config.write_text(config.read_text() + 'files {\n'
            ' shortcut "search-subfolders" "<Alt>s";\n'
            ' shortcut "more-search-results" "<Alt>m";\n'
            ' shortcut "stop-search" "<Alt>x";\n'
            ' shortcut "reveal-search-result" "<Alt>r";\n'
            ' shortcut "search-problems" "<Alt>d";\n}\n')
    if iced_actions_only:
        actions_spec = importlib.util.spec_from_file_location("files_iced_actions", ROOT / "scripts/desktop-fixture/files-iced-actions.py")
        actions_workflow = importlib.util.module_from_spec(actions_spec)
        actions_spec.loader.exec_module(actions_workflow)
        actions_workflow.setup(session, fixture, config)
    if iced_operations_only:
        operations_file = {
            "full": "files-iced-operations.py",
            "startup": "files-iced-operations.py",
            "startup-interrupted": "files-iced-interrupted.py",
            "window-navigation": "files-iced-operations.py",
            "location-editing": "files-iced-operations.py",
            "trash": "files-iced-trash.py",
            "recovery": "files-iced-recovery.py",
            "recovery-directories": "files-iced-recovery-directories.py",
            "interrupted": "files-iced-interrupted.py",
            "clipboard": "files-iced-clipboard.py",
            "clipboard-lifetime": "files-iced-clipboard-lifetime.py",
            "source-leases": "files-iced-source-leases.py",
            "clipboard-source-copy": "files-iced-clipboard-sources.py",
            "clipboard-source-move": "files-iced-clipboard-sources.py",
            "clipboard-capacity": "files-iced-clipboard-capacity.py",
            "drops": "files-iced-drops.py",
            "drop-exchange": "files-iced-drop-exchange.py",
            "drop-choices": "files-iced-drop-choices.py",
            "drop-stale": "files-iced-drop-stale.py",
            "drop-devices": "files-iced-drop-devices.py",
            "transfers": "files-iced-transfers.py",
            "continuation": "files-iced-continuation.py",
            "dismissal": "files-iced-dismissal.py",
            "transfer-completion": "files-iced-transfer-completion.py",
            "claim-review": "files-iced-claim-review.py",
            "prepared-rename": "files-iced-prepared-rename.py",
            "focus": "files-iced-transfers.py",
            "focus-expired": "files-iced-transfers.py",
            "parents": "files-iced-transfers.py",
            "compact": "files-iced-transfers.py",
            "conflicts": "files-iced-conflicts.py",
            "move-conflicts": "files-iced-conflicts.py",
            "question-focus": "files-iced-conflicts.py",
            "unavailable": "files-iced-unavailable.py",
            "mixed-copy": "files-iced-mixed-conflicts.py",
            "mixed-move": "files-iced-mixed-conflicts.py",
        }[iced_operations_case]
        operations_spec = importlib.util.spec_from_file_location("files_iced_operations", ROOT / "scripts/desktop-fixture" / operations_file)
        operations_workflow = importlib.util.module_from_spec(operations_spec)
        operations_spec.loader.exec_module(operations_workflow)
        if iced_operations_case == "location-editing":
            operations_workflow.setup(session, fixture, config, location_editing_only=True)
        else:
            operations_workflow.setup(session, fixture, config)
    if iced_interface_only:
        interface_spec = importlib.util.spec_from_file_location("files_iced_interface", ROOT / "scripts/desktop-fixture/files-iced-interface.py")
        interface_workflow = importlib.util.module_from_spec(interface_spec)
        interface_spec.loader.exec_module(interface_workflow)
        interface_workflow.setup(session, fixture, config)
    if iced_preferences_only:
        preferences_spec = importlib.util.spec_from_file_location("files_iced_preferences", ROOT / "scripts/desktop-fixture/files-iced-preferences.py")
        preferences_workflow = importlib.util.module_from_spec(preferences_spec)
        preferences_spec.loader.exec_module(preferences_workflow)
        preferences_workflow.setup(session, fixture, config, iced_preferences_case)
    if iced_views_only:
        views_spec = importlib.util.spec_from_file_location("files_iced_views", ROOT / "scripts/desktop-fixture/files-iced-views.py")
        views_workflow = importlib.util.module_from_spec(views_spec)
        views_spec.loader.exec_module(views_workflow)
        views_workflow.setup(session, fixture, config)
    # A private default application records the actual argv it receives. This
    # tests GIO desktop launching without invoking any installed user program.
    launched = session.runtime / "launched.json"
    helper = session.runtime / "open-fixture.py"
    helper.write_text(f'import json, sys\nfrom pathlib import Path\nPath({str(launched)!r}).write_text(json.dumps(sys.argv[1:]))\n')
    applications = session.runtime / "data/applications"
    applications.mkdir(parents=True)
    (applications / "canopa-files-fixture.desktop").write_text(
        f'[Desktop Entry]\nType=Application\nName=Files acceptance fixture\nExec={sys.executable} {helper} %f\n'
        'MimeType=text/plain;\nTerminal=false\nNoDisplay=true\n')
    (config.parent.parent / "mimeapps.list").write_text('[Default Applications]\ntext/plain=canopa-files-fixture.desktop;\n')
    session.env.update(GDK_BACKEND="wayland", GSK_RENDERER="cairo", GTK_A11Y="none", GSETTINGS_BACKEND="memory")
    if iced_browsing_only or iced_search_only or iced_preview_only or iced_actions_only or iced_interface_only or iced_views_only or iced_preferences_only or iced_operations_only or downloads_only:
        session.env["RUST_BACKTRACE"] = "1"
    session.start_niri(
        'animations { off; }\nxwayland-satellite { off; }\n'
        'hotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; '
        'focus-ring { off; }; default-column-width { proportion 1.0; }; }\n',
        width=round(1280 * scale), height=round(800 * scale), scale=scale,
    )
    session.start(["wtype", "-s", "600000"], "keyboard.log")
    notifications = output / "notifications.json"
    session.start([sys.executable, str(ROOT / "scripts/desktop-fixture/files-notifications.py"), str(notifications)], "notifications.log")
    session.wait_for(notifications.with_suffix(".ready").exists, "private notification receiver")
    if iced_operations_only or iced_preview_only or downloads_only:
        modules = output / "gio-modules"
        modules.mkdir()
        flags = subprocess.check_output(["pkg-config", "--cflags", "--libs", "gio-2.0"], text=True).split()
        subprocess.run(["cc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
                        str(ROOT / "scripts/desktop-fixture/files-volume-monitor.c"),
                        "-o", str(modules / "libcanopa-volume-fixture.so"), *flags], check=True, timeout=30)
        media = session.runtime / "volumes"
        (media / "Fixture USB").mkdir(parents=True)
        (media / "Fixture USB/Drive notes.txt").write_text("A real file on the private fixture volume.\n")
        session.env.update(GIO_EXTRA_MODULES=str(modules), GIO_USE_VOLUME_MONITOR="unix",
                           GIO_USE_VFS="local", CANOPA_FIXTURE_VOLUMES=str(media))
        inventory = session.run(["gio", "mount", "--list", "--detail"])
        (output / "gio-volumes.txt").write_text(inventory)
        assert "Fixture USB" in inventory and "FixtureVolume" in inventory, inventory
    if iced_media_only or iced_media_formats_only:
        media_spec = importlib.util.spec_from_file_location("files_iced_media", ROOT / "scripts/desktop-fixture/files-iced-media.py")
        iced_media_workflow = importlib.util.module_from_spec(media_spec)
        media_spec.loader.exec_module(iced_media_workflow)
        iced_media_workflow.setup(session, fixture, formats_only=iced_media_formats_only)
    initial_location = fixture
    launch_started = time.monotonic()
    if desktop_entry:
        initial_location = fixture / "Open café folder"
        initial_location.mkdir()
        shutil.copy2(desktop_entry, applications / "org.canopa.Files.desktop")
        session.env["PATH"] = f"{binary.parent}:{session.env['PATH']}"
        session.run(["desktop-file-validate", str(applications / "org.canopa.Files.desktop")])
        session.run(["update-desktop-database", str(applications)])
        session.run(["gio", "mime", "inode/directory", "org.canopa.Files.desktop"])
        # GIO returns after launching. A child subreaper keeps the launched
        # Files process in this fixture's process group and reports its actual
        # exit status as its own, so `process` still observes the application.
        process = session.start([sys.executable, "-c",
                                 "import ctypes,os,subprocess,sys\n"
                                 "assert ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) == 0\n"
                                 "subprocess.run(sys.argv[1:],check=True)\n"
                                 "_, status = os.wait()\n"
                                 "sys.exit(os.waitstatus_to_exitcode(status))\n",
                                 "gio", "open", str(initial_location)], "files.log")
    else:
        if iced_operations_only and iced_operations_case == "focus-expired":
            shim = session.runtime / "delay-activation.so"
            session.run(["cc", "-shared", "-fPIC", "-O2", "-Wall", "-Wextra",
                         str(ROOT / "scripts/desktop-fixture/delay-activation.c"),
                         "-o", str(shim), "-ldl", "-pthread"])
            scoped = {"LD_PRELOAD": str(shim), "WAYLAND_DEBUG": "client",
                      "CANOPA_FOCUS_DELAY_CONTROL": str(session.runtime / "focus-delay-source")}
            previous = {key: session.env.get(key) for key in scoped}
            session.env.update(scoped)
            try:
                process = session.start([str(binary), str(fixture)], "files.log")
            finally:
                for key, value in previous.items():
                    if value is None:
                        session.env.pop(key, None)
                    else:
                        session.env[key] = value
        elif downloads_only or (iced_operations_only and iced_operations_case in ("drops", "drop-exchange", "drop-choices", "drop-stale", "drop-devices")):
            previous = session.env.get("WAYLAND_DEBUG")
            session.env["WAYLAND_DEBUG"] = "client"
            try:
                process = session.start([str(binary), str(fixture)], "files.log")
            finally:
                if previous is None:
                    session.env.pop("WAYLAND_DEBUG")
                else:
                    session.env["WAYLAND_DEBUG"] = previous
        elif iced_operations_only and iced_operations_case == "clipboard-capacity":
            # Constrain only the Files child, not the compositor, input tools or
            # host process. 128 gives an independently known 32-source budget.
            process = session.start([sys.executable, "-c",
                "import os,resource,sys\n"
                "_,hard=resource.getrlimit(resource.RLIMIT_NOFILE)\n"
                "resource.setrlimit(resource.RLIMIT_NOFILE,(128,hard))\n"
                "os.execv(sys.argv[1],sys.argv[1:])\n",
                str(binary), str(fixture)], "files.log")
        else:
            process = session.start([str(binary), str(fixture)], "files.log")

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((window for window in windows() if window.get("title") == value), None)

    def wait_title(value):
        window = session.wait_for(lambda: title(value), value)
        # Title delivery precedes the Wayland configure/focus round trip. Input
        # injected by a fresh virtual keyboard must wait for that transition.
        time.sleep(.25)
        return window

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name])
            time.sleep(.25)

    def ctrl(key):
        session.run(["wtype", "-M", "ctrl", "-k", key, "-m", "ctrl"])
        time.sleep(.25)

    def pixel(name, expected):
        def rendered():
            frame = session.capture(name)
            color = frame.getpixel((frame.width // 2, frame.height // 2))
            return all(abs(a - b) < 12 for a, b in zip(color, expected))
        session.wait_for(rendered, f"rendered preview {name}")

    browser = wait_title(f"{initial_location.name} — Canopa Files")
    if iced_media_only or iced_media_formats_only:
        result = iced_media_workflow.verify(session, fixture, browser, launched, scheme, scale, process,
                                            formats_only=iced_media_formats_only)
        if desktop_entry:
            result["checks"].append("registered folder handler cold-launches a folder containing spaces and Unicode")
            result["desktop_entry"] = str(desktop_entry)
        return result
    if iced_operations_only and iced_operations_case in ("startup", "startup-interrupted"):
        session.capture("startup-first-browser.png")
        return {"status": "passed", "scheme": scheme, "scale": scale,
                "first_browser_capture_seconds": time.monotonic() - launch_started,
                "startup_fixture": "interrupted" if iced_operations_case == "startup-interrupted" else "operations",
                "checks": ["a fresh process maps its real first Browser within the existing readiness deadline"],
                "scope": "first-window startup only; full operation and resident activation acceptance remain separate"}
    if iced_operations_only:
        if iced_operations_case == "drop-choices":
            return operations_workflow.verify(session, fixture, browser, scheme, scale, process, cross_destination)
        if iced_operations_case == "location-editing":
            return operations_workflow.verify(session, fixture, browser, scheme, scale, process,
                                               location_editing_only=True)
        if iced_operations_case == "window-navigation":
            return operations_workflow.verify(session, fixture, browser, scheme, scale, process,
                                               window_navigation_only=True)
        if iced_operations_case in ("clipboard-source-copy", "clipboard-source-move"):
            return operations_workflow.verify(session, fixture, browser, scheme, scale, process, move=iced_operations_case == "clipboard-source-move")
        if iced_operations_case in ("mixed-copy", "mixed-move"):
            return operations_workflow.verify(session, fixture, browser, scheme, scale, process, move=iced_operations_case == "mixed-move")
        if iced_operations_case == "move-conflicts":
            return operations_workflow.verify(session, fixture, browser, scheme, scale, process, move=True)
        if iced_operations_case == "question-focus":
            return operations_workflow.verify(session, fixture, browser, scheme, scale, process, question_focus=True)
        if iced_operations_case == "compact":
            return operations_workflow.verify(session, fixture, browser, scheme, scale, process, compact_only=True)
        if iced_operations_case == "parents":
            return operations_workflow.verify(session, fixture, browser, scheme, scale, process, parent_closure=True)
        if iced_operations_case in ("focus", "focus-expired"):
            return operations_workflow.verify(session, fixture, browser, scheme, scale, process,
                                              focus_only=True, delay_focus=iced_operations_case == "focus-expired")
        return operations_workflow.verify(session, fixture, browser, scheme, scale, process)
    if iced_preferences_only:
        return preferences_workflow.verify(session, fixture, browser, config, scheme, scale, iced_preferences_case, launched=launched)
    if iced_views_only:
        return views_workflow.verify(session, fixture, browser, launched, config, scheme, scale, binary, process, iced_views_case)
    if iced_interface_only:
        return interface_workflow.verify(session, fixture, browser, config, scheme, scale)
    if iced_actions_only:
        return actions_workflow.verify(session, fixture, browser, config, scheme, scale)
    if iced_preview_only:
        workflow_spec = importlib.util.spec_from_file_location("files_iced_previews", ROOT / "scripts/desktop-fixture/files-iced-previews.py")
        workflow = importlib.util.module_from_spec(workflow_spec)
        workflow_spec.loader.exec_module(workflow)
        return workflow.verify(session, fixture, browser, launched, scheme, scale)
    if iced_search_only:
        workflow_spec = importlib.util.spec_from_file_location("files_iced_search", ROOT / "scripts/desktop-fixture/files-iced-search.py")
        workflow = importlib.util.module_from_spec(workflow_spec)
        workflow_spec.loader.exec_module(workflow)
        return workflow.verify(session, fixture, browser, launched, scheme, scale)
    if iced_browsing_only:
        workflow_spec = importlib.util.spec_from_file_location("files_iced_browsing", ROOT / "scripts/desktop-fixture/files-iced-browsing.py")
        workflow = importlib.util.module_from_spec(workflow_spec)
        workflow_spec.loader.exec_module(workflow)
        return workflow.verify(session, fixture, browser, launched, scheme, scale)
    if downloads_only:
        workflow_spec = importlib.util.spec_from_file_location("files_downloads", ROOT / "scripts/desktop-fixture/files-downloads.py")
        workflow = importlib.util.module_from_spec(workflow_spec)
        workflow_spec.loader.exec_module(workflow)
        return workflow.verify(session, fixture, browser, config, scheme, scale)
    if hidden_navigation_only:
        navigation_spec = importlib.util.spec_from_file_location("files_hidden_navigation", ROOT / "scripts/desktop-fixture/files-hidden-navigation.py")
        navigation_fixture = importlib.util.module_from_spec(navigation_spec)
        navigation_spec.loader.exec_module(navigation_fixture)
        return navigation_fixture.verify(session, fixture, browser, scheme, scale)
    if reload_only:
        reload_spec = importlib.util.spec_from_file_location("files_reload", ROOT / "scripts/desktop-fixture/files-reload.py")
        reload_fixture = importlib.util.module_from_spec(reload_spec)
        reload_spec.loader.exec_module(reload_fixture)
        return reload_fixture.verify(session, config, fixture, scheme, scale)
    session.capture("01-wide.png")
    keys("Down", "Right")
    wait_title("A-project — Canopa Files")
    keys("Left")
    wait_title("Downloads — Canopa Files")
    keys("Down", "space")
    preview = wait_title("Quick Look: B-notes.txt — Canopa Files")
    assert preview["is_floating"], preview
    assert preview["layout"]["window_size"][0] >= 600, preview
    session.capture("02-preview.png")
    keys("Down")
    wait_title("Quick Look: C-photo.png — Canopa Files")
    pixel("03-image.png", (24, 160, 120))
    keys("Down")
    wait_title("Quick Look: D-document.pdf — Canopa Files")
    pixel("04-pdf-first-page.png", (255, 0, 0))
    keys("Page_Down")
    pixel("05-pdf-second-page.png", (0, 255, 0))

    # Another Browser Window must not claim this preview's selection.
    ctrl("n")
    other = session.wait_for(lambda: next((window for window in windows() if window["id"] != browser["id"] and window.get("title") == "Downloads — Canopa Files"), None), "second browser")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(other["id"])])
    time.sleep(.25)
    keys("Down")
    assert title("Quick Look: D-document.pdf — Canopa Files"), windows()
    ctrl("w")
    session.wait_for(lambda: all(window["id"] != other["id"] for window in windows()), "second browser closed")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(preview["id"])])
    time.sleep(.25)
    keys("Escape")
    session.wait_for(lambda: len(windows()) == 1, "preview dismissed")

    # Collapse to one column, retain selection and navigate with the same keys.
    session.run(["niri", "msg", "action", "set-column-width", "360"])
    time.sleep(.5)
    assert title("Downloads — Canopa Files")["layout"]["window_size"][0] == 360
    keys("space")
    narrow_preview = wait_title("Quick Look: D-document.pdf — Canopa Files")
    assert narrow_preview["is_floating"] and narrow_preview["layout"]["window_size"][0] >= 600
    session.capture("06-narrow-preview.png")
    keys("Escape", "Left")
    wait_title("home — Canopa Files")
    keys("Right")
    wait_title("Downloads — Canopa Files")
    session.capture("07-narrow-return.png")

    # Each view uses the same selection, including after restoring columns.
    for number in ("2", "3", "1"):
        ctrl(number)
        keys("space")
        wait_title("Quick Look: D-document.pdf — Canopa Files")
        keys("Escape")
    ctrl("t")
    ctrl("l")
    session.run(["wtype", str(fixture / "A-project")])
    keys("Return")
    wait_title("A-project — Canopa Files")
    keys("Down", "space")
    wait_title("Quick Look: notes.rs — Canopa Files")
    session.capture("08-code-preview.png")
    keys("Escape")
    ctrl("w")
    wait_title("Downloads — Canopa Files")
    keys("space")
    wait_title("Quick Look: D-document.pdf — Canopa Files")
    keys("Escape")
    assert process.poll() is None
    session.run(["niri", "msg", "action", "set-column-width", "100%"])
    time.sleep(.4)
    ctrl("f")
    session.run(["wtype", "document-17"])
    time.sleep(.6)
    keys("Down", "space")
    wait_title("Quick Look: document-17.txt — Canopa Files")
    session.capture("09-search-preview.png")
    keys("Escape", "Escape", "space")
    wait_title("Quick Look: D-document.pdf — Canopa Files")
    keys("Escape")

    # A real filesystem event refreshes the listing without explicit reload.
    (fixture / "new-arrival.txt").write_text("arrived while browsing\n")
    time.sleep(.4)
    session.run(["wtype", "new-arrival"])
    keys("space")
    wait_title("Quick Look: new-arrival.txt — Canopa Files")
    keys("Escape")
    keys("Return")
    session.wait_for(launched.exists, "default application launch")
    assert json.loads(launched.read_text()) == [str(fixture / "new-arrival.txt")]
    # Native name prompts run real mutations on a worker and share durable Undo.
    session.run(["wtype", "-M", "ctrl", "-M", "shift", "-k", "n", "-m", "shift", "-m", "ctrl"])
    wait_title("New folder — Canopa Files")
    session.run(["wtype", "Created in Files"])
    keys("Return")
    created = fixture / "Created in Files"
    session.wait_for(created.is_dir, "created folder")
    time.sleep(.3)
    keys("F2")
    wait_title("Rename — Canopa Files")
    session.run(["wtype", "Renamed in Files"])
    keys("Return")
    renamed = fixture / "Renamed in Files"
    session.wait_for(renamed.is_dir, "renamed folder")
    assert not created.exists()
    # A second invocation forwards to the primary process; Undo is shared even
    # when invoked in a window that did not perform the original operation.
    session.run([str(binary), str(fixture)])
    other = session.wait_for(lambda: next((window for window in windows() if window["id"] != browser["id"] and window.get("title") == "Downloads — Canopa Files"), None), "remote invocation window")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(other["id"])])
    time.sleep(.3)
    ctrl("z")
    session.wait_for(created.is_dir, "undo rename")
    assert not renamed.exists()
    time.sleep(.3)
    ctrl("z")
    session.wait_for(lambda: not created.exists(), "undo creation")
    ctrl("w")
    session.wait_for(lambda: len(windows()) == 1, "remote window closed")
    session.capture("10-operations.png")
    # File clipboard data crosses Browser Windows and uses the transfer engine.
    source = fixture / "Clipboard source"
    target = fixture / "Clipboard target"
    source.mkdir()
    target.mkdir()
    (source / "copied.txt").write_text("clipboard payload\n")

    def navigate(path):
        ctrl("l")
        session.run(["wtype", str(path)])
        keys("Return")
        wait_title(f"{path.name} — Canopa Files")

    navigate(source)
    keys("Down")
    ctrl("c")
    clipboard = session.run(["wl-paste", "--no-newline", "--type", "x-special/gnome-copied-files"])
    assert clipboard == f"copy\n{(source / 'copied.txt').as_uri()}\n", clipboard
    navigate(target)
    ctrl("v")
    session.wait_for(lambda: (target / "copied.txt").exists(), "pasted file")
    assert (target / "copied.txt").read_text() == "clipboard payload\n"
    assert (source / "copied.txt").exists()
    ctrl("z")
    session.wait_for(lambda: not (target / "copied.txt").exists(), "undo clipboard copy")
    navigate(source)
    keys("Down")
    ctrl("x")
    session.run([str(binary), str(target)])
    destination_window = wait_title("Clipboard target — Canopa Files")
    ctrl("v")
    session.wait_for(lambda: (target / "copied.txt").exists() and not (source / "copied.txt").exists(), "inter-window clipboard move")
    ctrl("z")
    session.wait_for(lambda: (source / "copied.txt").exists() and not (target / "copied.txt").exists(), "undo clipboard move")
    ctrl("j")
    wait_title("Operations — Canopa Files")
    session.capture("11-shared-operations.png")
    keys("Escape")
    # Conflicts, waiting decisions, isolated failures and Retry, unavailable
    # destinations and Trash belong to the Iced cases files-iced-conflicts,
    # -continuation, -unavailable and -trash. The former GTK walkthrough here
    # used mnemonics, GApplication actions and notification actions that the
    # Iced frontend does not provide.
    log = (output / "files.log").read_text()
    assert "panicked" not in log and "CRITICAL" not in log and "Gtk-WARNING" not in log, log
    # ADR0171: view shortcuts change only the tab's temporary view and never
    # save folder views, so this walkthrough leaves no views.json to inspect.
    browsing_checks = [
        'wide/narrow keyboard navigation',
        'floating independent preview size',
        'source-bound preview',
        'image pixels',
        'PDF paging pixels',
        'independent tab locations',
        'view selection preservation',
        'filename search and restoration',
        'live folder updates',
        'default application receives real file path',
        'native create/rename/undo',
        'remote invocation and shared Undo',
    ]
    transfer_checks = [
        'native file clipboard copy/move across windows and Undo',
        'shared Operations view',
    ]
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "checks": browsing_checks + transfer_checks,
            "windows": windows()}


def main():
    def interrupted(signum, frame):
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        raise InterruptedError("Files acceptance received SIGTERM")

    signal.signal(signal.SIGTERM, interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/canopa-files")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scheme", choices=("dark", "light"), default="dark")
    parser.add_argument("--scale", type=float, default=1.0)
    workflow = parser.add_mutually_exclusive_group()
    parser.add_argument("--iced-operations-case", choices=("full", "startup", "startup-interrupted", "window-navigation", "location-editing", "trash", "recovery", "recovery-directories", "interrupted", "clipboard", "clipboard-lifetime", "source-leases", "clipboard-source-copy", "clipboard-source-move", "clipboard-capacity", "drops", "drop-exchange", "drop-choices", "drop-stale", "drop-devices", "transfers", "continuation", "dismissal", "transfer-completion", "claim-review", "prepared-rename", "focus", "focus-expired", "parents", "compact", "conflicts", "question-focus", "unavailable", "mixed-copy", "mixed-move", "move-conflicts"), default="full", help="Select name/Undo, clipboard, transfer inspection or conflict acceptance")
    workflow.add_argument("--iced-operations-only", action="store_true", help="Verify floating Iced name prompts, actual mutations and shared Undo")
    parser.add_argument("--iced-preferences-case", choices=("full", "toolbar-fast", "locations"), default="full", help="Select full preferences, toolbar activation or actual XDG Locations navigation")
    workflow.add_argument("--iced-preferences-only", action="store_true", help="Verify transactional Iced global Views/Toolbar/Locations/Shortcuts editing and raw bookmarks")
    parser.add_argument("--iced-views-case", choices=("full", "reflow", "reflow-prefix", "reflow-stress", "gutter"), default="full", help="Select full folder-view acceptance or a focused reflow/gutter regression")
    workflow.add_argument("--iced-views-only", action="store_true", help="Verify actual Iced list/icon layouts, sparse folder preferences, reflow, persistence and failed-save Retry")
    workflow.add_argument("--iced-interface-only", action="store_true", help="Verify editing focus/caret across tiling adaptation and external configuration notices")
    workflow.add_argument("--iced-actions-only", action="store_true", help="Verify Iced action drafts, saving, captured command arguments, output and lifetime")
    workflow.add_argument("--iced-browsing-only", action="store_true", help="Verify the Iced migration's browsing slice; excludes unported features")
    workflow.add_argument("--iced-preview-only", action="store_true", help="Verify Iced transient preview ownership, text, images and PDF controls; media migration remains separate")
    workflow.add_argument("--iced-media-only", action="store_true", help="Verify native Iced media controls, decoded video and isolated recorded PCM")
    workflow.add_argument("--iced-media-formats-only", action="store_true", help="Verify native Iced MP3, FLAC, Opus, AAC and H.264 playback and seeking")
    workflow.add_argument("--iced-search-only", action="store_true", help="Verify the Iced filename search interface, restoration, cancellation and result budgets")
    workflow.add_argument("--downloads-only", action="store_true", help="Run the integrated Downloads preview, tiled move, Undo and image-action walkthrough")
    workflow.add_argument("--hidden-navigation-only", action="store_true", help="Run hidden ancestor and parent-selection navigation at wide and narrow widths")
    workflow.add_argument("--reload-only", action="store_true", help="Run automatic shared configuration reload acceptance")
    parser.add_argument("--desktop-entry", type=Path, help="Cold-launch --iced-media-formats-only through this registered folder application entry")
    args = parser.parse_args()
    if args.iced_views_case != "full" and not args.iced_views_only:
        parser.error("--iced-views-case requires --iced-views-only")
    if args.iced_preferences_case != "full" and not args.iced_preferences_only:
        parser.error("--iced-preferences-case requires --iced-preferences-only")
    if args.iced_media_only or args.iced_media_formats_only:
        # Keep decoder assertion evidence in the fixture log without allowing
        # system core collection to fill the host disk or stall failure cleanup.
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    if args.desktop_entry:
        if not args.iced_media_formats_only:
            parser.error("--desktop-entry requires --iced-media-formats-only")
        args.desktop_entry = args.desktop_entry.resolve()
    if args.iced_operations_case != "full" and not args.iced_operations_only:
        parser.error("--iced-operations-case requires --iced-operations-only")
    for command in ("pdfinfo", "pdftoppm", "wl-copy", "wl-paste", "gio"):
        if not shutil.which(command):
            parser.error(f"{command} is required for Files acceptance")
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="canopa-files-native-") as directory:
        runtime = Path(directory)
        runtime.chmod(0o700)
        session = desktop.Session(runtime, args.output.resolve())
        result = {"status": "failed"}
        cross_destination = None
        try:
            if args.iced_operations_only and args.iced_operations_case == "drop-choices":
                for parent in (ROOT, Path("/dev/shm")):
                    if parent.exists() and parent.stat().st_dev != runtime.stat().st_dev:
                        cross_destination = tempfile.TemporaryDirectory(prefix=".canopa-native-cross-", dir=parent)
                        break
                assert cross_destination, "cross-filesystem acceptance requires two writable filesystems"
            result = verify(args.binary.resolve(), session, args.scheme, args.scale, args.reload_only, args.desktop_entry, Path(cross_destination.name) if cross_destination else None, args.hidden_navigation_only, args.downloads_only, iced_browsing_only=args.iced_browsing_only, iced_search_only=args.iced_search_only, iced_preview_only=args.iced_preview_only, iced_actions_only=args.iced_actions_only, iced_interface_only=args.iced_interface_only, iced_views_only=args.iced_views_only, iced_preferences_only=args.iced_preferences_only, iced_preferences_case=args.iced_preferences_case, iced_views_case=args.iced_views_case, iced_operations_only=args.iced_operations_only, iced_operations_case=args.iced_operations_case, iced_media_only=args.iced_media_only, iced_media_formats_only=args.iced_media_formats_only)
        except Exception as error:
            result["error"] = str(error)
            try:
                result["windows"] = json.loads(session.run(["niri", "msg", "--json", "windows"]))
                session.capture("failure.png")
            except Exception:
                pass
            raise
        finally:
            candidate = session.runtime / "config/canopa/config.kdl"
            if args.iced_preferences_only and candidate.exists():
                (session.output / "preferences-config.kdl").write_text(candidate.read_text())
            if args.reload_only and candidate.exists():
                (session.output / "reload-config.kdl").write_text(candidate.read_text())
            if args.iced_operations_only and (session.runtime / "volumes/events").exists():
                (session.output / "device-events.log").write_text((session.runtime / "volumes/events").read_text())
            (session.output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
            session.close()
            if cross_destination:
                cross_path = Path(cross_destination.name)
                cross_destination.cleanup()
                assert not cross_path.exists(), cross_path
                result["private_destination_removed"] = True
                if result["status"] == "passed":
                    result["checks"].append("private secondary filesystem directory removed after session shutdown")
                (session.output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
