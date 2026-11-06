#!/usr/bin/env python3
"""Verify real capture retirement on creator loss, including queued requests."""
import argparse
import json
import os
from pathlib import Path
import signal
import tempfile
import time
import traceback
import xml.etree.ElementTree as ET

from screencast_media import MediaSession, digest

ROOT = Path(__file__).resolve().parents[1]
SERVICE = "org.gnome.Mutter.ScreenCast"
BASE = "/org/gnome/Mutter/ScreenCast"
BUS = "org.freedesktop.DBus"
BUS_PATH = "/org/freedesktop/DBus"


def verify(args, session, result):
    from gi.repository import Gio, GLib

    output = session.start_niri(
        "debug { dbus-interfaces-in-non-session-instances; }\n"
        "animations { off; }\nxwayland-satellite { off; }\n"
        "hotkey-overlay { skip-at-startup; }\n", scale=1.25)
    clients, closed, nodes = [], [], {}

    def connect():
        connection = Gio.DBusConnection.new_for_address_sync(
            session.env["DBUS_SESSION_BUS_ADDRESS"],
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT |
            Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        clients.append(connection)
        return connection

    def call(connection, path, interface, method, signature="()", values=(), *, destination=SERVICE):
        return connection.call_sync(
            destination, path, interface, method, GLib.Variant(signature, values),
            None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None).unpack()

    def wait(predicate, description, timeout=3):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            while GLib.MainContext.default().pending():
                GLib.MainContext.default().iteration(False)
            value = predicate()
            if value:
                return value
            time.sleep(.02)
        raise AssertionError("Timed out: " + description)

    observer = connect()
    observer.signal_subscribe(
        SERVICE, SERVICE + ".Stream", "PipeWireStreamAdded", None, None,
        Gio.DBusSignalFlags.NONE,
        lambda connection, sender, path, interface, member, value:
            nodes.__setitem__(path, value.unpack()[0]))
    observer.signal_subscribe(
        SERVICE, SERVICE + ".Session", "Closed", None, None, Gio.DBusSignalFlags.NONE,
        lambda connection, sender, path, interface, member, value: closed.append(path))

    def introspect(path):
        try:
            return ET.fromstring(call(observer, path, "org.freedesktop.DBus.Introspectable", "Introspect")[0])
        except GLib.Error as error:
            assert Gio.DBusError.get_remote_error(error) in (
                BUS + ".Error.UnknownObject", BUS + ".Error.UnknownMethod"), error
            return None

    def object_present(path):
        node = introspect(path)
        return node is not None and any(
            item.get("name") == SERVICE + ".Session" for item in node.findall("interface"))

    def objects():
        found = {}
        for kind in ("Session", "Stream"):
            node = introspect(BASE + "/" + kind)
            found[kind] = sorted(item.get("name") for item in node.findall("node")) if node is not None else []
        return found

    def producer_present(capture):
        return any(item["id"] == capture["node"] and
                   int(item.get("info", {}).get("props", {}).get("object.serial", 0)) == capture["serial"]
                   for item in session.registry())

    def create(connection):
        return call(connection, BASE, SERVICE, "CreateSession", "(a{sv})", ({},))[0]

    def start(connection):
        path = create(connection)
        stream = call(connection, path, SERVICE + ".Session", "RecordMonitor", "(sa{sv})",
                      (output, {"cursor-mode": GLib.Variant("u", 0)}))[0]
        call(connection, path, SERVICE + ".Session", "Start")
        node = wait(lambda: nodes.get(stream), "real producer announced", timeout=6)
        item = wait(lambda: next((item for item in session.registry() if item["id"] == node), None),
                    "real producer registered", timeout=6)
        return {"path": path, "stream": stream, "node": node,
                "serial": int(item["info"]["props"]["object.serial"]),
                "owner": connection.get_unique_name()}

    def stop(connection, capture):
        call(connection, capture["path"], SERVICE + ".Session", "Stop")
        wait(lambda: not producer_present(capture) and not object_present(capture["path"]),
             "explicit Stop retires producer and object")
        wait(lambda: capture["path"] in closed, "Closed delivered")
        assert closed.count(capture["path"]) == 1

    def owners():
        first_client, second_client = connect(), connect()
        first, second = start(first_client), start(second_client)
        first_client.close_sync(None)
        retired = False
        try:
            wait(lambda: not producer_present(first) and not object_present(first["path"]),
                 "dead creator recording retired")
            retired = True
        except AssertionError:
            pass
        details = {"first_retired": retired, "first_object_present": object_present(first["path"]),
                   "first_producer_present": producer_present(first),
                   "second_object_present": object_present(second["path"]),
                   "second_producer_present": producer_present(second)}
        result["owner_loss"] = details
        assert details["second_object_present"] and details["second_producer_present"]
        if not retired:
            stop(observer, first)
        else:
            wait(lambda: first["path"] in closed, "dead creator Closed delivered")
            assert closed.count(first["path"]) == 1
        stop(second_client, second)
        result["explicit_stop_controls_passed"] = True
        assert retired, "Creator disappeared but its real producer and Session survived"
        result["checks"].extend([
            "creator-loss-retires-real-producer-and-session", "other-owner-stays-live",
            "explicit-stop-retires-only-its-capture", "closed-delivered-once"])

    def queued(kind):
        healthy_client, creator = connect(), connect()
        healthy = start(healthy_client)
        expected = objects()
        path = create(creator) if kind == "record" else BASE
        creator_name = creator.get_unique_name()
        service_owner = call(observer, BUS_PATH, BUS, "GetNameOwner", "(s)",
                             (SERVICE,), destination=BUS)[0]
        monitor, replies, serial = connect(), [], {}

        def receive(connection, message, incoming, user_data):
            if (incoming and message.get_sender() == service_owner and
                    message.get_destination() == creator_name and
                    message.get_reply_serial() == serial.get("value")):
                replies.append({"type": int(message.get_message_type()),
                                "error": message.get_error_name(),
                                "reply_serial": message.get_reply_serial()})
            return message

        filter_id = monitor.add_filter(receive, None)
        # The private-bus monitor observes the actual reply even after its
        # destination disappears. This prevents an empty-tree check from passing
        # before the queued call has actually completed.
        rules = [f"type='{message_type}',sender='{service_owner}',destination='{creator_name}'"
                 for message_type in ("method_return", "error")]
        call(monitor, BUS_PATH, BUS + ".Monitoring", "BecomeMonitor", "(asu)",
             (rules, 0), destination=BUS)
        niri = next(process for process in session.processes if process.args[0] == str(session.compositor))
        assert niri.poll() is None
        stopped = False
        try:
            os.kill(niri.pid, signal.SIGSTOP)
            stopped = True
            message = Gio.DBusMessage.new_method_call(
                SERVICE, path, SERVICE + (".Session" if kind == "record" else ""),
                "RecordMonitor" if kind == "record" else "CreateSession")
            message.set_body(GLib.Variant("(sa{sv})", (output, {})) if kind == "record"
                             else GLib.Variant("(a{sv})", ({},)))
            sent, serial["value"] = creator.send_message(message, Gio.DBusSendMessageFlags.NONE)
            assert sent
            creator.flush_sync(None)
            creator.close_sync(None)
            assert not call(observer, BUS_PATH, BUS, "NameHasOwner", "(s)",
                            (creator_name,), destination=BUS)[0]
            os.kill(niri.pid, signal.SIGCONT)
            stopped = False
            wait(lambda: replies, "queued call reply actually emitted", timeout=6)
            details = {"reply": replies[0], "before": expected, "objects_after_reply": objects(),
                       "other_owner_live_at_reply": producer_present(healthy) and object_present(healthy["path"])}
            result.setdefault("queued", {})[kind] = details
            if kind == "create":
                assert replies[0]["type"] == int(Gio.DBusMessageType.ERROR), replies
            wait(lambda: objects() == expected, "queued dead-owner objects retired")
            assert producer_present(healthy) and object_present(healthy["path"])
            details.update(after=objects(), other_owner_live=True)
            result["checks"].append("queued-" + kind + "-creator-loss-does-not-leak-or-stop-other-owner")
        finally:
            if stopped and niri.poll() is None:
                os.kill(niri.pid, signal.SIGCONT)
            monitor.remove_filter(filter_id)
            monitor.close_sync(None)
        stop(healthy_client, healthy)

    try:
        if args.case in ("all", "owners"):
            owners()
        for kind in ("create", "record"):
            if args.case in ("all", "queued-" + kind):
                queued(kind)
    finally:
        result["closed"] = closed
        for connection in clients:
            if not connection.is_closed():
                connection.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--niri", required=True, type=Path)
    parser.add_argument("--niri-sha256", required=True)
    parser.add_argument("--case", choices=("all", "owners", "queued-create", "queued-record"), default="all")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.niri = args.niri.resolve()
    assert digest(args.niri) == args.niri_sha256
    args.output = args.output.resolve()
    args.output.mkdir(exist_ok=False, parents=True)
    sources = [Path(__file__), ROOT / "scripts/screencast_media.py",
               ROOT / "scripts/native_compositor.py", ROOT / "scripts/validate-desktop.py"]
    pins = {str(path): digest(path) for path in sources}
    result = {"status": "running", "checks": [], "case": args.case, "source_pins": pins,
              "niri_sha256": args.niri_sha256}
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="canopa-capture-owner-") as directory:
        session = MediaSession(Path(directory), args.output, args.niri, args.niri_sha256, reference_frozen=True)
        try:
            verify(args, session, result)
            result["status"] = "passed"
        except Exception as error:
            result.update(status="failed", error=repr(error), traceback=traceback.format_exc())
        finally:
            processes = list(session.processes)
            session.close()
            result["processes"] = [{"pid": process.pid, "command": process.args, "exit_code": process.poll()}
                                   for process in processes]
    result.update(seconds=round(time.monotonic() - started, 2),
                  changed_sources=[str(path) for path in sources if digest(path) != pins[str(path)]],
                  binary_unchanged=digest(args.niri) == args.niri_sha256)
    (args.output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({key: result.get(key) for key in ("status", "seconds", "case", "error", "checks")}), flush=True)
    raise SystemExit(0 if result["status"] == "passed" and not result["changed_sources"] and result["binary_unchanged"] else 1)


if __name__ == "__main__":
    main()
