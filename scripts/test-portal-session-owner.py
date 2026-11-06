#!/usr/bin/env python3
"""Exercise real frontend session ownership on an isolated bus, without a desktop."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import traceback

PUBLIC = "org.freedesktop.portal.Desktop"
BACKEND = "org.freedesktop.impl.portal.desktop.canopa"
PATH = "/org/freedesktop/portal/desktop"
PORTAL = "org.freedesktop.portal."
IMPL = "org.freedesktop.impl.portal."
INTERFACES = ("RemoteDesktop", "ScreenCast", "GlobalShortcuts", "InputCapture", "Inhibit")
PENDING_SCENARIOS = ("pending", "reacquire", "early-closed")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify(args, root, result):
    from gi.repository import Gio, GLib

    processes, connections, pending, registrations = [], [], [], []
    calls, responses, closed = [], {}, []
    busy_calls, busy_reply = [], {}
    env = dict(os.environ, XDG_RUNTIME_DIR=str(root), XDG_CURRENT_DESKTOP="Canopa",
               XDG_CONFIG_HOME=str(root / "config"), XDG_DATA_HOME=str(root / "data"),
               XDG_CACHE_HOME=str(root / "cache"), XDG_DATA_DIRS=str(root / "data"),
               GSETTINGS_SCHEMA_DIR="/usr/share/glib-2.0/schemas", GSETTINGS_BACKEND="memory",
               GIO_USE_VFS="local", DBUS_SYSTEM_BUS_ADDRESS="unix:path=" + str(root / "no-system-bus"))
    for key in ("DISPLAY", "WAYLAND_DISPLAY", "DBUS_STARTER_ADDRESS", "DBUS_STARTER_BUS_TYPE"):
        env.pop(key, None)
    for name in ("config", "data", "cache", "portals"):
        (root / name).mkdir()
    (root / "data/applications").mkdir()
    (root / "data/applications/org.canopa.SessionOwnerFixture.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=Session owner fixture\nExec=true\n")
    portals = root / "portals"
    env["XDG_DESKTOP_PORTAL_DIR"] = str(portals)
    (portals / "canopa.portal").write_text("[portal]\nDBusName=" + BACKEND + "\nInterfaces=" +
                                           ";".join(IMPL + name for name in INTERFACES) + ";\nUseIn=Canopa;\n")
    (portals / "portals.conf").write_text("[preferred]\ndefault=none\n" +
                                         "".join(IMPL + name + "=canopa\n" for name in INTERFACES))
    bus = subprocess.Popen(["dbus-daemon", "--session", "--nofork", "--print-address=1"],
                           stdout=subprocess.PIPE, stderr=(args.output / "bus.log").open("w"), text=True)
    processes.append(bus)
    env["DBUS_SESSION_BUS_ADDRESS"] = bus.stdout.readline().strip()

    def connect():
        connection = Gio.DBusConnection.new_for_address_sync(
            env["DBUS_SESSION_BUS_ADDRESS"], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT |
            Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        connections.append(connection)
        return connection

    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)

    def wait(condition, label, timeout=6):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            drain()
            for process in processes:
                assert process.poll() is None, (process.args, process.returncode, label)
            value = condition()
            if value:
                return value
            time.sleep(.01)
        raise AssertionError("Timed out: " + label)

    def call(connection, destination, path, interface, method, signature="()", values=(), fail=False):
        answer = {}

        def done(connection, reply):
            try:
                answer["value"] = connection.call_finish(reply).unpack()
            except GLib.Error as error:
                answer["error"] = str(error)
        connection.call(destination, path, interface, method, GLib.Variant(signature, values), None,
                        Gio.DBusCallFlags.NO_AUTO_START, 6000, None, done)
        wait(lambda: answer, "D-Bus " + method, timeout=8)
        assert ("error" in answer) == fail, answer
        return answer.get("error") if fail else answer["value"]

    def bus_call(connection, method, signature, values):
        return call(connection, "org.freedesktop.DBus", "/org/freedesktop/DBus",
                    "org.freedesktop.DBus", method, signature, values)

    def passed(name):
        result["checks"].append(name)
        (args.output / "progress.json").write_text(json.dumps(result, indent=2) + "\n")
        print(name, flush=True)

    infos = {name: Gio.DBusNodeInfo.new_for_xml((args.source / "data" /
             (IMPL + name + ".xml")).read_text()).interfaces[0] for name in (*INTERFACES, "Session")}
    original, replacement, caller, outsider = (connect() for _ in range(4))

    def property_get(connection, sender, path, interface, name):
        return GLib.Variant("u", {"version": 1, "AvailableDeviceTypes": 3,
                                "AvailableSourceTypes": 3, "AvailableCursorModes": 1,
                                "SupportedCapabilities": 3}.get(name, 0))

    def register_session(connection, path):
        def close(connection, sender, path, interface, method, parameters, invocation):
            calls.append({"owner": connection.get_unique_name(), "path": path, "method": method})
            if connection == original and path.endswith("/first0"):
                for busy in busy_calls:
                    busy.return_dbus_error("org.freedesktop.DBus.Error.Failed", "Session retired")
                busy_calls.clear()
            if args.scenario == "stalled" and connection == original:
                pending.append(invocation)
            else:
                invocation.return_value(GLib.Variant("()", ()))
        registrations.append((connection, connection.register_object(path, infos["Session"], close, None, None)))

    def create(connection, sender, path, interface, method, parameters, invocation):
        if method == "ConnectToEIS":
            busy_calls.append(invocation)
            return
        if method in ("Start", "SelectDevices"):
            invocation.return_value(GLib.Variant("(ua{sv})", (0, {"devices": GLib.Variant("u", 3)})))
            return
        assert method in ("CreateSession", "CreateMonitor"), method
        values = parameters.unpack()
        session = values[1]
        register_session(connection, session)
        reply = GLib.Variant("(u)", (0,)) if method == "CreateMonitor" else GLib.Variant(
            "(ua{sv})", (0, {"capabilities": GLib.Variant("u", 3)} if interface == IMPL + "InputCapture" else {}))
        if args.scenario == "early-closed" and "/first" in session:
            connection.emit_signal(None, session, IMPL + "Session", "Closed", GLib.Variant("()", ()))
        if args.scenario in ("pending", "reacquire") and connection == original and "/first" in session:
            pending.append((invocation, reply))
        else:
            invocation.return_value(reply)

    for connection in (original, replacement):
        for name in INTERFACES:
            registrations.append((connection, connection.register_object(PATH, infos[name], create, property_get, None)))

    caller.signal_subscribe(PUBLIC, PORTAL + "Request", "Response", None, None, Gio.DBusSignalFlags.NONE,
                            lambda c, s, p, i, n, v: responses.__setitem__(p, v.unpack()))
    caller.signal_subscribe(PUBLIC, PORTAL + "Session", "Closed", None, None, Gio.DBusSignalFlags.NONE,
                            lambda c, s, p, i, n, v: closed.append(p))
    session_paths = []

    def create_sessions(prefix):
        requests = []
        for index, name in enumerate(INTERFACES):
            token = prefix + str(index)
            options = {"handle_token": GLib.Variant("s", token),
                       "session_handle_token": GLib.Variant("s", token)}
            if name == "InputCapture":
                options["capabilities"] = GLib.Variant("u", 3)
            method = "CreateMonitor" if name == "Inhibit" else "CreateSession"
            signature, values = ("(sa{sv})", ("", options)) if name in ("InputCapture", "Inhibit") else ("(a{sv})", (options,))
            request = call(caller, PUBLIC, PATH, PORTAL + name, method, signature, values)[0]
            requests.append(request)
            session_paths.append(PATH + "/session/" + caller.get_unique_name()[1:].replace(".", "_") + "/" + token)
        return requests

    def completed(requests, success=True):
        for request in requests:
            response, values = wait(lambda: responses.get(request), "CreateSession response")
            assert (response == 0) == success, (request, response, values)
            if success:
                assert values["session_handle"] in session_paths, values

    try:
        assert bus_call(original, "RequestName", "(su)", (BACKEND, 1))[0] == 1
        with (args.output / "frontend.log").open("w") as log:
            frontend = subprocess.Popen([str(args.frontend), "--verbose"], stdout=log, stderr=log,
                                        env=env, start_new_session=True)
        processes.append(frontend)
        wait(lambda: bus_call(caller, "NameHasOwner", "(s)", (PUBLIC,))[0], "frontend activation")
        call(caller, PUBLIC, PATH, "org.freedesktop.host.portal.Registry", "Register", "(sa{sv})",
             ("org.canopa.SessionOwnerFixture", {}))
        requests = create_sessions("first")
        if args.scenario not in PENDING_SCENARIOS:
            completed(requests)
            for path in session_paths:
                error = call(outsider, PUBLIC, path, PORTAL + "Session", "Close", fail=True)
                assert "AccessDenied" in error, error
            assert not calls and not closed, (calls, closed)
            passed("five-session-types-preserve-caller-ownership")

        if args.scenario == "busy":
            path = session_paths[0]
            request = call(caller, PUBLIC, PATH, PORTAL + "RemoteDesktop", "SelectDevices",
                           "(oa{sv})", (path, {"types": GLib.Variant("u", 3)}))[0]
            assert wait(lambda: responses.get(request), "SelectDevices")[0] == 0
            request = call(caller, PUBLIC, PATH, PORTAL + "RemoteDesktop", "Start",
                           "(osa{sv})", (path, "", {}))[0]
            assert wait(lambda: responses.get(request), "Start")[0] == 0

            def connected(connection, reply):
                try:
                    busy_reply["value"] = connection.call_finish(reply).unpack()
                except GLib.Error as error:
                    busy_reply["error"] = str(error)
            caller.call(PUBLIC, PATH, PORTAL + "RemoteDesktop", "ConnectToEIS",
                        GLib.Variant("(oa{sv})", (path, {})), None,
                        Gio.DBusCallFlags.NO_AUTO_START, 15000, None, connected)
            wait(lambda: busy_calls, "backend call holding the frontend session lock")
            passed("real-frontend-worker-is-busy-in-a-synchronous-backend-call")

        if args.scenario == "early-closed":
            completed(requests, success=False)
            passed("late-success-cannot-reexport-a-backend-closed-session")
        elif args.scenario == "reacquire":
            assert bus_call(original, "ReleaseName", "(s)", (BACKEND,))[0] == 1
            assert bus_call(original, "RequestName", "(su)", (BACKEND, 1))[0] == 1
            for invocation, reply in pending:
                invocation.return_value(reply)
            completed(requests, success=False)
            passed("a-lost-and-reacquired-name-cannot-revive-pending-sessions")
        elif args.scenario == "loss":
            original.close_sync(None)
        elif args.scenario in ("replace", "pending", "stalled", "busy"):
            for path in session_paths:
                register_session(replacement, path)
            assert bus_call(replacement, "RequestName", "(su)", (BACKEND, 6))[0] == 1
            replaced_at = time.monotonic()
            if args.scenario == "pending":
                assert len(pending) == len(INTERFACES), pending
                for invocation, reply in pending:
                    invocation.return_value(reply)
                completed(requests, success=False)
                passed("late-original-success-cannot-export-replaced-sessions")
        elif args.scenario == "backend-closed":
            for path in session_paths:
                original.emit_signal(None, path, IMPL + "Session", "Closed", GLib.Variant("()", ()))
        elif args.scenario == "client-close":
            for path in session_paths:
                call(caller, PUBLIC, path, PORTAL + "Session", "Close")
        elif args.scenario == "client-loss":
            caller.close_sync(None)

        if args.scenario not in ("client-close", "client-loss", *PENDING_SCENARIOS):
            wait(lambda: len(closed) == len(INTERFACES), "public Session.Closed for every backend-owned session", timeout=3)
            assert sorted(closed) == sorted(session_paths), closed
            passed("every-public-session-closes-on-" + args.scenario)
        if args.scenario in ("replace", "pending", "reacquire", "stalled", "busy", "client-close", "client-loss"):
            wait(lambda: len(calls) == len(INTERFACES), "original backend session cleanup")
            assert all(item["owner"] == original.get_unique_name() for item in calls), calls
            assert sorted(item["path"] for item in calls) == sorted(session_paths), calls
            passed("cleanup-reaches-each-original-owner-once-and-never-the-replacement")
        if args.scenario == "backend-closed":
            assert not calls, calls
            passed("backend-closed-does-not-echo-close")
        if args.scenario not in ("client-loss",):
            for path in session_paths:
                error = call(caller, PUBLIC, path, PORTAL + "Session", "Close", fail=True)
                assert "UnknownMethod" in error or "UnknownObject" in error, error
            passed("retired-session-paths-cannot-be-used-again")
        if args.scenario in ("replace", "pending", "reacquire", "stalled", "busy"):
            assert bus_call(caller, "NameHasOwner", "(s)", (original.get_unique_name(),))[0]
            fresh_owner = original if args.scenario == "reacquire" else replacement
            assert bus_call(caller, "GetNameOwner", "(s)", (BACKEND,))[0] == fresh_owner.get_unique_name()
            previous = len(session_paths)
            completed(create_sessions("fresh"))
            assert len(closed) == (0 if args.scenario in PENDING_SCENARIOS else previous), closed
            for path in session_paths[previous:]:
                call(caller, PUBLIC, path, PORTAL + "Session", "Close")
            assert len(calls) == previous * 2 and all(item["owner"] == fresh_owner.get_unique_name()
                                                    for item in calls[previous:]), calls
            passed("replacement-accepts-only-fresh-sessions-while-old-owner-remains-alive")
        if args.scenario == "busy":
            wait(lambda: busy_reply, "busy frontend worker retired")
            assert "error" in busy_reply and not busy_calls, busy_reply
            passed("busy-worker-revocation-keeps-frontend-responsive-and-cleans-original-owner")
        if args.scenario == "stalled":
            wait(lambda: time.monotonic() - replaced_at > 5.5, "orphan cleanup timeout", timeout=7)
            assert call(caller, PUBLIC, PATH, "org.freedesktop.DBus.Properties", "Get",
                        "(ss)", (PORTAL + "RemoteDesktop", "version"))[0] >= 1
            passed("unresponsive-old-owner-cannot-block-frontend-or-outlive-bounded-cleanup")
        drain()
        if args.scenario in ("client-close", "client-loss", *PENDING_SCENARIOS):
            assert not closed, closed
            passed("no-spurious-closed-for-client-or-unexported-session")
        else:
            assert len(closed) == len(INTERFACES), closed
    finally:
        result.update(backend_calls=calls, closed_signals=closed)
        for connection in connections:
            if not connection.is_closed():
                connection.close_sync(None)
        for process in reversed(processes):
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        result["processes"] = [{"pid": p.pid, "command": p.args, "exit_code": p.poll()} for p in processes]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--frontend", required=True, type=Path)
    parser.add_argument("--frontend-sha256", required=True)
    parser.add_argument("--source", required=True, type=Path, help="Pinned upstream frontend XML directory parent")
    parser.add_argument("--scenario", required=True, choices=("loss", "replace", *PENDING_SCENARIOS, "stalled", "busy", "backend-closed", "client-close", "client-loss"))
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.frontend, args.source, args.output = args.frontend.resolve(), args.source.resolve(), args.output.resolve()
    assert digest(args.frontend) == args.frontend_sha256
    args.output.mkdir(exist_ok=False)
    shutil.copy2(__file__, args.output / Path(__file__).name)
    inputs = {str(args.source / "data" / (IMPL + name + ".xml")):
              digest(args.source / "data" / (IMPL + name + ".xml")) for name in (*INTERFACES, "Session")}
    result = {"status": "running", "scenario": args.scenario, "checks": [], "frontend": str(args.frontend),
              "frontend_sha256": args.frontend_sha256, "fixture_sha256": digest(Path(__file__)), "xml_inputs": inputs}
    started = time.monotonic()
    try:
        with tempfile.TemporaryDirectory(prefix="canopa-session-owner-") as directory:
            verify(args, Path(directory), result)
        result["status"] = "passed"
    except Exception as error:
        result.update(status="failed", error=repr(error), traceback=traceback.format_exc())
    result.update(seconds=round(time.monotonic() - started, 2),
                  frontend_unchanged=digest(args.frontend) == args.frontend_sha256,
                  fixture_unchanged=digest(Path(__file__)) == result["fixture_sha256"],
                  xml_unchanged=all(digest(Path(p)) == h for p, h in inputs.items()))
    (args.output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({key: result.get(key) for key in ("status", "seconds", "error")}))
    if result["status"] != "passed":
        print(result["traceback"])
    raise SystemExit(0 if result["status"] == "passed" and result["frontend_unchanged"] and result["fixture_unchanged"] and result["xml_unchanged"] else 1)


if __name__ == "__main__":
    main()
