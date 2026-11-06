#!/usr/bin/env python3
"""Private compositor pointer capture; no public portal/EI/consent claim."""
import argparse
import json
import os
from pathlib import Path
import select
import socket
import struct
import subprocess
import sys
import tempfile
import time
import traceback
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
from native_compositor import freeze_niri
from screencast_media import MediaSession, digest

ROOT = Path(__file__).resolve().parents[1]
SERVICE = 'org.canopa.Niri.InputCapture1'
PATH = '/org/canopa/Niri/InputCapture1'


def verify(args, session, result):
    from gi.repository import Gio, GLib
    output = session.start_niri(
        'debug { dbus-interfaces-in-non-session-instances; }\n'
        'prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
        'hotkey-overlay { skip-at-startup; }\nlayout { border { off; }; focus-ring { off; }; }\n',
        scale=1.25)
    connection = Gio.DBusConnection.new_for_address_sync(
        session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT |
        Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

    def call(interface, method, signature='()', values=()):
        return connection.call_sync(SERVICE, PATH, interface, method,
                                    GLib.Variant(signature, values), None,
                                    Gio.DBusCallFlags.NO_AUTO_START, 5000, None).unpack()

    def passed(name):
        result['checks'].append(name)
        (session.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
        print(name, flush=True)

    node = ET.fromstring(call('org.freedesktop.DBus.Introspectable', 'Introspect')[0])
    interface = next(item for item in node.findall('interface') if item.get('name') == SERVICE)
    method = next(item for item in interface.findall('method') if item.get('name') == 'OpenPointer')
    assert ''.join(arg.get('type') for arg in method.findall('arg') if arg.get('direction') == 'out') == 'hua(uuii)'
    properties = call('org.freedesktop.DBus.Properties', 'GetAll', '(s)', (SERVICE,))[0]
    assert (properties['Version'], properties['SupportedCapabilities']) in {(1, 2), (2, 3)}, properties
    passed('private-pointer-entrypoint-exact-fd-zone-signature-and-versioned-capabilities')

    fixture = ROOT / 'scripts/desktop-fixture'
    for protocol, source in [('virtual-pointer', fixture / 'wlr-virtual-pointer-unstable-v1.xml'),
                             ('xdg-shell', Path('/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml'))]:
        for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', mode, str(source), str(session.output / (protocol + suffix))],
                           check=True, timeout=15)
    pointer_path = session.output / 'pointer'
    receiver_path = session.output / 'receiver'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output),
                    str(fixture / 'input-capture-pointer.c'), str(session.output / 'virtual-pointer.c'),
                    '-lwayland-client', '-lm', '-o', str(pointer_path)], check=True, timeout=30)
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output),
                    str(fixture / 'remote-input-pointer-receiver.c'), str(session.output / 'xdg-shell.c'),
                    '-lwayland-client', '-lxkbcommon', '-o', str(receiver_path)], check=True, timeout=30)
    receiver = session.start([str(receiver_path)], 'receiver.log')
    receiver_log = session.output / 'receiver.log'
    session.wait_for(lambda: 'focused' in receiver_log.read_text(), 'local input receiver')

    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))

    window = session.wait_for(lambda: next((window for window in windows() if window['pid'] == receiver.pid), None),
                              'receiver window')
    session.run(['niri', 'msg', 'action', 'fullscreen-window', '--id', str(window['id'])])
    pointer_log = session.output / 'pointer.log'
    with pointer_log.open('w') as log:
        pointer = subprocess.Popen([str(pointer_path)], stdin=subprocess.PIPE, stdout=log, stderr=log,
                                   env=session.env, start_new_session=True)
    session.processes.append(pointer)
    session.wait_for(lambda: 'ready' in pointer_log.read_text(), 'persistent local pointer')
    serial = 0

    def mouse(command):
        nonlocal serial
        serial += 1
        pointer.stdin.write((command + '\n').encode()); pointer.stdin.flush()
        session.wait_for(lambda: f'ack {serial}\n' in pointer_log.read_text(), 'pointer command acknowledged')

    def center():
        mouse('absolute 40000 40000 100000 100000')
        session.wait_for(lambda: 'motion ' in receiver_log.read_text() or 'enter ' in receiver_log.read_text(),
                         'ordinary local pointer delivery')

    def local_pointer_events():
        return [line for line in receiver_log.read_text().splitlines()
                if line.startswith(('motion ', 'button ', 'axis'))]

    sockets = []

    class Lease:
        def __init__(self):
            reply, fds = connection.call_with_unix_fd_list_sync(
                SERVICE, PATH, SERVICE, 'OpenPointer', None, GLib.VariantType.new('(hua(uuii))'),
                Gio.DBusCallFlags.NO_AUTO_START, 5000, None, None)
            handle, self.generation, self.zones = reply.unpack()
            owned = fds.steal_fds(); assert handle == 0 and len(owned) == 1
            self.socket = socket.socket(fileno=owned[0]); self.socket.settimeout(3)
            self.sequence = 0; self.events = []; sockets.append(self.socket)

        def exact(self, length):
            data = bytearray()
            while len(data) < length:
                chunk = self.socket.recv(length - len(data))
                if not chunk: raise EOFError('capture lease closed')
                data.extend(chunk)
            return bytes(data)

        def receive(self):
            size, = struct.unpack('<I', self.exact(4)); assert 0 < size <= 16384
            value = json.loads(self.exact(size)); self.events.append(value); return value

        def command(self, op, **options):
            self.sequence += 1
            data = json.dumps({'sequence': self.sequence, 'command': {'op': op, **options}}).encode()
            self.socket.sendall(struct.pack('<I', len(data)) + data)
            expected = 'zones' if op == 'get_zones' else 'reply'
            for _ in range(64):
                event = self.receive()
                if event['event'] == expected and event['sequence'] == self.sequence: return event
            raise AssertionError('No matching capture reply')

        def event(self, name):
            for _ in range(64):
                event = self.receive()
                if event['event'] == name: return event
            raise AssertionError('Missing ' + name)

        def barriers(self):
            width, height, x, y = self.zones[0]
            return [[7, [x + width, y, x + width, y + height - 1]]]

        def arm(self):
            assert self.command('set_barriers', zone_set=self.generation, barriers=self.barriers())['error'] is None
            assert self.command('enable')['error'] is None

        def activate(self):
            center(); mouse('relative 5000 0')
            event = self.event('activated'); assert event['barrier'] == 7
            width, _, x, _ = self.zones[0]
            assert event['position'][0] > x + width
            return event['activation']

    first = Lease()
    logical = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))[output]['logical']
    assert first.zones == [(logical['width'], logical['height'], logical['x'], logical['y'])], first.zones
    failed = first.command('set_barriers', zone_set=first.generation,
                           barriers=[[0, [0, 0, 0, 20]], [8, [5, 5, 8, 9]], *first.barriers()])
    assert failed['failed_barriers'] == [0, 8] and failed['error'] is None
    center(); first.command('enable'); activation = first.activate()
    # This control roundtrip flushes the compositor before observing local logs.
    before = local_pointer_events()
    mouse('relative 3.25 -1.5')
    assert first.event('motion')['delta'] == [3.25, -1.5]
    mouse('button 272 1'); assert first.event('button')['pressed'] is True
    mouse('axis 0 6.5'); axis = first.event('axis')
    assert axis['source'] == 2 and axis['amount'][1] == 6.5
    first.command('get_zones')
    # A keyboard marker reaches this same Wayland client after all preceding
    # pointer events. Keep its source alive through the FD-close assertion so
    # device destruction cannot stand in for capture revocation.
    barrier = session.start(['wtype', '-k', 'F24', '-s', '2000'], 'local-barrier.log')
    session.wait_for(lambda: sum(line.startswith('key ') and line.endswith(' F24')
                                for line in receiver_log.read_text().splitlines()) == 2,
                     'ordered local client event marker')
    assert barrier.poll() is None
    assert local_pointer_events() == before
    assert first.command('release', activation=activation + 1)['error'] == 'stale_activation'
    mouse('relative 1 0'); first.event('motion')
    # Close while a button remains held; its eventual release must stay local-suppressed.
    first.socket.close(); mouse('barrier'); mouse('button 272 0')
    center(); mouse('relative 2 1'); mouse('button 272 1'); mouse('button 272 0')
    session.wait_for(lambda: receiver_log.read_text().count('button 272 1') == 1 and
                     receiver_log.read_text().count('button 272 0') == 1, 'fresh local button cycle after revocation')
    assert barrier.poll() is None
    assert barrier.wait(timeout=4) == 0; session.processes.remove(barrier)
    result.setdefault('retired', []).append({'pid': barrier.pid, 'exit_code': 0})
    passed('real-preclip-crossing-diverts-motion-buttons-scroll-and-fd-close-restores-local-input')

    second, contender = Lease(), Lease(); second.arm()
    assert contender.command('enable')['error'] == 'busy'
    activation = second.activate()
    assert second.command('enable')['error'] is None
    assert second.command('release', activation=activation)['error'] is None
    assert second.events[-2]['event'] == 'deactivated'
    newer = second.activate(); assert newer != activation
    assert second.command('release', activation=activation)['error'] == 'stale_activation'
    before = len(second.events)
    assert second.command('set_barriers', zone_set=second.generation, barriers=[])['error'] is None
    assert [event['event'] for event in second.events[before:]] == ['deactivated', 'disabled', 'reply']
    center(); mouse('relative 5000 0')
    assert contender.command('enable')['error'] is None
    assert contender.command('disable')['error'] is None
    passed('exclusive-enablement-repeated-enable-stale-release-and-empty-barrier-suspension')

    second.arm(); second.activate(); invalidated = second.generation
    session.run(['niri', 'msg', 'output', output, 'scale', '1.5'])
    assert second.event('deactivated')['activation']
    second.event('disabled'); assert second.event('zones_changed')['zone_set'] == invalidated
    current = second.command('get_zones'); assert current['zone_set'] != invalidated
    assert current['zones'] != second.zones
    assert second.command('set_barriers', zone_set=invalidated, barriers=second.barriers())['error']
    second.generation, second.zones = current['zone_set'], current['zones']
    second.arm(); second.activate()
    passed('live-output-change-deactivates-before-invalidated-zone-event-and-requires-new-barriers')

    # Hold the escape-producing keyboard alive, separating escape from device loss.
    escape = session.start(['wtype', '-M', 'ctrl', '-M', 'alt', '-k', 'Escape', '-m', 'alt', '-m', 'ctrl', '-s', '1000'], 'escape.log')
    second.event('deactivated'); second.event('disabled')
    assert escape.poll() is None, 'escape helper exited before capture stopped'
    assert escape.wait(timeout=3) == 0; session.processes.remove(escape)
    result.setdefault('retired', []).append({'pid': escape.pid, 'exit_code': 0})
    passed('compositor-control-alt-escape-stops-capture-before-the-triggering-device-disappears')
    second.socket.close(); contender.socket.close()

    # A slow event reader cannot retain interception or accumulate an output queue.
    slow = Lease(); slow.socket.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 4096)
    slow.arm(); slow.activate(); mouse('burst 12000')
    drained = 0
    while True:
        chunk = slow.socket.recv(65536)
        if not chunk: break
        drained += len(chunk); assert drained <= 1024 * 1024
    slow.socket.close(); center(); mouse('relative 2 0')
    passed('unread-event-backpressure-retires-the-lease-with-bounded-buffering-and-restores-input')

    held = [Lease() for _ in range(8)]
    try: Lease()
    except GLib.Error as error:
        assert Gio.DBusError.get_remote_error(error) == 'org.freedesktop.DBus.Error.LimitsExceeded', str(error)
    else: raise AssertionError('ninth capture lease admitted')
    for lease in held: lease.socket.close()
    mouse('barrier')
    malformed = Lease(); malformed.socket.sendall(struct.pack('<I', 16385))
    assert malformed.socket.recv(1) == b''; malformed.socket.close()
    healthy = Lease(); assert healthy.command('get_zones')['zone_set']
    healthy.socket.close()
    passed('finite-lease-admission-and-malformed-request-retirement-preserve-the-compositor')

    lost = Lease(); lost.arm(); lost.activate()
    mouse('button 272 1'); assert lost.event('button')['pressed'] is True
    pointer.stdin.write(b'quit\n'); pointer.stdin.flush(); pointer.stdin.close()
    assert pointer.wait(timeout=3) == 0; session.processes.remove(pointer)
    result.setdefault('retired', []).append({'pid': pointer.pid, 'exit_code': 0})
    lost.event('deactivated'); lost.event('disabled')
    assert lost.command('get_zones')['zone_set'] == lost.generation
    click = session.build_pointer()
    session.run([str(click), '640', '400'])
    session.wait_for(lambda: receiver_log.read_text().count('button 272 1') == 2 and
                     receiver_log.read_text().count('button 272 0') == 2,
                     'fresh local button cycle after source loss')
    passed('source-loss-disables-active-capture-and-retires-held-buttons-before-a-new-device')
    for descriptor in sockets: descriptor.close()
    connection.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--niri', type=Path, required=True)
    parser.add_argument('--niri-sha256', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(); args.niri = args.niri.resolve(strict=True)
    assert digest(args.niri) == args.niri_sha256
    args.output = args.output.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    frozen = freeze_niri(args.niri, args.niri_sha256)
    paths = [Path(__file__), ROOT / 'scripts/desktop-fixture/input-capture-pointer.c',
             ROOT / 'scripts/desktop-fixture/remote-input-pointer-receiver.c',
             ROOT / 'scripts/screencast_media.py', ROOT / 'scripts/validate-desktop.py', frozen]
    pins = {str(path): digest(path) for path in paths}
    result = {'status': 'running', 'scope': __doc__, 'checks': [], 'pins': pins}
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='canopa-input-capture-') as directory:
        session = MediaSession(Path(directory), args.output, frozen, args.niri_sha256, reference_frozen=True)
        try:
            verify(args, session, result); result['status'] = 'passed'
        except Exception as error:
            result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
        finally:
            processes = list(session.processes); session.close()
            result['processes'] = [{'pid': process.pid, 'command': process.args, 'exit_code': process.poll()}
                                   for process in processes]
    result.update(seconds=round(time.monotonic() - started, 2),
                  changed_inputs=[name for name, checksum in pins.items() if digest(Path(name)) != checksum])
    if result['changed_inputs'] or any(process['exit_code'] is None for process in result['processes']):
        result['status'] = 'failed'
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({name: result.get(name) for name in ['status', 'error', 'seconds']}))
    raise SystemExit(0 if result['status'] == 'passed' else 1)


if __name__ == '__main__':
    main()
