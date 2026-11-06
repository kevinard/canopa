#!/usr/bin/env python3
"""Actual libei-to-Niri keyboard worker proof on a private nested desktop."""
import argparse
import array
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
SERVICE = 'org.canopa.Niri.RemoteInput1'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify(args, session, result):
    from gi.repository import Gio, GLib
    session.env['PATH'] = str(args.niri.parent) + os.pathsep + session.env['PATH']
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "fr"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
''')
    def connect():
        return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    backend = connect()
    def bus(method, signature, values):
        return backend.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
            method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()
    def owner_ready():
        try:
            return bus('GetNameOwner', '(s)', (SERVICE,))[0]
        except GLib.Error:
            return None
    owner = session.wait_for(owner_ready, 'private input compositor')
    pid = bus('GetConnectionUnixProcessID', '(s)', (owner,))[0]
    assert pid == next(p.pid for p in session.processes if p.args[0] == 'niri')
    for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
        subprocess.run(['wayland-scanner', mode, '/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml',
                        str(session.output / ('xdg-shell' + suffix))], check=True, timeout=15)
    receiver_bin = session.output / 'receiver'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output),
                    str(ROOT / 'scripts/desktop-fixture/remote-input-receiver.c'), str(session.output / 'xdg-shell.c'),
                    '-lwayland-client', '-lxkbcommon', '-o', str(receiver_bin)], check=True, timeout=30)
    client_bin = session.output / 'ei-client'
    flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'libei-1.0', 'xkbcommon'], text=True).split()
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', str(ROOT / 'scripts/desktop-fixture/remote-input-ei-client.c'),
                    '-o', str(client_bin), *flags], check=True, timeout=30)
    receiver = session.start([str(receiver_bin)], 'receiver.log')
    log = session.output / 'receiver.log'
    session.wait_for(lambda: log.exists() and 'focused' in log.read_text(), 'harmless receiver')
    def lines():
        assert receiver.poll() is None
        return log.read_text().splitlines()
    def wait_line(text, before):
        session.wait_for(lambda: any(text in line for line in lines()[before:]), text)
    def passed(name):
        result['checks'].append(name)
        (session.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
        print(name + ': passed', flush=True)
    def finish(process, expected):
        status = process.wait(timeout=5)
        session.processes.remove(process)
        assert status == expected, (process.args, status, expected)
    def spawn(name, caller, override=None):
        parent, child = socket.socketpair()
        parent.settimeout(6)
        request = dict(compositor=owner, compositor_pid=pid, ipc_socket=session.env['NIRI_SOCKET'],
                       backend=backend.get_unique_name(), caller=caller.get_unique_name(), grants=1)
        request.update(override or {})
        with (session.output / (name + '-worker.log')).open('w') as stream:
            worker = subprocess.Popen([str(args.binary), '--remote-input-worker'], stdin=subprocess.PIPE,
                                      stdout=child, stderr=stream, env=session.env, start_new_session=True)
        child.close()
        session.processes.append(worker)
        worker.stdin.write(json.dumps(request).encode() + b'\n')
        worker.stdin.flush()
        return worker, parent
    def receive_fd(parent):
        payload, ancillary, flags, _ = parent.recvmsg(1, socket.CMSG_SPACE(4), socket.MSG_CMSG_CLOEXEC)
        fds = array.array('i')
        for level, kind, data in ancillary:
            assert level == socket.SOL_SOCKET and kind == socket.SCM_RIGHTS
            fds.frombytes(data)
        assert payload == b'\x01' and len(fds) == 1 and not flags & ~socket.MSG_CMSG_CLOEXEC, (payload, fds, flags)
        return socket.socket(fileno=fds[0])
    def client(name, fd, mode):
        with (session.output / (name + '-client.log')).open('w') as stream:
            process = subprocess.Popen([str(client_bin), mode], stdin=fd, stdout=stream, stderr=stream,
                                       env=session.env, start_new_session=True)
        fd.close()
        session.processes.append(process)
        return process

    modes = ['repeat', 'request'] if args.case == 'all' else [args.case]
    for mode in modes:
        name = mode + '-period'
        caller = connect()
        before = len(lines())
        worker, channel = spawn(name, caller)
        process = client(name, receive_fd(channel), mode)
        wait_line('key 16 1 A', before)
        if mode == 'repeat':
            wait_line('key 16 1 a', before)
            wait_line('key 42 0 Shift_L', before)
        else:
            session.wait_for(lambda: 'modifier update' in (session.output / (name + '-client.log')).read_text(), 'requested-device modifiers')
        worker.stdin.close()
        finish(worker, 0)
        finish(process, 0)
        channel.close(); caller.close_sync(None)
        passed(name + '-keeps-eis-connection-and-cleans-source')
    if args.case != 'all':
        backend.close_sync(None)
        return

    for name in ['parent-eof', 'client-sigkill', 'worker-sigkill', 'caller-loss']:
        caller = connect()
        before = len(lines())
        worker, channel = spawn(name, caller)
        process = client(name, receive_fd(channel), 'abrupt' if name == 'worker-sigkill' else 'held')
        wait_line('key 16 1 A', before)
        client_log = session.output / (name + '-client.log')
        session.wait_for(lambda: 'modifier update' in client_log.read_text(), 'libei authoritative modifiers')
        assert 'keyboard-only sealed keymap' in client_log.read_text()
        if name == 'parent-eof':
            worker.stdin.close()
        elif name == 'client-sigkill':
            process.kill()
            finish(process, -signal.SIGKILL)
        elif name == 'worker-sigkill':
            worker.kill()
            finish(worker, -signal.SIGKILL)
        else:
            caller.close_sync(None)
        if name != 'worker-sigkill':
            finish(worker, 0)
        if name != 'client-sigkill':
            finish(process, 0)
        wait_line('key 42 0 Shift_L', before)
        wait_line('mods 0 ', before)
        channel.close()
        if not caller.is_closed():
            caller.close_sync(None)
        passed(name + '-actual-input-and-held-cleanup')

    for name, mode, error in [('receiver-role', 'receiver', 'role'), ('unframed-flood', 'flood', 'unframed')]:
        caller = connect()
        before = len(lines())
        worker, channel = spawn(name, caller)
        process = client(name, receive_fd(channel), mode)
        finish(worker, 1)
        finish(process, 0)
        assert error in (session.output / (name + '-worker.log')).read_text()
        assert not any(l.startswith('key ') for l in lines()[before:]), 'unapproved input reached receiver'
        channel.close()
        caller.close_sync(None)
        passed(name + '-rejected-before-native-input')

    wrong = socket.socket(socket.AF_UNIX)
    wrong_path = session.runtime / 'wrong-ipc.sock'
    wrong.bind(str(wrong_path)); wrong.listen(1)
    for name, override in [('unsupported-grant', {'grants': 4}), ('wrong-pid', {'compositor_pid': pid + 1}),
                           ('wrong-ipc-peer', {'ipc_socket': str(wrong_path)})]:
        caller = connect()
        before = len(lines())
        worker, channel = spawn(name, caller, override)
        assert channel.recv(1) == b''
        finish(worker, 1)
        assert not any(l.startswith('key ') for l in lines()[before:])
        channel.close(); caller.close_sync(None)
        passed(name + '-rejected-before-ready-fd')
    wrong.close()

    caller = connect()
    before = len(lines())
    worker, channel = spawn('keymap-loss', caller)
    process = client('keymap-loss', receive_fd(channel), 'held')
    wait_line('key 16 1 A', before)
    session.wait_for(lambda: 'modifier update' in (session.output / 'keymap-loss-client.log').read_text(), 'modifier response')
    config = session.runtime / 'niri.kdl'
    config.write_text(config.read_text().replace('layout "fr"', 'layout "us"'))
    finish(worker, 0)
    finish(process, 0)
    wait_line('key 42 0 Shift_L', before)
    channel.close(); caller.close_sync(None)
    passed('keymap-loss-retires-eis-and-held-input')
    backend.close_sync(None)
    result['scope'] = 'Keyboard worker only; no portal consent, parent adapter, pointer or legacy Notify implementation.'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--niri', required=True, type=Path)
    parser.add_argument('--niri-sha256', required=True)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--binary-sha256', required=True)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--case', choices=['all', 'repeat', 'request'], default='all')
    args = parser.parse_args()
    args.niri = args.niri.resolve(); args.binary = args.binary.resolve(); args.output = args.output.resolve()
    assert args.niri.name == 'niri' and digest(args.niri) == args.niri_sha256
    assert digest(args.binary) == args.binary_sha256
    args.output.mkdir(parents=True, exist_ok=False)
    result = {'status': 'running', 'checks': [], 'niri_sha256': args.niri_sha256, 'worker_sha256': args.binary_sha256,
              'fixture_sources': [{'path': str(p.relative_to(ROOT)), 'sha256': digest(p)} for p in
                                  (Path(__file__), ROOT / 'scripts/desktop-fixture/remote-input-ei-client.c',
                                   ROOT / 'scripts/desktop-fixture/remote-input-receiver.c', ROOT / 'scripts/validate-desktop.py')]}
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-eis-worker-') as directory:
            session = desktop.Session(Path(directory), args.output)
            try:
                verify(args, session, result)
            finally:
                session.close()
        assert digest(args.niri) == args.niri_sha256 and digest(args.binary) == args.binary_sha256
        result.update(status='passed', binaries_unchanged=True)
    except Exception as error:
        result.update(status='failed', error=str(error))
        raise
    finally:
        (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
