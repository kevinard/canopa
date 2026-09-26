#!/usr/bin/env python3
"""Send one portal notification with a sealable memfd icon."""

import base64
import os
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402


PNG_1X1 = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
)


def add_fd_icon(portal_id: str) -> int:
    icon_fd = os.memfd_create("canopa-portal-icon", os.MFD_ALLOW_SEALING)
    try:
        os.write(icon_fd, PNG_1X1)
        os.lseek(icon_fd, 0, os.SEEK_SET)

        fd_list = Gio.UnixFDList.new()
        handle = fd_list.append(icon_fd)
        notification = {
            "icon": GLib.Variant(
                "(sv)",
                ("file-descriptor", GLib.Variant("h", handle)),
            ),
        }
        parameters = GLib.Variant("(sa{sv})", (portal_id, notification))

        connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        proxy = Gio.DBusProxy.new_sync(
            connection,
            Gio.DBusProxyFlags.NONE,
            None,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Notification",
            None,
        )
        proxy.call_with_unix_fd_list_sync(
            "AddNotification",
            parameters,
            Gio.DBusCallFlags.NONE,
            -1,
            fd_list,
            None,
        )
    finally:
        os.close(icon_fd)

    return 0


def serve_application() -> int:
    application = Gio.Application(
        application_id="org.canopa.PortalFixture",
        flags=Gio.ApplicationFlags.IS_SERVICE,
    )
    application.connect("activate", lambda _application: None)
    application.hold()
    return application.run([sys.argv[0]])


def main() -> int:
    if sys.argv[1:] == ["--serve"]:
        return serve_application()
    if len(sys.argv) == 2:
        return add_fd_icon(sys.argv[1])

    print(f"usage: {sys.argv[0]} [--serve | PORTAL_ID]", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
