#!/usr/bin/python3
"""Independent GTK wire peer. Only the private fixture tree may be mutated."""
import json
import os
from pathlib import Path
import shutil
import sys
from urllib.parse import quote_from_bytes, unquote_to_bytes, urlsplit
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, GLib, Gtk

root, source, output, report, release = map(Path, sys.argv[1:6])
action = Gdk.DragAction.MOVE if sys.argv[6] == "move" else Gdk.DragAction.COPY
title = sys.argv[7] if len(sys.argv) > 7 else "GTK DnD transport"
application_id = sys.argv[8] if len(sys.argv) > 8 else "org.canopa.DragTransportFixture"
assert source.resolve().is_relative_to(root.resolve())
assert output.resolve().is_relative_to(root.resolve())
state = {"ready": False, "drops": [], "sources": []}

def record():
    pending = report.with_suffix(".new")
    pending.write_text(json.dumps(state))
    pending.replace(report)


def activate(app):
    window = Gtk.ApplicationWindow(application=app, title=title)
    window.set_default_size(640, 650)
    window.set_titlebar(Gtk.HeaderBar())
    body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    window.set_child(body)
    # Separate controllers allow a second grab while the first native source
    # awaits completion. Reusing one Gtk.DragSource loses its active-drag slot.
    drag_sources = []
    for index in range(2):
        label = Gtk.Label(label=f"Independent GTK drag source {index + 1}")
        label.set_vexpand(True)
        body.append(label)
        drag = Gtk.DragSource()
        drag_sources.append(drag)
        drag.set_actions(Gdk.DragAction.COPY | Gdk.DragAction.MOVE)
        uri = ("file://" + quote_from_bytes(os.fsencode(source), safe="/") + "\r\n").encode("ascii")
        drag.connect("prepare", lambda *_: Gdk.ContentProvider.new_for_bytes("text/uri-list", GLib.Bytes.new(uri)))
        def began(_source, native):
            entry = {"dropped": False, "finished": False, "cancelled": False}
            state["sources"].append(entry)
            def changed(native, field):
                entry[field] = True
                entry["action"] = int(native.get_selected_action())
                record()
            native.connect("drop-performed", lambda native: changed(native, "dropped"))
            native.connect("dnd-finished", lambda native: changed(native, "finished"))
            native.connect("cancel", lambda native, _reason: changed(native, "cancelled"))
            record()
        drag.connect("drag-begin", began)
        # File-URI receivers own Move; never delete again on source completion.
        label.add_controller(drag)
    def cancel_requested():
        if not release.with_suffix(".cancel").exists():
            return True
        active = 0
        for drag in drag_sources:
            if drag.get_drag() is not None:
                active += 1
                drag.drag_cancel()
        state["cancel_requested"] = True
        state["cancel_active"] = active
        record()
        return False
    GLib.timeout_add(50, cancel_requested)
    formats = Gdk.ContentFormats.new(["text/uri-list"])
    target = Gtk.DropTargetAsync.new(formats, action)
    target.connect("accept", lambda _target, drop: drop.get_formats().contain_mime_type("text/uri-list"))
    def motion(_target, drop, _x, _y):
        drop.status(action, action)
        return action
    target.connect("drag-enter", motion)
    target.connect("drag-motion", motion)
    def dropped(_target, drop, _x, _y):
        cancellable = Gio.Cancellable()
        data = bytearray()
        timer = GLib.timeout_add(5000, lambda: (cancellable.cancel(), False)[1])
        entry = {"received": False, "finished": False, "action": int(action)}
        state["drops"].append(entry)
        def fail(error):
            entry["error"] = str(error)
            drop.finish(Gdk.DragAction(0))
            record()
        def opened(drop, result):
            try:
                stream, mime = drop.read_finish(result)
                assert mime == "text/uri-list"
                def received(stream, result):
                    try:
                        chunk = stream.read_bytes_finish(result).get_data()
                        data.extend(chunk)
                        assert len(data) <= 16 * 1024 * 1024
                        if chunk:
                            stream.read_bytes_async(65536, GLib.PRIORITY_DEFAULT, cancellable, received)
                            return
                        GLib.source_remove(timer)
                        stream.close(None)
                        paths = []
                        for line in data.decode("ascii").splitlines():
                            if not line or line.startswith("#"):
                                continue
                            uri = urlsplit(line)
                            assert uri.scheme == "file" and uri.netloc in ("", "localhost")
                            path = Path(os.fsdecode(unquote_to_bytes(uri.path)))
                            assert path.is_absolute() and path.resolve().is_relative_to(root.resolve())
                            paths.append(path)
                        assert paths
                        output.mkdir(exist_ok=True)
                        for path in paths:
                            destination = output / path.name
                            assert not destination.exists()
                            if action == Gdk.DragAction.MOVE:
                                shutil.move(path, destination)
                            else:
                                shutil.copy2(path, destination)
                        entry.update(received=True, uris=list(data), paths=[os.fsencode(path).hex() for path in paths])
                        record()
                        def finish():
                            if not release.exists():
                                return True
                            drop.finish(action)
                            entry["finished"] = True
                            record()
                            return False
                        if finish():
                            GLib.timeout_add(50, finish)
                    except Exception as error:
                        fail(error)
                stream.read_bytes_async(65536, GLib.PRIORITY_DEFAULT, cancellable, received)
            except Exception as error:
                fail(error)
        drop.read_async(["text/uri-list"], GLib.PRIORITY_DEFAULT, cancellable, opened)
        return True
    target.connect("drop", dropped)
    body.add_controller(target)
    window.present()
    state["ready"] = True
    record()


application = Gtk.Application(application_id=application_id, flags=Gio.ApplicationFlags.NON_UNIQUE)
application.connect("activate", activate)
application.run([])
