#!/usr/bin/env python3
"""Exercise shared forks on private Sway outputs, including real grabbed tray popups.

Niri integration is covered separately by Notify's desktop and shell placement fixtures.
Requires the desktop runner dependencies plus wtype. Never uses live desktop services.
"""

import argparse
import importlib.util
import json
from pathlib import Path
import re
import shlex
import sys
import tempfile
import time

from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def serve_tray():
    import gi
    gi.require_version("Gio", "2.0")
    from gi.repository import Gio, GLib
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    xml = """<node><interface name="org.kde.StatusNotifierItem">
      <property name="IconName" type="s" access="read"/>
      <property name="Status" type="s" access="read"/>
      <property name="ItemIsMenu" type="b" access="read"/>
      <property name="Menu" type="o" access="read"/>
    </interface><interface name="com.canonical.dbusmenu">
      <method name="GetLayout"><arg type="i" direction="in"/><arg type="i" direction="in"/>
        <arg type="as" direction="in"/><arg type="u" direction="out"/>
        <arg type="(ia{sv}av)" direction="out"/></method>
      <method name="AboutToShow"><arg type="i" direction="in"/><arg type="b" direction="out"/></method>
      <method name="Event"><arg type="i" direction="in"/><arg type="s" direction="in"/>
        <arg type="v" direction="in"/><arg type="u" direction="in"/></method>
    </interface></node>"""
    properties = {"IconName": GLib.Variant("s", "application-x-executable"),
                  "Status": GLib.Variant("s", "Active"), "ItemIsMenu": GLib.Variant("b", True),
                  "Menu": GLib.Variant("o", "/Menu")}
    def method(_bus, _sender, _path, _interface, name, _parameters, invocation):
        if name == "GetLayout":
            item = GLib.Variant("(ia{sv}av)", (1, {"label": GLib.Variant("s", "Fork fixture action")}, []))
            invocation.return_value(GLib.Variant("(u(ia{sv}av))", (1, (0, {}, [item]))))
        elif name == "AboutToShow":
            invocation.return_value(GLib.Variant("(b)", (False,)))
        else:
            print("menu event", flush=True)
            invocation.return_value(GLib.Variant("()", ()))
    interfaces = Gio.DBusNodeInfo.new_for_xml(xml).interfaces
    bus.register_object("/StatusNotifierItem", interfaces[0], method,
                        lambda _b, _s, _p, _i, name: properties[name], None)
    bus.register_object("/Menu", interfaces[1], method, None, None)
    bus.call_sync("org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher",
                  "org.kde.StatusNotifierWatcher", "RegisterStatusNotifierItem",
                  GLib.Variant("(s)", ("/StatusNotifierItem",)), None,
                  Gio.DBusCallFlags.NONE, 5000, None)
    print("tray registered", flush=True)
    GLib.MainLoop().run()


def verify(args, session):
    runtime, output = session.runtime, session.output
    bus = runtime / "bus.conf"
    bus.write_text(f'''<busconfig><type>session</type><listen>unix:tmpdir={runtime}</listen>
<policy context="default"><allow send_destination="*"/><allow receive_sender="*"/>
<allow own="*"/></policy></busconfig>''')
    session.start(["dbus-daemon", f"--config-file={bus}", "--nofork", "--print-address=1"], "dbus.log")
    session.env["DBUS_SESSION_BUS_ADDRESS"] = session.wait_for(
        lambda: (output / "dbus.log").read_text().strip(), "private bus").splitlines()[0]
    config = runtime / "sway.conf"
    config.write_text("output HEADLESS-1 mode 1600x1000\nseat seat0 fallback true\nxwayland disable\n")
    session.env.update(WLR_BACKENDS="headless", WLR_HEADLESS_OUTPUTS="1", WLR_RENDERER="pixman")
    session.start(["sway", "-c", str(config)], "sway.log")
    session.env["SWAYSOCK"] = str(session.wait_for(lambda: list(runtime.glob("sway-ipc.*.sock")), "Sway")[0])
    session.env["WAYLAND_DISPLAY"] = str(session.wait_for(
        lambda: [p for p in runtime.glob("wayland-*") if p.is_socket()], "Wayland")[0])
    config = runtime / "config/niribar/config.kdl"
    config.parent.mkdir(parents=True)
    config.write_text('bar "test" { output "all"; edge "top"; collapsed-width 44; modules { start "tray"; }; }\nmodules { module "tray" { type "tray"; }; }\n')
    source = ROOT / "scripts/desktop-fixture"
    protocol = str(source / "wlr-virtual-pointer-unstable-v1.xml")
    session.run(["wayland-scanner", "client-header", protocol, str(runtime / "virtual-pointer.h")])
    session.run(["wayland-scanner", "private-code", protocol, str(runtime / "virtual-pointer.c")])
    flags = shlex.split(session.run(["pkg-config", "--cflags", "--libs", "wayland-client"]))
    pointer = str(runtime / "click")
    session.run(["cc", "-Wall", "-Wextra", "-Werror", "-I"+str(runtime), str(source / "click.c"),
                 str(runtime / "virtual-pointer.c"), "-o", pointer, *flags])
    session.env["WAYLAND_DEBUG"] = "client"
    session.start([str(args.niribar)], "niribar.log")
    session.env.pop("WAYLAND_DEBUG")
    session.wait_for(lambda: (runtime / "niribar.sock").is_socket(), "Niribar")
    session.wait_for(lambda: "org.kde.StatusNotifierWatcher" in session.run([
        "busctl", "--address="+session.env["DBUS_SESSION_BUS_ADDRESS"], "list"]), "tray watcher")
    session.start([sys.executable, str(Path(__file__).resolve()), "--serve-tray"], "tray.log")
    session.wait_for(lambda: "tray registered" in (output / "tray.log").read_text(), "tray fixture")
    def bar_visible():
        session.run(["grim", str(output / "closed.png")])
        return Image.open(output / "closed.png").convert("RGB").getpixel((800, 20)) == (30, 30, 46)
    session.wait_for(bar_visible, "initial bar frame")
    time.sleep(1)
    session.run([pointer, "15", "20"])
    trace = lambda: (output / "niribar.log").read_text()
    session.wait_for(lambda: "xdg_popup" in trace() and ".grab(" in trace(), "grabbed popup")
    session.wait_for(lambda: re.search(r"xdg_popup[@#]\d+\.configure\(", trace()), "popup configuration")
    time.sleep(1)
    before_click = trace()
    assert ".popup_done(" not in before_click, "Popup was dismissed before the outside click"
    assert not re.search(r"xdg_popup[@#]\d+\.destroy\(", before_click), "Popup closed before outside click"
    before = Image.open(output / "closed.png").convert("RGB")
    def popup_visible():
        session.run(["grim", str(output / "popup.png")])
        after = Image.open(output / "popup.png").convert("RGB")
        return ImageChops.difference(before, after).crop((0, 44, 500, 400)).getbbox()
    session.wait_for(popup_visible, "rendered popup below the bar")
    before_click = trace()
    assert ".popup_done(" not in before_click, "Popup was dismissed before the outside click"
    session.run([pointer, "1200", "500"])
    session.wait_for(lambda: ".popup_done(" in trace()[len(before_click):], "outside-click popup dismissal")
    session.run(["swaymsg", "output", "HEADLESS-1", "scale", "1.25"])
    session.wait_for(lambda: "preferred_scale(150)" in trace(), "fractional scale")
    time.sleep(1)
    session.run(["grim", str(output / "scaled.png")])
    # Remove every output with the same shell process alive, then restore it.
    creates = trace().count(".get_layer_surface(")
    session.run(["swaymsg", "output", "HEADLESS-1", "disable"])
    session.wait_for(lambda: ".global_remove(" in trace(), "output removal")
    time.sleep(1)
    session.run(["swaymsg", "output", "HEADLESS-1", "enable"])
    session.wait_for(lambda: trace().count(".get_layer_surface(") > creates, "restored bar surface")
    def restored_bar_visible():
        session.run(["grim", str(output / "restored.png")])
        frame = Image.open(output / "restored.png").convert("RGB")
        return frame.getpixel((800, 50)) == (30, 30, 46) and frame.getpixel((800, 60)) != (30, 30, 46)
    session.wait_for(restored_bar_visible, "rendered replacement bar at scale 1.25")
    desktop.write_json(output / "result.json", {"passed": True, "sway": session.run(["sway", "--version"]).strip(),
        "checks": ["tray popup grab", "outside-click dismissal", "fractional scale 1.25",
                   "all-output removal and restoration without shell restart"]})
    print(f"Shared fork desktop checks passed: {output}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serve-tray", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--niribar", type=Path)
    parser.add_argument("--artifacts", type=Path)
    args = parser.parse_args()
    if args.serve_tray:
        serve_tray()
        return
    if args.niribar is None or args.artifacts is None:
        parser.error("--niribar and --artifacts are required")
    args.niribar = args.niribar.resolve(strict=True)
    output = args.artifacts.resolve()
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="canopa-fork-desktop-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(args, session)
        finally:
            session.close()


if __name__ == "__main__":
    main()
