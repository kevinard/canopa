#!/usr/bin/env python3
"""Check source teardown against actual ordinary/consumed Wayland key delivery."""
import argparse
import importlib.util
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
from native_compositor import freeze_niri, sha256

SERVICE = 'org.canopa.Niri.RemoteInput1'
PATH = '/org/canopa/Niri/RemoteInput1'


def verify(args, session, result):
    from gi.repository import Gio, GLib
    session.env['PATH'] = str(args.niri.parent) + os.pathsep + session.env['PATH']
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "us"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
binds {
    F6 { toggle-window-floating; }
    Shift+Shift_L { toggle-window-floating; }
    F4 { press { toggle-window-floating; }; release { toggle-window-floating; }; }
}
''')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    sockets = []
    try:
        def bus(method, values):
            return connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                'org.freedesktop.DBus', method, GLib.Variant('(s)', values), None,
                Gio.DBusCallFlags.NO_AUTO_START, 5000, None).unpack()[0]
        def owner_ready():
            try: return bus('GetNameOwner', (SERVICE,))
            except GLib.Error: return None
        owner = session.wait_for(owner_ready, 'compositor input service')
        pid = bus('GetConnectionUnixProcessID', (owner,))
        assert any(p.pid == pid for p in session.processes)
        assert Path(f'/proc/{pid}/exe').resolve() == args.niri
        for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', mode,
                '/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml',
                str(session.output / ('xdg-shell' + suffix))], check=True, timeout=15)
        receiver_bin = session.output / 'receiver'
        subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output),
            str(ROOT / 'scripts/desktop-fixture/remote-input-receiver.c'),
            str(session.output / 'xdg-shell.c'), '-lwayland-client', '-lxkbcommon',
            '-o', str(receiver_bin)], check=True, timeout=30)
        receiver = session.start([str(receiver_bin)], 'receiver.log')
        log = session.output / 'receiver.log'
        session.wait_for(lambda: log.exists() and 'focused' in log.read_text(), 'receiver focus')
        def lines():
            assert receiver.poll() is None
            return log.read_text().splitlines()
        def floating():
            windows = json.loads(session.run(['niri', 'msg', '--json', 'windows']))
            return next(w['is_floating'] for w in windows if w['app_id'] == 'org.canopa.RemoteInputReceiver')
        def open_lease(method='OpenKeyboard'):
            value, descriptors = connection.call_with_unix_fd_list_sync(owner, PATH, SERVICE,
                method, GLib.Variant('()', ()), None, Gio.DBusCallFlags.NO_AUTO_START,
                5000, None, None)
            index, keymap, mods = value.unpack()
            lease = socket.socket(fileno=descriptors.get(index)); lease.settimeout(3)
            sockets.append(lease)
            return lease
        def send(lease, key, down): lease.sendall(struct.pack('<II', key, int(down)))
        def close(lease, frame_bytes=16):
            lease.shutdown(socket.SHUT_WR)
            data = bytearray()
            while piece := lease.recv(4096):
                data.extend(piece); assert len(data) <= 4096
            assert len(data) % frame_bytes == 0
            lease.close()
        def marker(lease):
            before = len(lines())
            send(lease, 66, True); send(lease, 66, False)
            session.wait_for(lambda: any(line.startswith('key 66 0 ') for line in lines()[before:]),
                             'ordered receiver F8 release')
            deadline = time.monotonic() + .2
            while time.monotonic() < deadline: time.sleep(.01)
        def edges(observed, key):
            return [int(line.split()[2]) for line in observed if line.startswith(f'key {key} ')]
        def record(label, before, **extra):
            row = dict(name=label, receiver_lines=lines()[before:], ordered_marker=True, **extra)
            result['cases'].append(row)
            print(label, flush=True)
            (session.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
            return row
        observer = open_lease()
        for label, key, explicit in [('ordinary-key-disconnect', 65, False),
                                     ('bound-key-explicit-release', 64, True),
                                     ('bound-key-disconnect', 64, False),
                                     ('consumed-modifier-disconnect', 42, False)]:
            before = len(lines()); lease = open_lease(); send(lease, key, True)
            if explicit: send(lease, key, False)
            marker(lease); close(lease); marker(observer)
            row = record(label, before, key=key)
            row['receiver_edges'] = edges(row['receiver_lines'], key)
            if key == 65: assert row['receiver_edges'] == [1, 0], row
            if explicit: assert row['receiver_edges'] == [], row
            if key == 42:
                mods = [int(line.split()[1]) for line in row['receiver_lines'] if line.startswith('mods ')]
                assert mods and any(mods) and mods[-1] == 0, row
                row['modifier_state_changed_and_cleared'] = True
        before = len(lines()); first = open_lease(); second = open_lease()
        send(first, 64, True); marker(first); send(second, 64, True); marker(second)
        close(first); marker(observer)
        first_closed = lines()[before:]
        assert edges(first_closed, 64) == [], first_closed
        close(second); marker(observer)
        row = record('overlapping-consumed-sources', before, first_close_edges=edges(first_closed, 64))
        row['receiver_edges'] = edges(row['receiver_lines'], 64)
        before = len(lines()); lease = open_lease('OpenKeyboardSymbols')
        # XKB_KEY_F6, using the separately negotiated typed symbol protocol.
        lease.sendall(struct.pack('<4I', 1, 1, 0xffc3, 1))
        reply = bytearray()
        while len(reply) < 24:
            piece = lease.recv(24 - len(reply)); assert piece; reply.extend(piece)
        sequence, status, *mods = struct.unpack('<6I', reply)
        assert sequence == 1 and status == 0, (sequence, status)
        marker(observer); close(lease, 24); marker(observer)
        row = record('typed-symbol-consumed-key-disconnect', before)
        row['receiver_edges'] = edges(row['receiver_lines'], 64)
        before = len(lines()); lease = open_lease()
        send(lease, 64, True); send(lease, 65, True); marker(lease); close(lease); marker(observer)
        row = record('mixed-consumed-and-forwarded-keys', before)
        row['consumed_edges'] = edges(row['receiver_lines'], 64)
        row['ordinary_edges'] = edges(row['receiver_lines'], 65)
        assert row['ordinary_edges'] == [1, 0], row
        before = len(lines()); original = floating(); lease = open_lease()
        send(lease, 62, True); marker(lease); after_press = floating(); assert after_press != original
        close(lease); marker(observer); after_close = floating()
        row = record('disconnect-does-not-run-pending-release-action', before,
                     original_floating=original, after_press_floating=after_press, after_close_floating=after_close)
        row['receiver_edges'] = edges(row['receiver_lines'], 62)
        assert after_close == after_press, row
        before = len(lines()); lease = open_lease(); original = floating()
        send(lease, 62, True); marker(lease); assert floating() != original
        send(lease, 62, False); marker(lease); assert floating() == original
        close(lease); marker(observer)
        row = record('explicit-release-still-runs-configured-release-action', before)
        row['receiver_edges'] = edges(row['receiver_lines'], 62)
        assert row['receiver_edges'] == [], row
        before = len(lines())
        accessibility = 'org.freedesktop.a11y.Manager'
        monitor = 'org.freedesktop.a11y.KeyboardMonitor'
        a11y_owner = bus('GetNameOwner', (accessibility,))
        assert bus('GetConnectionUnixProcessID', (a11y_owner,)) == pid
        connection.call_sync(a11y_owner, '/org/freedesktop/a11y/Manager', monitor, 'SetKeyGrabs',
            GLib.Variant('(aua(uu))', ([], [(0xffc4, 0)])), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None)
        lease = open_lease(); send(lease, 65, True); marker(lease)
        before_close = edges(lines()[before:], 65); assert before_close == []
        close(lease); marker(observer)
        row = record('accessibility-consumed-key-disconnect', before, before_close_edges=before_close)
        row['receiver_edges'] = edges(row['receiver_lines'], 65)
        connection.call_sync(a11y_owner, '/org/freedesktop/a11y/Manager', monitor, 'SetKeyGrabs',
            GLib.Variant('(aua(uu))', ([], [])), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None)
        clean = all(not row.get('receiver_edges') for row in result['cases'] if row['name'] != 'ordinary-key-disconnect')
        clean = clean and not next(row for row in result['cases'] if row['name'] == 'mixed-consumed-and-forwarded-keys')['consumed_edges']
        result['consumed_releases_clean'] = clean
        if not args.observe_baseline: assert clean, result['cases']
    finally:
        for lease in sockets: lease.close()
        connection.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--niri', type=Path, required=True)
    parser.add_argument('--niri-sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--observe-baseline', action='store_true')
    args = parser.parse_args()
    args.niri = freeze_niri(args.niri.resolve(strict=True), args.niri_sha256)
    args.output = args.output.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    helper = ROOT / 'scripts/validate-desktop.py'
    spec = importlib.util.spec_from_file_location('desktop', helper)
    desktop = importlib.util.module_from_spec(spec); spec.loader.exec_module(desktop)
    paths = (Path(__file__), helper, ROOT / 'scripts/native_compositor.py',
             ROOT / 'scripts/desktop-fixture/remote-input-receiver.c', args.niri)
    pins = {str(p): sha256(p) for p in paths}
    result = dict(status='running', scope='Private raw keyboard leases, ordinary bindings and independent Wayland receiver; no GlobalShortcuts backend, physical input or installed routing.', pins=pins, cases=[])
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='canopa-source-close-') as directory:
        session = desktop.Session(Path(directory), args.output)
        try: verify(args, session, result); result['status'] = 'passed'
        except Exception as error: result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
        finally:
            processes = list(session.processes); session.close()
            result['processes'] = [dict(pid=p.pid, exit_code=p.poll()) for p in processes]
    result.update(seconds=round(time.monotonic() - started, 2), changed_inputs=[p for p,h in pins.items() if sha256(Path(p)) != h])
    if result['changed_inputs'] or any(p['exit_code'] is None for p in result['processes']): result['status'] = 'failed'
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: result.get(k) for k in ('status', 'error', 'consumed_releases_clean', 'seconds')}))
    raise SystemExit(0 if result['status'] == 'passed' else 1)


if __name__ == '__main__': main()
