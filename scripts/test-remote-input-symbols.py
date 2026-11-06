#!/usr/bin/env python3
"""Private native proof of compositor-owned legacy keyboard symbol leases.

This exercises the separately negotiated typed protocol, not portal Notify,
Iced consent, physical input or pointer/touch/distribution acceptance.
"""
import argparse
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
SERVICE = "org.canopa.Niri.RemoteInput1"
PATH = "/org/canopa/Niri/RemoteInput1"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


class Typed:
    def __init__(self, sock):
        self.socket = sock
        self.sequence = 0

    def packet(self):
        data = bytearray()
        while len(data) < 24:
            chunk = self.socket.recv(24 - len(data))
            assert chunk, "EOF before typed reply"
            data.extend(chunk)
        sequence, status, *mods = struct.unpack("<6I", data)
        assert status <= 4 and (sequence != 0 or status == 0), (sequence, status)
        return sequence, status, mods

    def send(self, kind, value, pressed, status=0):
        self.sequence += 1
        self.socket.sendall(struct.pack("<4I", self.sequence, kind, value, int(pressed)))
        for _ in range(64):
            sequence, actual, mods = self.packet()
            if sequence == 0:
                continue
            assert sequence == self.sequence and actual == status, (sequence, actual, self.sequence, status)
            return mods
        raise AssertionError("too many feedback packets before command acknowledgement")

    def eof(self):
        data = bytearray()
        while True:
            chunk = self.socket.recv(4096)
            if not chunk:
                break
            data.extend(chunk)
            assert len(data) <= 1024 * 1024
        assert len(data) % 24 == 0, "truncated typed feedback"


def verify(args, session, result):
    from gi.repository import Gio, GLib
    session.env["PATH"] = str(args.niri.parent) + os.pathsep + session.env["PATH"]
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "fr,us"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
''')
    connection = Gio.DBusConnection.new_for_address_sync(session.env["DBUS_SESSION_BUS_ADDRESS"],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

    def bus(method, signature, values):
        return connection.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
            method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()

    def owner_ready():
        try:
            return bus("GetNameOwner", "(s)", (SERVICE,))[0]
        except GLib.Error:
            return None

    owner = session.wait_for(owner_ready, "compositor input service")
    pid = bus("GetConnectionUnixProcessID", "(s)", (owner,))[0]
    niri = next(p for p in session.processes if p.args[0] == "niri")
    assert pid == niri.pid and Path(f"/proc/{pid}/exe").resolve() == args.niri
    result.update(service_owner=owner, service_pid_matches_compositor=True)
    version = connection.call_sync(owner, PATH, "org.freedesktop.DBus.Properties", "Get",
        GLib.Variant("(ss)", (SERVICE, "SymbolVersion")), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
    assert version == 1, version

    def snapshot():
        return connection.call_sync(owner, PATH, SERVICE, "KeyboardSnapshot", GLib.Variant("()", ()),
            None, Gio.DBusCallFlags.NONE, 5000, None).unpack()

    def open_lease(typed=True):
        value, descriptors = connection.call_with_unix_fd_list_sync(owner, PATH, SERVICE,
            "OpenKeyboardSymbols" if typed else "OpenKeyboard", GLib.Variant("()", ()),
            None, Gio.DBusCallFlags.NONE, 5000, None, None)
        index, keymap, initial = value.unpack()
        assert keymap and len(keymap.encode()) < 1024 * 1024
        sock = socket.socket(fileno=descriptors.get(index))
        peer_pid, peer_uid, _ = struct.unpack("3i", sock.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        assert peer_pid == pid and peer_uid == os.getuid()
        assert fcntl.fcntl(sock.fileno(), fcntl.F_GETFD) & fcntl.FD_CLOEXEC
        sock.settimeout(3)
        return (Typed(sock) if typed else sock), initial

    for mode, suffix in [("client-header", ".h"), ("private-code", ".c")]:
        subprocess.run(["wayland-scanner", mode, "/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml",
            str(session.output / ("xdg-shell" + suffix))], check=True, timeout=15)
    receiver_binary = session.output / "receiver"
    subprocess.run(["cc", "-Wall", "-Wextra", "-Werror", "-I", str(session.output),
        str(ROOT / "scripts/desktop-fixture/remote-input-receiver.c"), str(session.output / "xdg-shell.c"),
        "-lwayland-client", "-lxkbcommon", "-o", str(receiver_binary)], check=True, timeout=30)
    receiver = session.start([str(receiver_binary)], "receiver.log")
    log = session.output / "receiver.log"
    session.wait_for(lambda: "focused" in log.read_text(), "harmless receiver")

    def lines():
        assert receiver.poll() is None
        return log.read_text().splitlines()

    def key(code, pressed, before, symbol=None):
        prefix = f"key {code} {int(pressed)} "
        return session.wait_for(lambda: [line for line in lines()[before:] if line.startswith(prefix)
            and (symbol is None or line == prefix + symbol)], f"actual key {code}/{pressed}/{symbol}")

    def clear():
        session.wait_for(lambda: snapshot()[1][0] == 0, "source modifiers cleared")

    def passed(name):
        result["checks"].append(name)
        (session.output / "progress.json").write_text(json.dumps(result, indent=2) + "\n")
        print(name + ": passed", flush=True)

    lease, _ = open_lease()
    try:
        before = len(lines())
        shift = lease.send(1, ord("A"), True)[0]
        assert shift
        key(16, True, before, "A")
        assert lease.send(1, ord("B"), True)[0] == shift
        key(48, True, before, "B")
        assert lease.send(1, ord("A"), False)[0] == shift
        key(16, False, before)
        assert not any(line.startswith("key 42 0 ") for line in lines()[before:])
        assert lease.send(1, ord("B"), False)[0] == 0
        key(42, False, before)
    finally:
        lease.socket.close()
    clear()
    passed("french-symbol-chords-retain-shared-shift-after-authoritative-feedback")

    lease, _ = open_lease()
    try:
        before = len(lines())
        lease.send(1, ord("A"), True)
        lease.send(0, 42, True)
        assert lease.send(1, ord("A"), False)[0]
        assert lease.send(0, 42, False)[0] == 0
        key(42, False, before)
        before = len(lines())
        assert lease.send(1, ord("@"), True)[0]
        key(11, True, before, "at")
        assert lease.send(1, ord("@"), False)[0] == 0
    finally:
        lease.socket.close()
    clear()
    passed("mixed-raw-symbol-modifier-ownership-and-french-altgr")

    lease, initial = open_lease()
    try:
        before = len(lines())
        locked = lease.send(0, 58, True)[2]
        lease.send(0, 58, False)
        assert locked != initial[2]
        # No external snapshot or delay: the next command resolves on Niri's thread.
        assert lease.send(1, ord("a"), True)[2] == locked
        key(16, True, before, "a")
        assert lease.send(1, ord("a"), False)[0] == 0
        lease.send(0, 58, True)
        assert lease.send(0, 58, False)[2] == initial[2]
        session.run(["niri", "msg", "action", "switch-layout", "next"])
        sequence, status, mods = lease.packet()
        assert (sequence, status, mods[3]) == (0, 0, 1)
        before = len(lines())
        assert lease.send(1, ord("a"), True)[3] == 1
        key(30, True, before, "a")
        lease.send(1, ord("a"), False)
        session.run(["niri", "msg", "action", "switch-layout", "next"])
        sequence, status, mods = lease.packet()
        assert (sequence, status, mods[3]) == (0, 0, 0)
    finally:
        lease.socket.close()
    clear()
    passed("ordered-caps-and-live-layout-use-current-state-without-mirroring")

    lease, _ = open_lease()
    try:
        before = len(lines())
        lease.send(1, ord("a"), True)
        lease.send(1, ord("A"), True, status=3)
        lease.send(1, ord("a"), True, status=1)
        lease.send(1, 0x0101f600, True, status=2)
        lease.send(1, ord("z"), False, status=1)
        assert not any(line.startswith("key 42 1 ") for line in lines()[before:])
        lease.send(1, ord("a"), False)
        key(16, False, before)
    finally:
        lease.socket.close()
    clear()
    passed("logical-refusals-have-exact-acks-and-preserve-prior-owned-keys")

    external, _ = open_lease(False)
    lease, _ = open_lease()
    try:
        before = len(lines())
        external.sendall(struct.pack("<II", 42, 1))
        key(42, True, before)
        session.wait_for(lambda: snapshot()[1][0] != 0, "other source Shift")
        lease.send(1, ord("A"), True)
        lease.socket.close()
        key(16, False, before)
        assert snapshot()[1][0] != 0, "typed EOF released another source's Shift"
        external.sendall(struct.pack("<II", 42, 0))
        key(42, False, before)
    finally:
        lease.socket.close()
        external.close()
    clear()
    passed("typed-eof-releases-its-chord-and-preserves-independent-v1-source")

    for label, payload in [("out-of-order", struct.pack("<4I", 3, 0, 42, 0)),
                           ("unknown-kind", struct.pack("<4I", 2, 2, 42, 0)),
                           ("bad-state", struct.pack("<4I", 2, 0, 42, 2)),
                           ("truncated", b"\x02\0\0")]:
        lease, _ = open_lease()
        try:
            before = len(lines())
            lease.send(1, ord("A"), True)
            lease.socket.sendall(payload)
            lease.socket.shutdown(socket.SHUT_WR)
            lease.eof()
            key(42, False, before)
            clear()
        finally:
            lease.socket.close()
        passed(label + "-retires-source-and-held-chord")

    leases = [open_lease(index % 2 == 0)[0] for index in range(8)]
    try:
        try:
            unexpected, _ = open_lease()
        except GLib.Error as error:
            assert "LimitsExceeded" in str(error)
        else:
            unexpected.socket.close()
            raise AssertionError("typed lease bypassed common admission")
    finally:
        for lease in leases:
            (lease.socket if isinstance(lease, Typed) else lease).close()

    def recovered():
        try:
            lease, _ = open_lease()
            lease.socket.close()
            return True
        except GLib.Error as error:
            assert "LimitsExceeded" in str(error)
            return False

    session.wait_for(recovered, "shared lease admission recovery")
    passed("evdev-and-typed-leases-share-eight-slot-admission-and-recover")

    lease, _ = open_lease()
    try:
        before = len(lines())
        lease.send(1, ord("A"), True)
        # Do not read replies. A slow peer must retire rather than queue input
        # acknowledgements indefinitely or continue without its held state.
        payload = b"".join(struct.pack("<4I", sequence, 0, 30, int(sequence % 2 == 0))
            for sequence in range(2, 10002))
        try:
            lease.socket.sendall(payload)
        except (BrokenPipeError, ConnectionResetError):
            pass
        key(42, False, before)
        key(16, False, before)
        clear()
    finally:
        lease.socket.close()
    passed("slow-typed-feedback-reader-retires-source-and-held-chord")

    lease, _ = open_lease()
    try:
        before = len(lines())
        lease.send(1, ord("A"), True)
        old_map = snapshot()[0]
        config = session.runtime / "niri.kdl"
        config.write_text(config.read_text().replace('layout "fr,us"', 'layout "us"'))
        session.wait_for(lambda: snapshot()[0] != old_map, "configured keymap changed")
        lease.eof()
        key(42, False, before)
        clear()
    finally:
        lease.socket.close()
    passed("keymap-change-retires-idle-typed-source-and-held-chord")
    connection.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--niri", required=True, type=Path)
    parser.add_argument("--niri-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.niri = args.niri.resolve()
    assert args.niri.name == "niri" and digest(args.niri) == args.niri_sha256
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    result = {"status": "running", "niri": str(args.niri), "sha256": args.niri_sha256,
        "scope": __doc__, "checks": [], "fixture_sources": [{"path": str(path.relative_to(ROOT)), "sha256": digest(path)}
            for path in (Path(__file__), ROOT / "scripts/validate-desktop.py", ROOT / "scripts/desktop-fixture/remote-input-receiver.c")]}
    archive = args.output / "verified-sources"
    for entry in result["fixture_sources"]:
        target = archive / entry["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / entry["path"], target)
        assert digest(target) == entry["sha256"]
    (archive / "receipt.json").write_text(json.dumps(result["fixture_sources"], indent=2) + "\n")
    try:
        with tempfile.TemporaryDirectory(prefix="canopa-symbol-input-") as directory:
            session = desktop.Session(Path(directory), args.output)
            try:
                verify(args, session, result)
            finally:
                session.close()
        assert digest(args.niri) == args.niri_sha256
        result["changed_fixture_sources"] = [entry["path"] for entry in result["fixture_sources"] if digest(ROOT / entry["path"]) != entry["sha256"]]
        assert not result["changed_fixture_sources"]
        result.update(status="passed", compositor_bytes_unchanged=True)
    except Exception as error:
        result.update(status="failed", error=str(error))
        raise
    finally:
        (args.output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
