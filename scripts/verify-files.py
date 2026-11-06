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


def verify(binary, session, scheme, scale, transfers_only=False, actions_only=False, drops_only=False, preferences_only=False, undo_only=False, recovery_only=False, interrupted_only=False, reload_only=False, devices_only=False, device_case="browsing", media_only=False, media_formats_only=False, desktop_entry=None, undo_moves_only=False, undo_saved_only=False, undo_cross_only=False, cross_destination=None, undo_cross_case="original", hidden_navigation_only=False, undo_folders_only=False, undo_folders_case="nonempty", undo_directories_only=False, recovery_directories_only=False, recovery_directories_case="basic", downloads_only=False, *, transfer_case="standard", iced_browsing_only=False, iced_search_only=False, iced_preview_only=False, iced_actions_only=False, iced_interface_only=False, iced_views_only=False, iced_preferences_only=False, iced_preferences_case="full", iced_views_case="full", iced_operations_only=False, iced_operations_case="full", iced_media_only=False, iced_media_formats_only=False):
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
    device_recovery = devices_only and device_case == "recovery"
    if devices_only or iced_operations_only or iced_preview_only or downloads_only:
        modules = output / "gio-modules"
        modules.mkdir()
        flags = subprocess.check_output(["pkg-config", "--cflags", "--libs", "gio-2.0"], text=True).split()
        subprocess.run(["cc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
                        str(ROOT / "scripts/desktop-fixture/files-volume-monitor.c"),
                        "-o", str(modules / "libcanopa-volume-fixture.so"), *flags], check=True, timeout=30)
        media = session.runtime / "volumes"
        (media / "Fixture USB").mkdir(parents=True)
        (media / "Fixture USB/Drive notes.txt").write_text("A real file on the private fixture volume.\n")
        if device_case == "identityless":
            (media / "no-uuid").touch()
        session.env.update(GIO_EXTRA_MODULES=str(modules), GIO_USE_VOLUME_MONITOR="unix",
                           GIO_USE_VFS="local", CANOPA_FIXTURE_VOLUMES=str(media))
        inventory = session.run(["gio", "mount", "--list", "--detail"])
        (output / "gio-volumes.txt").write_text(inventory)
        assert "Fixture USB" in inventory and "FixtureVolume" in inventory, inventory
    if media_only or media_formats_only:
        media_spec = importlib.util.spec_from_file_location("files_media", ROOT / "scripts/desktop-fixture/files-media.py")
        media_fixture = importlib.util.module_from_spec(media_spec)
        media_spec.loader.exec_module(media_fixture)
        media_fixture.setup(session, fixture, formats_only=media_formats_only)
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
        # GIO returns after launching. Keep its isolated process group owned by
        # a helper until session cleanup, including on a failed native assertion.
        process = session.start([sys.executable, "-c",
                                 "import signal,subprocess,sys\nsubprocess.run(sys.argv[1:],check=True)\nsignal.pause()\n",
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
        return iced_media_workflow.verify(session, fixture, browser, launched, scheme, scale, process,
                                          formats_only=iced_media_formats_only)
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
    if transfers_only and transfer_case in ("lifetime", "lifetime-live"):
        workflow_spec = importlib.util.spec_from_file_location("files_job_lifetime", ROOT / "scripts/desktop-fixture/files-job-lifetime.py")
        workflow = importlib.util.module_from_spec(workflow_spec)
        workflow_spec.loader.exec_module(workflow)
        return workflow.verify(session, fixture, browser, scheme, scale, binary, process, keep_open=transfer_case == "lifetime-live")
    if downloads_only:
        workflow_spec = importlib.util.spec_from_file_location("files_downloads", ROOT / "scripts/desktop-fixture/files-downloads.py")
        workflow = importlib.util.module_from_spec(workflow_spec)
        workflow_spec.loader.exec_module(workflow)
        return workflow.verify(session, fixture, browser, config, scheme, scale)
    if recovery_directories_only:
        directory_spec = importlib.util.spec_from_file_location("files_recovery_directories", ROOT / "scripts/desktop-fixture/files-recovery-directories.py")
        directory_fixture = importlib.util.module_from_spec(directory_spec)
        directory_spec.loader.exec_module(directory_fixture)
        return directory_fixture.verify(session, fixture, browser, scheme, scale, expiry=recovery_directories_case == "expiry", tiling=recovery_directories_case == "tiling")
    if undo_directories_only:
        directory_spec = importlib.util.spec_from_file_location("files_undo_directories", ROOT / "scripts/desktop-fixture/files-undo-directories.py")
        directory_fixture = importlib.util.module_from_spec(directory_spec)
        directory_spec.loader.exec_module(directory_fixture)
        return directory_fixture.verify(session, fixture, browser, scheme, scale)
    if undo_folders_only:
        folder_spec = importlib.util.spec_from_file_location("files_undo_folders", ROOT / ("scripts/desktop-fixture/files-undo-folder-locations.py" if undo_folders_case == "locations" else "scripts/desktop-fixture/files-undo-folders.py"))
        folder_fixture = importlib.util.module_from_spec(folder_spec)
        folder_spec.loader.exec_module(folder_fixture)
        return folder_fixture.verify(session, fixture, browser, scheme, scale)
    if hidden_navigation_only:
        navigation_spec = importlib.util.spec_from_file_location("files_hidden_navigation", ROOT / "scripts/desktop-fixture/files-hidden-navigation.py")
        navigation_fixture = importlib.util.module_from_spec(navigation_spec)
        navigation_spec.loader.exec_module(navigation_fixture)
        return navigation_fixture.verify(session, fixture, browser, scheme, scale)
    if undo_cross_only:
        cross_scripts = {"original": "files-undo-cross.py", "current": "files-undo-cross-current.py", "directories": "files-undo-cross-directories.py", "moves": "files-undo-cross-directories.py", "retry": "files-directory-retry.py", "context": "files-recovery-context.py"}
        cross_spec = importlib.util.spec_from_file_location("files_undo_cross", ROOT / "scripts/desktop-fixture" / cross_scripts[undo_cross_case])
        cross_fixture = importlib.util.module_from_spec(cross_spec)
        cross_spec.loader.exec_module(cross_fixture)
        if undo_cross_case == "moves":
            return cross_fixture.verify(session, fixture, browser, cross_destination, scheme, scale, directory_moves=True)
        if undo_cross_case == "context":
            return cross_fixture.verify(session, fixture, browser, cross_destination, scheme, scale, binary, process)
        return cross_fixture.verify(session, fixture, browser, cross_destination, scheme, scale)
    if undo_saved_only:
        saved_spec = importlib.util.spec_from_file_location("files_undo_saved", ROOT / "scripts/desktop-fixture/files-undo-saved.py")
        saved_fixture = importlib.util.module_from_spec(saved_spec)
        saved_spec.loader.exec_module(saved_fixture)
        return saved_fixture.verify(session, fixture, browser, scheme, scale)
    if undo_moves_only:
        move_spec = importlib.util.spec_from_file_location("files_undo_moves", ROOT / "scripts/desktop-fixture/files-undo-moves.py")
        move_fixture = importlib.util.module_from_spec(move_spec)
        move_spec.loader.exec_module(move_fixture)
        return move_fixture.verify(session, fixture, browser, scheme, scale)
    if media_only or media_formats_only:
        result = media_fixture.verify(session, fixture, scheme, scale, formats_only=media_formats_only)
        if desktop_entry:
            result["checks"].append("registered folder handler cold-launches a folder containing spaces and Unicode")
            result["desktop_entry"] = str(desktop_entry)
        return result
    if devices_only and not device_recovery:
        device_script = ("files-devices.py" if device_case == "browsing" else
                         "files-device-transfers.py" if device_case in ("inflight", "sources") else
                         "files-device-clipboard.py" if device_case == "clipboard" else
                         "files-device-safety.py")
        device_spec = importlib.util.spec_from_file_location("files_devices", ROOT / "scripts/desktop-fixture" / device_script)
        device_fixture = importlib.util.module_from_spec(device_spec)
        device_spec.loader.exec_module(device_fixture)
        if device_case == "clipboard":
            return device_fixture.verify(session, config, fixture, scheme, scale, binary, process, cross_destination)
        if device_case == "browsing":
            return device_fixture.verify(session, config, fixture, scheme, scale, binary, process)
        if device_case in ("inflight", "sources"):
            return device_fixture.verify(session, config, fixture, scheme, scale, device_case)
        return device_fixture.verify(session, config, fixture, scheme, scale, binary, process, identityless=device_case == "identityless")
    if reload_only:
        reload_spec = importlib.util.spec_from_file_location("files_reload", ROOT / "scripts/desktop-fixture/files-reload.py")
        reload_fixture = importlib.util.module_from_spec(reload_spec)
        reload_spec.loader.exec_module(reload_fixture)
        return reload_fixture.verify(session, config, fixture, scheme, scale)
    if interrupted_only or device_recovery:
        def focus(window):
            session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
            session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "interrupted workflow focus")
            time.sleep(.3)

        def events():
            return json.loads(notifications.read_text()) if notifications.exists() else []

        def completed(previous):
            return any(event["body"].startswith("Completed") for event in events()[previous:])

        def clipboard_ready():
            try:
                return "text/uri-list" in session.run(["wl-paste", "--list-types"])
            except subprocess.CalledProcessError:
                return False

        target = (session.runtime / "volumes/Fixture USB" if device_recovery else fixture) / "Interrupted target"
        target.mkdir()
        source = fixture / "report.txt"
        line = b"Source data preserved for recovery.\n"
        source.write_bytes(b"Preserved from interrupted Undo.\n" + line * (16 * 1024 * 1024 // len(line)))
        original = target / source.name
        session.start(["wl-copy", "--foreground", "--type", "text/uri-list", f"{source.as_uri()}\r\n"], "clipboard-owner.log")
        session.wait_for(clipboard_ready, "interrupted workflow clipboard")
        ctrl("l"); session.run(["wtype", str(target)]); keys("Return")
        browser = wait_title("Interrupted target — Canopa Files")
        before = len(events()); ctrl("v")
        session.wait_for(lambda: completed(before), "copy before interrupted Undo")
        assert original.read_bytes() == source.read_bytes()
        evidence = output / "interrupted-process.json"
        watcher = session.start([sys.executable, str(ROOT / "scripts/desktop-fixture/files-interrupted.py"), str(process.pid), str(target), str(evidence)], "interruption-watcher.log")
        # Session normally fails on any child exit. These two exits are the
        # fixture's stimulus; restore live children to cleanup on an early error.
        session.processes.remove(process)
        session.processes.remove(watcher)
        try:
            ctrl("z")
            session.wait_for(lambda: process.poll() is not None and watcher.poll() is not None, "actual termination during Undo", timeout=50)
        finally:
            for child in (process, watcher):
                if child.poll() is None:
                    session.processes.append(child)
        assert watcher.returncode == 0 and process.returncode == -signal.SIGKILL
        quarantine = Path(json.loads(evidence.read_text())["quarantine"])
        assert quarantine.exists()
        if device_recovery:
            preserved = quarantine.read_bytes()
            (session.runtime / "volumes/unmounted").touch()
            process = session.start([str(binary), str(target)], "files-interrupted-offline.log")
            browser = wait_title("Interrupted target — Canopa Files")
            assert quarantine.read_bytes() == preserved, "startup replay mutated the unavailable device's backing folder"
            assert not original.exists(), "startup replay restored into the unavailable backing folder"
            recovery = wait_title("Recovery — Canopa Files")
            focus(recovery); time.sleep(.5)
            session.capture("device-recovery-01-offline.png")
            focus(browser)
            ctrl("l"); keys("Tab", "Tab", "Tab", "space"); time.sleep(.3)
            keys("space", *(["Down"] * 7), "space")
            wait_title("Fixture USB — Canopa Files")
            focus(recovery); before = len(events())
            session.run(["wtype", "-M", "alt", "-k", "i", "-m", "alt"])
            session.wait_for(lambda: completed(before), "explicit recovery retry after remount", timeout=50)
            assert original.read_bytes() == source.read_bytes()
            assert not quarantine.exists()
            time.sleep(.5); session.capture("device-recovery-02-retried.png")
            before = len(events()); ctrl("z")
            session.wait_for(lambda: completed(before), "Undo reconciled copy", timeout=50)
            assert not original.exists()
            assert source.read_bytes().startswith(b"Preserved from interrupted Undo.\n")
            return {"status": "passed", "scheme": scheme, "scale": scale, "checks": [
                "startup leaves interrupted data untouched on an unavailable device",
                "explicit recovery retry after remount reconciles the actual journal",
                "Undo remains usable after guarded startup recovery"], "windows": windows()}
        original.write_text("Newer occupant after interruption.\n")
        process = session.start([str(binary), str(target)], "files-interrupted.log")
        browser = wait_title("Interrupted target — Canopa Files")
        recovery = wait_title("Recovery — Canopa Files")
        focus(recovery); time.sleep(.5); keys("Down")
        session.capture("interrupted-01-attention.png")
        keys("space"); wait_title("report.txt — Quick Look")
        def preview_rendered():
            frame = session.capture("interrupted-02-preview.png").convert("RGB")
            # First text rows at the left of the centered preview. The centered
            # loading caption is outside this region; title existence is not
            # proof that the actual retained contents have rendered.
            region = frame.crop((round(frame.width * .225), round(frame.height * .205),
                                 round(frame.width * .445), round(frame.height * .255)))
            channels = region.tobytes()
            visible = sum((sum(pixel) > 360 if scheme == "dark" else sum(pixel) < 375)
                          for pixel in zip(channels[0::3], channels[1::3], channels[2::3]))
            return visible > 80
        session.wait_for(preview_rendered, "rendered quarantined file contents", timeout=30)
        keys("Escape"); focus(recovery); ctrl("r"); wait_title("Recover a copy")
        rescued = fixture / "Rescued report.txt"
        ctrl("a"); session.run(["wtype", str(rescued)]); before = len(events()); keys("Return")
        session.wait_for(rescued.exists, "copy quarantined data", timeout=50)
        session.wait_for(lambda: completed(before), "journaled recovery copy", timeout=50)
        assert rescued.read_bytes() == source.read_bytes()
        assert original.read_text() == "Newer occupant after interruption.\n" and quarantine.exists()
        # Restarting after copy-out must still report the original pending Undo.
        for window in windows():
            if window.get("pid") == process.pid:
                session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
        session.processes.remove(process)
        try:
            session.wait_for(lambda: process.poll() is not None, "exit before pending-journal restart")
        finally:
            if process.poll() is None:
                session.processes.append(process)
        process = session.start([str(binary), str(target)], "files-interrupted-restarted.log")
        browser = wait_title("Interrupted target — Canopa Files")
        recovery = wait_title("Recovery — Canopa Files")
        focus(recovery); time.sleep(.5); keys("Down")
        session.run(["niri", "msg", "action", "set-window-width", "360"])
        time.sleep(.8)
        def narrow_toolbar_rendered():
            frame = session.capture("interrupted-03-narrow-restarted.png").convert("RGB")
            region = frame.crop(tuple(round(value * scale) for value in (15, 45, 230, 80)))
            channels = region.tobytes()
            visible = sum((sum(pixel) > 360 if scheme == "dark" else sum(pixel) < 375)
                          for pixel in zip(channels[0::3], channels[1::3], channels[2::3]))
            return visible > 80
        session.wait_for(narrow_toolbar_rendered, "visible Recovery toolbar after narrowing", timeout=3)
        assert title("Recovery — Canopa Files")["layout"]["window_size"][0] == 360
        assert quarantine.exists()
        saved_newer = fixture / "Saved newer report.txt"
        original.rename(saved_newer)
        before = len(events())
        session.run(["wtype", "-M", "alt", "-k", "i", "-m", "alt"])
        session.wait_for(lambda: completed(before), "native retry of interrupted operation", timeout=50)
        session.wait_for(lambda: not quarantine.exists(), "retry releases quarantine")
        assert original.read_bytes() == source.read_bytes()
        time.sleep(.8)  # Catalog refresh follows the filesystem completion event.
        session.capture("interrupted-04-reconciled.png")
        focus(recovery); before = len(events()); ctrl("z")
        session.wait_for(lambda: completed(before), "Undo recovered copy", timeout=50)
        assert not rescued.exists() and original.exists()
        before = len(events()); ctrl("z")
        session.wait_for(lambda: completed(before), "Undo original copy after retry", timeout=50)
        assert not original.exists()
        assert saved_newer.read_text() == "Newer occupant after interruption.\n"
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": [
            "actual process termination after native Undo quarantine",
            "startup Recovery keeps interrupted history accessible",
            "original-name Quick Look of quarantined data",
            "native journaled recovery copy preserves both originals",
            "pending Undo survives copy-out and a second process restart",
            "360-pixel interrupted-operation details and controls",
            "native Retry reconciles after occupied path is cleared",
            "recovery copy and original copy remain separately undoable",
        ], "windows": windows()}
    if recovery_only:
        def clipboard_ready():
            try:
                return "text/uri-list" in session.run(["wl-paste", "--list-types"])
            except subprocess.CalledProcessError:
                # The private clipboard starts empty; wait for wl-copy to own it.
                return False

        def focus(window):
            session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
            session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "Recovery workflow keyboard focus")
            time.sleep(.3)

        def events():
            return json.loads(notifications.read_text()) if notifications.exists() else []

        def completed(previous):
            return any(event["body"].startswith("Completed") for event in events()[previous:])

        target = fixture / "Recovery target"
        target.mkdir()
        previous = target / "Retained report.txt"
        previous.write_text("Previous document for recovery.\n")
        source = fixture / "Retained report.txt"
        source.write_text("Incoming replacement.\n")
        # Keep the actual old file open as an editor could; later writes must
        # remain recoverable after the destination was replaced.
        document = previous.open("r+")
        session.start(["wl-copy", "--foreground", "--type", "text/uri-list", f"{source.as_uri()}\r\n"], "clipboard-owner.log")
        session.wait_for(clipboard_ready, "recovery source clipboard")
        ctrl("l"); session.run(["wtype", str(target)]); keys("Return")
        browser = wait_title("Recovery target — Canopa Files")
        ctrl("v"); wait_title("File conflict — Canopa Files")
        before = len(events())
        session.run(["wtype", "-M", "alt", "-k", "r", "-m", "alt"])
        session.wait_for(lambda: completed(before), "replacement before Recovery")
        assert previous.read_text() == "Incoming replacement.\n"
        session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
        recovery = wait_title("Recovery — Canopa Files")
        time.sleep(.5); keys("Down")
        session.capture("recovery-01-undo-backup.png")
        keys("space"); wait_title("Retained report.txt — Quick Look")
        session.capture("recovery-02-preview.png")
        keys("Escape")
        # The native save chooser supplies a new path; no data leaves the
        # private fixture. The original backup must stay available to Undo.
        ctrl("r"); wait_title("Recover a copy")
        recovered = fixture / "Recovered report.txt"
        ctrl("a"); session.run(["wtype", str(recovered)]); before = len(events()); keys("Return")
        session.wait_for(recovered.exists, "copy out from Recovery")
        session.wait_for(lambda: completed(before), "completed recovery copy")
        wait_title("Operations — Canopa Files")
        assert recovered.read_text() == "Previous document for recovery.\n"
        focus(recovery); before = len(events()); ctrl("z")
        session.wait_for(lambda: completed(before), "Undo recovery copy")
        assert not recovered.exists() and previous.read_text() == "Incoming replacement.\n"
        # Refreshing detects newer work through the pre-replacement open file.
        document.seek(0); document.write("New work in the retained version.\n"); document.truncate(); document.flush(); os.fsync(document.fileno())
        keys("F5"); time.sleep(.5); keys("Down")
        session.capture("recovery-03-protected-version.png")
        ctrl("r"); wait_title("Recover a copy")
        ctrl("a"); session.run(["wtype", str(recovered)]); before = len(events()); keys("Return")
        session.wait_for(recovered.exists, "copy changed retained version")
        session.wait_for(lambda: completed(before), "completed changed recovery copy")
        assert recovered.read_text() == "New work in the retained version.\n"
        assert previous.read_text() == "Incoming replacement.\n"
        document.close()
        focus(recovery)
        session.run(["niri", "msg", "action", "set-window-width", "360"])
        time.sleep(.4)
        assert title("Recovery — Canopa Files")["layout"]["window_size"][0] == 360
        session.capture("recovery-04-narrow.png")
        # Closing every window lets the app exit. Reopen the same private state
        # and inspect/recover again through the catalog after a real restart.
        for window in windows():
            if window.get("pid") == process.pid:
                session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
        session.processes.remove(process)
        session.wait_for(lambda: process.poll() is not None, "Files exits before recovery restart")
        process = session.start([str(binary), str(target)], "files-restarted.log")
        browser = wait_title("Recovery target — Canopa Files")
        session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
        recovery = wait_title("Recovery — Canopa Files")
        time.sleep(.5); keys("Down", "space"); wait_title("Retained report.txt — Quick Look")
        session.capture("recovery-05-restarted-preview.png")
        keys("Escape", "Escape")
        focus(browser); keys("Down"); before = len(events()); keys("Delete")
        session.wait_for(lambda: completed(before), "Trash replacement after Recovery restart")
        assert not previous.exists()
        session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
        wait_title("Recovery — Canopa Files")
        time.sleep(.5); keys("Down", "space"); wait_title("Retained report.txt — Quick Look")
        session.capture("recovery-06-after-trash.png")
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": ["native Recovery catalog", "source-named retained preview", "native choose-name copy-out", "Undo copy leaves retained backup", "modified retained data stays protected", "copy changed retained version", "360-pixel Recovery window", "Recovery and preview after process restart", "original backup name and preview after replacement is trashed"], "windows": windows()}
    if undo_only:
        def focus(window):
            session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
            session.wait_for(lambda: any(item["id"] == window["id"] and item["is_focused"] for item in windows()), "Undo workflow keyboard focus")
            time.sleep(.3)

        def chord(mods, key):
            session.run(["wtype", *sum((["-M", mod] for mod in mods), []), "-k", key,
                         *sum((["-m", mod] for mod in reversed(mods)), [])])
            time.sleep(.45)

        def events():
            return json.loads(notifications.read_text()) if notifications.exists() else []

        def completed_since(previous):
            return any(event["body"].startswith("Completed") for event in events()[previous:])

        original = fixture / "Undo original.txt"
        renamed = fixture / "Renamed for Undo.txt"
        original.write_text("original contents\n")
        time.sleep(.5)
        session.run(["wtype", "Undo"])
        keys("F2"); wait_title("Rename")
        session.run(["wtype", renamed.name])
        before = len(events()); keys("Return")
        session.wait_for(lambda: completed_since(before), "completed native rename")
        assert renamed.read_text() == "original contents\n"
        original.write_text("newer work stays at this name\n")
        time.sleep(.4)
        keys("space"); wait_title("Renamed for Undo.txt — Quick Look")
        ctrl("z"); wait_title("Undo conflict — Canopa Files")
        session.capture("undo-01-conflict.png")
        chord(["alt"], "s")
        session.wait_for(lambda: title("Undo conflict — Canopa Files") is None, "skipped Undo conflict")
        session.wait_for(lambda: any(event["body"].startswith("Partly undone") for event in events()), "partial Undo presentation")
        assert original.read_text() == "newer work stays at this name\n" and renamed.exists()
        ctrl("z"); wait_title("Undo conflict — Canopa Files")
        keys("Escape")
        session.wait_for(lambda: title("Undo conflict — Canopa Files") is None, "cancelled Undo conflict")
        assert original.read_text() == "newer work stays at this name\n" and renamed.exists()
        time.sleep(.3)
        ctrl("z"); wait_title("Undo conflict — Canopa Files")
        before = len(events()); keys("Return")
        session.wait_for(lambda: completed_since(before), "completed default Keep both Undo")
        returned = fixture / "Undo original (2).txt"
        assert returned.read_text() == "original contents\n"
        assert original.read_text() == "newer work stays at this name\n"
        assert not renamed.exists()
        session.capture("undo-02-kept-both.png")
        keys("Escape")
        session.wait_for(lambda: title("Undo original (2).txt — Quick Look") is None, "preview closed before Trash")
        before = len(events()); keys("Delete")
        session.wait_for(lambda: completed_since(before), "completed Trash for Undo")
        assert not returned.exists()
        returned.write_text("newer file after Trash\n")
        session.run(["gapplication", "action", "org.canopa.Files", "trash"])
        wait_title("Trash — Canopa Files")
        ctrl("z"); wait_title("Undo conflict — Canopa Files")
        session.capture("undo-03-trash-conflict.png")
        before = len(events()); chord(["alt"], "k")
        session.wait_for(lambda: completed_since(before), "Keep both Undo from Trash window")
        assert returned.read_text() == "newer file after Trash\n"
        assert (fixture / "Undo original (2) (2).txt").read_text() == "original contents\n"
        session.capture("undo-04-trash-restored.png")
        original = fixture / "Replace original.txt"
        current = fixture / "Renamed for Replace.txt"
        original.write_text("original document before rename\n")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        time.sleep(.5)
        session.run(["wtype", "Replace"])
        keys("F2"); wait_title("Rename")
        session.run(["wtype", current.name]); before = len(events()); keys("Return")
        session.wait_for(lambda: completed_since(before), "completed rename for Replace Undo")
        original.write_text("newer work preserved by Replace Undo\n")
        time.sleep(.4)
        keys("space"); wait_title("Renamed for Replace.txt — Quick Look")
        ctrl("z"); wait_title("Undo conflict — Canopa Files")
        session.capture("undo-05-replace-conflict.png")
        before = len(events()); chord(["alt"], "r")
        session.wait_for(lambda: completed_since(before), "completed Replace Undo")
        assert original.read_text() == "original document before rename\n" and not current.exists()
        wait_title("Replace original.txt — Quick Look")
        keys("Escape")
        session.wait_for(lambda: title("Replace original.txt — Quick Look") is None, "source preview closes before Recovery preview")
        session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
        wait_title("Recovery — Canopa Files")
        time.sleep(.5); keys("Down", "space"); wait_title("Replace original.txt — Quick Look")
        session.capture("undo-06-recovery-preview.png")
        keys("Escape"); ctrl("r"); wait_title("Recover a copy")
        recovered = fixture / "Recovered newer Replace document.txt"
        ctrl("a"); session.run(["wtype", str(recovered)]); before = len(events()); keys("Return")
        session.wait_for(recovered.exists, "recover displaced newer work")
        session.wait_for(lambda: completed_since(before), "completed recovery of Undo displacement")
        assert recovered.read_text() == "newer work preserved by Replace Undo\n"
        assert original.read_text() == "original document before rename\n"
        session.capture("undo-07-recovered-copy.png")
        source = fixture / "Replacement source"
        target = fixture / "Replacement target"
        source.mkdir(); target.mkdir()
        (source / "report.txt").write_text("incoming copied version\n")
        (target / "report.txt").write_text("previous destination before copying\n")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        time.sleep(.3)
        ctrl("l"); session.run(["wtype", str(source)]); keys("Return")
        wait_title("Replacement source — Canopa Files")
        keys("Down"); ctrl("c")
        ctrl("l"); session.run(["wtype", str(target)]); keys("Return")
        wait_title("Replacement target — Canopa Files")
        ctrl("v"); wait_title("File conflict — Canopa Files")
        before = len(events()); chord(["alt"], "r")
        session.wait_for(lambda: completed_since(before), "copied replacement")
        assert (target / "report.txt").read_text() == "incoming copied version\n"
        (target / "report.txt").write_text("newer work after first replacement\n")
        time.sleep(.4)
        ctrl("z"); wait_title("Undo conflict — Canopa Files")
        session.capture("undo-08-copy-conflict.png")
        before = len(events()); keys("Return")
        session.wait_for(lambda: completed_since(before), "Keep both Undo of changed replacement")
        assert (target / "report.txt").read_text() == "newer work after first replacement\n"
        assert (target / "report (2).txt").read_text() == "previous destination before copying\n"
        keys("space"); wait_title("report (2).txt — Quick Look")
        session.capture("undo-09-copy-kept-both.png")
        keys("Escape")
        ctrl("v"); wait_title("File conflict — Canopa Files")
        before = len(events()); chord(["alt"], "r")
        session.wait_for(lambda: completed_since(before), "second copied replacement")
        (target / "report.txt").write_text("newest work after second replacement\n")
        time.sleep(.4)
        ctrl("z"); wait_title("Undo conflict — Canopa Files")
        before = len(events()); chord(["alt"], "r")
        session.wait_for(lambda: completed_since(before), "Replace Undo of changed replacement")
        assert (target / "report.txt").read_text() == "newer work after first replacement\n"
        assert (target / "report (2).txt").read_text() == "previous destination before copying\n"
        session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
        recovery = wait_title("Recovery — Canopa Files")
        focus(recovery)
        time.sleep(.5); keys("Up", "Up", "Down", "space")
        session.wait_for(lambda: any(window["title"] == "report.txt — Quick Look" and window["is_focused"] for window in windows()), "focused Recovery preview")
        time.sleep(.25)
        session.capture("undo-10-copy-recovery.png")
        keys("Escape"); focus(recovery); ctrl("r"); wait_title("Recover a copy")
        recovered = fixture / "Recovered copy edit.txt"
        ctrl("a"); session.run(["wtype", str(recovered)]); before = len(events()); keys("Return")
        session.wait_for(recovered.exists, "recover newer edited copy")
        session.wait_for(lambda: completed_since(before), "completed recovery of edited copy")
        assert recovered.read_text() == "newest work after second replacement\n"
        linked = fixture / "Linked contents"
        linked.mkdir(); (linked / "sentinel.txt").write_text("directory-link target stays intact\n")
        (source / "folder-link").write_text("replacement of the link object\n")
        (target / "folder-link").symlink_to(linked, target_is_directory=True)
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        time.sleep(.3)
        ctrl("l"); session.run(["wtype", str(source)]); keys("Return")
        wait_title("Replacement source — Canopa Files")
        session.run(["wtype", "folder"]); ctrl("c")
        ctrl("l"); session.run(["wtype", str(target)]); keys("Return")
        wait_title("Replacement target — Canopa Files")
        ctrl("v"); wait_title("File conflict — Canopa Files")
        session.capture("undo-11-directory-link-conflict.png")
        before = len(events()); chord(["alt"], "r")
        session.wait_for(lambda: completed_since(before), "replace directory-link object")
        assert (target / "folder-link").read_text() == "replacement of the link object\n"
        assert (linked / "sentinel.txt").read_text() == "directory-link target stays intact\n"
        before = len(events()); ctrl("z")
        session.wait_for(lambda: completed_since(before), "restore replaced directory-link object")
        assert (target / "folder-link").is_symlink() and (target / "folder-link").readlink() == linked
        assert (linked / "sentinel.txt").read_text() == "directory-link target stays intact\n"
        def edited_copy(name):
            (source / name).write_text("original source for a new copy\n")
            focus(browser)
            ctrl("l"); session.run(["wtype", str(source)]); keys("Return")
            wait_title("Replacement source — Canopa Files")
            session.run(["wtype", name]); ctrl("c")
            ctrl("l"); session.run(["wtype", str(target)]); keys("Return")
            wait_title("Replacement target — Canopa Files")
            before = len(events()); ctrl("v")
            session.wait_for(lambda: completed_since(before), "new copy without previous destination")
            (target / name).write_text("edited newer work in the copied file\n")
            time.sleep(.4)
            ctrl("z"); wait_title("Undo conflict — Canopa Files")

        edited_copy("changed-keep.txt")
        session.capture("undo-12-changed-copy.png")
        before = len(events()); chord(["alt"], "s")
        session.wait_for(lambda: any(event["body"].startswith("Partly undone") for event in events()[before:]), "Skip changed copy")
        assert (target / "changed-keep.txt").read_text() == "edited newer work in the copied file\n"
        ctrl("z"); wait_title("Undo conflict — Canopa Files")
        before = len(events()); keys("Escape")
        session.wait_for(lambda: any(event["body"].startswith("Cancelled") for event in events()[before:]), "cancel changed-copy Undo")
        ctrl("z"); wait_title("Undo conflict — Canopa Files")
        before = len(events()); keys("Return")
        session.wait_for(lambda: completed_since(before), "safe Keep item default")
        assert any("1 item kept" in event["body"] for event in events()[before:])
        assert (target / "changed-keep.txt").read_text() == "edited newer work in the copied file\n"
        edited_copy("changed-remove.txt")
        before = len(events()); chord(["alt"], "m")
        session.wait_for(lambda: completed_since(before), "Move changed copy to Recovery")
        assert not (target / "changed-remove.txt").exists()
        session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
        recovery = wait_title("Recovery — Canopa Files")
        focus(recovery); time.sleep(.5)
        keys("Up", "Up", "Up", "Down", "Down", "Down", "space")
        session.wait_for(lambda: any(window["title"] == "changed-remove.txt — Quick Look" and window["is_focused"] for window in windows()), "removed changed-copy Recovery preview")
        time.sleep(.5); session.capture("undo-13-removed-copy-recovery.png")
        keys("Escape"); focus(recovery); ctrl("r"); wait_title("Recover a copy")
        recovered = fixture / "Recovered removed copy.txt"
        ctrl("a"); session.run(["wtype", str(recovered)]); before = len(events()); keys("Return")
        session.wait_for(recovered.exists, "copy out removed edits")
        session.wait_for(lambda: completed_since(before), "completed recovery of removed edits")
        assert recovered.read_text() == "edited newer work in the copied file\n"
        assert (source / "changed-remove.txt").read_text() == "original source for a new copy\n"
        saved_name = "saved-review.txt"
        (source / saved_name).write_text("incoming source stays intact\n")
        (target / saved_name).write_text("previous destination before editing\n")
        # A real editor's descriptor follows the displaced inode into its saved
        # folder. No private history records or application hooks are involved.
        with (target / saved_name).open("r+b") as editor:
            focus(browser)
            ctrl("l"); session.run(["wtype", str(source)]); keys("Return")
            wait_title("Replacement source — Canopa Files")
            session.run(["wtype", saved_name]); ctrl("c")
            ctrl("l"); session.run(["wtype", str(target)]); keys("Return")
            wait_title("Replacement target — Canopa Files")
            ctrl("v"); wait_title("File conflict — Canopa Files")
            before = len(events()); chord(["alt"], "r")
            session.wait_for(lambda: completed_since(before), "saved-version replacement")
            editor.seek(0); editor.write(b"edited saved version from an open editor\n"); editor.truncate()
            editor.flush(); os.fsync(editor.fileno())
            ctrl("z"); review = wait_title("Saved version changed — Canopa Files")
            session.capture("undo-14-saved-version-review.png")
            # Inspect a fresh catalog and its saved bytes before answering the
            # Undo worker. The active writer must not hold these readers up.
            session.run(["gapplication", "action", "org.canopa.Files", "recovery"])
            recovery = wait_title("Recovery — Canopa Files")
            focus(recovery); keys("F5"); time.sleep(.5)
            keys(*(["Up"] * 8), "space")
            wait_title("saved-review.txt — Quick Look")
            time.sleep(.5); session.capture("undo-14a-recovery-before-decision.png")
            assert title("Saved version changed — Canopa Files") is not None
            assert (target / saved_name).read_text() == "incoming source stays intact\n"
            keys("Escape"); focus(review)
            before = len(events()); keys("Return")
            session.wait_for(lambda: any(event["body"].startswith("Partly undone") for event in events()[before:]), "safe Skip default for changed saved version")
            assert (target / saved_name).read_text() == "incoming source stays intact\n"
            focus(browser)
            ctrl("z"); wait_title("Saved version changed — Canopa Files")
            before = len(events()); keys("Escape")
            session.wait_for(lambda: any(event["body"].startswith("Cancelled") for event in events()[before:]), "cancel saved-version review")
            assert (target / saved_name).read_text() == "incoming source stays intact\n"
            ctrl("z"); wait_title("Saved version changed — Canopa Files")
            before = len(events()); chord(["alt"], "r")
            session.wait_for(lambda: completed_since(before), "restore edited saved version")
        assert (target / saved_name).read_text() == "edited saved version from an open editor\n"
        assert (source / saved_name).read_text() == "incoming source stays intact\n"
        keys("space"); wait_title("saved-review.txt — Quick Look")
        session.capture("undo-15-saved-version-restored.png"); keys("Escape")
        with (target / saved_name).open("r+b") as editor:
            before = len(events()); ctrl("v"); wait_title("File conflict — Canopa Files")
            chord(["alt"], "r")
            session.wait_for(lambda: completed_since(before), "second saved-version replacement")
            editor.seek(0); editor.write(b"latest saved version\n"); editor.truncate()
            editor.flush(); os.fsync(editor.fileno())
            (target / saved_name).write_text("newer destination work\n")
            time.sleep(.4)
            ctrl("z"); wait_title("Saved version changed — Canopa Files")
            chord(["alt"], "r"); wait_title("Undo conflict — Canopa Files")
            session.capture("undo-16-saved-version-destination.png")
            before = len(events()); keys("Return")
            session.wait_for(lambda: completed_since(before), "Keep both after saved-version review")
        assert (target / saved_name).read_text() == "newer destination work\n"
        assert (target / "saved-review (2).txt").read_text() == "latest saved version\n"
        assert (source / saved_name).read_text() == "incoming source stays intact\n"
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": ["native Undo conflict metadata", "conflict appears from focused Quick Look", "Skip retains Undo and both files", "Escape cancels without mutation", "Return uses the safe Keep both default", "Keep both restores a new name and preserves newer work", "Trash-origin Undo conflict and Keep both", "Replace Undo restores the original file", "displaced newer file is previewable in Recovery", "native recovery copy preserves both versions", "previous destination metadata during Undo of edited copies", "Keep both restores previous destination with a new name", "Replace restores previous destination and retains newer copy edits", "native preview and recovery of newer copy edits", "directory-link conflict metadata and Replace preserve its target", "Undo restores the replaced directory link", "changed-copy current metadata and explicit choices", "Skip leaves changed-copy Undo retryable", "Escape preserves changed-copy history", "Return keeps changed item and reports it separately", "Move to Recovery removes only the selected changed copy", "removed copy edits remain previewable and recoverable", "changed saved version has its own metadata review", "Return safely skips saved-version review and preserves history", "Escape cancels saved-version review without changing files", "Restore this version explicitly restores the edited saved file", "restored saved version is previewable", "destination conflicts follow saved-version approval", "Keep both preserves newer destination work after saved-version approval", "Recovery refreshes while Undo awaits its decision", "saved contents can be previewed before answering Undo"], "windows": windows()}
    if preferences_only:
        def chord(mods, key):
            session.run(["wtype", *sum((["-M", mod] for mod in mods), []), "-k", key,
                         *sum((["-m", mod] for mod in reversed(mods)), [])])
            time.sleep(.45)

        def choose(mnemonic, index):
            chord(["alt"], mnemonic)
            time.sleep(.3)
            keys("Home", *(["Down"] * index), "Return")

        ctrl("l"); session.run(["wtype", str(fixture / "A-project")]); keys("Return")
        wait_title("A-project — Canopa Files")
        ctrl("3")
        state = session.runtime / "state/canopa-files/views.json"
        session.wait_for(state.exists, "deliberate folder view")
        ctrl("d")
        wait_title("Bookmark")
        session.wait_for(lambda: title("Bookmark")["is_focused"], "bookmark keyboard focus")
        session.capture("preferences-00-bookmark.png")
        chord(["alt"], "n"); ctrl("a"); session.run(["wtype", "Work"])
        time.sleep(.3)
        session.capture("preferences-00-bookmark-name.png")
        chord(["alt"], "u")
        session.wait_for(lambda: title("Bookmark") is None, "bookmark draft accepted")
        wait_title("Files Preferences")
        ctrl("s")
        time.sleep(.5)
        (session.output / "preferences-config.kdl").write_text(config.read_text())
        session.wait_for(lambda: 'Work' in config.read_text(), "graphical bookmark saved")
        session.capture("preferences-01-locations.png")
        chord(["alt"], "2")
        choose("t", 0)
        chord(["alt"], "r")
        choose("t", 4)
        chord(["alt"], "u")
        chord(["alt"], "a")
        time.sleep(.4)
        session.capture("preferences-02-command-search.png")
        session.run(["wtype", "Copy"]); time.sleep(.4); keys("Down", "Return")
        session.capture("preferences-02-available-command.png")
        chord(["alt"], "m")
        session.capture("preferences-02-added-command.png")
        ctrl("s")
        session.wait_for(lambda: 'toolbar forward parent location quick-look search copy' in config.read_text(), "toolbar remove, reorder and add")
        session.capture("preferences-02-toolbar.png")
        chord(["alt"], "1")
        choose("v", 1)
        choose("s", 2)
        chord(["alt"], "c"); ctrl("a"); session.run(["wtype", "320"])
        ctrl("s")
        session.wait_for(lambda: 'mode=list' in config.read_text() and 'column-width=320' in config.read_text(), "global folder view defaults")
        session.capture("preferences-03-views.png")
        chord(["alt"], "4")
        choose("c", 6)
        chord(["alt"], "s"); ctrl("a"); session.run(["wtype", "F6"])
        ctrl("s")
        session.wait_for(lambda: 'shortcut search F6' in config.read_text(), "remapped search saved")
        session.run(["niri", "msg", "action", "set-column-width", "360"])
        time.sleep(.4)
        assert title("Files Preferences")["layout"]["window_size"][0] == 360, windows()
        session.capture("preferences-04-shortcuts.png")
        keys("Escape")
        session.wait_for(lambda: title("Files Preferences") is None, "saved preference editor closed")
        keys("F6"); session.run(["wtype", "notes"]); time.sleep(.5); keys("Down", "space")
        wait_title("notes.rs — Quick Look")
        keys("Escape", "Escape")
        session.run(["niri", "msg", "action", "set-column-width", "360"])
        time.sleep(.5)
        session.capture("preferences-05-narrow.png")
        keys("Left"); wait_title("Downloads — Canopa Files")
        session.capture("preferences-05-inherited-list.png")
        keys("Right"); wait_title("A-project — Canopa Files")
        assert json.loads(state.read_text())["folders"][0]["view"] == {"mode": "icons"}, state.read_text()
        session.capture("preferences-06-folder-return.png")
        ctrl("l"); session.run(["wtype", str(fixture)]); keys("Return")
        wait_title("Downloads — Canopa Files")
        ctrl("l"); keys("Tab", "Tab", "Tab", "Tab", "space")
        time.sleep(.3)
        session.capture("preferences-07-narrow-locations.png")
        keys("space", *(["Down"] * 7), "Return")
        wait_title("A-project — Canopa Files")
        session.capture("preferences-08-bookmark-navigation.png")
        ctrl("l"); keys("Tab", "Tab", "Tab", "Tab", "space")
        chord(["alt"], "d")
        session.wait_for(lambda: json.loads(state.read_text())["folders"] == [], "Use defaults clears deliberate overrides")
        session.capture("preferences-09-use-defaults.png")
        ctrl("w")
        # wait_for audits all registered processes, so remove this deliberately
        # closed process before observing its expected exit.
        session.processes.remove(process)
        session.wait_for(lambda: process.poll() is not None, "browser exits before preference restart")
        process = session.start([str(binary), str(fixture / "A-project")], "files-restarted.log")
        wait_title("A-project — Canopa Files")
        keys("F6"); session.run(["wtype", "notes"]); time.sleep(.5); keys("Down", "space")
        wait_title("notes.rs — Quick Look"); keys("Escape", "Escape")
        session.capture("preferences-10-restarted.png")
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": ["graphical bookmark persistence", "toolbar add/remove/reorder", "global view defaults", "remapped native search", "sparse folder overrides", "360-pixel editor and browser", "narrow keyboard navigation", "bookmark navigation through narrow overflow", "Use defaults", "shared preferences after process restart"], "windows": windows()}
    if drops_only:
        source = fixture / "Drag source"
        destination = fixture / "Drop destination"
        source.mkdir(); destination.mkdir()
        selected = source / "photo space.png"
        selected.write_bytes(b"dragged image bytes")
        ctrl("l"); session.run(["wtype", str(source)]); keys("Return")
        wait_title("Drag source — Canopa Files")
        ctrl("2")
        session.run(["niri", "msg", "action", "set-column-width", "50%"])
        ctrl("n")
        session.wait_for(lambda: len(windows()) == 2, "second drag browser")
        session.run(["niri", "msg", "action", "set-column-width", "50%"])
        ctrl("l"); session.run(["wtype", str(destination)]); keys("Return")
        target = wait_title("Drop destination — Canopa Files")
        ctrl("2")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        session.wait_for(lambda: title("Drag source — Canopa Files")["is_focused"], "both drag tiles visible")
        time.sleep(.4)
        session.capture("drop-01-tiles.png")
        logical_height = target["layout"]["window_size"][1]
        logical_width = sum(window["layout"]["window_size"][0] for window in windows())
        pointer = session.build_pointer("drag")
        ready, release = session.runtime / "drag.ready", session.runtime / "drag.release"
        drag = session.start([str(pointer), str(round(100 * 1600 / logical_width)), str(round(116 * 1000 / logical_height)), str(round((logical_width * .5 + 100) * 1600 / logical_width)), str(round(300 * 1000 / logical_height)), str(ready), str(release)], "drag.log")
        session.wait_for(ready.exists, "real pointer drag held over destination")
        time.sleep(.5)
        session.capture("drop-02-before-release.png")
        assert selected.read_bytes() == b"dragged image bytes"
        assert not (destination / selected.name).exists()
        release.touch()
        # A successful helper exit is expected, so remove it from service liveness checks.
        drag.wait(timeout=10); session.processes.remove(drag)
        session.wait_for(lambda: (destination / selected.name).exists(), "default native drop moved the file")
        assert not selected.exists()
        assert (destination / selected.name).read_bytes() == b"dragged image bytes"
        session.capture("drop-03-moved.png")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(target["id"])])
        time.sleep(.25); ctrl("z")
        session.wait_for(selected.exists, "shared Undo reverses native drop")
        assert not (destination / selected.name).exists()

        def perform_drag(name, modifier=None, target_y=300, cancel=False, reverse=False, source_x=100, source_y=116):
            session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
            time.sleep(.3)
            ready, release = session.runtime / f"{name}.ready", session.runtime / f"{name}.release"
            keyboard = session.start(["wtype", "-M", modifier, "-s", "60000", "-m", modifier], f"{name}-keyboard.log") if modifier else None
            if keyboard: time.sleep(.3)
            first_x = logical_width * .5 + source_x if reverse else source_x
            last_x = 100 if reverse else logical_width * .5 + 100
            drag = session.start([str(pointer), str(round(first_x * 1600 / logical_width)), str(round(source_y * 1000 / logical_height)), str(round(last_x * 1600 / logical_width)), str(round(target_y * 1000 / logical_height)), str(ready), str(release)], f"{name}.log")
            session.wait_for(ready.exists, f"{name} held over destination")
            time.sleep(.5)
            session.capture(f"{name}-before-release.png")
            if cancel: keys("Escape")
            release.touch()
            drag.wait(timeout=10); session.processes.remove(drag)
            if keyboard:
                keyboard.terminate(); keyboard.wait(timeout=5); session.processes.remove(keyboard)
            time.sleep(.25)

        def undo_drop():
            def completed_drop():
                notices = json.loads(notifications.read_text())
                return notices and notices[-1]["title"].startswith(("Copy ", "Move ")) and notices[-1]["body"].startswith("Completed") and "Undo" in notices[-1]["actions"]
            session.wait_for(completed_drop, "drop committed and completion presented before Undo")
            session.run(["niri", "msg", "action", "focus-window", "--id", str(target["id"])])
            time.sleep(.25); ctrl("z")
            session.wait_for(selected.exists, "drop source available after Undo")
            time.sleep(.25)

        perform_drag("drop-04-control-copy", "ctrl")
        session.wait_for(lambda: (destination / selected.name).exists(), "Ctrl forces native Copy")
        assert selected.exists()
        undo_drop()
        session.wait_for(lambda: not (destination / selected.name).exists(), "Undo native forced Copy")
        nested = destination / "A-folder"
        nested.mkdir(); time.sleep(.5)
        perform_drag("drop-05-folder-target", target_y=116)
        session.wait_for(lambda: (nested / selected.name).exists(), "drop onto folder row uses that folder")
        assert not selected.exists()
        assert not (destination / selected.name).exists()
        undo_drop()
        session.wait_for(lambda: not (nested / selected.name).exists(), "Undo folder-row drop")
        perform_drag("drop-06-cancel", cancel=True)
        assert selected.exists() and not (destination / selected.name).exists()
        existing = destination / selected.name
        existing.write_bytes(b"existing destination work")
        time.sleep(.4)
        perform_drag("drop-06a-conflict")
        conflict = wait_title("File conflict — Canopa Files")
        session.wait_for(lambda: title("File conflict — Canopa Files")["is_focused"], "foreground drop conflict focus")
        session.capture("drop-06b-conflict.png")
        keys("Return")
        both = destination / "photo space (2).png"
        session.wait_for(both.exists, "Keep both completes the dropped move")
        session.wait_for(lambda: not selected.exists(), "completed conflict move removes only its source")
        assert existing.read_bytes() == b"existing destination work"
        undo_drop(); session.wait_for(lambda: not both.exists(), "Undo reverses kept-both drop")
        assert existing.read_bytes() == b"existing destination work"
        existing.unlink(); time.sleep(.3)
        cross_volume = None
        for base in (ROOT / ".scratch/file-manager", Path("/dev/shm")):
            candidate = tempfile.TemporaryDirectory(prefix="native-drop-volume-", dir=base)
            if Path(candidate.name).stat().st_dev != source.stat().st_dev:
                cross_volume = candidate
                break
            candidate.cleanup()
        assert cross_volume is not None, "a writable second filesystem is required for native drop acceptance"
        with cross_volume as directory:
            volume = Path(directory)
            assert source.stat().st_dev != volume.stat().st_dev, "native cross-filesystem fixture must use two actual filesystems"
            session.run(["niri", "msg", "action", "focus-window", "--id", str(target["id"])])
            time.sleep(.25); ctrl("l"); session.run(["wtype", str(volume)]); keys("Return")
            wait_title(f"{volume.name} — Canopa Files"); ctrl("2"); time.sleep(.4)
            perform_drag("drop-07-cross-copy")
            session.wait_for(lambda: (volume / selected.name).exists(), "cross-filesystem native Copy default")
            assert selected.exists()
            undo_drop(); session.wait_for(lambda: not (volume / selected.name).exists(), "Undo cross-filesystem drop copy")
            perform_drag("drop-08-shift-move", "shift")
            session.wait_for(lambda: (volume / selected.name).exists(), "Shift forces cross-filesystem native Move")
            session.wait_for(lambda: not selected.exists(), "forced Move removes source only after commit")
            undo_drop(); session.wait_for(lambda: not (volume / selected.name).exists(), "Undo cross-filesystem forced Move")
            raw = source / os.fsdecode(b"raw-\xff-newline\n.png")
            raw.write_bytes(b"raw drag bytes")
            time.sleep(.5)
            session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
            time.sleep(.25); ctrl("a")
            perform_drag("drop-09-multiple-raw")
            session.wait_for(lambda: (volume / selected.name).exists() and (volume / raw.name).exists(), "drag preserves multiple selection and raw filename bytes")
            assert selected.exists() and raw.exists()
            assert (volume / raw.name).read_bytes() == b"raw drag bytes"
            undo_drop(); session.wait_for(lambda: not (volume / raw.name).exists() and not (volume / selected.name).exists(), "one Undo reverses the multiple-file drop")
            session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
            time.sleep(.25); ctrl("3")
            perform_drag("drop-09a-icon-source", source_x=80, source_y=150)
            session.wait_for(lambda: (volume / selected.name).exists() and (volume / raw.name).exists(), "icon view preserves drag selection")
            undo_drop(); session.wait_for(lambda: not (volume / raw.name).exists() and not (volume / selected.name).exists(), "Undo icon-view drop")
            session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
            time.sleep(.25); ctrl("1")
            perform_drag("drop-09b-column-source", source_x=logical_width * .5 - 100, source_y=145)
            session.wait_for(lambda: (volume / selected.name).exists() and (volume / raw.name).exists(), "default column view preserves drag selection")
            undo_drop(); session.wait_for(lambda: not (volume / raw.name).exists() and not (volume / selected.name).exists(), "Undo column-view drop")
            session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
            time.sleep(.25); ctrl("2")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(target["id"])])
        time.sleep(.25); ctrl("w")
        session.wait_for(lambda: len(windows()) == 1, "destination browser closed before external peer")
        external_source = session.runtime / "external-source.txt"
        external_source.write_bytes(b"external URI drag")
        external_output = session.runtime / "external-copies"
        external_result = session.runtime / "external-drag-result.json"
        session.start(["/usr/bin/python3", str(ROOT / "scripts/desktop-fixture/files-dnd-peer.py"), str(session.runtime), str(external_output), str(external_result), str(external_source)], "dnd-peer.log")
        peer = wait_title("DnD Peer")
        session.run(["niri", "msg", "action", "set-column-width", "50%"])
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        time.sleep(.3); ctrl("a")
        perform_drag("drop-10-external-copy")
        session.wait_for(external_result.exists, "external process consumes native URI selection")
        assert set(json.loads(external_result.read_text())) == {str(selected), str(raw)}
        assert (external_output / raw.name).read_bytes() == b"raw drag bytes"
        assert selected.exists() and raw.exists()
        perform_drag("drop-11-external-incoming", reverse=True)
        received = source / external_source.name
        session.wait_for(received.exists, "Files accepts independent process URI drag")
        assert received.read_bytes() == b"external URI drag"
        session.wait_for(lambda: not external_source.exists(), "external Move source removed by destination transaction")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
        time.sleep(.25); ctrl("z")
        session.wait_for(external_source.exists, "Undo external inbound drag")
        assert not received.exists()
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": ["real drag between tiled browsers", "same-filesystem Move default", "no mutation before release", "shared Undo for drag transfer", "Ctrl forces Copy", "folder-row target", "Escape cancels drag", "foreground filename conflict, Keep both and Undo", "cross-filesystem Copy default", "Shift forces cross-filesystem Move", "multiple selection and raw filename URI round-trip", "icon-view source", "default column-view source", "outbound copy to independent native process", "inbound move from independent native process and Undo"], "windows": windows()}
    if actions_only:
        # The editor writes the actual shared file, then all three invocation
        # surfaces run a command against the native listing's current selection.
        action_result = session.runtime / "action-result.json"
        helper = session.runtime / "action-fixture.py"
        helper.write_text(f'import json, sys\nfrom pathlib import Path\nPath({str(action_result)!r}).write_text(json.dumps(sys.argv[1:]))\nprint("Selection received")\n')

        def chord(mods, key):
            session.run(["wtype", *sum((["-M", mod] for mod in mods), []), "-k", key,
                         *sum((["-m", mod] for mod in reversed(mods)), [])])
            time.sleep(.25)

        def field(mnemonic, value):
            chord(["alt"], mnemonic)
            ctrl("a")
            session.run(["wtype", value])

        chord(["ctrl", "shift"], "a")
        editor = wait_title("Custom Actions — Canopa Files")
        session.wait_for(lambda: title("Custom Actions — Canopa Files")["is_focused"], "action editor focus")
        time.sleep(.4)
        chord(["alt"], "a")
        session.run(["wtype", "Inspect selected images"])
        field("i", "image-x-generic-symbolic")
        field("c", f'{sys.executable} "{helper}" %F %d')
        field("p", "*.png; *.jpg")
        field("s", "<Control><Alt>i")
        chord(["alt"], "b")
        ctrl("s")
        session.wait_for(lambda: 'Inspect selected images' in config.read_text(), "graphically saved action")
        assert "toolbar=#true" in config.read_text(), config.read_text()
        assert f'color-scheme "{scheme}"' in config.read_text()
        session.capture("actions-01-editor.png")
        keys("Escape")
        session.wait_for(lambda: title("Custom Actions — Canopa Files") is None, "saved editor closed")
        ctrl("l")
        session.run(["wtype", str(fixture)])
        keys("Return", "c")
        chord(["ctrl", "alt"], "i")
        wait_title("Operations — Canopa Files")
        session.wait_for(action_result.exists, "custom action receives selection")
        assert json.loads(action_result.read_text()) == [str(fixture / "C-photo.png"), str(fixture)]
        session.capture("actions-02-output.png")
        keys("Escape")
        session.wait_for(lambda: title("Downloads — Canopa Files")["is_focused"], "browser focus after action")
        session.capture("actions-03-toolbar.png")
        action_result.unlink()
        chord(["shift"], "F10")
        time.sleep(.5)
        session.capture("actions-04-context.png")
        keys("Down", "Down", "Down", "Down", "Down", "Return")
        session.wait_for(action_result.exists, "context-menu custom action")
        assert json.loads(action_result.read_text()) == [str(fixture / "C-photo.png"), str(fixture)]
        wait_title("Operations — Canopa Files")
        keys("Escape")
        session.wait_for(lambda: title("Downloads — Canopa Files")["is_focused"], "browser focus for toolbar action")
        action_result.unlink()
        pointer = session.build_pointer()
        logical_width, logical_height = title("Downloads — Canopa Files")["layout"]["window_size"]
        session.run([str(pointer), str(round(28 * 1600 / logical_width)), str(round(127 * 1000 / logical_height))])
        session.wait_for(action_result.exists, "toolbar custom action")
        assert json.loads(action_result.read_text()) == [str(fixture / "C-photo.png"), str(fixture)]
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": ["graphical action creation and shared persistence", "image conditions and shortcut invocation", "context-menu and toolbar invocation", "real selection argv and working folder", "native execution feedback"], "windows": windows()}
    if not transfers_only:
        session.capture("01-wide.png")
        keys("Down", "Right")
        wait_title("A-project — Canopa Files")
        keys("Left")
        wait_title("Downloads — Canopa Files")
        keys("Down", "space")
        preview = wait_title("B-notes.txt — Quick Look")
        assert preview["is_floating"], preview
        assert preview["layout"]["window_size"][0] >= 600, preview
        session.capture("02-preview.png")
        keys("Down")
        wait_title("C-photo.png — Quick Look")
        pixel("03-image.png", (24, 160, 120))
        keys("Down")
        wait_title("D-document.pdf — Quick Look")
        pixel("04-pdf-first-page.png", (255, 0, 0))
        keys("Page_Down")
        pixel("05-pdf-second-page.png", (0, 255, 0))

        # Another Browser Window must not claim this preview's selection.
        ctrl("n")
        other = session.wait_for(lambda: next((window for window in windows() if window["id"] != browser["id"] and window.get("title") == "Downloads — Canopa Files"), None), "second browser")
        session.run(["niri", "msg", "action", "focus-window", "--id", str(other["id"])])
        time.sleep(.25)
        keys("Down")
        assert title("D-document.pdf — Quick Look"), windows()
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
        narrow_preview = wait_title("D-document.pdf — Quick Look")
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
            wait_title("D-document.pdf — Quick Look")
            keys("Escape")
        ctrl("t")
        ctrl("l")
        session.run(["wtype", str(fixture / "A-project")])
        keys("Return")
        wait_title("A-project — Canopa Files")
        keys("Down", "space")
        wait_title("notes.rs — Quick Look")
        session.capture("08-code-preview.png")
        keys("Escape")
        ctrl("w")
        wait_title("Downloads — Canopa Files")
        keys("space")
        wait_title("D-document.pdf — Quick Look")
        keys("Escape")
        assert process.poll() is None
        session.run(["niri", "msg", "action", "set-column-width", "100%"])
        time.sleep(.4)
        ctrl("f")
        session.run(["wtype", "document-17"])
        time.sleep(.6)
        keys("Down", "space")
        wait_title("document-17.txt — Quick Look")
        session.capture("09-search-preview.png")
        keys("Escape", "Escape", "space")
        wait_title("D-document.pdf — Quick Look")
        keys("Escape")

        # A real filesystem event refreshes the listing without explicit reload.
        (fixture / "new-arrival.txt").write_text("arrived while browsing\n")
        time.sleep(.4)
        session.run(["wtype", "new-arrival"])
        keys("space")
        wait_title("new-arrival.txt — Quick Look")
        keys("Escape")
        keys("Return")
        session.wait_for(launched.exists, "default application launch")
        assert json.loads(launched.read_text()) == [str(fixture / "new-arrival.txt")]
        # Native name prompts run real mutations on a worker and share durable Undo.
        session.run(["wtype", "-M", "ctrl", "-M", "shift", "-k", "n", "-m", "shift", "-m", "ctrl"])
        wait_title("New folder")
        session.run(["wtype", "Created in Files"])
        keys("Return")
        created = fixture / "Created in Files"
        session.wait_for(created.is_dir, "created folder")
        time.sleep(.3)
        keys("F2")
        wait_title("Rename")
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
    session.run(["niri", "msg", "action", "focus-window", "--id", str(destination_window["id"])])
    ctrl("w")
    # Pause at a real conflict. The operation's hold must survive every window,
    # independently of the separate hold used to serve a file clipboard.
    (target / "copied.txt").write_text("existing destination\n")
    navigate(source)
    keys("Down")
    ctrl("c")
    assert session.run(["wl-paste", "--no-newline", "--type", "x-special/gnome-copied-files"]) == f"copy\n{(source / 'copied.txt').as_uri()}\n"
    navigate(target)
    ctrl("v")
    wait_title("File conflict — Canopa Files")
    session.capture("12-file-conflict.png")
    keys("Escape")
    session.wait_for(lambda: not title("File conflict — Canopa Files"), "cancelled conflict")
    assert (target / "copied.txt").read_text() == "existing destination\n"
    time.sleep(.3)
    assert session.run(["wl-paste", "--no-newline", "--type", "x-special/gnome-copied-files"]) == f"copy\n{(source / 'copied.txt').as_uri()}\n"
    ctrl("v")
    wait_title("File conflict — Canopa Files")
    session.run(["wtype", "-M", "alt", "-k", "d", "-m", "alt"])
    session.wait_for(lambda: not title("File conflict — Canopa Files"), "decision put aside")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    time.sleep(.25)
    ctrl("l")
    ctrl("c")  # Replace the file clipboard with ordinary location-field text.
    keys("Escape")
    assert "x-special/gnome-copied-files" not in session.run(["wl-paste", "--list-types"])
    for window in windows():
        session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
    session.wait_for(lambda: not windows(), "all browser and operation windows closed")
    time.sleep(.8)
    assert process.poll() is None, "waiting operation lost its application hold"
    session.run([str(binary), str(target)])
    reopened = wait_title("Clipboard target — Canopa Files")
    assert reopened["pid"] == process.pid, reopened
    ctrl("j")
    wait_title("File conflict — Canopa Files")
    session.wait_for(lambda: title("File conflict — Canopa Files")["is_focused"], "resumed conflict receives focus")
    session.run(["wtype", "-M", "alt", "-k", "a", "-m", "alt"])
    session.run(["wtype", "-M", "alt", "-k", "k", "-m", "alt"])
    session.wait_for(lambda: (target / "copied (2).txt").exists(), "resumed Keep both transfer")
    assert (target / "copied (2).txt").read_text() == "clipboard payload\n"
    assert (target / "copied.txt").read_text() == "existing destination\n"
    session.run(["niri", "msg", "action", "focus-window", "--id", str(reopened["id"])])
    time.sleep(.25)
    ctrl("z")
    session.wait_for(lambda: not (target / "copied (2).txt").exists(), "undo resumed transfer")
    assert (target / "copied.txt").read_text() == "existing destination\n"
    session.wait_for(notifications.exists, "operation notifications")
    events = json.loads(notifications.read_text())
    assert any(event["body"].startswith("Cancelled") for event in events), events
    assert any(event["body"].startswith("Completed") and "Copy" in event["title"] for event in events), events
    assert any("Waiting for your decision" in event["body"] for event in events), events
    for window in windows():
        if window["title"] == "Operations — Canopa Files":
            session.run(["niri", "msg", "action", "close-window", "--id", str(window["id"])])
    # The real notification action reopens the shared Operations window.
    latest = events[-1]
    action = latest["actions"][0]
    session.run(["busctl", "--address", session.env["DBUS_SESSION_BUS_ADDRESS"], "call", "org.freedesktop.Notifications", "/org/freedesktop/Notifications", "org.canopa.FilesFixture", "Activate", "us", str(latest["id"]), action])
    wait_title("Operations — Canopa Files")
    session.capture("13-background-completed.png")
    keys("Escape")
    # External URI-list clipboard data uses the same operations. An unsupported
    # source fails independently; Retry targets that exact file, leaving the
    # successful prefix alone.
    retry_source = fixture / "Retry source"
    retry_source.mkdir()
    os.mkfifo(retry_source / "a.txt")
    (retry_source / "b.txt").write_text("successful prefix\n")
    session.start(["wl-copy", "--foreground", "--type", "text/uri-list",
                                    f"{(retry_source / 'a.txt').as_uri()}\r\n{(retry_source / 'b.txt').as_uri()}\r\n"], "clipboard-owner.log")
    session.wait_for(lambda: "text/uri-list" in session.run(["wl-paste", "--list-types"]), "external file clipboard")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(reopened["id"])])
    time.sleep(.25)
    ctrl("v")
    session.wait_for(lambda: (target / "b.txt").exists(), "successful item after isolated failure")
    time.sleep(.4)
    ctrl("j")
    wait_title("Operations — Canopa Files")
    session.capture("14-isolated-failure.png")
    (retry_source / "a.txt").unlink()
    (retry_source / "a.txt").write_text("repaired source\n")
    session.run(["wtype", "-M", "alt", "-k", "r", "-m", "alt"])
    session.wait_for(lambda: (target / "a.txt").exists(), "retry failed item")
    assert (target / "a.txt").read_text() == "repaired source\n"
    assert (target / "b.txt").read_text() == "successful prefix\n"
    session.wait_for(lambda: any("Retry failed" in event["title"] and "Undo" in event["actions"] for event in json.loads(notifications.read_text())), "retry Undo notification")
    retry_notice = next(event for event in reversed(json.loads(notifications.read_text())) if "Retry failed" in event["title"])
    undo_action = retry_notice["actions"][retry_notice["actions"].index("Undo") - 1]
    keys("Escape")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(reopened["id"])])
    time.sleep(.25)
    # Old Undo notifications must never reverse a subsequently created folder.
    session.run(["wtype", "-M", "ctrl", "-M", "shift", "-k", "n", "-m", "shift", "-m", "ctrl"])
    wait_title("New folder")
    session.run(["wtype", "Newer than notification"])
    keys("Return")
    newer = target / "Newer than notification"
    session.wait_for(newer.is_dir, "newer operation")
    time.sleep(.3)

    def activate_notice():
        session.run(["busctl", "--address", session.env["DBUS_SESSION_BUS_ADDRESS"], "call", "org.freedesktop.Notifications", "/org/freedesktop/Notifications", "org.canopa.FilesFixture", "Activate", "us", str(retry_notice["id"]), undo_action])

    activate_notice()
    session.wait_for(lambda: any(event["title"] == "Undo from notification" and event["body"] == "Failed" for event in json.loads(notifications.read_text())), "stale notification safely rejected")
    assert newer.is_dir() and (target / "a.txt").exists()
    wait_title("Operations — Canopa Files")
    keys("Escape")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(reopened["id"])])
    time.sleep(.25)
    ctrl("z")
    session.wait_for(lambda: not newer.exists(), "undo newer folder")
    time.sleep(.3)
    ctrl("z")
    session.wait_for(lambda: not (target / "a.txt").exists(), "undo retried item")
    assert (target / "b.txt").exists()
    time.sleep(.3)
    ctrl("z")
    session.wait_for(lambda: not (target / "b.txt").exists(), "undo successful prefix")
    # Desktop notification actions are consumed on activation. Use a fresh
    # notification for the successful activation case instead of clicking the
    # already consumed stale notification a second time.
    ctrl("v")
    session.wait_for(lambda: (target / "a.txt").exists() and (target / "b.txt").exists(), "fresh copy for notification Undo")
    old_notice_id = retry_notice["id"]
    session.wait_for(lambda: any(event["id"] > old_notice_id and event["title"] == "Copy 2 items to Clipboard target" and "Undo" in event["actions"] for event in json.loads(notifications.read_text())), "fresh Undo notification")
    retry_notice = next(event for event in reversed(json.loads(notifications.read_text())) if event["title"] == "Copy 2 items to Clipboard target" and "Undo" in event["actions"])
    undo_action = retry_notice["actions"][retry_notice["actions"].index("Undo") - 1]
    activate_notice()
    session.wait_for(lambda: not (target / "a.txt").exists() and not (target / "b.txt").exists(), "notification Undo targets its completed copy")
    wait_title("Operations — Canopa Files")
    session.wait_for(lambda: title("Operations — Canopa Files")["is_focused"], "notification Operations window receives focus")
    session.capture("15-notification-undo.png")
    # Simulate the destination disappearing without touching a real mount.
    session.run(["niri", "msg", "action", "focus-window", "--id", str(reopened["id"])])
    time.sleep(.25)
    away = fixture / "Temporarily unavailable"
    target.rename(away)
    ctrl("v")
    wait_title("Destination unavailable — Canopa Files")
    session.capture("16-unavailable-destination.png")
    away.rename(target)
    session.run(["wtype", "-M", "alt", "-k", "r", "-m", "alt"])
    session.wait_for(lambda: (target / "a.txt").exists() and (target / "b.txt").exists(), "retry available destination")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(reopened["id"])])
    time.sleep(.25)
    ctrl("z")
    session.wait_for(lambda: not (target / "a.txt").exists() and not (target / "b.txt").exists(), "undo resumed unavailable-destination transfer")
    # Trash is recoverable storage, with Restore and Undo shared with browsers.
    trash_source = fixture / "Trash workflow"
    trash_source.mkdir()
    recoverable = trash_source / "recover me.txt"
    recoverable.write_text("recoverable from the native interface\n")
    navigate(trash_source)
    keys("Down", "Delete")
    session.wait_for(lambda: not recoverable.exists(), "Delete moves to Trash")
    session.run(["gapplication", "action", "org.canopa.Files", "trash"])
    wait_title("Trash — Canopa Files")
    session.wait_for(lambda: title("Trash — Canopa Files")["is_focused"], "Trash receives focus before choosing its private store")
    time.sleep(.25)
    ctrl("1")  # Select only this fixture's private XDG_DATA_HOME Trash root.
    time.sleep(.4)
    session.run(["niri", "msg", "action", "set-column-width", "360"])
    session.wait_for(lambda: title("Trash — Canopa Files")["layout"]["window_size"][0] == 360, "narrow Trash window")
    keys("Down")
    session.capture("17-trash.png")
    keys("space")
    trash_preview = wait_title("recover me.txt — Quick Look")
    assert trash_preview["is_floating"] and trash_preview["layout"]["window_size"][0] >= 600
    session.capture("17a-trash-preview.png")
    keys("Escape")
    ctrl("r")
    session.wait_for(recoverable.exists, "Restore from Trash")
    assert recoverable.read_text() == "recoverable from the native interface\n"
    time.sleep(.3)
    ctrl("z")
    session.wait_for(lambda: not recoverable.exists(), "Undo Restore returns item to Trash")
    time.sleep(.3)
    ctrl("z")
    session.wait_for(recoverable.exists, "Undo Trash restores original file")
    external = trash_source / "external.txt"
    external.write_text("trashed by GIO\n")
    session.run(["gio", "trash", str(external)])
    keys("F5")
    time.sleep(.4)
    keys("Down")
    ctrl("r")
    session.wait_for(external.exists, "restore another desktop implementation's Trash item")
    assert external.read_text() == "trashed by GIO\n"
    session.capture("18-trash-restored.png")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(reopened["id"])])
    time.sleep(.25)
    navigate(trash_source)
    session.run(["wtype", "recover"])
    keys("Delete")
    session.wait_for(lambda: not recoverable.exists(), "trash before choosing another restore folder")
    trash_window = title("Trash — Canopa Files")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(trash_window["id"])])
    time.sleep(.4)
    keys("Down")
    session.run(["wtype", "-M", "ctrl", "-M", "shift", "-k", "r", "-m", "shift", "-m", "ctrl"])
    wait_title("Restore to folder")
    session.wait_for(lambda: title("Restore to folder")["is_focused"], "restore chooser receives focus")
    time.sleep(.25)
    session.capture("18a-restore-chooser.png")
    alternative = fixture / "Alternate restore"
    alternative.mkdir()
    ctrl("l")
    session.run(["wtype", str(alternative)])
    keys("Return")
    if title("Restore to folder"):
        session.run(["wtype", "-M", "alt", "-k", "r", "-m", "alt"])
    session.wait_for(lambda: (alternative / recoverable.name).exists(), "native restore-folder chooser")
    assert (alternative / recoverable.name).read_text() == "recoverable from the native interface\n"
    session.capture("19-restored-elsewhere.png")
    log = (output / "files.log").read_text()
    assert "panicked" not in log and "CRITICAL" not in log and "Gtk-WARNING" not in log, log
    if not transfers_only:
        state = session.runtime / "state/canopa-files/views.json"
        session.wait_for(state.exists, "saved folder view")
        assert state.stat().st_mode & 0o777 == 0o600
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
        'private preference persistence',
        'default application receives real file path',
        'native create/rename/undo',
        'remote invocation and shared Undo',
    ]
    transfer_checks = [
        'native file clipboard copy/move across windows and Undo',
        'shared Operations view',
        'native conflict cancellation and Keep both',
        'waiting transfer survives zero windows without clipboard hold',
        'resume and Undo after reopening',
        'completion/cancellation/attention notifications and real action activation',
        'external URI-list clipboard',
        'isolated failure and exact Retry failed item',
        'stale notification protects newer work and fresh notification Undo',
        'unavailable destination pauses and Retry resumes',
        'native Delete, Trash, Restore and shared Undo',
        'narrow Trash retains selection and floats original-name Quick Look independently',
        'interoperable restore of a GIO-trashed file',
        'native folder chooser restores to another location',
    ]
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "transfers_only": transfers_only,
            "checks": ([] if transfers_only else browsing_checks) + transfer_checks,
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
    workflow.add_argument("--transfers-only", action="store_true", help="Run the transfer acceptance slice without the browsing/preview prelude")
    workflow.add_argument("--actions-only", action="store_true", help="Run graphical custom-action acceptance")
    workflow.add_argument("--drops-only", action="store_true", help="Run real pointer drag-and-drop acceptance")
    workflow.add_argument("--preferences-only", action="store_true", help="Run graphical interface preference acceptance")
    workflow.add_argument("--undo-only", action="store_true", help="Run native Undo conflict acceptance")
    workflow.add_argument("--undo-directories-only", action="store_true", help="Run directory Merge/Keep both returns and protected Recovery acceptance")
    parser.add_argument("--undo-folders-case", choices=("nonempty", "locations"), default="nonempty", help="Select owned nonempty folders or changed/unavailable folder locations")
    workflow.add_argument("--undo-folders-only", action="store_true", help="Run created-folder Undo reviews and continuations")
    workflow.add_argument("--hidden-navigation-only", action="store_true", help="Run hidden ancestor and parent-selection navigation at wide and narrow widths")
    parser.add_argument("--undo-cross-case", choices=("original", "current", "directories", "moves", "retry", "context"), default="original", help="Select original-location, changed/unavailable destination, directory-child, native directory-move Undo, failed-directory retry, or Recovery context acceptance")
    workflow.add_argument("--undo-cross-only", action="store_true", help="Run Undo conflicts across two actual filesystems")
    workflow.add_argument("--undo-saved-only", action="store_true", help="Run unavailable saved-version Undo, partial move return and protected Recovery acceptance")
    workflow.add_argument("--undo-moves-only", action="store_true", help="Run moved-item Undo reviews, saved-only restoration and protected Keep/Finish acceptance")
    parser.add_argument("--recovery-directories-case", choices=("basic", "expiry", "tiling"), default="basic", help="Select directory Recovery, history expiry, or narrow Operations acceptance")
    workflow.add_argument("--recovery-directories-only", action="store_true", help="Run directory Recovery copy, cancellation, continuation and Undo acceptance")
    workflow.add_argument("--recovery-only", action="store_true", help="Run protected-file Recovery acceptance")
    workflow.add_argument("--interrupted-only", action="store_true", help="Run real interrupted-operation recovery acceptance")
    workflow.add_argument("--reload-only", action="store_true", help="Run automatic shared configuration reload acceptance")
    workflow.add_argument("--devices-only", action="store_true", help="Run removable-volume acceptance with a private GIO provider")
    workflow.add_argument("--media-only", action="store_true", help="Run native audio/video playback and control-focus acceptance")
    workflow.add_argument("--media-formats-only", action="store_true", help="Run actual MP3, FLAC, Opus, AAC and H.264 playback/seek acceptance")
    parser.add_argument("--desktop-entry", type=Path, help="Cold-launch --media-formats-only through this registered folder application entry")
    parser.add_argument("--device-case", choices=("browsing", "safety", "identityless", "recovery", "inflight", "sources", "clipboard"), default="browsing", help="Select the private-volume workflow")
    parser.add_argument("--transfer-case", choices=("standard", "lifetime", "lifetime-live"), default="standard", help="Select transfers, exact failed-request lifetime, or its same-process control")
    args = parser.parse_args()
    if args.iced_views_case != "full" and not args.iced_views_only:
        parser.error("--iced-views-case requires --iced-views-only")
    if args.iced_preferences_case != "full" and not args.iced_preferences_only:
        parser.error("--iced-preferences-case requires --iced-preferences-only")
    if args.media_only or args.media_formats_only or args.iced_media_only or args.iced_media_formats_only:
        # Keep decoder assertion evidence in the fixture log without allowing
        # system core collection to fill the host disk or stall failure cleanup.
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    if args.desktop_entry:
        if not args.media_formats_only:
            parser.error("--desktop-entry requires --media-formats-only")
        args.desktop_entry = args.desktop_entry.resolve()
    if args.recovery_directories_case != "basic" and not args.recovery_directories_only:
        parser.error("--recovery-directories-case requires --recovery-directories-only")
    if args.undo_folders_case != "nonempty" and not args.undo_folders_only:
        parser.error("--undo-folders-case requires --undo-folders-only")
    if args.undo_cross_case != "original" and not args.undo_cross_only:
        parser.error("--undo-cross-case requires --undo-cross-only")
    if args.transfer_case != "standard" and not args.transfers_only:
        parser.error("--transfer-case requires --transfers-only")
    if args.iced_operations_case != "full" and not args.iced_operations_only:
        parser.error("--iced-operations-case requires --iced-operations-only")
    if args.device_case != "browsing" and not args.devices_only:
        parser.error("--device-case requires --devices-only")
    for command in ("pdfinfo", "pdftoppm", "wl-copy", "wl-paste", "gio", "gapplication"):
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
            if args.undo_cross_only or (args.devices_only and args.device_case == "clipboard") or (args.iced_operations_only and args.iced_operations_case == "drop-choices"):
                for parent in (ROOT, Path("/dev/shm")):
                    if parent.exists() and parent.stat().st_dev != runtime.stat().st_dev:
                        cross_destination = tempfile.TemporaryDirectory(prefix=".canopa-native-cross-", dir=parent)
                        break
                assert cross_destination, "cross-filesystem acceptance requires two writable filesystems"
            result = verify(args.binary.resolve(), session, args.scheme, args.scale, args.transfers_only, args.actions_only, args.drops_only, args.preferences_only, args.undo_only, args.recovery_only, args.interrupted_only, args.reload_only, args.devices_only, args.device_case, args.media_only, args.media_formats_only, args.desktop_entry, args.undo_moves_only, args.undo_saved_only, args.undo_cross_only, Path(cross_destination.name) if cross_destination else None, args.undo_cross_case, args.hidden_navigation_only, args.undo_folders_only, args.undo_folders_case, args.undo_directories_only, args.recovery_directories_only, args.recovery_directories_case, args.downloads_only, transfer_case=args.transfer_case, iced_browsing_only=args.iced_browsing_only, iced_search_only=args.iced_search_only, iced_preview_only=args.iced_preview_only, iced_actions_only=args.iced_actions_only, iced_interface_only=args.iced_interface_only, iced_views_only=args.iced_views_only, iced_preferences_only=args.iced_preferences_only, iced_preferences_case=args.iced_preferences_case, iced_views_case=args.iced_views_case, iced_operations_only=args.iced_operations_only, iced_operations_case=args.iced_operations_case, iced_media_only=args.iced_media_only, iced_media_formats_only=args.iced_media_formats_only)
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
            if (args.preferences_only or args.iced_preferences_only) and candidate.exists():
                (session.output / "preferences-config.kdl").write_text(candidate.read_text())
            if args.reload_only and candidate.exists():
                (session.output / "reload-config.kdl").write_text(candidate.read_text())
            if (args.devices_only or args.iced_operations_only) and (session.runtime / "volumes/events").exists():
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
