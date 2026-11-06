#!/usr/bin/env python3
"""Check cursor capabilities against simulated linked-library versions on a private bus."""
import argparse
import importlib.util
import json
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
NAME = "org.freedesktop.impl.portal.desktop.canopa"
PATH = "/org/freedesktop/portal/desktop"
INTERFACE = "org.freedesktop.impl.portal.ScreenCast"


def verify(args, session):
    import subprocess
    from gi.repository import Gio, GLib
    source = session.output / "version.c"
    source.write_text('#include <stdlib.h>\nconst char *pw_get_library_version(void) { return getenv("CANOPA_TEST_PW_VERSION"); }\n')
    shim = session.output / "version.so"
    subprocess.run(["cc", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC", str(source), "-o", str(shim)], check=True, timeout=30)
    session.start_private_bus()
    flags = Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION
    connection = Gio.DBusConnection.new_for_address_sync(session.env["DBUS_SESSION_BUS_ADDRESS"], flags, None, None)
    def call(destination, path, interface, method, signature="()", values=()):
        return connection.call_sync(destination, path, interface, method, GLib.Variant(signature, values),
            None, Gio.DBusCallFlags.NONE, 10000, None).unpack()
    def owns():
        return call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "NameHasOwner", "(s)", (NAME,))[0]
    checks = []
    versions = [("0.3.64", 3), ("1.4.7", 3), ("1.4.8", 7), ("1.6.10+fixture", 7), ("unknown", 3)]
    try:
        for index, (version, expected) in enumerate(versions):
            env = session.env.copy()
            session.env.update(LD_PRELOAD=str(shim), CANOPA_TEST_PW_VERSION=version)
            try:
                backend = session.start([str(args.binary), "--defaults"], f"backend-{index}.log")
            finally:
                session.env = env
            session.wait_for(owns, "private backend")
            actual = call(NAME, PATH, "org.freedesktop.DBus.Properties", "Get", "(ss)", (INTERFACE, "AvailableCursorModes"))[0]
            assert actual == expected, (version, actual, expected)
            key = f"/org/freedesktop/portal/desktop/session/capability{index}"
            request = f"/org/freedesktop/portal/desktop/request/capability{index}"
            assert call(NAME, PATH, INTERFACE, "CreateSession", "(oosa{sv})", (request+"create", key, "fixture", {}))[0] == 0
            options = {"cursor_mode": GLib.Variant("u", 4)}
            rejected = False
            try:
                response = call(NAME, PATH, INTERFACE, "SelectSources", "(oosa{sv})", (request+"select", key, "fixture", options))
                assert response[0] == 0, response
            except GLib.Error as error:
                rejected = Gio.DBusError.get_remote_error(error) == "org.freedesktop.DBus.Error.InvalidArgs"
                assert rejected, error
            assert rejected == (expected == 3), (version, rejected)
            if not rejected:
                call(NAME, key, "org.freedesktop.impl.portal.Session", "Close")
            checks.append({"simulated_library_version":version, "cursor_modes":actual, "metadata_rejected":rejected})
            session.retire(backend)
            session.wait_for(lambda: not owns(), "backend retired")
    finally:
        connection.close_sync(None)
    return {"status":"passed", "checks":checks, "scope":"version-policy shim; not an older PipeWire ABI/media acceptance"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.binary = args.binary.resolve(strict=True)
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="canopa-cast-capabilities-") as directory:
        session = desktop.Session(Path(directory), args.output)
        try:
            result = verify(args, session)
        except Exception as error:
            desktop.write_json(args.output / "result.json", {"status":"failed", "error":str(error)})
            raise
        finally:
            session.close()
    desktop.write_json(args.output / "result.json", result)
    print(json.dumps(result))


if __name__ == "__main__":
    main()
