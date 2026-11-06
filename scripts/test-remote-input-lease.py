#!/usr/bin/env python3
"""Private nested-desktop proof for the unadvertised Niri keyboard-lease candidate."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
SERVICE = 'org.canopa.Niri.RemoteInput1'
PATH = '/org/canopa/Niri/RemoteInput1'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify(args, session, result):
    from gi.repository import Gio, GLib
    session.env['PATH'] = str(args.niri.parent) + os.pathsep + session.env['PATH']
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "fr,us"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
''')
    connection = Gio.DBusConnection.new_for_address_sync(
        session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
        None, None,
    )

    def bus(method, signature, values):
        return connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                                    'org.freedesktop.DBus', method, GLib.Variant(signature, values),
                                    None, Gio.DBusCallFlags.NONE, 5000, None).unpack()

    def owner_ready():
        try:
            return bus('GetNameOwner', '(s)', (SERVICE,))[0]
        except GLib.Error:
            return None

    owner = session.wait_for(owner_ready, 'compositor input service')
    pid = bus('GetConnectionUnixProcessID', '(s)', (owner,))[0]
    niri = next(p for p in session.processes if p.args[0] == 'niri')
    assert pid == niri.pid, (pid, niri.pid)
    result['service_owner'] = owner
    result['service_pid_matches_compositor'] = True

    def call(method):
        return connection.call_sync(owner, PATH, SERVICE, method, GLib.Variant('()', ()),
                                    None, Gio.DBusCallFlags.NONE, 5000, None).unpack()

    def open_lease():
        value, fds = connection.call_with_unix_fd_list_sync(
            owner, PATH, SERVICE, 'OpenKeyboard', GLib.Variant('()', ()), None,
            Gio.DBusCallFlags.NONE, 5000, None, None,
        )
        index, keymap, mods = value.unpack()
        lease = socket.socket(fileno=fds.get(index))
        lease.settimeout(3)
        return lease, keymap, mods

    def snapshot():
        return call('KeyboardSnapshot')

    def update(lease):
        data = bytearray()
        while len(data) < 16:
            chunk = lease.recv(16 - len(data))
            assert chunk, 'EOF before a complete modifier update'
            data.extend(chunk)
        return struct.unpack('<IIII', data)

    def eof(lease):
        data = bytearray()
        while True:
            chunk = lease.recv(4096)
            if not chunk:
                break
            data.extend(chunk)
            assert len(data) <= 1024 * 1024
        assert len(data) % 16 == 0, 'truncated modifier update'

    for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
        subprocess.run(['wayland-scanner', mode,
                        '/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml',
                        str(session.output / ('xdg-shell' + suffix))], check=True, timeout=15)
    receiver_binary = session.output / 'receiver'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output),
                    str(ROOT / 'scripts/desktop-fixture/remote-input-receiver.c'),
                    str(session.output / 'xdg-shell.c'), '-lwayland-client', '-lxkbcommon',
                    '-o', str(receiver_binary)], check=True, timeout=30)
    receiver = session.start([str(receiver_binary)], 'receiver.log')
    log = session.output / 'receiver.log'
    session.wait_for(lambda: log.exists() and 'focused' in log.read_text(), 'harmless receiver focus')

    def lines():
        assert receiver.poll() is None, 'receiver exited'
        return log.read_text().splitlines()

    def send(lease, key, pressed):
        lease.sendall(struct.pack('<II', key, int(pressed)))

    def wait_key(key, pressed, before, symbol=None):
        prefix = f'key {key} {int(pressed)} '
        return session.wait_for(lambda: [l for l in lines()[before:]
                                        if l.startswith(prefix) and (symbol is None or l == prefix + symbol)],
                                f'key {key} state {int(pressed)} symbol {symbol}')

    def mods_equal(depressed):
        session.wait_for(lambda: snapshot()[1][0] == depressed, f'authoritative depressed mask {depressed}')

    checks = result['checks']
    def passed(name):
        checks.append(name)
        print(name + ': passed', flush=True)
        (session.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')

    with open_lease()[0] as lease:
        keymap, initial = snapshot()
        assert initial[0] == 0
        (session.output / 'keymap.xkb').write_text(keymap)
        before = len(lines())
        send(lease, 16, True)
        wait_key(16, True, before, 'a')
        send(lease, 16, False)
        wait_key(16, False, before)
    passed('french-layout-actual-key-delivery')

    lease, _, initial = open_lease()
    before = len(lines())
    send(lease, 42, True)
    wait_key(42, True, before)
    shift_mask = session.wait_for(lambda: snapshot()[1][0] or None, 'nonzero Shift mask')
    send(lease, 16, True)
    wait_key(16, True, before, 'A')
    send(lease, 16, False)
    wait_key(16, False, before)
    lease.close()
    wait_key(42, False, before)
    mods_equal(0)
    passed('eof-releases-held-modifier')

    locked_before = snapshot()[1][2]
    with open_lease()[0] as lease:
        before = len(lines())
        send(lease, 58, True)
        wait_key(58, True, before)
        send(lease, 58, False)
        wait_key(58, False, before)
        locked_after = session.wait_for(lambda: snapshot()[1][2] if snapshot()[1][2] != locked_before else None,
                                        'Caps Lock state change')
        send(lease, 16, True)
        wait_key(16, True, before, 'A')
        send(lease, 16, False)
        wait_key(16, False, before)
    with open_lease()[0] as lease:
        assert snapshot()[1][2] == locked_after
        before = len(lines())
        send(lease, 58, True)
        wait_key(58, True, before)
        send(lease, 58, False)
        wait_key(58, False, before)
        session.wait_for(lambda: snapshot()[1][2] == locked_before, 'Caps Lock restoration')
    passed('locked-state-survives-lease-and-is-snapshotted')

    with open_lease()[0] as observer:
        session.run(['niri', 'msg', 'action', 'switch-layout', 'next'])
        assert update(observer)[3] == 1
        with open_lease()[0] as lease:
            assert snapshot()[1][3] == 1
            before = len(lines())
            send(lease, 16, True)
            wait_key(16, True, before, 'q')
            send(lease, 16, False)
            wait_key(16, False, before)
        session.run(['niri', 'msg', 'action', 'switch-layout', 'next'])
        assert update(observer)[3] == 0
    passed('compositor-layout-switch-publishes-state-and-preserves-lease')

    first, _, _ = open_lease()
    second, _, second_initial = open_lease()
    observer, _, observer_initial = open_lease()
    assert observer_initial[0] == 0
    before = len(lines())
    send(first, 42, True)
    wait_key(42, True, before)
    mods_equal(shift_mask)
    assert update(observer)[0] == shift_mask
    # The initial snapshot must also describe local/other-source state while unfocused.
    third, _, third_initial = open_lease()
    assert third_initial[0] == shift_mask
    third.close()
    send(second, 42, True)
    # Barrier: a subsequent ordinary key from this FD must have been delivered.
    send(second, 16, True)
    wait_key(16, True, before, 'A')
    send(second, 16, False)
    wait_key(16, False, before)
    first.shutdown(socket.SHUT_WR)
    eof(first)
    first.close()
    mods_equal(shift_mask)
    assert len([l for l in lines()[before:] if l.startswith('key 42 0 ')]) == 0
    second.close()
    wait_key(42, False, before)
    mods_equal(0)
    assert update(observer)[0] == 0
    observer.close()
    assert len([l for l in lines()[before:] if l.startswith('key 42 1 ')]) == 1
    assert len([l for l in lines()[before:] if l.startswith('key 42 0 ')]) == 1
    passed('independent-sources-preserve-overlapping-held-key')
    passed('initial-snapshot-includes-another-source-modifier')
    passed('asynchronous-other-source-modifier-updates')

    lease, _, _ = open_lease()
    child = subprocess.Popen([sys.executable, '-c',
                              'import os,struct,sys; f=int(sys.argv[1]); os.write(f,struct.pack("<II",42,1)); sys.stdin.read()',
                              str(lease.fileno())], pass_fds=(lease.fileno(),), stdin=subprocess.PIPE,
                             env=session.env, start_new_session=True)
    session.processes.append(child)
    before = len(lines())
    lease.close()
    wait_key(42, True, before)
    child.kill()
    assert child.wait(timeout=5) == -signal.SIGKILL
    session.processes.remove(child)
    wait_key(42, False, before)
    mods_equal(0)
    passed('worker-sigkill-releases-held-key')

    for label, payload in [('invalid-state', struct.pack('<II', 16, 2)),
                           ('unpaired-release', struct.pack('<II', 16, 0)),
                           ('duplicate-press', struct.pack('<II', 42, 1))]:
        with open_lease()[0] as lease:
            before = len(lines())
            send(lease, 42, True)
            wait_key(42, True, before)
            lease.sendall(payload)
            wait_key(42, False, before)
            eof(lease)
            mods_equal(0)
        passed(label + '-revokes-and-releases')

    leases = [open_lease()[0] for _ in range(8)]
    try:
        try:
            extra, _, _ = open_lease()
        except GLib.Error as error:
            assert 'LimitsExceeded' in str(error), str(error)
        else:
            extra.close()
            raise AssertionError('ninth lease admitted')
    finally:
        for lease in leases:
            lease.close()
    def admission_recovered():
        try:
            lease, _, _ = open_lease()
            lease.close()
            return True
        except GLib.Error:
            return False
    session.wait_for(admission_recovered, 'lease admission recovery')
    passed('bounded-admission-recovers-on-close')

    with open_lease()[0] as lease:
        before = len(lines())
        old_map = snapshot()[0]
        send(lease, 42, True)
        wait_key(42, True, before)
        config = session.runtime / 'niri.kdl'
        config.write_text(config.read_text().replace('layout "fr,us"', 'layout "us"'))
        session.wait_for(lambda: snapshot()[0] != old_map, 'private configured keymap reload')
        # No further input is sent on this lease: the compositor must retire it itself.
        eof(lease)
        wait_key(42, False, before)
        mods_equal(0)
    passed('keymap-reload-retires-idle-held-source')
    connection.close_sync(None)
    result['scope'] = 'Keyboard lease with asynchronous modifiers and keymap retirement only; no portal consent, EIS worker, pointer or physical-input acceptance.'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--niri', required=True, type=Path)
    parser.add_argument('--niri-sha256', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    args.niri = args.niri.resolve()
    assert args.niri.name == 'niri' and digest(args.niri) == args.niri_sha256
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    result = {'status': 'running', 'niri': str(args.niri), 'sha256': args.niri_sha256, 'checks': [],
              'fixture_sources': [{'path': str(p.relative_to(ROOT)), 'sha256': digest(p)} for p in
                                  (Path(__file__), ROOT / 'scripts/desktop-fixture/remote-input-receiver.c',
                                   ROOT / 'scripts/validate-desktop.py')]}
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-remote-input-') as directory:
            session = desktop.Session(Path(directory), args.output)
            try:
                verify(args, session, result)
            finally:
                session.close()
        assert digest(args.niri) == args.niri_sha256
        result['compositor_bytes_unchanged'] = True
        result['status'] = 'passed'
    except Exception as error:
        result.update(status='failed', error=str(error))
        raise
    finally:
        (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
