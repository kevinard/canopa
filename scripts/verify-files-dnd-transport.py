#!/usr/bin/env python3
"""Exercise the Iced drag transport against an independent GTK peer privately.

Build `cargo build -p canopa-files --example drag_transport` first. This verifies
native transport only; the actual Files operation workflow has separate gates.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import re
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from urllib.parse import quote_from_bytes

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def read(path):
    return json.loads(path.read_text()) if path.exists() else {}


def destroyed_source_acknowledged(path):
    wire = path.read_text()
    sources = re.findall(r"-> wl_data_device#\d+\.start_drag\(wl_data_source#(\d+)", wire)
    if not sources:
        return False
    destroyed = re.search(rf"-> wl_data_source#{sources[-1]}\.destroy\(\)", wire)
    return bool(destroyed) and f"wl_display#1.delete_id({sources[-1]})" in wire[destroyed.end():]


def verify(binary, session, scale, checks, cancel_control=False, gtk_receiver=False, compositor="niri"):
    home = session.runtime / "home"
    home.mkdir()
    session.env.update(HOME=str(home), GDK_BACKEND="wayland", GSK_RENDERER="cairo",
                       GTK_A11Y="none", GSETTINGS_BACKEND="memory", RUST_BACKTRACE="1")
    if compositor == "niri":
        session.start_niri('animations { off; }\nxwayland-satellite { off; }\n'
            'hotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; '
            'focus-ring { off; }; default-column-width { proportion 0.5; }; }\n',
            width=round(1280 * scale), height=round(800 * scale), scale=scale)
    else:
        session.start_private_bus()
        config = session.runtime / "sway.conf"
        config.write_text('output HEADLESS-1 mode 1280x800\nseat seat0 fallback true\n'
                          'xwayland disable\nfocus_follows_mouse no\ndefault_border none\n')
        session.env.pop("SWAYSOCK", None)
        session.env.update(WLR_BACKENDS="headless", WLR_HEADLESS_OUTPUTS="1", WLR_RENDERER="pixman")
        session.start(["sway", "-c", str(config)], "sway.log")
        session.env["SWAYSOCK"] = str(session.wait_for(lambda: list(session.runtime.glob("sway-ipc.*.sock")), "private Sway IPC")[0])
        session.env["WAYLAND_DISPLAY"] = str(session.wait_for(
            lambda: [p for p in session.runtime.glob("wayland-*") if p.is_socket()], "private Sway display")[0])
    session.start(["wtype", "-s", "600000"], "keyboard.log")
    pointer = session.build_pointer("drag")
    click = session.build_pointer("click")
    source = home / os.fsdecode(b"transport space \xff.txt")
    original = b"Original private native drag bytes\n"
    source.write_bytes(original)
    expected = ("file://" + quote_from_bytes(os.fsencode(source), safe="/") + "\r\n").encode()

    def windows():
        if compositor == "niri":
            return json.loads(session.run(["niri", "msg", "--json", "windows"]))
        result = []
        def visit(node):
            if node.get("app_id"):
                result.append({"id": node["id"], "title": node["name"], "is_focused": node["focused"],
                               "rect": node["rect"], "app_id": node["app_id"]})
            for child in node.get("nodes", []) + node.get("floating_nodes", []):
                visit(child)
        visit(json.loads(session.run(["swaymsg", "-t", "get_tree", "-r"])))
        return result

    def window(title):
        return next((w for w in windows() if w.get("title") == title), None)

    def focus(window):
        if compositor == "niri":
            session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        else:
            session.run(["swaymsg", f"[con_id={window['id']}]", "focus"])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "fixture window focus")

    def arrange(left, right):
        for w in [right, left]:
            focus(w)
            if compositor == "niri":
                session.run(["niri", "msg", "action", "set-column-width", "50%"])
        if compositor == "niri":
            session.run(["niri", "msg", "action", "move-column-to-first"])
        else:
            session.run(["swaymsg", "layout", "splith"])
            session.run(["swaymsg", f"[con_id={left['id']}]", "move", "left"])
            session.run(["swaymsg", f"[con_id={left['id']}]", "resize", "set", "width", "50", "ppt"])
            actual = windows()
            assert next(w for w in actual if w["id"] == left["id"])["rect"]["x"] == 0
            assert next(w for w in actual if w["id"] == right["id"])["rect"]["x"] == 640
        time.sleep(.3)

    def begin(name, reverse=False, source_y=500, trace=False):
        ready, release = session.runtime / f"{name}.ready", session.runtime / f"{name}.release"
        # Virtual coordinates normalize to 1600x1000, independently of scale.
        previous = session.env.get("WAYLAND_DEBUG")
        if trace:
            session.env["WAYLAND_DEBUG"] = "client"
        try:
            process = session.start([str(pointer), str(1200 if reverse else 400), str(source_y),
                str(400 if reverse else 1200), "500", str(ready), str(release)], f"{name}-pointer.log")
        finally:
            if previous is None:
                session.env.pop("WAYLAND_DEBUG", None)
            else:
                session.env["WAYLAND_DEBUG"] = previous
        session.wait_for(ready.exists, f"{name} held native drag")
        return process, release

    def release(drag):
        process, marker = drag
        marker.touch()
        process.wait(timeout=10)
        assert process.returncode == 0
        session.processes.remove(process)

    def clients(name, mode, action="copy", hold="hold"):
        peer_report = session.output / f"{name}-gtk.json"
        report = session.output / f"{name}-{'gtk-receiver' if gtk_receiver else 'iced'}.json"
        finish = session.runtime / f"{name}.finish"
        output = home / f"{name}-destination"
        session.env["WAYLAND_DEBUG"] = "client"
        peer = session.start(["/usr/bin/python3", str(ROOT / "scripts/desktop-fixture/files-dnd-transport-peer.py"),
            str(home), str(source), str(output), str(peer_report), str(finish), action], f"{name}-gtk.log")
        session.env.pop("WAYLAND_DEBUG")
        session.wait_for(lambda: read(peer_report).get("ready") and window("GTK DnD transport"), "GTK transport peer")
        session.env["WAYLAND_DEBUG"] = "client"
        if gtk_receiver:
            command = ["/usr/bin/python3", str(ROOT / "scripts/desktop-fixture/files-dnd-transport-peer.py"),
                str(home), str(source), str(output), str(report), str(finish), action,
                "GTK DnD receiver", "org.canopa.DragTransportReceiver"]
        else:
            command = [str(binary), mode, str(report), str(source), action, hold]
        probe = session.start(command, f"{name}-{'gtk-receiver' if gtk_receiver else 'iced'}.log")
        session.env.pop("WAYLAND_DEBUG")
        receiver_title = "GTK DnD receiver" if gtk_receiver else "Iced DnD transport"
        session.wait_for(lambda: read(report).get("ready") and window(receiver_title), "native receiver endpoint", timeout=40)
        return probe, peer, report, peer_report, finish, output

    if cancel_control:
        probe, peer, report, peer_report, finish, output = clients("cancel-control", "receiver")
        receiver_title = "GTK DnD receiver" if gtk_receiver else "Iced DnD transport"
        arrange(window("GTK DnD transport"), window(receiver_title))
        focus(window("GTK DnD transport"))
        drag = begin("cancel-control", source_y=250, trace=True)
        receiver_wire = session.output / f"cancel-control-{'gtk-receiver' if gtk_receiver else 'iced'}.log"
        if gtk_receiver:
            session.wait_for(lambda: re.search(r"wl_data_source#\d+\.action\(1\)", (session.output / "cancel-control-gtk.log").read_text()),
                             "independent GTK receiver negotiates Copy before cancellation")
        else:
            session.wait_for(lambda: read(report).get("offers") and read(report)["offers"][-1]["data"] is not None,
                             "independent GTK source serves URI bytes")
        assert drag[0].poll() is None and not drag[1].exists(), "pointer must remain held before cancellation"
        cancellation_offset = len(receiver_wire.read_text())
        finish.with_suffix(".cancel").touch()
        session.wait_for(lambda: read(peer_report).get("cancel_requested"), "GTK drag_cancel control")
        assert read(peer_report)["cancel_active"] == 1
        session.wait_for(lambda: destroyed_source_acknowledged(session.output / "cancel-control-gtk.log"),
                         "compositor acknowledges GTK source destruction")
        session.wait_for(lambda: re.search(r"wl_data_device#\d+\.leave\(\)", receiver_wire.read_text()[cancellation_offset:]),
                         "source destruction retires the receiver while the pointer remains held")
        pointer_events = re.findall(r"zwlr_virtual_pointer_v1#\d+\.button\(\d+, 272, (\d+)\)",
                                    (session.output / "cancel-control-pointer.log").read_text())
        assert pointer_events and pointer_events[-1] == "1", "cancellation was not acknowledged before native button release"
        release(drag)
        if gtk_receiver:
            session.wait_for(lambda: re.search(r"wl_data_device#\d+\.leave\(\)", receiver_wire.read_text()),
                             "independent GTK receiver ends the cancelled session")
        else:
            session.wait_for(lambda: read(report)["offers"][-1]["left"], "cancelled receiver ends the session")
            assert not read(report)["offers"][-1]["dropped"], "cancelled Iced offer received Drop after acknowledged source destruction"
        assert not re.search(r"wl_data_device#\d+\.drop\(\)", receiver_wire.read_text()), "cancelled native receiver got Drop after acknowledged source destruction"
        assert source.read_bytes() == original and not output.exists()
        checks.append("acknowledged independent GTK source cancellation prevents a drop on physical release")
        session.retire(probe); session.retire(peer)
        return checks

    probe, peer, report, peer_report, finish, output = clients("inbound", "receiver")
    arrange(window("GTK DnD transport"), window("Iced DnD transport"))
    for index in range(2):
        focus(window("GTK DnD transport"))
        drag = begin(f"inbound-{index}", source_y=250 if index == 0 else 750)
        session.wait_for(lambda: len(read(report).get("offers", [])) == index + 1 and read(report)["offers"][-1]["data"] is not None,
                         "prefetched incoming URI bytes")
        offers = read(report)["offers"]
        assert bytes(offers[-1]["data"]) == expected
        assert offers[-1]["selected"] == "Some(Copy)" and not offers[-1]["dropped"]
        assert source.read_bytes() == original and not output.exists()
        release(drag)
        session.wait_for(lambda: read(report)["offers"][-1]["dropped"], "latched native incoming drop")
        assert all(not offer["finished"] for offer in read(report)["offers"])
        assert all(offer["local_source"] is None for offer in read(report)["offers"])
    session.capture("transport-inbound-two-held.png")
    focus(window("Iced DnD transport"))
    session.run(["wtype", "-k", "Return"])
    session.wait_for(lambda: all(offer["finished"] for offer in read(report)["offers"]), "both exact held offers finish")
    session.wait_for(lambda: len(read(peer_report).get("sources", [])) == 2 and all((source["finished"] or source["cancelled"]) and source["dropped"] and source["action"] == 1 for source in read(peer_report)["sources"]), "both exact GTK sources have ended after their drops")
    # Niri can retire an already dropped source when a later grab starts.
    # That does not revoke accepted destination work or its retained offer.
    # Check each real wire source's Drop and terminal event independently.
    wire = (session.output / "inbound-gtk.log").read_text()
    native_sources = re.findall(r"-> wl_data_device#\d+\.start_drag\(wl_data_source#(\d+)", wire)
    assert len(native_sources) == 2 and len(set(native_sources)) == 2
    for source_id in native_sources:
        assert f"wl_data_source#{source_id}.dnd_drop_performed()" in wire
        assert any(f"wl_data_source#{source_id}.{event}()" in wire for event in ["dnd_finished", "cancelled"])
    assert not read(report)["errors"] and source.read_bytes() == original
    checks.append("GTK URI bytes preserve raw names; hover does not drop; a later drag preserves an earlier held offer; both finish independently")
    session.retire(probe); session.retire(peer)

    for action, mode in [("copy", "source"), ("move", "source-close")]:
        name = f"outbound-{action}"
        probe, peer, report, peer_report, finish, output = clients(name, mode, action)
        arrange(window("Iced DnD transport"), window("GTK DnD transport"))
        focus(window("Iced DnD transport"))
        drag = begin(name)
        session.wait_for(lambda: read(report).get("sources") and read(report)["sources"][-1]["selected"] == f"Some({action.title()})", "outgoing native action negotiation")
        assert source.read_bytes() == original and not output.exists()
        assert not read(report)["sources"][-1]["dropped"]
        release(drag)
        session.wait_for(lambda: read(peer_report).get("drops") and read(peer_report)["drops"][-1]["received"], "GTK receives and commits the private file")
        destination = output / source.name
        assert destination.read_bytes() == original
        assert bytes(read(peer_report)["drops"][-1]["uris"]) == expected
        session.wait_for(lambda: read(report)["sources"][-1]["dropped"], "outgoing DropPerformed is observable")
        assert not read(report)["sources"][-1]["finished"]
        if action == "move":
            session.wait_for(lambda: window("Iced DnD transport") is None and not read(report)["window_live"], "source has zero windows before completion")
            assert not source.exists() and probe.poll() is None
        else:
            assert source.read_bytes() == original
        session.capture(f"transport-{name}-held.png")
        finish.touch()
        session.wait_for(lambda: read(report)["sources"][-1]["finished"], "native completion after held GTK receiver")
        state = read(report)["sources"][-1]
        assert not state["cancelled"] and state["error"] is None and not read(report)["errors"]
        assert destination.read_bytes() == original
        if action == "move":
            assert not source.exists()
            source.write_bytes(original)
        checks.append(f"Iced to GTK {action}: no mutation before release, exact bytes, delayed native finish" + (", completion with zero Iced windows and no second deletion" if action == "move" else ", source preserved"))
        session.retire(probe); session.retire(peer)

    probe, peer, report, peer_report, finish, output = clients("stale", "stale")
    arrange(window("Iced DnD transport"), window("GTK DnD transport"))
    focus(window("Iced DnD transport"))
    for _ in range(2):
        session.run([str(click), "400", "500"])
    session.wait_for(lambda: read(report).get("stale_refused"), "original released grab refuses a later press")
    assert not read(report)["sources"] and not read(peer_report)["drops"]
    assert any("original pointer grab has ended" in error for error in read(report)["errors"])
    assert source.read_bytes() == original
    checks.append("a prepared original press cannot borrow a later live native pointer grab")
    session.retire(probe); session.retire(peer)

    probe, peer, report, peer_report, finish, output = clients("cancel", "source")
    arrange(window("Iced DnD transport"), window("GTK DnD transport"))
    focus(window("Iced DnD transport"))
    drag = begin("cancel")
    session.wait_for(lambda: read(report).get("sources"), "active outgoing source")
    session.run(["wtype", "-k", "Escape"])
    session.wait_for(lambda: read(report)["sources"][-1]["cancelled"], "native Escape cancellation")
    session.wait_for(lambda: destroyed_source_acknowledged(session.output / "cancel-iced.log"),
                     "compositor acknowledges cancelled Iced source destruction")
    release(drag)
    session.wait_for(lambda: any("wl_data_device#" in line and ".leave()" in line
                                 for line in (session.output / "cancel-gtk.log").read_text().splitlines()),
                     "receiver observes the ended native drag")
    assert source.read_bytes() == original, "Escape changed the source file"
    assert not output.exists(), "Escape admitted a destination mutation"
    assert not read(peer_report)["drops"], "receiver got a drop after acknowledged source destruction"
    checks.append("Escape cancels the actual native grab without a drop or mutation")
    session.retire(probe); session.retire(peer)

    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/canopa-files")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scale", type=float, default=1.)
    parser.add_argument("--cancel-control", action="store_true")
    parser.add_argument("--gtk-receiver", action="store_true", help="Use an independent GTK receiver for the cancellation control")
    parser.add_argument("--compositor", choices=("niri", "sway"), default="niri")
    args = parser.parse_args()
    if args.gtk_receiver and not args.cancel_control:
        parser.error("--gtk-receiver requires --cancel-control")
    if args.compositor != "niri" and (not args.gtk_receiver or args.scale != 1.):
        parser.error("the compositor comparison requires --cancel-control --gtk-receiver at scale 1")
    probe = args.binary.resolve().parent / "examples/drag_transport"
    assert args.gtk_receiver or probe.is_file(), "Build the drag_transport example before native acceptance"
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    result = {"status": "failed", "probe_sha256": None if args.gtk_receiver else hashlib.sha256(probe.read_bytes()).hexdigest(),
              "receiver": "gtk" if args.gtk_receiver else "iced", "compositor": args.compositor, "scale": args.scale, "checks": []}
    # Wayland and D-Bus socket paths must fit sockaddr_un even when artifacts
    # live beneath a long workspace path.
    with tempfile.TemporaryDirectory(prefix="canopa-dnd-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(probe, session, args.scale, result["checks"], args.cancel_control, args.gtk_receiver, args.compositor)
            result["status"] = "passed"
        except BaseException as error:
            result["error"] = str(error)
            try:
                command = (["swaymsg", "-t", "get_tree", "-r"] if args.compositor == "sway"
                           else ["niri", "msg", "--json", "windows"])
                result["windows"] = json.loads(session.run(command))
                session.capture("failure.png")
            except Exception:
                pass
            raise
        finally:
            try:
                session.close()
            finally:
                desktop.write_json(output / "summary.json", result)
    print(json.dumps(result))


if __name__ == "__main__":
    main()
