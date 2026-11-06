#!/usr/bin/env python3
"""Private public-frontend legacy Notify sessions, media and transition to EIS.

The test descriptor opts into the unadvertised implementations. This is not
shipped metadata, pointer/touch, physical input or distribution acceptance.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "scripts"))
from screencast_media import MediaSession, digest

_spec = importlib.util.spec_from_file_location("remote_fixture", ROOT / "scripts/test-remote-desktop-portal.py")
remote_fixture = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(remote_fixture)
FRONTEND = "org.freedesktop.portal.Desktop"
BACKEND = "org.freedesktop.impl.portal.desktop.canopa"
PATH = "/org/freedesktop/portal/desktop"
REMOTE = "org.freedesktop.portal.RemoteDesktop"
CAST = "org.freedesktop.portal.ScreenCast"
CASES = ("input-only", "unregistered-caller", "view-only", "owner-guards", "shortcut-burst", "parallel-consent", "ui-stop", "session-close", "worker-loss", "caller-loss", "legacy-to-eis", "budget-bytes", "budget-fds", "backend-kill")


def verify(args, session, result):
    from gi.repository import Gio, GLib
    config = session.runtime / "canopa.kdl"
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.scheme}"; }}\n')
    portals = session.runtime / "portals"
    portals.mkdir()
    implementations = ["org.freedesktop.impl.portal." + name for name in ("RemoteDesktop", "ScreenCast", "Settings")]
    (portals / "canopa.portal").write_text("[portal]\nDBusName=" + BACKEND + "\nInterfaces=" + ";".join(implementations) + ";\nUseIn=Canopa;\n")
    (portals / "portals.conf").write_text("[preferred]\ndefault=none\n" + "".join(name + "=canopa\n" for name in implementations))
    for source in (config, portals / "canopa.portal", portals / "portals.conf"):
        shutil.copy2(source, session.output / source.name)
    applications = Path(session.env["XDG_DATA_HOME"]) / "applications"
    applications.mkdir(parents=True)
    (applications / "org.canopa.RemoteDesktopFixture.desktop").write_text("[Desktop Entry]\nType=Application\nName=Remote desktop fixture\nExec=true\n")
    home = session.runtime / "home"
    home.mkdir()
    session.env.update(CANOPA_PORTAL_CONFIG=str(config), CANOPA_REMOTE_DESKTOP_NATIVE="1",
        CANOPA_PORTAL_BINARY=str(args.portal), XDG_CURRENT_DESKTOP="Canopa", XDG_DESKTOP_PORTAL_DIR=str(portals),
        XDG_DATA_DIRS=session.env["XDG_DATA_HOME"], HOME=str(home), GIO_USE_VFS="local", GSETTINGS_BACKEND="memory",
        GSETTINGS_SCHEMA_DIR="/usr/share/glib-2.0/schemas")
    output_name = session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "fr"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
layout { border { off; }; focus-ring { off; }; }
''', scale=args.scale)
    fixture = ROOT / "scripts/desktop-fixture"
    for name, xml in [("xdg-shell", "stable/xdg-shell/xdg-shell.xml"), ("xdg-foreign", "unstable/xdg-foreign/xdg-foreign-unstable-v2.xml")]:
        for mode, suffix in [("client-header", ".h"), ("private-code", ".c")]:
            subprocess.run(["wayland-scanner", mode, str(Path("/usr/share/wayland-protocols") / xml), str(session.output / (name + suffix))], check=True, timeout=15)
    receiver = session.output / "receiver"
    subprocess.run(["cc", "-Wall", "-Wextra", "-Werror", "-I", str(session.output), str(fixture / "remote-input-receiver.c"), str(session.output / "xdg-shell.c"), "-lwayland-client", "-lxkbcommon", "-o", str(receiver)], check=True, timeout=30)
    client = session.output / "ei-client"
    flags = subprocess.check_output(["pkg-config", "--cflags", "--libs", "libei-1.0", "xkbcommon"], text=True).split()
    subprocess.run(["cc", "-Wall", "-Wextra", "-Werror", str(fixture / "remote-input-ei-client.c"), "-o", str(client), *flags], check=True, timeout=30)
    parent_binary = session.output / "pattern"
    subprocess.run(["cc", "-Wall", "-Wextra", "-Werror", "-DPORTAL_CAPTURE_PATTERN", "-I", str(session.output), str(fixture / "portal-parent.c"), str(session.output / "xdg-shell.c"), str(session.output / "xdg-foreign.c"), "-lwayland-client", "-o", str(parent_binary)], check=True, timeout=30)
    receiver_process = session.start([str(receiver)], "receiver.log")
    receiver_log = session.output / "receiver.log"
    session.wait_for(lambda: "focused" in receiver_log.read_text(), "harmless receiver")
    parent = session.start([str(parent_binary)], "pattern.log")
    pointer = session.build_pointer()

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    receiver_window = session.wait_for(lambda: next((w for w in windows() if w["pid"] == receiver_process.pid), None), "receiver window")
    pattern = session.wait_for(lambda: next((w for w in windows() if w["pid"] == parent.pid), None), "pattern window")
    if not pattern["is_floating"]:
        session.run(["niri", "msg", "action", "move-window-to-floating", "--id", str(pattern["id"])])
    for dimension, value in [("width", 400), ("height", 300)]:
        session.run(["niri", "msg", "action", "set-window-" + dimension, "--id", str(pattern["id"]), str(value)])
    logical = json.loads(session.run(["niri", "msg", "--json", "outputs"]))[output_name]["logical"]
    px, py = logical["width"] - 420, logical["height"] - 320
    session.run(["niri", "msg", "action", "move-floating-window", "--id", str(pattern["id"]), "--x", str(px), "--y", str(py)])
    pattern_box = tuple(round(v * args.scale) for v in (px, py, px + 400, py + 300))

    backend_log = session.output / "backend.log"
    with backend_log.open("w") as stream:
        backend = subprocess.Popen([str(args.harness), "--exact", "screencast::remote::production::tests::native_backend_fixture", "--ignored", "--nocapture", "--test-threads=1"], stdin=subprocess.PIPE, stdout=stream, stderr=stream, env=session.env, start_new_session=True)
    session.processes.append(backend)
    session.wait_for(lambda: "remote-backend-ready " in backend_log.read_text(), "real production backend")
    flags = Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION

    def connect():
        return Gio.DBusConnection.new_for_address_sync(session.env["DBUS_SESSION_BUS_ADDRESS"], flags, None, None)

    connection = connect()

    def call(client_connection, destination, path, interface, method, signature, values):
        return client_connection.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 10000, None).unpack()

    def bus(method, signature, values):
        return call(connection, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", method, signature, values)

    bus("UpdateActivationEnvironment", "(a{ss})", ({k: v for k, v in session.env.items()},))
    frontend_binary = args.frontend
    frontend = session.start([str(frontend_binary), "--verbose"], "frontend.log")
    session.wait_for(lambda: bus("NameHasOwner", "(s)", (FRONTEND,))[0], "standard XDG frontend")
    if args.trace_notify:
        session.start(["dbus-monitor", "--session",
            "type='method_call',interface='org.freedesktop.portal.RemoteDesktop',member='NotifyKeyboardKeycode'",
            "type='method_call',interface='org.freedesktop.impl.portal.RemoteDesktop',member='NotifyKeyboardKeycode'"], "notify-order.log")
    count = 0
    responses, closed = {}, set()

    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)

    def registered(client_connection, *, identity=True):
        if identity:
            call(client_connection, FRONTEND, PATH, "org.freedesktop.host.portal.Registry", "Register", "(sa{sv})", ("org.canopa.RemoteDesktopFixture", {}))
        client_connection.signal_subscribe(FRONTEND, "org.freedesktop.portal.Request", "Response", None, None, Gio.DBusSignalFlags.NONE, lambda c, s, p, i, n, v: responses.__setitem__(p, v.unpack()))
        client_connection.signal_subscribe(FRONTEND, "org.freedesktop.portal.Session", "Closed", None, None, Gio.DBusSignalFlags.NONE, lambda c, s, p, i, n, v: closed.add(p))

    def options(**values):
        nonlocal count
        count += 1
        return {"handle_token": GLib.Variant("s", f"remote{count}"), **values}

    def response(path, status=0):
        value = session.wait_for(lambda: (drain(), responses.get(path))[1], "public portal response", timeout=40)
        assert value[0] == status, value
        return value[1]

    def request(caller, interface, method, signature, values):
        return call(caller, FRONTEND, PATH, interface, method, signature, values)[0]

    def finish(process, status=0):
        if process in session.processes:
            session.processes.remove(process)
        actual = process.wait(timeout=10)
        assert actual == status, (process.args, actual, status)

    for case in (args.case or CASES):
        capture_case = case not in ("input-only", "unregistered-caller")
        caller = connect()
        registered(caller, identity=case != "unregistered-caller")
        values = response(request(caller, REMOTE, "CreateSession", "(a{sv})", (options(session_handle_token=GLib.Variant("s", f"s{count}")),)))
        key = values["session_handle"]
        selections = [(REMOTE, "SelectDevices", options(types=GLib.Variant("u", 1), persist_mode=GLib.Variant("u", 2))), (CAST, "SelectSources", options(types=GLib.Variant("u", 1), multiple=GLib.Variant("b", False), cursor_mode=GLib.Variant("u", 1)))]
        if not capture_case:
            selections = selections[:1]
        if case in ("session-close", "caller-loss"):
            selections.reverse()
        for interface, method, selected in selections:
            response(request(caller, interface, method, "(oa{sv})", (key, selected)))
        path = request(caller, REMOTE, "Start", "(osa{sv})", (key, "", options()))

        def popup():
            drain()
            assert path not in responses, ("Start returned before fresh combined consent", responses.get(path))
            return next((w for w in windows() if w["app_id"] == "org.canopa.Portal" and w["title"] == ("Share and allow control — Canopa" if capture_case else "Allow keyboard control — Canopa")), None)

        dialog = session.wait_for(popup, "combined Iced consent", timeout=30)
        if not dialog["is_floating"]:
            session.run(["niri", "msg", "action", "move-window-to-floating", "--id", str(dialog["id"])])
        for dimension, value in [("width", 680 if capture_case else 520), ("height", min(660, logical["height"] - 20) if capture_case else 340)]:
            session.run(["niri", "msg", "action", "set-window-" + dimension, "--id", str(dialog["id"]), str(value)])
        session.run(["niri", "msg", "action", "move-floating-window", "--id", str(dialog["id"]), "--x", "20", "--y", "10"])
        session.run(["niri", "msg", "action", "focus-window", "--id", str(dialog["id"])])
        time.sleep(.2)
        session.run(["wtype", "-M", "ctrl", "-k", "Return", "-m", "ctrl"])
        drain()
        assert path not in responses
        session.capture(case + "-fresh-consent.png")
        if case != "view-only":
            session.run([str(pointer), str(round(280 / logical["width"] * 1600)), str(round(130 / logical["height"] * 1000))])
        if capture_case:
            session.run(["wtype", "-k", "Down", "-k", "space"])
        time.sleep(.2)  # Let the selected state reach the next captured frame.
        session.capture(case + "-selected-consent.png")
        session.run(["wtype", "-M", "ctrl", "-k", "Return", "-m", "ctrl"])
        values = response(path)
        devices = 0 if case == "view-only" else 1
        result.setdefault("starts", []).append({"case": case, "results": values})
        assert values["devices"] == devices and not values["clipboard_enabled"] and "restore_token" not in values, values
        title = "Screen sharing — Canopa" if devices == 0 else "Remote control — Canopa"
        controls = session.wait_for(lambda: next((w for w in windows() if w["app_id"] == "org.canopa.Portal" and w["title"] == title), None), "ongoing combined controls")
        # Keep the asymmetric reference source visible at fractional scales.
        session.run(["niri", "msg", "action", "move-floating-window", "--id", str(controls["id"]), "--x", "20", "--y", "10"])
        session.capture(case + "-controls.png")
        remote = None
        worker_pid = None
        owned = None

        def children():
            found = set()
            for task in Path(f"/proc/{backend.pid}/task").glob("*"):
                try:
                    found.update(map(int, (task / "children").read_text().split()))
                except (OSError, ValueError):
                    pass
            return {pid: state for pid in found if (state := remote_fixture.identity(pid))
                and state["ppid"] == backend.pid and str(args.portal) in state["command"]
                and "--remote-input-worker" in state["command"] and state["state"] != "Z"}

        def notify(method, value, state):
            return call(caller, FRONTEND, PATH, REMOTE, method, "(oa{sv}iu)", (key, {}, value, state))

        def terminal(pid, state):
            now = remote_fixture.identity(pid)
            return now is None or now["start_ticks"] != state["start_ticks"] or now["state"] == "Z"

        def received(fragment):
            return any(fragment in line for line in receiver_log.read_text().splitlines()[before:])

        assert not children(), "input worker started before an input method was called"
        before = len(receiver_log.read_text().splitlines())
        if devices:
            session.run(["niri", "msg", "action", "focus-window", "--id", str(receiver_window["id"])])
            # The frontend's Notify calls may forward asynchronously. Actual key
            # delivery, not the empty method reply, is the acceptance barrier.
            try:
                notify("NotifyKeyboardKeysym", 0x0101f600, 1)
            except GLib.Error as error:
                result.setdefault("unavailable_symbol_responses", []).append(str(error))
            notify("NotifyKeyboardKeysym", 0x41, 1)
            session.wait_for(lambda: received("key 16 1 A"), "actual French symbol A")
            notify("NotifyKeyboardKeysym", 0x42, 1)
            session.wait_for(lambda: received("key 48 1 B"), "actual French symbol B")
            notify("NotifyKeyboardKeysym", 0x41, 0)
            session.wait_for(lambda: received("key 16 0 A"), "first symbol released")
            assert not received("key 42 0 "), "first symbol released another symbol's Shift"
            notify("NotifyKeyboardKeycode", 42, 1)
            workers = session.wait_for(lambda: children(), "owned legacy input worker")
            assert len(workers) == 1, workers
            worker_pid, owned = next(iter(workers.items()))
            if case == "owner-guards":
                properties = call(caller, FRONTEND, PATH, "org.freedesktop.DBus.Properties", "GetAll", "(s)", (REMOTE,))[0]
                assert properties["version"] == 2 and properties["AvailableDeviceTypes"] == args.available_devices, properties
                errors = []
                for identity in (False, True):
                    outsider = connect()
                    if identity:
                        registered(outsider)
                    try:
                        call(outsider, FRONTEND, PATH, REMOTE, "NotifyKeyboardKeycode", "(oa{sv}iu)", (key, {}, 30, 1))
                    except GLib.Error as error:
                        name = Gio.dbus_error_get_remote_error(error)
                        assert name == "org.freedesktop.DBus.Error.AccessDenied", str(error)
                        errors.append(name)
                    else:
                        raise AssertionError("another caller controlled this session")
                    finally:
                        outsider.close_sync(None)
                result["owner_guard_errors"] = errors
                notify("NotifyKeyboardKeycode", 184, 1)
                notify("NotifyKeyboardKeycode", 184, 0)
                session.wait_for(lambda: received("key 184 0 "), "owner input remains live after foreign denial")
            if case == "parallel-consent":
                pending = connect()
                registered(pending)
                pending_values = response(request(pending, REMOTE, "CreateSession", "(a{sv})", (options(session_handle_token=GLib.Variant("s", f"s{count}")),)))
                pending_key = pending_values["session_handle"]
                response(request(pending, REMOTE, "SelectDevices", "(oa{sv})", (pending_key, options(types=GLib.Variant("u", 1)))))
                pending_request = request(pending, REMOTE, "Start", "(osa{sv})", (pending_key, "", options()))
                pending_dialog = session.wait_for(lambda: next((w for w in windows() if w["app_id"] == "org.canopa.Portal" and w["title"] == "Allow keyboard control — Canopa"), None), "another session awaiting consent")
                if not pending_dialog["is_floating"]:
                    session.run(["niri", "msg", "action", "move-window-to-floating", "--id", str(pending_dialog["id"])])
                for dimension, value in [("width", 520), ("height", 340)]:
                    session.run(["niri", "msg", "action", "set-window-" + dimension, "--id", str(pending_dialog["id"]), str(value)])
                session.run(["niri", "msg", "action", "move-floating-window", "--id", str(pending_dialog["id"]), "--x", "20", "--y", str(max(10, logical["height"] - 340))])
                try:
                    call(pending, FRONTEND, PATH, REMOTE, "NotifyKeyboardKeycode", "(oa{sv}iu)", (pending_key, {}, 30, 1))
                except GLib.Error as error:
                    assert Gio.dbus_error_get_remote_error(error) == "org.freedesktop.DBus.Error.Failed", str(error)
                    result["pending_input_denied"] = True
                else:
                    raise AssertionError("input was accepted before consent")
                session.capture("parallel-consent-pending.png")
                session.run(["niri", "msg", "action", "focus-window", "--id", str(receiver_window["id"])])
                notify("NotifyKeyboardKeycode", 184, 1)
                notify("NotifyKeyboardKeycode", 184, 0)
                session.wait_for(lambda: received("key 184 0 "), "active input while another consent is unanswered")
                drain()
                assert pending_request not in responses
                call(pending, FRONTEND, pending_request, "org.freedesktop.portal.Request", "Close", "()", ())
                session.wait_for(lambda: not any(w["id"] == pending_dialog["id"] for w in windows()), "pending consent canceled")
                pending.close_sync(None)
            if case == "shortcut-burst":
                burst_before = len(receiver_log.read_text().splitlines())
                replies = {}
                sequence = [(29, 1), (56, 1), (54, 1), (183, 1), (183, 0), (54, 0), (56, 0), (29, 0)] * args.burst_repetitions
                def complete(client_connection, reply, index):
                    try:
                        replies[index] = client_connection.call_finish(reply).unpack()
                    except GLib.Error as error:
                        replies[index] = str(error)
                for index, (code, state) in enumerate(sequence):
                    if args.notify_mode == "reply-barriers":
                        replies[index] = notify("NotifyKeyboardKeycode", code, state)
                    elif args.notify_mode == "fire-and-forget":
                        message = Gio.DBusMessage.new_method_call(FRONTEND, PATH, REMOTE, "NotifyKeyboardKeycode")
                        message.set_body(GLib.Variant("(oa{sv}iu)", (key, {}, code, state)))
                        message.set_flags(Gio.DBusMessageFlags.NO_REPLY_EXPECTED)
                        assert caller.send_message(message, Gio.DBusSendMessageFlags.NONE)[0]
                        replies[index] = ()
                    else:
                        caller.call(FRONTEND, PATH, REMOTE, "NotifyKeyboardKeycode", GLib.Variant("(oa{sv}iu)", (key, {}, code, state)),
                            None, Gio.DBusCallFlags.NONE, 10000, None, complete, index)
                def delivered():
                    drain()
                    actual = []
                    for line in receiver_log.read_text().splitlines()[burst_before:]:
                        if line.startswith("key "):
                            fields = line.split()
                            code, state = int(fields[1]), int(fields[2])
                            if code in {29, 56, 54, 183}:
                                actual.append((code, state))
                    result["shortcut_burst"] = {"expected": sequence, "actual": actual,
                        "replies": replies, "session_closed": key in closed,
                        "controls_alive": any(w["id"] == controls["id"] for w in windows())}
                    return actual == sequence and len(replies) == len(sequence)
                session.wait_for(delivered, "ordered public keyboard shortcut burst")
                assert all(value == () for value in replies.values()), replies
            if case == "legacy-to-eis":
                previous_pid, previous = worker_pid, owned
                reply, fdlist = caller.call_with_unix_fd_list_sync(FRONTEND, PATH, REMOTE, "ConnectToEIS", GLib.Variant("(oa{sv})", (key, {})), None, Gio.DBusCallFlags.NONE, 40000, None, None)
                fds = fdlist.steal_fds()
                assert reply.unpack() == (0,) and len(fds) == 1
                ei = socket.socket(fileno=fds[0])
                worker_pid, uid, _ = struct.unpack("3i", ei.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
                owned = remote_fixture.identity(worker_pid)
                assert uid == os.getuid() and owned and owned["ppid"] == backend.pid and str(args.portal) in owned["command"] and "--remote-input-worker" in owned["command"]
                session.wait_for(lambda: terminal(previous_pid, previous), "legacy worker reaped before EIS")
                session.wait_for(lambda: received("key 48 0 ") and received("key 42 0 Shift_L") and received("mods 0 "), "legacy chord released before EIS delivery")
                result.setdefault("workers", []).append({"pid": previous_pid, "start_ticks": previous["start_ticks"], "terminal": True, "mode": "legacy-before-transition"})
                before = len(receiver_log.read_text().splitlines())
                try:
                    notify("NotifyKeyboardKeycode", 30, 1)
                except GLib.Error as error:
                    result["notify_after_eis_error"] = str(error)
                client_log = session.output / (case + "-client.log")
                with client_log.open("w") as stream:
                    remote = subprocess.Popen([str(client), "held"], stdin=ei, stdout=stream, stderr=stream, env=session.env, start_new_session=True)
                ei.close()
                fdlist = reply = None
                session.processes.append(remote)
                session.wait_for(lambda: received("key 16 1 A"), "actual EIS input after legacy transition")
                session.wait_for(lambda: "modifier update" in client_log.read_text(), "EI authoritative modifier feedback")
                assert not received("key 30 1 "), "Notify injected input after EIS connection"
        else:
            try:
                notify("NotifyKeyboardKeycode", 42, 1)
            except GLib.Error as error:
                result["view_only_notify_error"] = str(error)
            # Both frontend ownership/grants and the direct backend contracts
            # must deny this. No source worker may be created for view-only.
            assert not children()

        release_before = len(receiver_log.read_text().splitlines())

        def close():
            if not devices:
                assert not children(), "view-only approval started an input worker"
                assert not received("key 42 1 "), "view-only approval injected keyboard input"
            if remote is not None:
                session.processes.remove(remote)
            if case in ("budget-bytes", "budget-fds"):
                descriptors = None
                owned_fds = []
                pipe_identity = None
                if case == "budget-bytes":
                    extra = {"oversized": GLib.Variant("s", "x" * 8192)}
                else:
                    read_fd, write_fd = os.pipe()
                    owned_fds = [read_fd, write_fd]
                    pipe_stat = os.fstat(read_fd)
                    pipe_identity = (pipe_stat.st_dev, pipe_stat.st_ino)
                    descriptors = Gio.UnixFDList.new()
                    assert descriptors.append(read_fd) == 0
                    extra = {"unexpected_fd": GLib.Variant("h", 0)}
                try:
                    if descriptors is None:
                        call(caller, FRONTEND, PATH, REMOTE, "NotifyKeyboardKeycode", "(oa{sv}iu)", (key, extra, 42, 0))
                    else:
                        caller.call_with_unix_fd_list_sync(FRONTEND, PATH, REMOTE, "NotifyKeyboardKeycode", GLib.Variant("(oa{sv}iu)", (key, extra, 42, 0)), None, Gio.DBusCallFlags.NONE, 10000, descriptors, None)
                except GLib.Error as error:
                    name = Gio.dbus_error_get_remote_error(error)
                    assert name == ("org.freedesktop.DBus.Error.LimitsExceeded" if case == "budget-bytes" else "org.freedesktop.DBus.Error.InvalidArgs"), str(error)
                    result.setdefault("budget_errors", {})[case] = name
                else:
                    raise AssertionError("over-budget input was accepted")
                finally:
                    for fd in owned_fds:
                        os.close(fd)
                    if descriptors is not None:
                        for fd in descriptors.steal_fds():
                            os.close(fd)
                if pipe_identity is not None:
                    def released_fd():
                        for descriptor in Path(f"/proc/{frontend.pid}/fd").iterdir():
                            try:
                                value = descriptor.stat()
                            except FileNotFoundError:
                                continue
                            if (value.st_dev, value.st_ino) == pipe_identity:
                                return False
                        return True
                    session.wait_for(released_fd, "rejected input descriptor released by frontend")
                    result["budget_fd_released"] = True
            elif case in ("input-only", "unregistered-caller", "view-only", "owner-guards", "shortcut-burst", "parallel-consent", "ui-stop", "legacy-to-eis"):
                if case == "owner-guards":
                    assert not received("key 30 1 "), "foreign caller injected keyboard input"
                session.run(["niri", "msg", "action", "close-window", "--id", str(controls["id"])])
            elif case == "session-close":
                call(caller, FRONTEND, key, "org.freedesktop.portal.Session", "Close", "()", ())
            elif case == "worker-loss":
                current = remote_fixture.identity(worker_pid)
                assert current and current["start_ticks"] == owned["start_ticks"] and current["ppid"] == backend.pid
                os.kill(worker_pid, signal.SIGKILL)
            elif case == "caller-loss":
                caller.close_sync(None)
            else:
                session.processes.remove(backend)
                backend.kill()
                finish(backend, -signal.SIGKILL)

        if capture_case:
            session.check_stream(caller, key, values, 1, logical, pattern_box, close, label=case)
        else:
            assert "streams" not in values, values
            close()
        if remote is not None:
            finish(remote)
        if devices:
            def released(fragment):
                return any(fragment in line for line in receiver_log.read_text().splitlines()[release_before:])
            session.wait_for(lambda: released("key 42 0 Shift_L") and released("mods 0 ")
                and (remote is not None or released("key 48 0 ")), "held input released with common session resources")
            session.wait_for(lambda: terminal(worker_pid, owned), "owned input worker terminal")
            result.setdefault("workers", []).append({"pid": worker_pid, "start_ticks": owned["start_ticks"], "terminal": True, "mode": "eis" if remote is not None else "legacy"})
        if case == "legacy-to-eis":
            assert not received("key 30 1 "), "Notify injected input after EIS connection"
        session.wait_for(lambda: not any(w["id"] == controls["id"] for w in windows()), "shared controls retired")
        if case in ("input-only", "unregistered-caller", "view-only", "owner-guards", "shortcut-burst", "parallel-consent", "ui-stop", "worker-loss", "legacy-to-eis", "budget-bytes", "budget-fds"):
            session.wait_for(lambda: (drain(), key in closed)[1], "public shared Session.Closed")
        if not caller.is_closed():
            caller.close_sync(None)
        result["checks"].append(case + "-public-frontend-legacy-notify-and-common-lifetime")
        (session.output / "progress.json").write_text(json.dumps(result, indent=2) + "\n")
        print(case + ": passed", flush=True)
    assert frontend.poll() is None, ("frontend exited unexpectedly", frontend.returncode)
    connection.close_sync(None)
    result.update(session.media_result())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("niri", "portal", "harness"):
        parser.add_argument("--" + name, required=True, type=Path)
        parser.add_argument("--" + name + "-sha256", required=True)
    parser.add_argument("--frontend", type=Path, help="explicit private frontend candidate; requires its checksum")
    parser.add_argument("--frontend-sha256")
    parser.add_argument("--reference-frozen-compositor", action="store_true", help="reference the verified content-addressed compositor snapshot instead of copying it")
    parser.add_argument("--scheme", choices=("dark", "light"), default="dark")
    parser.add_argument("--available-devices", type=int, choices=(1,3), default=1, help="expected runtime device mask of the explicit compositor")
    parser.add_argument("--scale", type=float, default=1.0)
    parser.add_argument("--case", choices=CASES, action="append", help="run only these cases, in order; backend-kill must be last")
    parser.add_argument("--trace-notify", action="store_true", help="record private-bus public/backend Notify order")
    parser.add_argument("--burst-repetitions", type=int, choices=range(1, 5), default=1, help="repeat the shortcut within one bounded burst")
    parser.add_argument("--notify-mode", choices=("concurrent", "reply-barriers", "fire-and-forget"), default="concurrent", help="concurrent replies, public reply barriers or NoReplyExpected messages")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.case and "backend-kill" in args.case and args.case[-1] != "backend-kill":
        parser.error("backend-kill must be the final case")
    if bool(args.frontend) != bool(args.frontend_sha256):
        parser.error("--frontend and --frontend-sha256 must be supplied together")
    frontend_override = args.frontend is not None
    if args.frontend is None:
        args.frontend = next(p for p in [Path("/usr/libexec/xdg-desktop-portal"), Path("/usr/lib/xdg-desktop-portal")] if p.is_file())
        args.frontend_sha256 = digest(args.frontend)
    for name in ("niri", "portal", "harness", "frontend"):
        path = getattr(args, name).resolve()
        setattr(args, name, path)
        assert digest(path) == getattr(args, name + "_sha256")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    sources = [Path(__file__), ROOT / "scripts/test-remote-desktop-portal.py", ROOT / "scripts/screencast_media.py", ROOT / "scripts/validate-desktop.py", ROOT / "scripts/desktop-fixture/remote-input-ei-client.c", ROOT / "scripts/desktop-fixture/remote-input-receiver.c", ROOT / "scripts/desktop-fixture/portal-parent.c"]
    result = {"status": "running", "scheme": args.scheme, "scale": args.scale, "requested_cases": args.case or list(CASES), "notify_mode": args.notify_mode,
        "frontend_override": frontend_override, "reference_frozen_compositor": args.reference_frozen_compositor,
        "scope": "Private descriptor and standard XDG frontend; public legacy keyboard Notify, combined monitor pixels, transition to EIS and shared lifetime. Not pointer/touch, physical input, shipped metadata or distribution acceptance.", "checks": [],
        "binaries": {name: {"path": str(getattr(args, name)), "sha256": getattr(args, name + "_sha256")} for name in ("niri", "portal", "harness", "frontend")},
        "fixture_sources": [{"path": str(p.relative_to(ROOT)), "sha256": digest(p)} for p in sources]}
    archive = args.output / "verified-sources"
    for entry in result["fixture_sources"]:
        target = archive / entry["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / entry["path"], target)
        assert digest(target) == entry["sha256"]
    (archive / "receipt.json").write_text(json.dumps({"status": "exact sources archived before execution", "sources": result["fixture_sources"]}, indent=2) + "\n")
    try:
        with tempfile.TemporaryDirectory(prefix="canopa-remote-media-") as directory:
            session = MediaSession(Path(directory), args.output, args.niri, args.niri_sha256, reference_frozen=args.reference_frozen_compositor)
            try:
                verify(args, session, result)
            finally:
                session.close()
        for entry in result["binaries"].values():
            assert digest(Path(entry["path"])) == entry["sha256"]
        result["changed_fixture_sources"] = [entry["path"] for entry in result["fixture_sources"] if digest(ROOT / entry["path"]) != entry["sha256"]]
        assert not result["changed_fixture_sources"]
        result.update(status="passed", binaries_unchanged=True)
    except Exception as error:
        result.update(status="failed", error=str(error))
        raise
    finally:
        (args.output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
