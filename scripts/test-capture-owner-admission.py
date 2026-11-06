#!/usr/bin/env python3
"""Reject a real compositor lacking creator cleanup before Iced capture consent."""
import argparse
import json
from pathlib import Path
import sys
import tempfile
import time
import traceback
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
from screencast_media import MediaSession, digest

ROOT = Path(__file__).resolve().parents[1]
BACKEND = "org.freedesktop.impl.portal.desktop.canopa"
PATH = "/org/freedesktop/portal/desktop"
IMPL = "org.freedesktop.impl.portal.ScreenCast"
CAPTURE = "org.gnome.Mutter.ScreenCast"
CAPTURE_PATH = "/org/gnome/Mutter/ScreenCast"


def verify(args, session, result):
    from gi.repository import Gio, GLib

    home = session.runtime / "home"
    home.mkdir()
    session.env.update(HOME=str(home), XDG_CURRENT_DESKTOP="Canopa")
    applications = Path(session.env["XDG_DATA_HOME"]) / "applications"
    applications.mkdir(parents=True)
    (applications / "org.canopa.OwnerTest.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=Owner contract test\nExec=true\n")
    session.start_niri(
        "debug { dbus-interfaces-in-non-session-instances; }\n"
        "animations { off; }\nxwayland-satellite { off; }\n"
        "hotkey-overlay { skip-at-startup; }\n", scale=1.25)
    connection = Gio.DBusConnection.new_for_address_sync(
        session.env["DBUS_SESSION_BUS_ADDRESS"],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT |
        Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

    def call(destination, path, interface, method, signature="()", values=(), timeout=5000):
        return connection.call_sync(
            destination, path, interface, method, GLib.Variant(signature, values),
            None, Gio.DBusCallFlags.NO_AUTO_START, timeout, None).unpack()

    def property_value(destination, path, interface, name):
        return call(destination, path, "org.freedesktop.DBus.Properties", "Get",
                    "(ss)", (interface, name))[0]

    def objects():
        found = {}
        for kind in ("Session", "Stream"):
            try:
                xml = call(CAPTURE, CAPTURE_PATH + "/" + kind,
                           "org.freedesktop.DBus.Introspectable", "Introspect")[0]
                found[kind] = sorted(node.get("name") for node in ET.fromstring(xml).findall("node"))
            except GLib.Error as error:
                assert Gio.DBusError.get_remote_error(error) in (
                    "org.freedesktop.DBus.Error.UnknownObject",
                    "org.freedesktop.DBus.Error.UnknownMethod"), error
                found[kind] = []
        return found

    try:
        result["capture_version"] = property_value(CAPTURE, CAPTURE_PATH, CAPTURE, "Version")
        assert result["capture_version"] >= 4, "Fixture lacks the otherwise supported capture API"
        try:
            value = property_value(CAPTURE, CAPTURE_PATH, CAPTURE, "CanopaCaptureOwnerVersion")
        except GLib.Error as error:
            assert Gio.DBusError.get_remote_error(error) == "org.freedesktop.DBus.Error.UnknownProperty", error
            value = None
        result["creator_cleanup_version"] = value
        assert value != 1, "This refusal test requires an unproven compositor"
        before = objects()
        session.start([str(args.portal)], "portal.log")
        session.wait_for(lambda: call(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
            "NameHasOwner", "(s)", (BACKEND,))[0], "private Rust backend")
        assert property_value(BACKEND, PATH, IMPL, "AvailableSourceTypes") == 3
        handle = "/org/freedesktop/portal/desktop/request/owner_test/"
        key = "/org/freedesktop/portal/desktop/session/owner_test/capture"
        app = "org.canopa.OwnerTest"
        assert call(BACKEND, PATH, IMPL, "CreateSession", "(oosa{sv})",
                    (handle + "create", key, app, {}))[0] == 0
        assert call(BACKEND, PATH, IMPL, "SelectSources", "(oosa{sv})",
                    (handle + "select", key, app, {"types": GLib.Variant("u", 1)}))[0] == 0
        response, timed_out = None, False
        try:
            response = call(BACKEND, PATH, IMPL, "Start", "(oossa{sv})",
                            (handle + "start", key, app, "", {}))
        except GLib.Error as error:
            if (Gio.DBusError.get_remote_error(error) == "org.freedesktop.DBus.Error.NoReply" or
                    error.matches(Gio.io_error_quark(), Gio.IOErrorEnum.TIMED_OUT)):
                timed_out = True
            else:
                raise
        windows = json.loads(session.run(["niri", "msg", "--json", "windows"]))
        result.update(response=response, start_timeout=timed_out, windows=windows,
                      objects_before=before, objects_after=objects())
        assert response == (2, {}), "Unproven compositor was not rejected before capture consent"
        assert not windows, "Unproven compositor exposed capture consent"
        assert result["objects_after"] == before, "Refusal mutated recording objects"
        result["checks"].extend([
            "otherwise-supported-compositor-has-no-creator-cleanup-contract",
            "production-start-rejects-before-consent",
            "refusal-creates-no-recording-objects"])
    finally:
        connection.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("portal", "niri"):
        parser.add_argument("--" + name, required=True, type=Path)
        parser.add_argument("--" + name + "-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    for name in ("portal", "niri"):
        path = getattr(args, name).resolve(strict=True)
        setattr(args, name, path)
        if digest(path) != getattr(args, name + "_sha256"):
            parser.error(name + " checksum mismatch")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    paths = [args.portal, args.niri, Path(__file__),
             ROOT / "scripts/screencast_media.py", ROOT / "scripts/validate-desktop.py"]
    pins = {str(path): digest(path) for path in paths}
    result = {"status": "running", "checks": [], "pins": pins}
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="canopa-capture-admission-") as directory:
        session = MediaSession(Path(directory), args.output, args.niri, args.niri_sha256,
                               reference_frozen=True)
        try:
            verify(args, session, result)
            result["status"] = "passed"
        except Exception as error:
            result.update(status="failed", error=repr(error), traceback=traceback.format_exc())
        finally:
            processes = list(session.processes)
            session.close()
            result["processes"] = [{"pid": process.pid, "command": process.args,
                                    "exit_code": process.poll()} for process in processes]
    result.update(seconds=round(time.monotonic() - started, 2),
                  changed_inputs=[path for path, checksum in pins.items()
                                  if digest(Path(path)) != checksum])
    if result["changed_inputs"] or any(row["exit_code"] is None for row in result["processes"]):
        result["status"] = "failed"
    (args.output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({key: result.get(key) for key in ("status", "error", "response", "start_timeout")}))
    raise SystemExit(0 if result["status"] == "passed" else 1)


if __name__ == "__main__":
    main()
