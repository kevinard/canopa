#!/usr/bin/env python3
"""Private Niri-to-Rust-to-libei capture transport; no public portal/consent claim."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import select
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time
import traceback

sys.dont_write_bytecode = True
from native_compositor import freeze_niri
from screencast_media import MediaSession, digest

ROOT = Path(__file__).resolve().parents[1]
SERVICE = 'org.canopa.InputCaptureFixture'
PATH = '/org/canopa/InputCaptureFixture'


def verify(args, session, result):
    from gi.repository import Gio, GLib
    output = session.start_niri(
        'debug { dbus-interfaces-in-non-session-instances; }\n'
        'prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
        'hotkey-overlay { skip-at-startup; }\nlayout { border { off; }; focus-ring { off; }; }\n',
        scale=1.25)
    session.env.update(CANOPA_INPUT_CAPTURE_NATIVE='1', CANOPA_PORTAL_BINARY=str(args.binary))

    def connect():
        return Gio.DBusConnection.new_for_address_sync(
            session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT |
            Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

    def passed(name):
        result['checks'].append(name)
        (session.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
        print(name, flush=True)

    def retire(process, expected=0):
        code = process.wait(timeout=8)
        session.processes.remove(process)
        result.setdefault('retired', []).append({'pid': process.pid, 'command': process.args, 'exit_code': code})
        assert code == expected, (process.args, code, expected)

    def wait(condition, description, allowed=()):
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            for process in session.processes:
                if process not in allowed and process.poll() is not None:
                    raise RuntimeError(f'Unexpected exit: {process.args}: {process.returncode}')
            value = condition()
            if value: return value
            time.sleep(.05)
        raise RuntimeError('Timed out waiting for ' + description)

    fixture_log = session.output / 'fixture.log'
    with fixture_log.open('w') as log:
        fixture = subprocess.Popen(
            [str(args.harness), '--exact', 'input_capture_transport::tests::native_receiver_fixture',
             '--ignored', '--nocapture', '--test-threads=1'], stdin=subprocess.PIPE, stdout=log, stderr=log,
            env=session.env, cwd=ROOT / 'crates/canopa-portal', start_new_session=True)
    session.processes.append(fixture)
    session.wait_for(lambda: 'capture-receiver-fixture-ready' in fixture_log.read_text(), 'private Rust fixture')
    fixture_source = ROOT / 'scripts/desktop-fixture'
    for protocol, source in [('virtual-pointer', fixture_source / 'wlr-virtual-pointer-unstable-v1.xml'),
                             ('xdg-shell', Path('/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml'))]:
        for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', mode, str(source), str(session.output / (protocol + suffix))],
                           check=True, timeout=15)
    pointer_bin, local_bin, ei_bin = [session.output / name for name in ('pointer', 'local', 'ei-client')]
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output),
                    str(fixture_source / 'input-capture-pointer.c'), str(session.output / 'virtual-pointer.c'),
                    '-lwayland-client', '-lm', '-o', str(pointer_bin)], check=True, timeout=30)
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output),
                    str(fixture_source / 'remote-input-pointer-receiver.c'), str(session.output / 'xdg-shell.c'),
                    '-lwayland-client', '-lxkbcommon', '-o', str(local_bin)], check=True, timeout=30)
    flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'libei-1.0'], text=True).split()
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', str(fixture_source / 'input-capture-ei-client.c'),
                    '-o', str(ei_bin), *flags], check=True, timeout=30)
    local_log = session.output / 'local.log'
    local = session.start([str(local_bin)], 'local.log')
    session.wait_for(lambda: 'focused' in local_log.read_text(), 'ordinary local pointer receiver')
    window = session.wait_for(lambda: next((w for w in json.loads(session.run(['niri', 'msg', '--json', 'windows']))
                                           if w['pid'] == local.pid), None), 'local receiver window')
    session.run(['niri', 'msg', 'action', 'fullscreen-window', '--id', str(window['id'])])
    pointer_log = session.output / 'pointer.log'
    with pointer_log.open('w') as log:
        pointer = subprocess.Popen([str(pointer_bin)], stdin=subprocess.PIPE, stdout=log, stderr=log,
                                   env=session.env, start_new_session=True)
    session.processes.append(pointer)
    session.wait_for(lambda: 'ready' in pointer_log.read_text(), 'persistent native pointer')
    serial = 0

    def mouse(command):
        nonlocal serial
        serial += 1
        pointer.stdin.write((command + '\n').encode()); pointer.stdin.flush()
        session.wait_for(lambda: f'ack {serial}\n' in pointer_log.read_text(), 'native pointer roundtrip')

    def center():
        mouse('absolute 40000 40000 100000 100000')

    def local_events():
        return [line for line in local_log.read_text().splitlines() if line.startswith(('motion ', 'button ', 'axis'))]

    def local_cycle():
        before = local_log.read_text().count('button 272 1')
        released = local_log.read_text().count('button 272 0')
        mouse('button 272 0')  # physical release after a captured press stays suppressed
        center(); mouse('relative 2 1'); mouse('button 272 1'); mouse('button 272 0')
        session.wait_for(lambda: local_log.read_text().count('button 272 1') == before + 1 and
                         local_log.read_text().count('button 272 0') == released + 1,
                         'fresh local button cycle without an unmatched release')

    class Lease:
        def __init__(self, name, mode='normal'):
            self.owner = connect(); self.name = name
            self.id, self.pid = self.call('Start')
            self.pidfd = os.pidfd_open(self.pid)
            session.capture_workers.append({'pid': self.pid, 'pidfd': os.dup(self.pidfd)})
            assert Path(f'/proc/{self.pid}/cmdline').read_bytes().split(b'\0')[:-1] == [
                os.fsencode(args.binary), b'--input-capture-worker']
            self.process = None; self.control = None
            self.log = session.output / (name + '-ei.log')
            self.mode = mode

        def call(self, method, signature='()', values=()):
            return self.owner.call_sync(SERVICE, PATH, SERVICE, method, GLib.Variant(signature, values), None,
                                        Gio.DBusCallFlags.NO_AUTO_START, 12000, None).unpack()

        def state(self):
            return json.loads(self.call('State', '(u)', (self.id,))[0])

        def command(self, op, **options):
            return json.loads(self.call('Control', '(us)', (self.id, json.dumps({'op': op, **options})))[0])

        def attach(self):
            reply, fds = self.owner.call_with_unix_fd_list_sync(
                SERVICE, PATH, SERVICE, 'Connect', GLib.Variant('(u)', (self.id,)), GLib.VariantType.new('(h)'),
                Gio.DBusCallFlags.NO_AUTO_START, 12000, None, None)
            owned = fds.steal_fds(); assert reply.unpack() == (0,) and len(owned) == 1
            channel = socket.socket(fileno=owned[0])
            assert channel.family == socket.AF_UNIX and channel.type == socket.SOCK_STREAM
            assert struct.unpack('3i', channel.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))[0] == self.pid
            assert self.pid != fixture.pid
            assert fcntl.fcntl(channel.fileno(), fcntl.F_GETFD) & fcntl.FD_CLOEXEC
            read, write = os.pipe(); self.control = os.fdopen(write, 'wb', buffering=0)
            with self.log.open('w') as log:
                self.process = subprocess.Popen([str(ei_bin), str(read), self.mode], stdin=channel,
                    pass_fds=(read,), stdout=log, stderr=log, env=session.env, start_new_session=True)
            os.close(read); channel.close(); session.processes.append(self.process)
            if self.mode != 'sender':
                session.wait_for(lambda: any('resumed' in line for line in self.log.read_text().splitlines()),
                                 'passive libei receiver configured')
                session.wait_for(lambda: self.state()['state']['receiver_ready'], 'Rust receiver ready')

        def events(self):
            return [json.loads(line) for line in self.log.read_text().splitlines() if line.startswith('{')]

        def wait_event(self, key, start=0, value=None):
            return wait(lambda: next((e for e in self.events()[start:] if key in e and
                        (value is None or e[key] == value)), None), f'{self.name}: EI {key} {value}', (self.process,))

        def arm(self):
            zones = self.command('get_zones'); assert zones['result'] == 'zones'
            width, height, x, y = zones['zones'][0]
            assert self.command('set_barriers', zone_set=zones['zone_set'],
                                barriers=[[7, [x + width, y, x + width, y + height - 1]]]) == {
                                    'result': 'barriers', 'failed_barriers': []}
            assert self.command('enable') == {'result': 'unit'}

        def activate(self):
            before = len(self.events()); center(); mouse('relative 5000 0')
            state = session.wait_for(lambda: (s if (s := self.state())['state']['activation'] is not None else None),
                                     'Niri activation through Rust parent')
            activation = state['state']['activation']
            self.wait_event('start', before, activation)
            assert any(e['event'] == 'activated' and e['activation'] == activation and e['barrier'] == 7
                       for e in state['events']), state
            return activation

        def held(self):
            before = len(self.events()); mouse('button 272 1'); self.wait_event('button', before, [272, 1])
            mouse('axis 0 6.5'); self.wait_event('scroll', before, [0, 6.5])

        def terminal(self):
            wait(lambda: bool(select.select([self.pidfd], [], [], 0)[0]), 'capture child exit', (self.process,))

        def stop(self, expected_failure=False):
            try:
                self.call('Stop', '(u)', (self.id,))
            except GLib.Error as error:
                assert expected_failure and Gio.DBusError.get_remote_error(error) == 'org.freedesktop.DBus.Error.Failed', str(error)
            self.terminal()
            if self.process:
                retire(self.process)
            if self.control: self.control.close()
            if not self.owner.is_closed(): self.owner.close_sync(None)
            os.close(self.pidfd)

    def denied(operation, name):
        try:
            operation()
        except GLib.Error as error:
            assert Gio.DBusError.get_remote_error(error) in (
                'org.freedesktop.DBus.Error.Failed', 'org.freedesktop.DBus.Error.AccessDenied'), str(error)
        else:
            raise AssertionError(name + ' unexpectedly succeeded')

    lease = Lease('cycles')
    denied(lambda: lease.command('enable'), 'Enable before Connect')
    lease.attach()
    denied(lambda: lease.call('Connect', '(u)', (lease.id,)), 'duplicate EI handoff')
    foreign = connect()
    denied(lambda: foreign.call_sync(SERVICE, PATH, SERVICE, 'State', GLib.Variant('(u)', (lease.id,)), None,
                                     Gio.DBusCallFlags.NO_AUTO_START, 5000, None), 'foreign owner')
    foreign.close_sync(None)
    assert not any('start' in e or 'motion' in e for e in lease.events())
    passed('exact-once-worker-owned-receiver-fd-and-authenticated-owner-admission')

    lease.arm(); activation = lease.activate(); before = len(lease.events()); local_before = local_events()
    mouse('relative 3.25 -1.5'); lease.wait_event('motion', before, [3.25, -1.5])
    mouse('wheel 0 3.25 1'); lease.wait_event('wheel', before, [0, 120]); lease.held()
    assert lease.command('enable') == {'result': 'unit'}
    denied(lambda: lease.command('release', activation=activation + 1), 'stale release')
    mouse('relative 1 0'); lease.wait_event('motion', before, [1, 0])
    # Marker reaches the same local client after captured source events.
    markers_before = sum(line.startswith('key ') and line.endswith(' F24') for line in local_log.read_text().splitlines())
    marker = session.start(['wtype', '-k', 'F24', '-s', '4000'], 'marker.log')
    session.wait_for(lambda: sum(line.startswith('key ') and line.endswith(' F24')
                                for line in local_log.read_text().splitlines()) == markers_before + 2,
                     'ordered local marker')
    assert local_events() == local_before, 'captured data was duplicated to ordinary clients'
    assert marker.poll() is None, 'marker device disappeared before capture cleanup'
    assert lease.command('release', activation=activation) == {'result': 'unit'}
    lease.wait_event('button', before, [272, 0]); lease.wait_event('scroll_end', before, [0, 1]); lease.wait_event('stop', before)
    assert lease.state()['state']['enabled'] and lease.state()['state']['activation'] is None
    mouse('button 272 0'); activation2 = lease.activate(); assert activation2 != activation
    before = len(lease.events()); lease.held(); assert lease.command('disable') == {'result': 'unit'}
    lease.wait_event('button', before, [272, 0]); lease.wait_event('stop', before); local_cycle()
    retire(marker)
    passed('real-captured-motion-button-scroll-frames-and-repeated-activation-share-one-ei-connection')

    lease.arm(); lease.activate(); before = len(lease.events()); old_generation = lease.state()['state']['zone_set']
    session.run(['niri', 'msg', 'output', output, 'scale', '1.5'])
    session.wait_for(lambda: not lease.state()['state']['zones_valid'], 'invalidated zones')
    lease.wait_event('stop', before)
    assert any(e['event'] == 'zones_changed' and e['zone_set'] == old_generation for e in lease.state()['events'])
    lease.arm(); assert lease.state()['state']['zone_set'] != old_generation
    lease.activate(); assert lease.command('disable') == {'result': 'unit'}
    passed('topology-change-stops-ei-activation-and-requires-current-zones-and-barriers')

    lease.arm(); lease.activate(); before = len(lease.events()); lease.held()
    lease.control.write(b'close\n')
    session.wait_for(lambda: not lease.state()['state']['enabled'] and
                     not lease.state()['state']['receiver_ready'], 'closed receiving device disables Niri')
    lease.wait_event('removed', before); mouse('button 272 0')
    lease.control.write(b'request\n')
    lease.wait_event('resumed', before)
    session.wait_for(lambda: lease.state()['state']['receiver_ready'], 'replacement receiver ready')
    lease.arm(); lease.activate(); assert lease.command('disable') == {'result': 'unit'}
    lease.stop()
    passed('receiver-device-close-retires-held-state-and-replacement-requires-explicit-enable')

    for index in range(8):
        invalid = Lease('sender-' + str(index), 'sender'); invalid.attach()
        wait(lambda: invalid.state()['closed'], 'sender role rejected', (invalid.process,))
        invalid.wait_event('disconnected'); invalid.stop(expected_failure=True)
    healthy = Lease('after-role-denials'); healthy.attach(); healthy.arm(); healthy.activate(); healthy.stop()
    local_cycle()
    passed('sender-role-denied-before-seat-and-repeated-failures-release-worker-and-compositor-slots')

    lost = Lease('caller-loss'); lost.attach(); lost.arm(); lost.activate(); before = len(lost.events()); lost.held()
    lost.owner.close_sync(None); lost.terminal()
    lost.wait_event('button', before, [272, 0]); lost.wait_event('stop', before); lost.wait_event('disconnected', before)
    retire(lost.process); lost.control.close(); os.close(lost.pidfd); local_cycle()
    passed('original-caller-loss-cleans-held-ei-input-before-child-exit-and-restores-local-input')

    killed = Lease('worker-kill', 'abrupt'); killed.attach(); killed.arm(); killed.activate(); killed.held()
    signal.pidfd_send_signal(killed.pidfd, signal.SIGKILL)
    killed.terminal(); killed.wait_event('disconnected'); killed.stop(expected_failure=True); local_cycle()
    passed('exact-worker-sigkill-revokes-niri-interception-and-closes-the-receiver')

    slow = Lease('slow-receiver', 'abrupt'); slow.attach(); slow.arm(); slow.activate()
    slow.control.write(b'pause\n'); slow.wait_event('paused')
    log_start = len(fixture_log.read_text())
    for _ in range(128):
        if slow.state()['closed']: break
        mouse('burst 128')
        try:
            # Drain each Niri batch before sending another. This distinguishes
            # EI output pressure from overflowing the upstream capture socket.
            slow.command('get_zones')
        except GLib.Error as error:
            assert Gio.DBusError.get_remote_error(error) == 'org.freedesktop.DBus.Error.Failed', str(error)
            break
    slow.terminal()
    assert 'Resource temporarily unavailable' in fixture_log.read_text()[log_start:]
    slow.control.write(b'resume\n'); slow.wait_event('disconnected'); slow.stop(expected_failure=True); local_cycle()
    passed('slow-receiver-has-bounded-output-and-cannot-retain-niri-interception')

    backend_lost = Lease('backend-loss'); backend_lost.attach(); backend_lost.arm(); backend_lost.activate()
    before = len(backend_lost.events()); backend_lost.held(); fixture.kill(); retire(fixture, -signal.SIGKILL)
    backend_lost.terminal(); backend_lost.wait_event('button', before, [272, 0])
    backend_lost.wait_event('stop', before); backend_lost.wait_event('disconnected', before)
    retire(backend_lost.process); backend_lost.control.close(); backend_lost.owner.close_sync(None)
    os.close(backend_lost.pidfd); local_cycle()
    passed('backend-process-loss-leaves-child-time-to-clean-ei-held-state-and-revoke-niri')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'harness', 'niri'):
        parser.add_argument('--' + name, type=Path, required=True)
        parser.add_argument('--' + name + '-sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    for name in ('binary', 'harness', 'niri'):
        path = getattr(args, name).resolve(strict=True); setattr(args, name, path)
        assert digest(path) == getattr(args, name + '_sha256')
    args.output = args.output.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    frozen = freeze_niri(args.niri, args.niri_sha256)
    paths = [Path(__file__), args.binary, args.harness, frozen,
             ROOT / 'scripts/desktop-fixture/input-capture-ei-client.c',
             ROOT / 'scripts/desktop-fixture/input-capture-pointer.c',
             ROOT / 'scripts/desktop-fixture/remote-input-pointer-receiver.c',
             ROOT / 'scripts/screencast_media.py', ROOT / 'scripts/validate-desktop.py']
    pins = {str(path): digest(path) for path in paths}
    result = {'status': 'running', 'scope': __doc__, 'checks': [], 'pins': pins}
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='canopa-capture-receiver-') as directory:
        session = MediaSession(Path(directory), args.output, frozen, args.niri_sha256, reference_frozen=True)
        session.capture_workers = []
        try:
            verify(args, session, result); result['status'] = 'passed'
        except Exception as error:
            result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
        finally:
            processes = list(session.processes); session.close()
            result['processes'] = [{'pid': p.pid, 'command': p.args, 'exit_code': p.poll()} for p in processes]
            result['workers'] = []
            for worker in session.capture_workers:
                terminal = bool(select.select([worker['pidfd']], [], [], 5)[0])
                result['workers'].append({'pid': worker['pid'], 'terminal': terminal})
                os.close(worker['pidfd'])
    result.update(seconds=round(time.monotonic() - started, 2),
                  changed_inputs=[name for name, checksum in pins.items() if digest(Path(name)) != checksum])
    if (result['changed_inputs'] or any(p['exit_code'] is None for p in result['processes'])
            or not all(w['terminal'] for w in result['workers'])): result['status'] = 'failed'
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({name: result.get(name) for name in ['status', 'error', 'seconds']}))
    raise SystemExit(0 if result['status'] == 'passed' else 1)


if __name__ == '__main__':
    main()
