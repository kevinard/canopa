#!/usr/bin/env python3
"""Rust shortcut FD transport on private Niri; no public portal/consent claim."""
import argparse
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
import time
import traceback

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from native_compositor import freeze_niri, sha256


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


def verify(args, session, result):
    from gi.repository import Gio, GLib
    Channel = load('shortcut_protocol', ROOT / 'scripts/test-global-shortcuts-lease.py').Channel
    session.env['PATH'] = str(args.niri.parent) + os.pathsep + session.env['PATH']
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "us"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
''')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    probe = None; probe_pidfd = None; streams = []
    try:
        def bus(method, name):
            return connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
                method, GLib.Variant('(s)', (name,)), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None).unpack()[0]
        service = 'org.canopa.Niri.GlobalShortcuts1'; path = '/org/canopa/Niri/GlobalShortcuts1'
        def ready():
            try:
                return bus('GetNameOwner', service)
            except GLib.Error:
                return None
        owner = session.wait_for(ready, 'shortcut service')
        pid = bus('GetConnectionUnixProcessID', owner)
        assert Path(f'/proc/{pid}/exe').resolve() == args.niri
        remote = 'org.canopa.Niri.RemoteInput1'; remote_path = '/org/canopa/Niri/RemoteInput1'
        remote_owner = bus('GetNameOwner', remote); assert bus('GetConnectionUnixProcessID', remote_owner) == pid
        for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', mode, '/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml',
                str(session.output / ('xdg-shell' + suffix))], check=True, timeout=15)
        binary = session.output / 'receiver'
        subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output),
            str(ROOT / 'scripts/desktop-fixture/remote-input-receiver.c'), str(session.output / 'xdg-shell.c'),
            '-lwayland-client', '-lxkbcommon', '-o', str(binary)], check=True, timeout=30)
        receiver = session.start([str(binary)], 'receiver.log'); log = session.output / 'receiver.log'
        session.wait_for(lambda: log.exists() and 'focused' in log.read_text(), 'receiver focus')
        value, descriptors = connection.call_with_unix_fd_list_sync(remote_owner, remote_path, remote, 'OpenKeyboard',
            GLib.Variant('()', ()), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None, None)
        keyboard = socket.socket(fileno=descriptors.get(value.unpack()[0])); keyboard.settimeout(3); streams.append(keyboard)
        def key(code, down):
            keyboard.sendall(struct.pack('<II', code, int(down)))
        def marker():
            before = len(log.read_text().splitlines()); key(66, True); key(66, False)
            session.wait_for(lambda: any(line.startswith('key 66 0 ') for line in log.read_text().splitlines()[before:]),
                             'ordered receiver F8 release')
        receipt = session.output / 'rust-transport.jsonl'
        environment = dict(session.env, CANOPA_SHORTCUT_TRANSPORT_RECEIPT=str(receipt),
                           CANOPA_SHORTCUT_TRANSPORT_RUNTIME=str(session.runtime))
        command = [str(args.harness), '--ignored', '--exact',
                   'global_shortcuts::transport::tests::native_transport_fixture', '--nocapture']
        with (session.output / 'harness.log').open('w') as output:
            probe = subprocess.Popen(command, cwd=ROOT / 'crates/canopa-portal', env=environment,
                                     stdout=output, stderr=output, start_new_session=True)
        probe_pidfd = os.pidfd_open(probe.pid)
        start_ticks = Path(f'/proc/{probe.pid}/stat').read_text().rsplit(')', 1)[1].split()[19]
        result['harness_process'] = dict(pid=probe.pid, start_ticks=start_ticks, command=command)
        def records():
            if not receipt.exists():
                return []
            data = receipt.read_text()
            assert len(data) <= 65536
            return [json.loads(line) for line in data.split('\n')[:-1] if line]
        def event(kind, binding=None):
            if probe.poll() is not None:
                raise AssertionError((probe.returncode, (session.output / 'harness.log').read_text()))
            return next((r for r in records() if r['event'] == kind and (binding is None or r['binding'] == binding)), None)
        ready = session.wait_for(lambda: event('ready'), 'Rust authenticated shortcut transport')
        assert ready['pid'] == pid and ready['owner'] == owner
        result['checks'].append('rust-pins-original-owner-ipc-process-and-descriptor-and-commits')
        before = len(log.read_text().splitlines())
        key(29, True); key(30, True); marker()
        activated = session.wait_for(lambda: event('activated', 1), 'Rust Ctrl+a activation')
        key(29, False); marker(); assert not event('deactivated', 1)
        key(30, False); marker()
        deactivated = session.wait_for(lambda: event('deactivated', 1), 'Rust modifiers-first release')
        assert isinstance(activated['timestamp_ms'], int) and isinstance(deactivated['timestamp_ms'], int)
        assert not any(line.startswith('key 30 ') for line in log.read_text().splitlines()[before:])
        result['checks'].append('rust-forwards-source-time-and-original-press-pair-without-ordinary-key-leak')
        key(67, True); marker()
        session.wait_for(lambda: event('activated', 2), 'Rust held F9 activation')
        assert Path(f'/proc/{probe.pid}/exe').resolve() == args.harness
        assert Path(f'/proc/{probe.pid}/stat').read_text().rsplit(')', 1)[1].split()[19] == start_ticks
        signal.pidfd_send_signal(probe_pidfd, signal.SIGKILL); assert probe.wait(timeout=5) == -signal.SIGKILL
        result['harness_process'].update(exit_code=probe.returncode, killed_by_pidfd=True)
        value, descriptors = connection.call_with_unix_fd_list_sync(owner, path, service, 'OpenShortcuts',
            GLib.Variant('()', ()), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None, None)
        replacement = Channel(socket.socket(fileno=descriptors.get(value.unpack()[0]))); streams.append(replacement.stream)
        session.wait_for(lambda: replacement.replace([(77, 'F9')])['error'] is None, 'abrupt backend loss frees exact binding')
        replacement.events(); before = len(log.read_text().splitlines())
        key(67, False); marker()
        assert not any(line.startswith('key 67 ') for line in log.read_text().splitlines()[before:])
        assert not any(r['event'] in ('activated', 'deactivated') for r in replacement.events())
        key(67, True); key(67, False); marker()
        events = [r for r in replacement.events() if r['event'] in ('activated', 'deactivated')]
        assert [r['event'] for r in events] == ['activated', 'deactivated'] and all(r['binding'] == 77 for r in events), events
        result['checks'].append('backend-sigkill-revokes-fd-and-keeps-held-suppression-before-reacquisition')
        result['rust_events'] = records()
    finally:
        for stream in streams:
            stream.close()
        if probe is not None and probe.poll() is None:
            if probe_pidfd is not None:
                signal.pidfd_send_signal(probe_pidfd, signal.SIGKILL)
            else:
                probe.kill()
            probe.wait(timeout=5)
        if probe_pidfd is not None:
            os.close(probe_pidfd)
        if probe is not None:
            result.setdefault('harness_process', {}).update(exit_code=probe.poll())
        connection.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['niri', 'harness']:
        parser.add_argument('--' + name, required=True, type=Path)
        parser.add_argument('--' + name + '-sha256', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args(); args.output = args.output.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    args.niri = freeze_niri(args.niri.resolve(strict=True), args.niri_sha256); args.harness = args.harness.resolve(strict=True)
    assert sha256(args.harness) == args.harness_sha256
    paths = [Path(__file__).resolve(), ROOT / 'scripts/test-global-shortcuts-lease.py', ROOT / 'scripts/native_compositor.py',
             ROOT / 'scripts/validate-desktop.py', ROOT / 'scripts/desktop-fixture/remote-input-receiver.c', args.niri, args.harness]
    paths.extend((ROOT / 'crates/canopa-portal/src/global_shortcuts').rglob('*.rs'))
    paths.append(ROOT / 'crates/canopa-portal/src/global_shortcuts.rs')
    pins = {str(p): sha256(p) for p in paths}
    result = dict(status='running', checks=[], pins=pins, scope=__doc__); session = None; started = time.monotonic()
    desktop = load('desktop', ROOT / 'scripts/validate-desktop.py')
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-shortcut-rust-') as directory:
            session = desktop.Session(Path(directory), args.output)
            try:
                verify(args, session, result)
            finally:
                session.close()
        result['status'] = 'passed'
    except Exception as error:
        result.update(status='failed', error=str(error), traceback=traceback.format_exc()); raise
    finally:
        result['changed_inputs'] = [path for path, digest in pins.items() if sha256(Path(path)) != digest]
        result['processes'] = [dict(pid=p.pid, exit_code=p.poll()) for p in session.processes] if session else []
        result['seconds'] = round(time.monotonic() - started, 2)
        (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    assert not result['changed_inputs'] and all(p['exit_code'] is not None for p in result['processes'])
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
