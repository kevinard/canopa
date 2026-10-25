#!/usr/bin/env python3
"""Private notification receiver for the Files native acceptance test."""
import json
import argparse
from pathlib import Path
import sys

import gi
gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("output", type=Path)
parser.add_argument("--release", type=Path, help="Hold Notify replies until this private fixture file exists")
args = parser.parse_args()
output = args.output
events = []
connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
node = Gio.DBusNodeInfo.new_for_xml('''<node>
<interface name="org.freedesktop.Notifications">
 <method name="GetCapabilities"><arg type="as" direction="out"/></method>
 <method name="GetServerInformation"><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/></method>
 <method name="Notify"><arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/><arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/></method>
 <method name="CloseNotification"><arg type="u" direction="in"/></method>
 <signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal>
</interface>
<interface name="org.canopa.FilesFixture">
 <method name="Activate"><arg type="u" direction="in"/><arg type="s" direction="in"/></method>
</interface></node>''')


def call(bus, sender, path, interface, method, parameters, invocation):
    if method == "GetCapabilities":
        invocation.return_value(GLib.Variant("(as)", (["actions", "body"],)))
    elif method == "GetServerInformation":
        invocation.return_value(GLib.Variant("(ssss)", ("Files fixture", "Canopa", "1", "1.2")))
    elif method == "Notify":
        app, replaces, icon, title, body, actions, hints, timeout = parameters.unpack()
        number = len(events) + 1
        events.append({"id": number, "app": app, "title": title, "body": body, "actions": actions})
        pending = output.with_suffix(".tmp")
        pending.write_text(json.dumps(events))
        pending.replace(output)
        def answer():
            if args.release is not None and not args.release.exists():
                return True
            invocation.return_value(GLib.Variant("(u)", (number,)))
            return False
        if args.release is not None and not args.release.exists():
            output.with_suffix(".held").touch()
            GLib.timeout_add(10, answer)
        else:
            answer()
    elif method == "Activate":
        connection.emit_signal(None, path, "org.freedesktop.Notifications", "ActionInvoked", parameters)
        invocation.return_value(GLib.Variant("()", ()))
    else:
        invocation.return_value(GLib.Variant("()", ()))


registrations = [connection.register_object("/org/freedesktop/Notifications", interface, call, None, None)
                 for interface in node.interfaces]
owner = Gio.bus_own_name_on_connection(connection, "org.freedesktop.Notifications", Gio.BusNameOwnerFlags.NONE,
                                      lambda *args: output.with_suffix(".ready").touch(), None)
GLib.MainLoop().run()
