#!/usr/bin/python3
"""Independent GTK file-URI drag peer; every copied path is private fixture data."""
import json
from pathlib import Path
import shutil
import sys
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, GLib, Gtk

root, output, result, source = map(Path, sys.argv[1:])
assert output.resolve().is_relative_to(root.resolve())
assert source.resolve().is_relative_to(root.resolve())


def activate(app):
    window = Gtk.ApplicationWindow(application=app, title="DnD Peer")
    window.set_default_size(640, 600)
    window.set_titlebar(Gtk.HeaderBar())
    body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
    body.set_margin_top(20); body.set_margin_bottom(20)
    body.set_margin_start(20); body.set_margin_end(20)
    button = Gtk.Button(label="Drag fixture file")
    button.set_vexpand(True)
    body.append(button)
    window.set_child(body)
    drag = Gtk.DragSource()
    drag.set_actions(Gdk.DragAction.COPY | Gdk.DragAction.MOVE)
    uri = Gio.File.new_for_path(str(source)).get_uri() + "\r\n"
    drag.connect("prepare", lambda *_: Gdk.ContentProvider.new_for_bytes("text/uri-list", GLib.Bytes.new(uri.encode())))
    # As in a file manager, the receiving URI consumer owns a Move transaction.
    drag.connect("drag-end", lambda *_: None)
    button.add_controller(drag)
    target = Gtk.DropTarget.new(Gdk.FileList.__gtype__, Gdk.DragAction.COPY)

    def dropped(_target, value, _x, _y):
        paths = [Path(file.get_path()) for file in value.get_files()]
        assert paths and all(path.resolve().is_relative_to(root.resolve()) for path in paths)
        output.mkdir(exist_ok=True)
        for path in paths:
            shutil.copy2(path, output / path.name)
        pending = result.with_suffix(".new")
        pending.write_text(json.dumps([str(path) for path in paths]))
        pending.replace(result)
        return True

    target.connect("drop", dropped)
    body.add_controller(target)
    window.present()


application = Gtk.Application(application_id="org.canopa.FilesFixture", flags=Gio.ApplicationFlags.NON_UNIQUE)
application.connect("activate", activate)
application.run([])
