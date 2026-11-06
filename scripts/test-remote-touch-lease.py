#!/usr/bin/env python3
"""Private mapped touch/Wayland acceptance; no public portal or physical-device claim."""
import argparse
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import tempfile
import time
import traceback

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from screencast_media import MediaSession, digest

INPUT = 'org.canopa.Niri.RemoteInput1'
INPUT_PATH = '/org/canopa/Niri/RemoteInput1'
CAST = 'org.gnome.Mutter.ScreenCast'
CAST_PATH = '/org/gnome/Mutter/ScreenCast'
TOOLS = ROOT / 'target/native-test-tools/remote-touch'


def receiver():
    """One small stable native caller, reused across runs."""
    TOOLS.mkdir(parents=True, exist_ok=True)
    source = ROOT / 'scripts/desktop-fixture/remote-input-touch-receiver.c'
    xml = Path('/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml')
    idle_xml = Path('/usr/share/wayland-protocols/staging/ext-idle-notify/ext-idle-notify-v1.xml')
    inputs = {str(p): digest(p) for p in (source, xml, idle_xml)}
    inputs['compiler'] = subprocess.check_output(['cc', '--version'], text=True).splitlines()[0]
    receipt = TOOLS / 'build.json'; binary = TOOLS / 'receiver'
    if receipt.exists() and binary.exists():
        old = json.loads(receipt.read_text())
        if old.get('inputs') == inputs and old.get('sha256') == digest(binary):
            return binary
    for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
        subprocess.run(['wayland-scanner', mode, str(xml), str(TOOLS / ('xdg-shell' + suffix))], check=True)
        subprocess.run(['wayland-scanner', mode, str(idle_xml), str(TOOLS / ('idle-notify' + suffix))], check=True)
    command = ['cc', '-Wall', '-Wextra', '-Werror', '-I', str(TOOLS), str(source),
               str(TOOLS / 'xdg-shell.c'), str(TOOLS / 'idle-notify.c'), '-lwayland-client', '-o', str(binary)]
    subprocess.run(command, check=True)
    receipt.write_text(json.dumps(dict(inputs=inputs, command=command, sha256=digest(binary),
                                      bytes=binary.stat().st_size), indent=2) + '\n')
    return binary


def verify(args, session, result):
    from gi.repository import Gio, GLib
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
layout { border { off; }; focus-ring { off; }; }
''')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    sockets = []
    def call(dest, path, interface, method, signature='()', values=()):
        return connection.call_sync(dest, path, interface, method, GLib.Variant(signature, values),
            None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None).unpack()
    def owner():
        try: return call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
                         'GetNameOwner', '(s)', (INPUT,))[0]
        except GLib.Error: return None
    name = session.wait_for(owner, 'private input service')
    pid = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
               'GetConnectionUnixProcessID', '(s)', (name,))[0]
    assert any(p.pid == pid for p in session.processes)
    assert Path(f'/proc/{pid}/exe').resolve() == session.compositor.resolve()
    assert call(name, INPUT_PATH, 'org.freedesktop.DBus.Properties', 'Get', '(ss)', (INPUT, 'TouchVersion')) == (1,)
    client = session.start([str(args.receiver)], 'receiver.log')
    log = session.output / 'receiver.log'
    def lines():
        assert client.poll() is None
        return log.read_text().splitlines() if log.exists() else []
    def wait(predicate, label): return session.wait_for(lambda: predicate(lines()), label)
    wait(lambda ls: any(v.startswith('configured ') for v in ls), 'Wayland receiver mapped')
    initial_caps = int(next(v for v in lines() if v.startswith('caps ')).split()[1])
    assert initial_caps & 4, 'Nested Winit touchscreen capability was removed before any lease'
    def action(*values): return session.run(['niri', 'msg', 'action', *map(str, values)])
    wid = session.wait_for(lambda: next((w['id'] for w in json.loads(session.run(
        ['niri', 'msg', '--json', 'windows'])) if w['pid'] == client.pid), None), 'receiver window')
    action('move-window-to-floating', '--id', wid)
    action('set-window-width', '--id', wid, 400); action('set-window-height', '--id', wid, 300)
    action('move-floating-window', '--id', wid, '--x', 100, '--y', 100)
    wait(lambda ls: 'configured 400 300' in ls, 'fixed capture geometry')
    capture = call(CAST, CAST_PATH, CAST, 'CreateSession', '(a{sv})', ({},))[0]
    stream = call(CAST, capture, CAST + '.Session', 'RecordWindow', '(a{sv})',
                  ({'window-id': GLib.Variant('t', wid)},))[0]
    sequences = {}
    def open_touch():
        value, fds = connection.call_with_unix_fd_list_sync(name, INPUT_PATH, INPUT,
            'OpenMappedTouch', GLib.Variant('(oao)', (capture, [stream])), None,
            Gio.DBusCallFlags.NO_AUTO_START, 5000, None, None)
        index, mappings = value.unpack(); assert len(mappings) == 1 and list(mappings[0][2]) == [400, 300], mappings
        sock = socket.socket(fileno=fds.get(index)); sock.settimeout(5); sockets.append(sock)
        sequences[sock.fileno()] = 0
        return sock, mappings[0][0]
    def passed(label, **details):
        result['checks'].append(dict(name=label, **details))
        (session.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
        print(label, flush=True)
    try:
        try: open_touch()
        except GLib.Error: pass
        else: raise AssertionError('Unstarted capture admitted touch')
        passed('unstarted-capture-rejected')
        nodes = []
        token = connection.signal_subscribe(None, CAST + '.Stream', 'PipeWireStreamAdded', stream,
            None, Gio.DBusSignalFlags.NONE, lambda c,s,p,i,n,v: nodes.append(v.unpack()[0]))
        call(CAST, capture, CAST + '.Session', 'Start')
        def drain():
            while GLib.MainContext.default().pending(): GLib.MainContext.default().iteration(False)
        session.wait_for(lambda: (drain(), nodes)[1], 'real capture producer node')
        connection.signal_unsubscribe(token)
        def send(sock, kind, contact=0, stream_id=0, x=0., y=0., ack=True):
            sequence = sequences.get(sock.fileno(), 0) + 1; sequences[sock.fileno()] = sequence
            sock.sendall(struct.pack('<IIIIQdd', sequence, kind, contact, 0, stream_id, x, y))
            if ack:
                data = b''
                while len(data) < 16:
                    chunk = sock.recv(16-len(data)); assert chunk, 'Closed before command acknowledgment'; data += chunk
                assert struct.unpack('<IIII', data) == (sequence, 0, 0, 0)
        def down(sock, sid, contact, x, y):
            before = len(lines()); send(sock, 1, contact, sid, x, y); send(sock, 0)
            found = wait(lambda ls: next((v for v in ls[before:] if v.startswith('down ')), None), 'actual touch down')
            values = found.split(); assert list(map(float, values[2:])) == [x, y], found
            return int(values[1])
        first, sid = open_touch(); second, sid2 = open_touch()
        wait(lambda ls: any(v.startswith('caps ') and int(v.split()[1]) & 4 for v in ls), 'remote-only seat touch capability')
        if args.idle_check:
            wait(lambda ls: [v for v in ls if v in ['idled','resumed']][-1:] == ['idled'], 'receiver idle before remote touch')
        idle_before=len(lines())
        a = down(first, sid, 7, 10.25, 20.5)
        if args.idle_check:
            wait(lambda ls: 'resumed' in ls[idle_before:], 'remote touch resets actual seat idle notification')
            passed('remote-touch-resumes-the-actual-idle-notifier')
        b = down(second, sid2, 7, 70., 80.)
        assert a != b
        passed('existing-touchscreen-capability-and-independent-contact-identities', seat_slots=[a, b], node=nodes[0])
        before = len(lines()); first.close()
        wait(lambda ls: f'up {a}' in ls[before:] and 'frame' in ls[ls.index(f'up {a}', before)+1:], 'source-only release and frame')
        assert f'up {b}' not in lines()[before:] and 'cancel' not in lines()[before:]
        send(second, 2, 7, sid2, 90.5, 100.25); send(second, 0)
        wait(lambda ls: f'motion {b} 90.50 100.25' in ls[before:], 'other source remains usable')
        passed('socket-close-releases-only-its-owner-and-flushes-before-reuse')
        third, sid3 = open_touch(); c = down(third, sid3, 7, 30., 40.)
        before = len(lines()); send(third, 1, 7, sid3, 50., 60., ack=False)
        assert third.recv(16) == b''
        wait(lambda ls: f'up {c}' in ls[before:], 'malformed source cleanup')
        assert f'up {b}' not in lines()[before:] and 'cancel' not in lines()[before:]
        send(second, 2, 7, sid2, 80., 90.); send(second, 0)
        wait(lambda ls: f'motion {b} 80.00 90.00' in ls[before:], 'surviving source after rejected duplicate')
        passed('invalid-contact-transition-retires-only-the-offending-source')
        fourth, sid4 = open_touch(); d = down(fourth, sid4, 0, 25., 35.)
        before = len(lines()); send(fourth, 3, 0); fourth.close()
        wait(lambda ls: f'up {d}' in ls[before:] and 'frame' in ls[ls.index(f'up {d}', before)+1:],
             'source close flushes its final up even without remaining contacts')
        passed('socket-close-flushes-an-unframed-final-up')
        before = len(lines()); call(CAST, capture, CAST + '.Session', 'Stop')
        assert second.recv(16) == b''
        wait(lambda ls: f'up {b}' in ls[before:] and 'frame' in ls[ls.index(f'up {b}', before)+1:],
             'capture-stop release and frame')
        assert int(next(v for v in reversed(lines()) if v.startswith('caps ')).split()[1]) == initial_caps
        assert 'cancel' not in lines()[before:]
        passed('capture-retirement-closes-the-lease-releases-contact-and-preserves-physical-capability')
    finally:
        for sock in sockets: sock.close()
        connection.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--niri', type=Path, required=True)
    parser.add_argument('--niri-sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--idle-check',action='store_true')
    args = parser.parse_args(); args.niri = args.niri.resolve(strict=True)
    assert digest(args.niri) == args.niri_sha256
    args.receiver = receiver(); args.output = args.output.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    result = dict(status='running', scope=__doc__, checks=[], compositor_sha256=args.niri_sha256,
                  receiver_sha256=digest(args.receiver), public_touch_available=False)
    sources = [Path(__file__), ROOT/'scripts/desktop-fixture/remote-input-touch-receiver.c',
               ROOT/'scripts/screencast_media.py', ROOT/'scripts/native_compositor.py',
               ROOT/'scripts/validate-desktop.py']
    (args.output/'sources').mkdir()
    for source in sources: (args.output/'sources'/source.name).write_bytes(source.read_bytes())
    result['sources'] = {str(p.relative_to(ROOT)): digest(p) for p in sources}
    start = time.monotonic(); code = 1
    with tempfile.TemporaryDirectory(prefix='canopa-remote-touch-') as temporary:
        session = MediaSession(Path(temporary), args.output, args.niri, args.niri_sha256)
        try: verify(args, session, result); result['status'] = 'passed'; code = 0
        except Exception as error: result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
        finally:
            processes = list(session.processes); session.close()
            result['processes'] = [dict(pid=p.pid, command=p.args, exit_code=p.poll()) for p in processes]
    result.update(seconds=round(time.monotonic()-start, 2), exit_code=code)
    (args.output/'result.json').write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(result, indent=2)); return code


if __name__ == '__main__': raise SystemExit(main())
