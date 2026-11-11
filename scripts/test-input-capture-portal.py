#!/usr/bin/env python3
"""Real InputCapture frontend, Rust session, Iced consent and EI receiver acceptance."""
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
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
from native_compositor import freeze_niri
from screencast_media import MediaSession, digest

ROOT = Path(__file__).resolve().parents[1]
FRONT = 'org.freedesktop.portal.Desktop'
BACK = 'org.freedesktop.impl.portal.desktop.canopa'
PATH = '/org/freedesktop/portal/desktop'
PUBLIC = 'org.freedesktop.portal.InputCapture'
IMPL = 'org.freedesktop.impl.portal.InputCapture'
CLIP = 'org.freedesktop.portal.Clipboard'
APP = 'org.canopa.InputCaptureFixture'


def verify(args, session, result):
    from gi.repository import Gio, GLib
    config = session.runtime / 'canopa.kdl'
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.scheme}"; }}\n')
    portals = session.runtime / 'portals'; portals.mkdir()
    clip_interface = 'org.freedesktop.impl.portal.Clipboard'
    descriptor = (args.portal_descriptor.read_text() if args.portal_descriptor else
                  '[portal]\nDBusName=' + BACK + '\nInterfaces=' + IMPL + ';org.freedesktop.impl.portal.Settings;' + (clip_interface + ';' if args.clipboard else '') + '\nUseIn=Canopa;\n')
    assert IMPL + ';' in descriptor and (not args.clipboard or clip_interface + ';' in descriptor)
    (portals / 'canopa.portal').write_text(descriptor)
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\n' + IMPL + '=canopa\norg.freedesktop.impl.portal.Settings=canopa\n' + (clip_interface + '=canopa\n' if args.clipboard else ''))
    applications = Path(session.env['XDG_DATA_HOME']) / 'applications'; applications.mkdir(parents=True)
    (applications / (APP + '.desktop')).write_text('[Desktop Entry]\nType=Application\nName=Input capture fixture\nExec=true\n')
    home = session.runtime / 'home'; home.mkdir()
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home), XDG_CURRENT_DESKTOP='Canopa', XDG_DESKTOP_PORTAL_DIR=str(portals),
                       XDG_DATA_DIRS=session.env['XDG_DATA_HOME'], GIO_USE_VFS='local', GSETTINGS_BACKEND='memory',
                       GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas')
    output = session.start_niri('debug { dbus-interfaces-in-non-session-instances; }\nprefer-no-csd\n'
        'animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n'
        'layout { border { off; }; focus-ring { off; }; }\n', scale=1.25)

    def call(connection, destination, path, interface, method, signature='()', values=()):
        return connection.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None,
                                    Gio.DBusCallFlags.NONE, 12000, None).unpack()

    def connect(register=True):
        caller = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        if register:
            call(caller, FRONT, PATH, 'org.freedesktop.host.portal.Registry', 'Register', '(sa{sv})', (APP, {}))
        return caller

    observer = connect(False)
    call(observer, 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'UpdateActivationEnvironment', '(a{ss})', ({**session.env},))
    backend = session.start([str(args.binary), '--canopa-config', str(config)], 'backend.log')
    session.wait_for(lambda: call(observer, 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
        'NameHasOwner', '(s)', (BACK,))[0], 'actual backend main')
    session.start([str(args.frontend), '--verbose'], 'frontend.log')
    session.wait_for(lambda: call(observer, 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
        'NameHasOwner', '(s)', (FRONT,))[0], 'canonical frontend')
    trace = session.output / 'backend-calls.log'
    session.start(['dbus-monitor', '--session', "type='method_call',interface='" + IMPL + "',member='CreateSession2'",
                   "type='method_call',interface='" + IMPL + "',member='Start'",
                   "type='method_call',interface='org.freedesktop.impl.portal.Clipboard',member='SelectionRead'"], trace.name)

    def passed(name):
        result['checks'].append(name); (session.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
        print(name, flush=True)

    def drain():
        while GLib.MainContext.default().pending(): GLib.MainContext.default().iteration(False)

    expected_exits = set()
    def wait(condition, description, allowed=(), timeout=20):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            drain()
            for process in session.processes:
                if process not in allowed and process not in expected_exits and process.poll() is not None:
                    raise RuntimeError(f'Unexpected process exit: {process.args}: {process.returncode}')
            value = condition()
            if value: return value
            time.sleep(.05)
        raise RuntimeError('Timed out waiting for ' + description)

    def retire(process, expected=0):
        code = process.wait(timeout=8); session.processes.remove(process)
        result.setdefault('retired', []).append({'pid': process.pid, 'command': process.args, 'exit_code': code})
        expected_exits.discard(process)
        assert code in expected if isinstance(expected, tuple) else code == expected, (process.args, code)

    def rejected(operation):
        try: operation()
        except GLib.Error as error:
            name = Gio.DBusError.get_remote_error(error)
            assert name and name not in ('org.freedesktop.DBus.Error.NoReply', 'org.freedesktop.DBus.Error.Timeout', 'org.freedesktop.DBus.Error.TimedOut'), str(error)
            return
        raise AssertionError('Unexpectedly accepted unapproved capture operation')

    xml = call(observer, BACK, PATH, 'org.freedesktop.DBus.Introspectable', 'Introspect')[0]
    interface = next(i for i in ET.fromstring(xml).findall('interface') if i.get('name') == IMPL)
    signatures = {m.get('name'): (''.join(a.get('type') for a in m.findall('arg') if a.get('direction') != 'out'),
                                ''.join(a.get('type') for a in m.findall('arg') if a.get('direction') == 'out'))
                  for m in interface.findall('method')}
    assert signatures == {'CreateSession': ('oossa{sv}', 'ua{sv}'), 'CreateSession2': ('osa{sv}', 'a{sv}'),
        'Start': ('oossa{sv}', 'ua{sv}'), 'GetZones': ('oosa{sv}', 'ua{sv}'),
        'SetPointerBarriers': ('oosa{sv}aa{sv}u', 'ua{sv}'), 'Enable': ('osa{sv}', 'ua{sv}'),
        'Disable': ('osa{sv}', 'ua{sv}'), 'Release': ('osa{sv}', 'ua{sv}'), 'ConnectToEIS': ('osa{sv}', 'h')}, signatures
    properties = call(observer, BACK, PATH, 'org.freedesktop.DBus.Properties', 'GetAll', '(s)', (IMPL,))[0]
    assert properties['version'] == 2 and properties['SupportedCapabilities'] == (3 if args.keyboard else 2), properties
    assert call(observer, FRONT, PATH, 'org.freedesktop.DBus.Properties', 'Get', '(ss)', (PUBLIC, 'version'))[0] == 2
    passed('exact-backend-v2-wire-and-truthful-approved-input-capabilities-through-standard-frontend')

    responses, closed, phases, clients = {}, set(), [], []
    offers, transfers = {}, []
    counter = 0

    def client():
        caller = connect(); clients.append(caller)
        caller.signal_subscribe(FRONT, 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE,
            lambda c,s,p,i,n,v: responses.__setitem__(p, v.unpack()))
        caller.signal_subscribe(FRONT, 'org.freedesktop.portal.Session', 'Closed', None, None, Gio.DBusSignalFlags.NONE,
            lambda c,s,p,i,n,v: closed.add(p))
        caller.signal_subscribe(FRONT, PUBLIC, None, None, None, Gio.DBusSignalFlags.NONE,
            lambda c,s,p,i,n,v: phases.append((n, *v.unpack())))
        caller.signal_subscribe(FRONT, CLIP, 'SelectionOwnerChanged', None, None, Gio.DBusSignalFlags.NONE,
            lambda c,s,p,i,n,v: offers.__setitem__(v.unpack()[0], v.unpack()[1]))
        caller.signal_subscribe(FRONT, CLIP, 'SelectionTransfer', None, None, Gio.DBusSignalFlags.NONE,
            lambda c,s,p,i,n,v: transfers.append(v.unpack()))
        return caller

    def options(**extra):
        nonlocal counter
        counter += 1
        return {'handle_token': GLib.Variant('s', f'capture{counter}'), **extra}

    def public(caller, method, signature, values):
        return call(caller, FRONT, PATH, PUBLIC, method, signature, values)

    def response(path, expected=0):
        status, values = wait(lambda: responses.get(path), 'public request response', timeout=35)
        assert status == expected, (status, values)
        return values

    def create(caller):
        return public(caller, 'CreateSession2', '(a{sv})', (options(session_handle_token=GLib.Variant('s', f's{counter}')),))[0]['session_handle']

    def start(caller, key, capabilities=2):
        return public(caller, 'Start', '(osa{sv})', (key, '', options(capabilities=GLib.Variant('u', capabilities))))[0]

    def windows(): return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
    def action(*args): return session.run(['niri', 'msg', 'action', *map(str, args)])
    def keys(*args): return session.run(['wtype', *args])
    def popup(clipboard=False):
        window = wait(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.Portal' and w['title'] == 'Input capture — Canopa'), None), 'Iced capture window')
        if not window['is_floating']: action('move-window-to-floating', '--id', window['id'])
        consent = b'--input-capture-dialog' in Path(f"/proc/{window['pid']}/cmdline").read_bytes().split(b'\0')
        action('set-window-width', '--id', window['id'], 560 if consent else 480)
        action('set-window-height', '--id', window['id'], ((460 if clipboard else 380) + (60 if args.keyboard else 0)) if consent else (340 if clipboard else 280))
        action('move-floating-window', '--id', window['id'], '--x', 20, '--y', 10); action('focus-window', '--id', window['id'])
        time.sleep(.15); return window
    def gone(window): wait(lambda: not any(w['id'] == window['id'] for w in windows()), 'capture surface retired')
    def approve(path, clipboard=False, capabilities=2):
        dialog = popup(clipboard)
        if capabilities & 2: keys('-M', 'alt', '-k', 'p', '-m', 'alt')
        if capabilities & 1: keys('-M', 'alt', '-k', 'k', '-m', 'alt')
        if clipboard: keys('-M', 'alt', '-k', 'c', '-m', 'alt')
        keys('-M', 'alt', '-k', 'a', '-m', 'alt')
        values = response(path); gone(dialog)
        assert values['capabilities'] == capabilities and values.get('clipboard_enabled', False) == clipboard, values
        return values

    caller = client(); key = create(caller)
    assert not any(w['app_id'] == 'org.canopa.Portal' for w in windows())
    rejected(lambda: public(caller, 'ConnectToEIS', '(oa{sv})', (key, {})))
    pending = start(caller, key, 3 if args.keyboard else 2); dialog = popup()
    keys('-M', 'alt', '-k', 'a', '-m', 'alt'); drain(); assert pending not in responses
    assert any(w['id'] == dialog['id'] for w in windows()), 'unchecked permission was granted'
    session.capture('consent-' + args.scheme + '.png')
    keys('-k', 'Escape'); response(pending, 1); gone(dialog)
    passed('modern-session-does-not-start-or-connect-before-explicit-unchecked-iced-consent')

    if not args.keyboard:
        caller = client(); opts = options(capabilities=GLib.Variant('u', 2), session_handle_token=GLib.Variant('s', f'legacy{counter}'))
        trace_before = len(trace.read_text())
        legacy = public(caller, 'CreateSession', '(sa{sv})', ('', opts))[0]
        values = approve(legacy); key = values['session_handle']
        controls = popup(); session.capture('controls-' + args.scheme + '.png')
        legacy_trace = trace.read_text()[trace_before:]
        assert 'member=CreateSession2' in legacy_trace and 'member=Start' in legacy_trace
        passed('legacy-public-create-translates-to-v2-create-and-start-with-real-iced-controls')

    fixture = ROOT / 'scripts/desktop-fixture'
    for protocol, source in [('virtual-pointer', fixture / 'wlr-virtual-pointer-unstable-v1.xml'),
                             ('xdg-shell', Path('/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml'))]:
        for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', mode, str(source), str(session.output / (protocol + suffix))], check=True, timeout=15)
    if args.keyboard:
        for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', mode, str(fixture / 'virtual-keyboard-unstable-v1.xml'),
                str(session.output / ('virtual-keyboard' + suffix))], check=True, timeout=15)
        subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output),
            str(fixture / 'input-capture-keyboard.c'), str(session.output / 'virtual-keyboard.c'),
            '-lwayland-client', '-lxkbcommon', '-o', str(session.output / 'keyboard')], check=True, timeout=30)
    pointer_bin, local_bin, ei_bin = [session.output / n for n in ('pointer', 'local', 'ei-client')]
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output), str(fixture / 'input-capture-pointer.c'),
        str(session.output / 'virtual-pointer.c'), '-lwayland-client', '-lm', '-o', str(pointer_bin)], check=True, timeout=30)
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output), str(fixture / 'remote-input-pointer-receiver.c'),
        str(session.output / 'xdg-shell.c'), '-lwayland-client', '-lxkbcommon', '-o', str(local_bin)], check=True, timeout=30)
    flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'libei-1.0'], text=True).split()
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', str(fixture / 'input-capture-ei-client.c'), '-o', str(ei_bin), *flags], check=True, timeout=30)
    local = session.start([str(local_bin)], 'local.log'); local_log = session.output / 'local.log'
    wait(lambda: 'focused' in local_log.read_text(), 'ordinary receiver')
    local_window = wait(lambda: next((w for w in windows() if w['pid'] == local.pid), None), 'local receiver window')
    action('fullscreen-window', '--id', local_window['id'])
    with (session.output / 'pointer.log').open('w') as log:
        pointer = subprocess.Popen([str(pointer_bin)], stdin=subprocess.PIPE, stdout=log, stderr=log, env=session.env, start_new_session=True)
    session.processes.append(pointer); pointer_log = session.output / 'pointer.log'
    wait(lambda: 'ready' in pointer_log.read_text(), 'native pointer source'); serial = 0
    def mouse(command):
        nonlocal serial
        serial += 1; pointer.stdin.write((command + '\n').encode()); pointer.stdin.flush()
        wait(lambda: f'ack {serial}\n' in pointer_log.read_text(), 'pointer roundtrip')
    def center(): mouse('absolute 40000 40000 100000 100000')

    class Receiver:
        def __init__(self, caller, key, name, grants=2):
            self.caller, self.key, self.name = caller, key, name
            reply, fds = caller.call_with_unix_fd_list_sync(FRONT, PATH, PUBLIC, 'ConnectToEIS', GLib.Variant('(oa{sv})', (key, {})),
                GLib.VariantType.new('(h)'), Gio.DBusCallFlags.NONE, 12000, None, None)
            owned = fds.steal_fds(); assert reply.unpack() == (0,) and len(owned) == 1
            channel = socket.socket(fileno=owned[0]); assert channel.family == socket.AF_UNIX and channel.type == socket.SOCK_STREAM
            assert fcntl.fcntl(channel.fileno(), fcntl.F_GETFD) & fcntl.FD_CLOEXEC
            self.pid = struct.unpack('3i', channel.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))[0]
            argv = Path(f'/proc/{self.pid}/cmdline').read_bytes().split(b'\0')[:-1]
            assert argv == [os.fsencode(args.binary), b'--canopa-config', os.fsencode(config), b'--input-capture-worker'], argv
            self.pidfd = os.pidfd_open(self.pid); session.capture_workers.append({'pid': self.pid, 'pidfd': os.dup(self.pidfd)})
            self.log = session.output / (name + '-ei.log'); read, write = os.pipe(); self.control = os.fdopen(write, 'wb', buffering=0)
            with self.log.open('w') as log:
                self.process = subprocess.Popen([str(ei_bin), str(read), 'normal', str(grants)], stdin=channel, pass_fds=(read,), stdout=log, stderr=log, env=session.env, start_new_session=True)
            os.close(read); channel.close(); session.processes.append(self.process)
            wait(lambda: any('resumed' in e for e in self.events()), 'passive EI receiver')
        def events(self): return [json.loads(line) for line in self.log.read_text().splitlines() if line.startswith('{')]
        def event(self, field, value=None, since=0):
            return wait(lambda: next((e for e in self.events()[since:] if field in e and (value is None or e[field] == value)), None),
                        self.name + ': ' + field, (self.process,))
        def terminal(self): wait(lambda: bool(select.select([self.pidfd], [], [], 0)[0]), 'capture worker exit', (self.process,))
        def finish(self):
            self.terminal(); self.event('disconnected'); retire(self.process); self.control.close(); os.close(self.pidfd)

    def zones(caller, key): return response(public(caller, 'GetZones', '(oa{sv})', (key, options()))[0])
    def barriers(caller, key):
        current = zones(caller, key); width, height, x, y = current['zones'][0]
        values = [dict(barrier_id=GLib.Variant('u', 0), position=GLib.Variant('(iiii)', (x, y, x, y + height - 1))),
                  dict(barrier_id=GLib.Variant('u', 7), position=GLib.Variant('(iiii)', (x + width, y, x + width, y + height - 1)))]
        answer = response(public(caller, 'SetPointerBarriers', '(oa{sv}aa{sv}u)', (key, options(), values, current['zone_set']))[0])
        assert answer['failed_barriers'] == [0], answer
        return current
    def enable(caller, key):
        public(caller, 'Enable', '(oa{sv})', (key, {})); zones(caller, key)
    def activate(receiver):
        zone = zones(receiver.caller, receiver.key)['zones'][0]
        before = len(phases); center(); mouse('relative 5000 0')
        event = wait(lambda: next((v for v in phases[before:] if v[0] == 'Activated' and v[1] == receiver.key), None), 'public activation')
        activation = event[2]['activation_id']; receiver.event('start', activation)
        assert event[2]['barrier_id'] == 7 and event[2]['cursor_position'][0] > zone[2] + zone[0]
        return activation
    def held(receiver):
        before = len(receiver.events()); mouse('button 272 1'); receiver.event('button', [272, 1], before)
    def local_cycle():
        before = local_log.read_text().count('button 272 1'); releases = local_log.read_text().count('button 272 0')
        mouse('button 272 0'); center(); mouse('button 272 1'); mouse('button 272 0')
        wait(lambda: local_log.read_text().count('button 272 1') == before + 1 and local_log.read_text().count('button 272 0') == releases + 1,
             'clean local button cycle')

    if args.keyboard:
        keyboard_log = session.output / 'keyboard.log'
        with keyboard_log.open('w') as log:
            keyboard = subprocess.Popen([str(session.output / 'keyboard')], stdin=subprocess.PIPE,
                stdout=log, stderr=log, env=session.env, start_new_session=True)
        session.processes.append(keyboard)
        wait(lambda: 'ready' in keyboard_log.read_text(), 'persistent raw keyboard')
        keyboard_serial = 0
        def raw(command):
            nonlocal keyboard_serial
            keyboard_serial += 1
            keyboard.stdin.write((command + '\n').encode()); keyboard.stdin.flush()
            wait(lambda: f'ack {keyboard_serial}\n' in keyboard_log.read_text(), 'raw keyboard roundtrip')
        def focus_local():
            action('focus-window', '--id', local_window['id']); center()
        def marker(offset):
            raw('key 194 1'); raw('key 194 0')
            wait(lambda: 'key 194 0 F24' in local_log.read_text()[offset:], 'ordinary input marker')
        def snapshot_map():
            revision, text = call(observer, 'org.canopa.Niri.InputCapture1',
                '/org/canopa/Niri/InputCapture1', 'org.canopa.Niri.InputCapture1', 'KeyboardSnapshot')
            assert revision != 0
            body = text.encode() + b'\0'; value = 14695981039346656037
            for byte in body: value = ((value ^ byte) * 1099511628211) & ((1 << 64) - 1)
            return {'size': len(body), 'fnv': f'{value:016x}'}

        caller = client(); key = create(caller)
        answer = approve(start(caller, key, 3), capabilities=1)
        assert answer['capabilities'] == 1
        controls = popup(); session.capture('keyboard-controls-' + args.scheme + '.png')
        receiver = Receiver(caller, key, 'keyboard-only', 1)
        assert receiver.event('keymap')['keymap'] == snapshot_map()
        barriers(caller, key); focus_local(); enable(caller, key); activate(receiver)
        before = len(receiver.events()); offset = len(local_log.read_text())
        raw('key 42 1'); receiver.event('key', [42, 1], before)
        receiver.event('modifiers', [1, 0, 0, 0], before); raw('sync')
        raw('key 30 1'); receiver.event('key', [30, 1], before)
        raw('key 30 0'); receiver.event('key', [30, 0], before)
        events = receiver.events()[before:]
        press = next(i for i, event in enumerate(events) if event.get('key') == [42, 1])
        assert 'frame' in events[press + 1] and events[press + 2].get('modifiers') == [1, 0, 0, 0], events
        # This keyboard-only grant leaves ordinary pointer input available.
        mouse_before = local_log.read_text().count('button 272 1')
        mouse('button 272 1'); mouse('button 272 0')
        wait(lambda: local_log.read_text().count('button 272 1') == mouse_before + 1, 'local pointer during keyboard capture')
        public(caller, 'Disable', '(oa{sv})', (key, {})); zones(caller, key)
        receiver.event('key', [42, 0], before); receiver.event('modifiers', [0, 0, 0, 0], before); receiver.event('stop', since=before)
        raw('sync')
        enable(caller, key); activate(receiver)
        resumed = len(receiver.events()); raw('key 30 1'); receiver.event('key', [30, 1], resumed)
        raw('key 30 0'); receiver.event('key', [30, 0], resumed)
        assert not any(event.get('modifiers', [0])[0] & 1 for event in receiver.events()[resumed:])
        public(caller, 'Disable', '(oa{sv})', (key, {})); zones(caller, key)
        raw('key 42 0'); raw('sync'); marker(offset)
        ordinary = local_log.read_text()[offset:]
        assert 'key 42 ' not in ordinary and 'key 30 ' not in ordinary, ordinary
        assert all(line.split()[1] == '0' for line in ordinary.splitlines() if line.startswith('mods ')), ordinary
        passed('public-keyboard-only-approval-keymap-ordered-key-modifier-frames-and-stop-suppressed-tails')

        enable(caller, key); activate(receiver); before = len(receiver.events())
        previous = receiver.event('keymap')['keymap']; raw('map fr')
        receiver.event('stop', since=before); receiver.event('removed', since=before)
        receiver.event('added', since=before); receiver.event('resumed', since=before)
        replacement = receiver.event('keymap', since=before)['keymap']
        assert replacement != previous and replacement == snapshot_map()
        enable(caller, key); activate(receiver); before = len(receiver.events())
        raw('key 30 1'); receiver.event('key', [30, 1], before)
        raw('key 30 0'); receiver.event('key', [30, 0], before)
        call(caller, FRONT, key, 'org.freedesktop.portal.Session', 'Close')
        receiver.finish(); gone(controls)
        passed('public-keymap-change-stops-capture-replaces-immutable-ei-device-and-reenables-current-map')

        # End the preceding layout-change case before a fresh US-layout consent source.
        raw('map us')
        caller = client(); key = create(caller)
        approve(start(caller, key, 3), capabilities=3); controls = popup()
        receiver = Receiver(caller, key, 'combined', 3)
        barriers(caller, key); focus_local(); enable(caller, key); activate(receiver)
        local_before = len(local_log.read_text())
        before = len(receiver.events()); mouse('relative 3.25 -1.5'); receiver.event('motion', [3.25, -1.5], before)
        raw('key 29 1'); receiver.event('key', [29, 1], before); receiver.event('modifiers', [4, 0, 0, 0], before)
        passed('independent-iced-approval-of-both-classes-delivers-pointer-and-keyboard-on-one-ei-device')
        keyboard.stdin.write(b'quit\n'); keyboard.stdin.flush(); retire(keyboard)
        receiver.event('key', [29, 0], before); receiver.event('modifiers', [0, 0, 0, 0], before); receiver.event('stop', since=before)
        wait(lambda: any(v[0] == 'Disabled' and v[1] == key for v in phases), 'public source-loss pause')
        focus_local(); keys('-k', 'F24')
        wait(lambda: any(line.startswith('key ') and line.endswith(' 0 F24') for line in local_log.read_text()[local_before:].splitlines()), 'ordinary marker after source withdrawal')
        assert 'key 29 ' not in local_log.read_text()[local_before:], local_log.read_text()[local_before:]
        call(caller, FRONT, key, 'org.freedesktop.portal.Session', 'Close'); receiver.finish(); gone(controls)
        passed('keyboard-source-withdrawal-lifts-held-ei-keys-and-modifiers-before-public-disable')
        for caller in clients:
            if not caller.is_closed(): caller.close_sync(None)
        observer.close_sync(None)
        return

    text_type = 'text/plain;charset=utf-8'
    clipboard_counter = 0
    def clipboard(caller, method, signature, values):
        return call(caller, FRONT, PATH, CLIP, method, signature, values)
    def request_clipboard(caller, key):
        clipboard(caller, 'RequestClipboard', '(oa{sv})', (key, {}))
    def clipboard_fd(caller, key, method, value):
        signature = '(os)' if method == 'SelectionRead' else '(ou)'
        reply, fds = caller.call_with_unix_fd_list_sync(FRONT, PATH, CLIP, method, GLib.Variant(signature, (key, value)),
            GLib.VariantType.new('(h)'), Gio.DBusCallFlags.NONE, 12000, None, None)
        owned = fds.steal_fds(); assert reply.unpack() == (0,) and len(owned) == 1
        assert fcntl.fcntl(owned[0], fcntl.F_GETFD) & fcntl.FD_CLOEXEC
        os.set_blocking(owned[0], False)
        return owned[0]
    def read_clipboard(fd, limit=3 * 1024 * 1024):
        data = bytearray(); deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            drain()
            try: chunk = os.read(fd, 65536)
            except BlockingIOError: time.sleep(.01); continue
            if not chunk: return bytes(data)
            data.extend(chunk); assert len(data) <= limit
        raise AssertionError('Clipboard reader did not reach EOF')
    def clipboard_workers():
        try:
            return sum(path.read_text().strip().startswith('portal-clipboar')
                       for path in Path(f'/proc/{backend.pid}/task').glob('*/comm'))
        except FileNotFoundError: return 0
    def publish(caller, key):
        offers.pop(key, None)
        clipboard(caller, 'SetSelection', '(oa{sv})', (key, {'mime_types': GLib.Variant('as', [text_type])}))
        wait(lambda: offers.get(key, {}).get('session_is_owner'), 'session-owned clipboard offer')
    def hold_clipboard(caller, key, name):
        nonlocal clipboard_counter
        clipboard_counter += 1
        publish(caller, key); before = len(transfers)
        reader = clipboard_fd(caller, key, 'SelectionRead', text_type)
        own = wait(lambda: next((v for v in transfers[before:] if v[0] == key), None), 'own-selection transfer')
        writer = clipboard_fd(caller, key, 'SelectionWrite', own[2])
        payload = b'active-prefix-' + b'A' * (2 * 1024 * 1024)
        written = os.write(writer, payload); assert 0 < written < len(payload)
        before = len(transfers)
        log = session.output / f'{name}-{clipboard_counter}-paste.log'
        with log.open('w') as errors:
            paste = subprocess.Popen(['wl-paste', '--no-newline', '--type', text_type], stdout=subprocess.PIPE, stderr=errors,
                                     env=session.env, start_new_session=True)
        session.processes.append(paste)
        local_transfer = wait(lambda: next((v for v in transfers[before:] if v[0] == key), None), 'local-consumer transfer')
        writable = clipboard_fd(caller, key, 'SelectionWrite', local_transfer[2])
        return {'reader': reader, 'writers': [writer, writable], 'prefix': payload[:written], 'paste': paste,
                'serials': [own[2], local_transfer[2]]}
    def expect_clipboard_retirement(pipes):
        expected_exits.add(pipes['paste'])
    def revoked_clipboard(pipes):
        for fd in pipes['writers']:
            try: os.write(fd, b'after-revocation')
            except BrokenPipeError: pass
            else: raise AssertionError('Retained clipboard writer accepted data after revocation')
            os.close(fd)
        data = read_clipboard(pipes['reader'], 65536); assert pipes['prefix'].startswith(data)
        os.close(pipes['reader'])
        if pipes['paste'] is not None:
            retire(pipes['paste'], (0, 1)); pipes['paste'].stdout.close()

    current = barriers(caller, key); receiver = Receiver(caller, key, 'legacy')
    rejected(lambda: public(caller, 'ConnectToEIS', '(oa{sv})', (key, {})))
    foreign = client(); rejected(lambda: public(foreign, 'ConnectToEIS', '(oa{sv})', (key, {})))
    enable(caller, key); activation = activate(receiver); before = len(receiver.events())
    mouse('relative 3.25 -1.5'); receiver.event('motion', [3.25, -1.5], before); held(receiver)
    public(caller, 'Release', '(oa{sv})', (key, {'activation_id': GLib.Variant('u', activation)}))
    receiver.event('button', [272, 0], before); receiver.event('stop', since=before); mouse('button 272 0')
    newer = activate(receiver); assert newer != activation
    public(caller, 'Disable', '(oa{sv})', (key, {})); zones(caller, key)
    passed('public-zones-barriers-one-ei-fd-and-real-pointer-activation-release-and-reuse')

    enable(caller, key); activate(receiver); before = len(receiver.events()); phase_before = len(phases)
    held(receiver); previous = zones(caller, key)
    session.run(['niri', 'msg', 'output', output, 'scale', '1.5'])
    receiver.event('button', [272, 0], before); receiver.event('stop', since=before)
    changed = wait(lambda: next((v for v in phases[phase_before:] if v[0] == 'ZonesChanged' and v[1] == key), None), 'public zone invalidation')
    assert changed[2]['zone_set'] == previous['zone_set']
    assert [v[0] for v in phases[phase_before:] if v[1] == key] == ['Deactivated', 'Disabled', 'ZonesChanged']
    current = zones(caller, key); assert current['zone_set'] != previous['zone_set'] and current['zones'] != previous['zones']
    mouse('button 272 0'); barriers(caller, key); enable(caller, key); activate(receiver)
    public(caller, 'Disable', '(oa{sv})', (key, {})); zones(caller, key)
    passed('public-topology-change-cleans-held-input-orders-phase-signals-and-requires-fresh-zones')

    enable(caller, key); activate(receiver); before = len(receiver.events()); held(receiver)
    controls = popup(); keys('-k', 'Escape'); receiver.event('button', [272, 0], before); receiver.event('stop', since=before)
    receiver.finish(); wait(lambda: key in closed, 'public Closed after Iced stop'); gone(controls); local_cycle()
    passed('iced-stop-revokes-held-capture-before-clean-local-input-and-public-session-retirement')

    if args.clipboard:
        # Requesting Clipboard must not select its independent consent choice.
        caller = client(); key = create(caller); request_clipboard(caller, key)
        pending = start(caller, key); dialog = popup(True)
        session.capture('clipboard-unchecked-' + args.scheme + '.png')
        keys('-M', 'alt', '-k', 'p', '-m', 'alt'); keys('-M', 'alt', '-k', 'a', '-m', 'alt')
        answer = response(pending); assert not answer['clipboard_enabled']; gone(dialog); controls = popup()
        receiver = Receiver(caller, key, 'clipboard-denied'); barriers(caller, key); enable(caller, key); activate(receiver)
        rejected(lambda: clipboard_fd(caller, key, 'SelectionRead', text_type))
        rejected(lambda: clipboard(caller, 'SetSelection', '(oa{sv})', (key, {'mime_types': GLib.Variant('as', [text_type])})))
        assert clipboard_workers() == 0
        call(caller, FRONT, key, 'org.freedesktop.portal.Session', 'Close'); receiver.finish(); gone(controls)
        passed('clipboard-request-does-not-grant-unchecked-consent-even-during-real-capture')

        caller = client(); key = create(caller); request_clipboard(caller, key); approve(start(caller, key), True); controls = popup(True)
        session.capture('clipboard-controls-' + args.scheme + '.png')
        receiver = Receiver(caller, key, 'clipboard-phases'); barriers(caller, key)
        rejected(lambda: clipboard_fd(caller, key, 'SelectionRead', text_type))
        rejected(lambda: clipboard(caller, 'SetSelection', '(oa{sv})', (key, {'mime_types': GLib.Variant('as', [text_type])})))
        rejected(lambda: clipboard_fd(caller, key, 'SelectionWrite', 1))
        assert clipboard_workers() == 0
        def copied(name, data):
            with (session.output / (name + '.log')).open('w') as log:
                process = subprocess.Popen(['wl-copy', '--foreground', '--type', text_type], stdin=subprocess.PIPE,
                    stdout=log, stderr=log, env=session.env, start_new_session=True)
            session.processes.append(process); process.stdin.write(data); process.stdin.close()
            def ready():
                try: return session.run(['wl-paste', '--no-newline', '--type', text_type]).encode() == data
                except subprocess.CalledProcessError: return False
            wait(ready, 'ordinary local clipboard source')
            return process
        local_copy = copied('inactive-copy', b'local clipboard while inactive')
        drain(); assert key not in offers
        enable(caller, key); activation = activate(receiver)
        wait(lambda: text_type in offers.get(key, {}).get('mime_types', []), 'initial active clipboard owner')
        reader = clipboard_fd(caller, key, 'SelectionRead', text_type)
        assert read_clipboard(reader) == b'local clipboard while inactive'; os.close(reader)
        phase_cases = ('repeated-enable', 'stale-release') if args.phase_case == 'all' else (args.phase_case,)
        for phase_case in phase_cases:
            before = len(receiver.events()); phase_before = len(phases)
            reads_before = trace.read_text().count('member=SelectionRead')
            if phase_case == 'repeated-enable':
                enable(caller, key)
            else:
                public(caller, 'Release', '(oa{sv})', (key, {'activation_id': GLib.Variant('u', activation + 1)}))
                zones(caller, key)
            mouse('relative 1 0'); receiver.event('motion', [1, 0], before)
            assert not any(v[1] == key for v in phases[phase_before:]), 'no-op/rejected control changed capture phase'
            observed = dict(case=phase_case, actual_ei_motion=True,
                            clipboard_workers=clipboard_workers(), phase_events=[],
                            backend_reads_before=reads_before)
            result.setdefault('phase_observations', []).append(observed)
            assert observed['clipboard_workers'] == 1, observed
            # Let the method-call observer consume preceding successful reads.
            time.sleep(.1)
            observed['backend_reads_before'] = trace.read_text().count('member=SelectionRead')
            reader = clipboard_fd(caller, key, 'SelectionRead', text_type)
            assert read_clipboard(reader) == b'local clipboard while inactive'; os.close(reader)
            wait(lambda: trace.read_text().count('member=SelectionRead') > observed['backend_reads_before'], 'forwarded phase-regression Clipboard read')
            observed['backend_reads_after'] = trace.read_text().count('member=SelectionRead')
            passed(phase_case + '-preserves-actual-capture-and-public-clipboard-readability')
        public(caller, 'Release', '(oa{sv})', (key, {'activation_id': GLib.Variant('u', activation)})); zones(caller, key)
        wait(lambda: clipboard_workers() == 0, 'inactive clipboard worker retirement')
        offers.pop(key, None); expected_exits.add(local_copy)
        newer_copy = copied('new-inactive-copy', b'new inactive clipboard content'); retire(local_copy)
        deadline = time.monotonic() + .25
        while time.monotonic() < deadline: drain(); time.sleep(.01)
        assert key not in offers, 'owner-change metadata leaked while inactive'
        rejected(lambda: clipboard_fd(caller, key, 'SelectionRead', text_type))
        passed('clipboard-bytes-and-owner-events-are-denied-outside-actual-capture-activation')

        activate(receiver); wait(lambda: text_type in offers.get(key, {}).get('mime_types', []), 'fresh activation owner')
        reader = clipboard_fd(caller, key, 'SelectionRead', text_type)
        assert read_clipboard(reader) == b'new inactive clipboard content'; os.close(reader)
        expected_exits.add(newer_copy)
        pipes = hold_clipboard(caller, key, 'epoch-one'); retire(newer_copy)
        old_serial = pipes['serials'][-1]; expect_clipboard_retirement(pipes)
        public(caller, 'Release', '(oa{sv})', (key, {})); zones(caller, key); revoked_clipboard(pipes)
        wait(lambda: clipboard_workers() == 0, 'deactivated byte transport retired')
        passed('deactivation-cancels-held-readable-and-writable-pipes-with-bounded-admitted-data')

        activate(receiver); pipes = hold_clipboard(caller, key, 'epoch-two')
        assert min(pipes['serials']) > old_serial, (old_serial, pipes['serials'])
        rejected(lambda: clipboard_fd(caller, key, 'SelectionWrite', old_serial))
        clipboard(caller, 'SelectionWriteDone', '(oub)', (key, old_serial, False))
        writer = pipes['writers'].pop(); expected_exits.add(pipes['paste'])
        assert os.write(writer, b'fresh-current-activation') == len(b'fresh-current-activation'); os.close(writer)
        clipboard(caller, 'SelectionWriteDone', '(oub)', (key, pipes['serials'][-1], True))
        retire(pipes['paste']); assert pipes['paste'].stdout.read() == b'fresh-current-activation'
        pipes['paste'].stdout.close(); pipes['paste'] = None
        passed('late-completion-and-old-serial-cannot-affect-a-fresh-activation-transfer')

        before = len(receiver.events()); held(receiver)
        public(caller, 'Disable', '(oa{sv})', (key, {})); zones(caller, key)
        receiver.event('button', [272, 0], before); receiver.event('stop', since=before)
        revoked_clipboard(pipes); mouse('button 272 0')
        wait(lambda: clipboard_workers() == 0, 'disabled Clipboard transport retired')
        rejected(lambda: clipboard_fd(caller, key, 'SelectionWrite', pipes['serials'][0]))
        passed('explicit-disable-revokes-clipboard-and-held-input-without-ending-the-session')

        enable(caller, key); activate(receiver); pipes = hold_clipboard(caller, key, 'topology')
        before = len(receiver.events()); phase_before = len(phases); held(receiver)
        previous = zones(caller, key); expect_clipboard_retirement(pipes)
        session.run(['niri', 'msg', 'output', output, 'scale', '1.75'])
        receiver.event('button', [272, 0], before); receiver.event('stop', since=before)
        wait(lambda: any(v[0] == 'ZonesChanged' and v[1] == key for v in phases[phase_before:]), 'Clipboard topology invalidation')
        assert [v[0] for v in phases[phase_before:] if v[1] == key] == ['Deactivated', 'Disabled', 'ZonesChanged']
        current = zones(caller, key); assert current['zone_set'] != previous['zone_set']
        wait(lambda: clipboard_workers() == 0, 'topology-invalidated Clipboard transport retired')
        revoked_clipboard(pipes); mouse('button 272 0')
        rejected(lambda: clipboard_fd(caller, key, 'SelectionWrite', pipes['serials'][-1]))
        passed('live-topology-invalidation-revokes-clipboard-pipes-and-requires-fresh-capture-zones')

        barriers(caller, key); enable(caller, key); activate(receiver)
        pipes = hold_clipboard(caller, key, 'iced-stop'); expect_clipboard_retirement(pipes)
        before = len(receiver.events()); held(receiver); controls = popup(True); keys('-k', 'Escape')
        receiver.event('button', [272, 0], before); receiver.event('stop', since=before); receiver.finish()
        wait(lambda: key in closed, 'shared clipboard session Closed'); revoked_clipboard(pipes); gone(controls); local_cycle()
        assert clipboard_workers() == 0
        passed('iced-stop-retires-clipboard-pipes-and-capture-as-one-owned-session')

    caller = client(); key = create(caller)
    if args.clipboard: request_clipboard(caller, key)
    approve(start(caller, key), args.clipboard); controls = popup(args.clipboard)
    receiver = Receiver(caller, key, 'caller-loss'); barriers(caller, key); enable(caller, key); activate(receiver)
    before = len(receiver.events()); held(receiver)
    pipes = hold_clipboard(caller, key, 'caller-loss') if args.clipboard else None
    if pipes: expect_clipboard_retirement(pipes)
    caller.close_sync(None)
    receiver.event('button', [272, 0], before); receiver.event('stop', since=before); receiver.finish(); gone(controls); local_cycle()
    if pipes: revoked_clipboard(pipes)
    passed('original-public-caller-loss-retires-its-backend-session-controls-and-held-receiver')

    caller = client(); key = create(caller)
    if args.clipboard: request_clipboard(caller, key)
    approve(start(caller, key), args.clipboard); controls = popup(args.clipboard)
    receiver = Receiver(caller, key, 'backend-loss'); barriers(caller, key); enable(caller, key); activate(receiver)
    before = len(receiver.events()); held(receiver)
    pipes = hold_clipboard(caller, key, 'backend-loss') if args.clipboard else None
    if pipes: expect_clipboard_retirement(pipes)
    backend.kill(); retire(backend, -signal.SIGKILL)
    receiver.event('button', [272, 0], before); receiver.event('stop', since=before); receiver.finish()
    wait(lambda: key in closed, 'public Closed after original backend death'); gone(controls); local_cycle()
    if pipes: revoked_clipboard(pipes)
    passed('original-backend-death-cleans-held-ei-input-and-standard-frontend-session')
    for caller in clients:
        if not caller.is_closed(): caller.close_sync(None)
    observer.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'niri', 'frontend'):
        parser.add_argument('--' + name, type=Path, required=True); parser.add_argument('--' + name + '-sha256', required=True)
    parser.add_argument('--scheme', choices=['dark', 'light'], default='dark'); parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--portal-descriptor', type=Path, help='Use an exact optional package descriptor')
    parser.add_argument('--keyboard', action='store_true', help='exercise public keyboard-only/combined EI and Iced support instead of the pointer/Clipboard matrix')
    parser.add_argument('--clipboard', action='store_true', help='also exercise active-only InputCapture Clipboard consent and byte transfers')
    parser.add_argument('--phase-case', choices=['all', 'repeated-enable', 'stale-release'], default='all',
                        help='select a Clipboard phase regression for baseline diagnosis; default exercises both')
    args = parser.parse_args()
    if args.keyboard and args.clipboard: parser.error('--keyboard and --clipboard select separate fixtures')
    if args.phase_case != 'all' and not args.clipboard: parser.error('--phase-case requires --clipboard')
    for name in ('binary', 'niri', 'frontend'):
        path = getattr(args, name).resolve(strict=True); setattr(args, name, path); assert digest(path) == getattr(args, name + '_sha256')
    args.output = args.output.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    frozen = freeze_niri(args.niri, args.niri_sha256)
    paths = [Path(__file__), args.binary, args.frontend, frozen, ROOT / 'scripts/screencast_media.py', ROOT / 'scripts/validate-desktop.py',
             *[ROOT / 'scripts/desktop-fixture' / name for name in ('input-capture-ei-client.c', 'input-capture-pointer.c', 'remote-input-pointer-receiver.c')]]
    if args.keyboard: paths.extend(ROOT / 'scripts/desktop-fixture' / name for name in ('input-capture-keyboard.c', 'virtual-keyboard-unstable-v1.xml'))
    if args.portal_descriptor:
        args.portal_descriptor = args.portal_descriptor.resolve(strict=True); paths.append(args.portal_descriptor)
    pins = {str(path): digest(path) for path in paths}; started = time.monotonic()
    result = {'status': 'running', 'scope': 'Private routing and canonical frontend; real Rust/Iced ' + ('keyboard InputCapture' if args.keyboard else 'pointer InputCapture') + (' and active-only Clipboard' if args.clipboard else '') + '. No physical keyboard/touch, installed routing or distribution acceptance.',
              'checks': [], 'pins': pins, 'scheme': args.scheme}
    with tempfile.TemporaryDirectory(prefix='canopa-input-capture-portal-') as directory:
        session = MediaSession(Path(directory), args.output, frozen, args.niri_sha256, reference_frozen=True); session.capture_workers = []
        try:
            verify(args, session, result); result['status'] = 'passed'
        except Exception as error:
            result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
            try: session.capture('failure.png')
            except Exception: pass
        finally:
            processes = list(session.processes); session.close()
            result['processes'] = [{'pid': p.pid, 'command': p.args, 'exit_code': p.poll()} for p in processes]
            result['workers'] = []
            for worker in session.capture_workers:
                terminal = bool(select.select([worker['pidfd']], [], [], 5)[0]); os.close(worker['pidfd'])
                result['workers'].append({'pid': worker['pid'], 'terminal': terminal})
    result.update(seconds=round(time.monotonic() - started, 2), changed_inputs=[name for name,h in pins.items() if digest(Path(name)) != h])
    if result['changed_inputs'] or any(p['exit_code'] is None for p in result['processes']) or not all(w['terminal'] for w in result['workers']): result['status'] = 'failed'
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: result.get(k) for k in ('status', 'error', 'seconds')}))
    raise SystemExit(0 if result['status'] == 'passed' else 1)


if __name__ == '__main__': main()
