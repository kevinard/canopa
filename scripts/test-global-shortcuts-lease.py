#!/usr/bin/env python3
"""Private Niri shortcut leases and real Wayland input; no public portal claim."""
import argparse
import importlib.util
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
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from native_compositor import freeze_niri, sha256

SERVICE = 'org.canopa.Niri.GlobalShortcuts1'
PATH = '/org/canopa/Niri/GlobalShortcuts1'
REMOTE = 'org.canopa.Niri.RemoteInput1'
REMOTE_PATH = '/org/canopa/Niri/RemoteInput1'


class Channel:
    def __init__(self, stream):
        self.stream = stream
        self.stream.settimeout(3)
        self.buffer = bytearray()
        self.pending = []
        self.sequence = 0

    def send(self, value):
        payload = json.dumps(value, separators=(',', ':')).encode()
        assert 0 < len(payload) <= 32768
        self.stream.sendall(struct.pack('<I', len(payload)) + payload)

    def take(self, timeout=3):
        deadline = time.monotonic() + timeout
        while True:
            if len(self.buffer) >= 4:
                size = struct.unpack_from('<I', self.buffer)[0]
                assert 0 < size <= 32768, size
                if len(self.buffer) >= 4 + size:
                    value = json.loads(self.buffer[4:4 + size])
                    del self.buffer[:4 + size]
                    return value
            if not select.select([self.stream], [], [], max(0, deadline - time.monotonic()))[0]:
                return None
            piece = self.stream.recv(32772)
            if not piece:
                assert not self.buffer, 'truncated event before EOF'
                raise EOFError('shortcut authority retired')
            self.buffer.extend(piece)
            assert len(self.buffer) <= 65544

    def command(self, command):
        self.sequence += 1
        self.send(dict(sequence=self.sequence, command=command))
        while value := self.take():
            if value['event'] == 'reply':
                assert value['sequence'] == self.sequence, value
                return value
            self.pending.append(value)
            assert len(self.pending) <= 512
        raise AssertionError('no shortcut command reply')

    def current(self):
        reply = self.command(dict(op='list'))
        assert reply['error'] is None, reply
        return reply

    def replace(self, bindings, generation=None):
        if generation is None:
            generation = self.current()['generation']
        return self.command(dict(op='replace', generation=generation, bindings=bindings))

    def events(self):
        result, self.pending = self.pending, []
        while value := self.take(0):
            result.append(value)
            assert len(result) <= 512
        return result

    def close(self):
        self.stream.close()


def verify(args, session, result):
    from gi.repository import Gio, GLib
    session.env['PATH'] = str(args.niri.parent) + os.pathsep + session.env['PATH']
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "us,fr"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
binds { F6 { toggle-window-floating; }; }
''')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    sockets, channels = [], []
    try:
        def bus(method, value):
            return connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                'org.freedesktop.DBus', method, GLib.Variant('(s)', (value,)), None,
                Gio.DBusCallFlags.NO_AUTO_START, 5000, None).unpack()[0]
        def ready():
            try:
                return bus('GetNameOwner', SERVICE)
            except GLib.Error:
                return None
        owner = session.wait_for(ready, 'compositor shortcut owner')
        pid = bus('GetConnectionUnixProcessID', owner)
        assert any(p.pid == pid for p in session.processes)
        assert Path(f'/proc/{pid}/exe').resolve() == args.niri
        remote_owner = bus('GetNameOwner', REMOTE)
        assert bus('GetConnectionUnixProcessID', remote_owner) == pid
        node = ET.fromstring(connection.call_sync(owner, PATH, 'org.freedesktop.DBus.Introspectable',
            'Introspect', None, None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None).unpack()[0])
        interface = next(i for i in node.findall('interface') if i.get('name') == SERVICE)
        method = next(m for m in interface.findall('method') if m.get('name') == 'OpenShortcuts')
        assert ''.join(a.get('type') for a in method.findall('arg') if a.get('direction') == 'out') == 'h'
        properties = connection.call_sync(owner, PATH, 'org.freedesktop.DBus.Properties', 'GetAll',
            GLib.Variant('(s)', (SERVICE,)), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None).unpack()[0]
        assert properties['Version'] == 1, properties
        result['service_pid_matches_compositor'] = True
        def passed(name, **values):
            result['checks'].append(dict(name=name, **values))
            (session.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
            print(name, flush=True)
        passed('private-v1-fd-contract-and-compositor-owner')
        for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', mode, '/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml',
                str(session.output / ('xdg-shell' + suffix))], check=True, timeout=15)
        binary = session.output / 'receiver'
        subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output),
            str(ROOT / 'scripts/desktop-fixture/remote-input-receiver.c'), str(session.output / 'xdg-shell.c'),
            '-lwayland-client', '-lxkbcommon', '-o', str(binary)], check=True, timeout=30)
        receiver = session.start([str(binary)], 'receiver.log')
        log = session.output / 'receiver.log'
        session.wait_for(lambda: log.exists() and 'focused' in log.read_text(), 'receiver focus')
        def lines():
            assert receiver.poll() is None
            return log.read_text().splitlines()
        def open_keys():
            value, fds = connection.call_with_unix_fd_list_sync(remote_owner, REMOTE_PATH, REMOTE, 'OpenKeyboard',
                GLib.Variant('()', ()), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None, None)
            stream = socket.socket(fileno=fds.get(value.unpack()[0])); stream.settimeout(3); sockets.append(stream)
            return stream
        def open_shortcuts():
            value, fds = connection.call_with_unix_fd_list_sync(owner, PATH, SERVICE, 'OpenShortcuts',
                GLib.Variant('()', ()), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None, None)
            channel = Channel(socket.socket(fileno=fds.get(value.unpack()[0]))); channels.append(channel)
            channel.current()
            return channel
        def key(stream, code, down):
            stream.sendall(struct.pack('<II', code, int(down)))
        observer = open_keys()
        def marker(stream=None):
            if stream is None:
                stream = observer
            before = len(lines()); key(stream, 66, True); key(stream, 66, False)
            session.wait_for(lambda: any(x.startswith('key 66 0 ') for x in lines()[before:]), 'ordered F8 receiver marker')
        def edges(before, code):
            return [int(x.split()[2]) for x in lines()[before:] if x.startswith(f'key {code} ')]
        def close_keys(stream):
            stream.shutdown(socket.SHUT_WR)
            received = bytearray()
            while piece := stream.recv(4096):
                received.extend(piece); assert len(received) <= 65536
            assert len(received) % 16 == 0
            stream.close(); marker()
        def action_events(channel):
            return [e for e in channel.events() if e['event'] in ('activated', 'deactivated')]
        def bind(channel, bindings):
            reply = channel.replace(bindings)
            assert reply['error'] is None, reply
            channel.events()
            return reply['generation']
        channel = open_shortcuts()
        generation = bind(channel, [(1, 'CTRL+a'), (2, 'CTRL+q')])
        source = open_keys(); before = len(lines())
        key(source, 29, True); key(source, 30, True); marker(source)
        events = action_events(channel)
        assert len(events) == 1 and events[0]['event'] == 'activated' and events[0]['binding'] == 1 and events[0]['generation'] == generation, events
        assert isinstance(events[0]['timestamp_ms'], int) and events[0]['timestamp_ms'] > 0
        key(source, 29, False); marker(source); assert action_events(channel) == []
        key(source, 30, False); marker(source)
        released = action_events(channel)
        assert len(released) == 1 and released[0]['event'] == 'deactivated' and released[0]['binding'] == 1, released
        assert edges(before, 30) == [], lines()[before:]
        passed('modifier-first-release-pairs-original-action-and-suppresses-ordinary-key', events=events + released)
        key(source, 29, True); key(source, 30, True); marker(source); action_events(channel)
        session.run(['niri', 'msg', 'action', 'switch-layout', 'next']); marker(source)
        key(source, 30, False); marker(source)
        events = action_events(channel); assert len(events) == 1 and events[0]['binding'] == 1 and events[0]['event'] == 'deactivated', events
        key(source, 30, True); key(source, 30, False); marker(source)
        events = action_events(channel); assert [e['binding'] for e in events] == [2, 2], events
        key(source, 16, True); key(source, 16, False); key(source, 29, False); marker(source)
        events = action_events(channel); assert [e['binding'] for e in events] == [1, 1], events
        session.run(['niri', 'msg', 'action', 'switch-layout', 'next']); marker(source)
        passed('current-layout-resolution-and-held-recipient-survive-group-switch')
        generation = bind(channel, [(3, 'F7')]); before = len(lines())
        first, second = open_keys(), open_keys()
        key(first, 65, True); marker(first); key(second, 65, True); marker(second)
        events = action_events(channel); assert [e['event'] for e in events] == ['activated'], events
        close_keys(first); assert action_events(channel) == []
        close_keys(second); events = action_events(channel)
        assert len(events) == 1 and events[0]['event'] == 'deactivated', events
        assert edges(before, 65) == [], lines()[before:]
        passed('overlapping-source-eof-retires-only-last-holder-without-stray-keyup')
        key(source, 65, True); marker(source); action_events(channel)
        reply = channel.replace([(4, 'F9')]); assert reply['error'] is None
        events = action_events(channel); assert len(events) == 1 and events[0]['event'] == 'deactivated' and events[0]['generation'] == generation, events
        current_generation = reply['generation']; assert current_generation > generation
        # RemoteInput rejects a duplicate physical press as a protocol error.
        # Release the existing hold once; repeat semantics have their own source
        # and core tests rather than using an invalid RemoteInput frame here.
        key(source, 65, False); marker(source); assert action_events(channel) == []
        before = len(lines()); key(source, 65, True); key(source, 65, False); key(source, 67, True); key(source, 67, False); marker(source)
        events = action_events(channel); assert [e['binding'] for e in events] == [4, 4] and all(e['generation'] == current_generation for e in events), events
        assert edges(before, 65) == [1, 0] and edges(before, 67) == [], lines()[before:]
        passed('held-rebind-retires-old-generation-before-new-trigger')
        previous = channel.current(); reply = channel.replace([(5, 'F6')]); assert reply['error'] == 'conflict' and reply['failed'] == [5], reply
        assert channel.current()['bindings'] == previous['bindings']
        reply = channel.replace([(6, 'not_a_keysym')]); assert reply['error'] == 'unresolved'
        assert channel.current()['bindings'] == previous['bindings']
        reply = channel.replace([(6, 'F10')], generation=generation); assert reply['error'] == 'stale_generation'
        other = open_shortcuts(); reply = other.replace([(7, 'F9')]); assert reply['error'] == 'conflict' and reply['failed'] == [7], reply
        other.close(); marker()
        passed('ordinary-and-foreign-conflicts-invalid-triggers-and-stale-config-preserve-set')
        a11y_owner = bus('GetNameOwner', 'org.freedesktop.a11y.Manager')
        def grabs(values):
            connection.call_sync(a11y_owner, '/org/freedesktop/a11y/Manager', 'org.freedesktop.a11y.KeyboardMonitor', 'SetKeyGrabs',
                GLib.Variant('(aua(uu))', ([], values)), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None)
            marker()
        key(source, 67, True); marker(source); action_events(channel)
        grabs([(0xffc6, 0)])
        events = channel.events()
        assert [e['event'] for e in events] == ['deactivated', 'changed'] and events[-1]['bindings'] == [] and events[-1]['failed'] == [4], events
        key(source, 67, False); marker(source); assert action_events(channel) == []
        grabs([]); events = channel.events(); assert events[-1]['event'] == 'changed' and events[-1]['bindings'] == [[4, 'F9']], events
        key(source, 67, True); key(source, 67, False); marker(source)
        assert [e['event'] for e in action_events(channel)] == ['activated', 'deactivated']
        passed('a11y-reservation-change-retires-live-press-and-restores-approved-trigger')
        generation = bind(channel, [(11, 'Shift_L')]); before = len(lines())
        key(source, 42, True); marker(source)
        events = action_events(channel)
        assert len(events) == 1 and events[0]['event'] == 'activated' and events[0]['binding'] == 11, events
        grabs([]); assert channel.events() == [], 'identical a11y reservations retired an action'
        key(source, 42, False); marker(source)
        events = action_events(channel)
        assert len(events) == 1 and events[0]['event'] == 'deactivated' and events[0]['generation'] == generation, events
        assert edges(before, 42) == [], lines()[before:]
        passed('modifier-trigger-uses-post-press-state-and-identical-reservations-preserve-hold')
        bind(channel, [(4, 'F9')])
        key(source, 67, True); marker(source); action_events(channel)
        channel.send(dict(sequence=channel.sequence + 1, command=dict(op='list', extra=True)))
        try:
            while channel.take():
                pass
            raise AssertionError('malformed shortcut command did not close')
        except EOFError:
            pass
        before = len(lines()); key(source, 67, False); marker(source); assert edges(before, 67) == [], lines()[before:]
        replacement = open_shortcuts(); bind(replacement, [(8, 'F9')]); replacement.close(); marker()
        passed('malformed-control-revokes-own-authority-and-preserves-held-suppression')
        close_keys(source)
        leases = [open_shortcuts() for _ in range(16)]
        try:
            open_shortcuts()
            raise AssertionError('seventeenth shortcut lease admitted')
        except GLib.Error as error:
            assert 'LimitsExceeded' in str(error), error
        leases.pop().close(); marker()
        recovered = open_shortcuts(); recovered.close()
        for lease in leases:
            lease.close()
        marker()
        passed('sixteen-lease-admission-recovers-after-fd-close')
        slow, fast = open_shortcuts(), open_shortcuts()
        bind(slow, [(22, 'F10')]); bind(fast, [(23, 'F11')])
        frames = bytearray()
        for sequence in range(slow.sequence + 1, slow.sequence + 4097):
            payload = json.dumps(dict(sequence=sequence, command=dict(op='list')), separators=(',', ':')).encode()
            frames.extend(struct.pack('<I', len(payload)) + payload)
        try:
            slow.stream.sendall(frames)
        except (BrokenPipeError, ConnectionResetError):
            pass
        marker()
        hangup = select.POLLHUP | select.POLLERR | getattr(select, 'POLLRDHUP', 0)
        poller = select.poll(); poller.register(slow.stream, hangup)
        observed = session.wait_for(lambda: next((mask for _, mask in poller.poll(0) if mask & hangup), None),
                                    'unread shortcut output retires the slow lease')
        marker()
        # The last write may be partial when output fills. EOF revokes the FD;
        # no incomplete event is accepted as a new frame or activation.
        received = bytearray()
        reset = False
        try:
            while piece := slow.stream.recv(32768):
                received.extend(piece); assert len(received) <= 1024 * 1024
        except ConnectionResetError:
            # Shutting down with unread control requests may report reset
            # instead of clean EOF. The observed pre-read hangup still proves
            # retirement; either terminal result revokes this FD.
            reset = True
        key(observer, 87, True); key(observer, 87, False); marker()
        assert [e['event'] for e in action_events(fast)] == ['activated', 'deactivated']
        assert fast.replace([(24, 'F10')])['error'] is None, 'slow lease retained its conflict'
        passed('slow-output-retires-only-its-lease-and-keeps-input-responsive',
               drained_bytes=len(received), hangup_before_read=observed, connection_reset=reset)
        def snapshot():
            return connection.call_sync(remote_owner, REMOTE_PATH, REMOTE, 'KeyboardSnapshot',
                GLib.Variant('()', ()), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None).unpack()[0]
        def changes(channel, expected):
            observed = []
            def collect():
                observed.extend(channel.events())
                return next((e for e in reversed(observed)
                             if e['event'] == 'changed' and e['bindings'] == expected), None)
            session.wait_for(collect, 'updated admitted shortcut set')
            return observed
        config = session.runtime / 'niri.kdl'; original = config.read_text()
        old_map = snapshot(); before = len(lines())
        key(observer, 68, True); marker(); action_events(fast)
        assert original.count('F6 { toggle-window-floating; }') == 1
        config.write_text(original.replace('F6 { toggle-window-floating; }',
            'F6 { toggle-window-floating; }; F10 { toggle-window-floating; }'))
        events = changes(fast, [])
        assert sum(e['event'] == 'deactivated' for e in events) == 1, events
        map_changed = snapshot() != old_map
        if not map_changed:
            key(observer, 68, False); marker()
        else:
            observer.close(); observer = open_keys(); marker()
        assert edges(before, 68) == [], lines()[before:]
        config.write_text(original)
        changes(fast, [[24, 'F10']])
        passed('config-reservation-reload-withdraws-and-restores-approved-binding',
               keymap_changed_during_bind_reload=map_changed)
        bind(fast, [(25, 'a')]); old_map = snapshot(); before = len(lines())
        key(observer, 30, True); marker(); action_events(fast)
        assert 'layout "us,fr"' in original
        config.write_text(original.replace('layout "us,fr"', 'layout "fr,us"'))
        session.wait_for(lambda: snapshot() != old_map, 'configured keymap identity changes')
        events = changes(fast, [[25, 'a']])
        assert sum(e['event'] == 'deactivated' for e in events) == 1, events
        assert not any(e['event'] == 'activated' for e in events), events
        # The injected source is pinned to the old keymap. Its real EOF and
        # source cleanup must not close the independent shortcut configuration.
        remaining = bytearray()
        while piece := observer.recv(4096):
            remaining.extend(piece); assert len(remaining) <= 65536
        observer.close(); observer = open_keys()
        key(observer, 16, True); key(observer, 16, False); marker()
        events = action_events(fast)
        assert [e['binding'] for e in events] == [25, 25], events
        assert edges(before, 30) == [] and edges(before, 16) == [], lines()[before:]
        fast.close(); marker()
        passed('keymap-reload-retires-active-source-and-resolves-the-new-logical-trigger')
    finally:
        for channel in channels:
            channel.close()
        for stream in sockets:
            stream.close()
        connection.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--niri', required=True, type=Path)
    parser.add_argument('--niri-sha256', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    args.output = args.output.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    args.niri = freeze_niri(args.niri.resolve(strict=True), args.niri_sha256)
    pins = {str(p): sha256(p) for p in [Path(__file__).resolve(), ROOT / 'scripts/native_compositor.py',
        ROOT / 'scripts/validate-desktop.py', ROOT / 'scripts/desktop-fixture/remote-input-receiver.c', args.niri]}
    result = dict(status='running', pins=pins, checks=[], scope=__doc__)
    spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
    desktop = importlib.util.module_from_spec(spec); spec.loader.exec_module(desktop)
    session = None; start = time.monotonic()
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-shortcut-lease-') as directory:
            session = desktop.Session(Path(directory), args.output)
            try:
                verify(args, session, result)
            finally:
                session.close()
        result['status'] = 'passed'
    except Exception as error:
        result.update(status='failed', error=str(error), traceback=traceback.format_exc())
        raise
    finally:
        result['changed_inputs'] = [p for p, h in pins.items() if sha256(Path(p)) != h]
        result['processes'] = [dict(pid=p.pid, exit_code=p.poll()) for p in session.processes] if session else []
        result['seconds'] = round(time.monotonic() - start, 2)
        (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    assert not result['changed_inputs'] and all(p['exit_code'] is not None for p in result['processes'])
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
