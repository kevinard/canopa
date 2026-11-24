#!/usr/bin/env python3
"""Private public-frontend/Rust/Iced shortcut acceptance on a pinned compositor."""
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

FRONT = 'org.freedesktop.portal.Desktop'
BACK = 'org.freedesktop.impl.portal.desktop.canopa'
PATH = '/org/freedesktop/portal/desktop'
PUBLIC = 'org.freedesktop.portal.GlobalShortcuts'
IMPL = 'org.freedesktop.impl.portal.GlobalShortcuts'
APP = 'org.canopa.ShortcutsFixture'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def verify(args, session, result):
    from gi.repository import Gio, GLib
    Channel = load('shortcut_lease', ROOT / 'scripts/test-global-shortcuts-lease.py').Channel
    config = session.runtime / 'canopa.kdl'
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.scheme}"; }}\n')
    portals = session.runtime / 'portals'
    portals.mkdir()
    descriptor = (args.portal_descriptor or ROOT / 'packaging/portal/canopa.portal').read_text()
    if not args.portal_descriptor:
        assert IMPL + ';' not in descriptor, 'Update the fixture when normal advertisement changes'
        descriptor = descriptor.replace('Interfaces=', 'Interfaces=' + IMPL + ';')
    assert IMPL + ';' in descriptor
    (portals / 'canopa.portal').write_text(descriptor)
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\n' + IMPL + '=canopa\n')
    data = Path(session.env['XDG_DATA_HOME'])
    (data / 'applications').mkdir(parents=True)
    (data / 'applications' / (APP + '.desktop')).write_text('[Desktop Entry]\nType=Application\nName=Shortcut test application\nExec=true\n')
    home = session.runtime / 'home'
    home.mkdir()
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home), XDG_STATE_HOME=str(session.runtime / 'state'),
                       XDG_CURRENT_DESKTOP='Canopa', XDG_DESKTOP_PORTAL_DIR=str(portals),
                       XDG_DATA_DIRS=str(data), GIO_USE_VFS='local', GSETTINGS_BACKEND='memory',
                       GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas')
    niri_config = '''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "us"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
binds { F10 { spawn "true"; }; }
'''
    keyboard_config = 'layout "' + args.layout + '";'
    if args.xkb_options:
        keyboard_config += ' options "' + args.xkb_options + '";'
    session.start_niri(niri_config.replace('layout "us";', keyboard_config), width=round(1280 * args.scale), height=round(800 * args.scale), scale=args.scale)
    clients, streams = [], []

    def connect(register=False):
        client = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        clients.append(client)
        if register:
            call(client, FRONT, PATH, 'org.freedesktop.host.portal.Registry', 'Register', '(sa{sv})', (APP, {}))
        return client

    def call(client, destination, path, interface, method, signature='()', values=()):
        return client.call_sync(destination, path, interface, method, GLib.Variant(signature, values),
            None, Gio.DBusCallFlags.NO_AUTO_START, 10000, None).unpack()

    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)

    def wait(condition, label):
        return session.wait_for(lambda: (drain(), condition())[1], label, timeout=20)

    def passed(name):
        result['checks'].append(name)
        (session.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
        print(name, flush=True)

    def denied(operation):
        try:
            operation()
        except GLib.Error as error:
            assert Gio.DBusError.get_remote_error(error) not in (None, 'org.freedesktop.DBus.Error.NoReply', 'org.freedesktop.DBus.Error.Timeout')
            return
        raise AssertionError('Foreign operation unexpectedly succeeded')

    observer = connect()
    backend = session.start([str(args.binary), '--canopa-config', str(config)], 'backend.log')

    def owner(name):
        return call(observer, 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', '(s)', (name,))[0]

    wait(lambda: owner(BACK), 'Rust shortcut backend')
    assert call(observer, 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', '(s)', (BACK,))[0] == backend.pid
    session.start([str(args.frontend), '--verbose'], 'frontend.log')
    wait(lambda: owner(FRONT), 'canonical frontend')
    caller, outsider = connect(True), connect(True)
    responses, events, foreign = {}, [], []
    for client in (caller, outsider):
        client.signal_subscribe(FRONT, 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE,
            lambda c, s, p, i, n, v: responses.__setitem__(p, v.unpack()))
    caller.signal_subscribe(FRONT, PUBLIC, None, PATH, None, Gio.DBusSignalFlags.NONE,
        lambda c, s, p, i, n, v: events.append((n, v.unpack())))
    outsider.signal_subscribe(FRONT, PUBLIC, None, PATH, None, Gio.DBusSignalFlags.NONE,
        lambda c, s, p, i, n, v: foreign.append((n, v.unpack())))
    sequence = 0
    request_sequence = 0

    def options(values=None):
        nonlocal request_sequence
        request_sequence += 1
        return dict(values or {}, handle_token=GLib.Variant('s', 'request' + str(request_sequence)))

    def request(method, signature, values, status=0, client=caller):
        values = list(values)
        values[-1] = options(values[-1])
        path = call(client, FRONT, PATH, PUBLIC, method, signature, values)[0]
        value = wait(lambda: responses.get(path), method + ' response')
        assert value[0] == status, value
        return value[1]

    def create(client=caller):
        nonlocal sequence
        sequence += 1
        return request('CreateSession', '(a{sv})', ({'session_handle_token': GLib.Variant('s', 's' + str(sequence))},), client=client)['session_handle']

    def listed(key, client=caller):
        return request('ListShortcuts', '(oa{sv})', (key, {}), client=client)['shortcuts']

    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))

    def action(*values):
        session.run(['niri', 'msg', 'action', *map(str, values)])

    def keys(*values):
        session.run(['wtype', *values])
        time.sleep(.15)
        drain()

    protocols = Path('/usr/share/wayland-protocols')
    for name, xml_path in [('xdg-shell', protocols / 'stable/xdg-shell/xdg-shell.xml'), ('xdg-foreign', protocols / 'unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', mode, str(xml_path), str(session.output / (name + suffix))], env=session.env, check=True, timeout=15)
    parent = session.output / 'portal-parent'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output), str(ROOT / 'scripts/desktop-fixture/portal-parent.c'), str(session.output / 'xdg-shell.c'), str(session.output / 'xdg-foreign.c'), '-lwayland-client', '-o', str(parent)], env=session.env, check=True, timeout=30)
    session.start([str(parent)], 'parent.log')
    exported = wait(lambda: next((line for line in (session.output / 'parent.log').read_text().splitlines() if line.startswith('wayland:')), None), 'exported Wayland parent')
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    pointer = session.build_pointer()
    logical = next(o['logical'] for o in json.loads(session.run(['niri', 'msg', '--json', 'outputs'])).values() if o.get('logical'))

    def click(x, y):
        session.run([str(pointer), str(round(x / logical['width'] * 1600)), str(round(y / logical['height'] * 1000))])
        time.sleep(.15)
        drain()

    def confirm(window):
        width, height = window['layout']['window_size']
        click(10 + width - 68, 10 + height - 42)

    def dialog(before):
        window = wait(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.Portal' and w['id'] not in before), None), 'Iced shortcut dialog')
        command = Path(f"/proc/{window['pid']}/cmdline").read_bytes().split(b'\0')
        assert b'--global-shortcuts-dialog' in command
        assert Path(f"/proc/{window['pid']}/exe").resolve() == args.binary
        window['pidfd'] = os.pidfd_open(window['pid'])
        session.shortcut_helpers.append(window)
        def sockets(pid):
            values = set()
            for fd in Path(f'/proc/{pid}/fd').iterdir():
                try:
                    target = os.readlink(fd)
                except FileNotFoundError:
                    continue
                if target.startswith('socket:['):
                    values.add(target)
            return values
        shared = sorted(sockets(backend.pid) & sockets(window['pid']))
        result.setdefault('helper_socket_isolation', []).append(dict(pid=window['pid'], shared_backend_sockets=shared))
        assert not shared, 'An Iced helper inherited backend sockets'
        assert window['is_floating'] and abs(window['layout']['window_size'][0] - 680) <= 1, window
        action('move-floating-window', '--id', window['id'], '--x', 20, '--y', 10)
        action('focus-window', '--id', window['id'])
        time.sleep(.4)
        return window

    def begin(key, client=caller):
        before = {w['id'] for w in windows()}
        path = call(client, FRONT, PATH, PUBLIC, 'BindShortcuts', '(oa(sa{sv})sa{sv})',
            (key, [('record', {'description': GLib.Variant('s', 'Record a moment'), 'preferred_trigger': GLib.Variant('s', 'F9')})], exported, options()))[0]
        return path, dialog(before)

    def configure(key):
        before = {w['id'] for w in windows()}
        call(caller, FRONT, PATH, PUBLIC, 'ConfigureShortcuts', '(osa{sv})', (key, exported, {}))
        return dialog(before)

    def gone(window):
        wait(lambda: not any(w['id'] == window['id'] for w in windows()), 'shortcut dialog closes')
        terminal = bool(select.select([window['pidfd']], [], [], 5)[0])
        os.close(window['pidfd'])
        window['pidfd'] = None
        result.setdefault('helpers', []).append(dict(pid=window['pid'], terminal=terminal))
        assert terminal

    def response(path, status=0):
        value = wait(lambda: responses.get(path), 'BindShortcuts response')
        assert value[0] == status, value
        return value[1]

    def open_keyboard():
        value, descriptors = observer.call_with_unix_fd_list_sync('org.canopa.Niri.RemoteInput1', '/org/canopa/Niri/RemoteInput1', 'org.canopa.Niri.RemoteInput1', 'OpenKeyboard', GLib.Variant('()', ()), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None, None)
        stream = socket.socket(fileno=descriptors.get(value.unpack()[0]))
        streams.append(stream)
        return stream

    def keyboard(code, down):
        remote.sendall(struct.pack('<II', code, int(down)))
        time.sleep(.05)
        drain()

    try:
        xml = call(observer, BACK, PATH, 'org.freedesktop.DBus.Introspectable', 'Introspect')[0]
        interface = next(i for i in ET.fromstring(xml).findall('interface') if i.get('name') == IMPL)
        methods = {m.get('name'): (''.join(a.get('type') for a in m.findall('arg') if a.get('direction') != 'out'), ''.join(a.get('type') for a in m.findall('arg') if a.get('direction') == 'out')) for m in interface.findall('method')}
        assert methods == {'CreateSession': ('oosa{sv}', 'ua{sv}'), 'BindShortcuts': ('ooa(sa{sv})sa{sv}', 'ua{sv}'), 'ListShortcuts': ('oo', 'ua{sv}'), 'ConfigureShortcuts': ('osa{sv}', '')}, methods
        assert call(observer, FRONT, PATH, 'org.freedesktop.DBus.Properties', 'Get', '(ss)', (PUBLIC, 'version'))[0] == 2
        passed('exact-v2-contract-through-canonical-public-frontend')
        empty = create()
        assert listed(empty) == []
        path, window = begin(empty)
        session.capture('01-unchecked-' + args.scheme + '.png')
        confirm(window)
        assert response(path)['shortcuts'] == []
        gone(window)
        assert listed(empty) == []
        call(caller, FRONT, empty, 'org.freedesktop.portal.Session', 'Close')
        passed('preferred-trigger-is-unchecked-and-empty-explicit-bind-returns-empty-live-set')

        key = create()
        path, window = begin(key)
        click(64, 153)
        session.capture('02-selected-' + args.scheme + '.png')
        confirm(window)
        rows = response(path)['shortcuts']
        assert rows == [('record', {'description': 'Record a moment', 'trigger_description': 'F9'})], rows
        gone(window)
        assert listed(key) == rows
        passed('real-iced-selection-commits-only-the-approved-application-action')

        remote = open_keyboard()
        keyboard(67, True)
        wait(lambda: any(n == 'Activated' and v[:2] == (key, 'record') for n, v in events), 'public F9 activation')
        keyboard(67, False)
        wait(lambda: any(n == 'Deactivated' and v[:2] == (key, 'record') for n, v in events), 'public F9 deactivation')
        pair = [(n, v) for n, v in events if n in ('Activated', 'Deactivated')]
        assert [n for n, v in pair] == ['Activated', 'Deactivated']
        assert all(isinstance(v[2], int) and v[2] >= 0 and v[3] == {} for n, v in pair)
        assert not foreign
        passed('real-compositor-press-release-reaches-only-original-public-session-with-source-time')

        window = configure(key)
        if args.check_editor_inhibition:
            concurrent = create(outsider)
            concurrent_path, concurrent_window = begin(concurrent, outsider)
            assert any(w['id'] == window['id'] for w in windows())
            keys('-k', 'Escape')
            assert response(concurrent_path, 1) == {}
            gone(concurrent_window)
            call(outsider, FRONT, concurrent, 'org.freedesktop.portal.Session', 'Close')
            action('focus-window', '--id', window['id'])
            time.sleep(.3)
            drain()
            passed('another-public-bind-can-open-and-cancel-while-an-editor-inhibits-shortcuts')
        editor_events = len(events)
        if args.check_editor_inhibition:
            keys('-k', 'F9')
            value, descriptors = observer.call_with_unix_fd_list_sync('org.canopa.Niri.GlobalShortcuts1', '/org/canopa/Niri/GlobalShortcuts1', 'org.canopa.Niri.GlobalShortcuts1', 'OpenShortcuts', GLib.Variant('()', ()), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None, None)
            probe = Channel(socket.socket(fileno=descriptors.get(value.unpack()[0])))
            try:
                assert probe.replace([(99, 'F9')])['error'] == 'conflict'
            finally:
                probe.stream.close()
        click(150, 187)
        keys('-M', 'ctrl', '-k', 'a', '-m', 'ctrl', 'F10')
        confirm(window)
        time.sleep(.5)
        assert any(w['id'] == window['id'] for w in windows())
        assert listed(key) == rows
        session.capture('03-conflict-' + args.scheme + '.png')
        click(150, 187)
        keys('-M', 'ctrl', '-k', 'a', '-m', 'ctrl', 'F11')
        confirm(window)
        gone(window)
        changed = listed(key)
        assert changed[0][1]['trigger_description'] == 'F11', changed
        passed('conflict-keeps-same-iced-editor-and-old-binding-until-successful-reconfiguration')
        if args.check_editor_inhibition:
            # The same lease emits input edges before the subsequent committed
            # replacement. Waiting for its directed metadata signal fences the
            # negative assertion, rather than depending on a short observation.
            wait(lambda: any(n == 'ShortcutsChanged' and v[0] == key and v[1] and v[1][0][1].get('trigger_description') == 'F11' for n, v in events[editor_events:]), 'ordered replacement after editor input')
            assert not any(n in ('Activated', 'Deactivated') for n, v in events[editor_events:]), 'A live shortcut intercepted keyboard input in its configuration editor'
            passed('focused-editor-suppresses-live-actions-without-freeing-their-reservations')

        if args.check_keyboard_recording:
            # The manual-entry checks used wtype, which installs new keymaps.
            # A raw lease intentionally belongs to its opening keymap. Start
            # a new source here and keep it for all recording input below.
            remote.close()
            remote = open_keyboard()

            def present(window):
                return any(w['id'] == window['id'] for w in windows())

            def approve(window):
                # Native recording completes before its result reaches Iced.
                # Under compilation load a single immediate click can hit the
                # still-disabled button. Exercise the real enabled action and
                # wait for its accepted close, rather than a fixed frame delay.
                def accepted():
                    if not present(window):
                        return True
                    confirm(window)
                    return not present(window)
                wait(accepted, 'recording approval accepted by the dialog')

            def save(window, expected):
                before = len(events)
                approve(window)
                gone(window)
                result = listed(key)
                assert result[0][1]['trigger_description'] == expected, result
                wait(lambda: any(n == 'ShortcutsChanged' and v[0] == key for n, v in events[before:]), 'recorded shortcut replacement')
                return result

            def record(window):
                click(590, 187)

            def chord(*codes):
                for code in codes:
                    keyboard(code, True)
                # Deliberately release the modifier before the trigger.
                for code in codes:
                    keyboard(code, False)

            def activated(code, modifier=None):
                before = len(events)
                if modifier is not None:
                    keyboard(modifier, True)
                keyboard(code, True)
                wait(lambda: any(n == 'Activated' and v[:2] == (key, 'record') for n, v in events[before:]), 'recorded shortcut activates')
                if modifier is not None:
                    keyboard(modifier, False)
                keyboard(code, False)
                wait(lambda: any(n == 'Deactivated' and v[:2] == (key, 'record') for n, v in events[before:]), 'recorded shortcut deactivates')
                assert [n for n, v in events[before:] if n in ('Activated', 'Deactivated')] == ['Activated', 'Deactivated']

            window = configure(key)
            record(window)
            session.capture('05-recording-armed-' + args.scheme + '.png')
            keyboard(29, True)
            keyboard(30, True)
            keyboard(29, False)
            confirm(window)
            assert present(window) and listed(key) == changed
            session.capture('05-recording-held-' + args.scheme + '.png')
            keyboard(30, False)
            changed = save(window, 'Ctrl + ' + ('Q' if args.layout == 'fr' else 'A'))
            activated(30, 29)
            passed('recorded-chord-keeps-press-modifiers-after-early-release-and-requires-separate-apply')

            window = configure(key)
            click(150, 187)
            chord(29, 16 if args.layout == 'fr' else 30)
            chord(42, 33)  # F
            if args.layout == 'fr':
                chord(42, 2)  # 1
                chord(42, 3)  # 2
            else:
                chord(2)  # 1
                chord(3)  # 2
            before = len(events)
            record(window)
            chord(1)
            assert present(window)
            changed = save(window, 'F12')
            assert not any(n in ('Activated', 'Deactivated') for n, v in events[before:])
            passed('escape-cancels-only-recording-and-preserves-the-manual-draft-and-consent')

            unchecked = create(outsider)
            path, window = begin(unchecked, outsider)
            record(window)
            chord(66)
            approve(window)
            assert response(path)['shortcuts'] == []
            gone(window)
            assert listed(unchecked, outsider) == []
            call(outsider, FRONT, unchecked, 'org.freedesktop.portal.Session', 'Close')
            passed('recording-a-preferred-key-does-not-check-the-action-or-grant-application-authority')

            window = configure(key)
            # Keep a non-modifier held before arming. Its repeats and release
            # cannot become the selected trigger or reach a text field.
            click(150, 50)
            keyboard(30, True)
            record(window)
            time.sleep(.7)
            keyboard(30, False)
            chord(66)
            changed = save(window, 'F8')
            activated(66)
            passed('recording-waits-for-preexisting-held-keys-and-discards-their-repeats')

            window = configure(key)
            record(window)
            keyboard(29, True)
            other = next(w for w in windows() if w['app_id'] != 'org.canopa.Portal')
            action('focus-window', '--id', other['id'])
            time.sleep(.2)
            keyboard(29, False)
            action('focus-window', '--id', window['id'])
            time.sleep(.3)
            chord(88)
            changed = save(window, 'F8')
            passed('native-focus-loss-cancels-recording-and-focus-regain-cannot-revive-the-attempt')

            window = configure(key)
            record(window)
            chord(68)
            confirm(window)
            assert present(window) and listed(key) == changed
            record(window)
            chord(87)
            changed = save(window, 'F11')
            activated(87)
            passed('recorded-reserved-key-conflicts-and-the-same-editor-can-record-a-successful-retry')

            window = configure(key)
            record(window)
            chord(29)
            changed = save(window, 'Left Ctrl')
            activated(29)
            passed('bare-modifier-recording-resolves-to-a-real-compositor-shortcut')

            letter = 'Q' if args.layout == 'fr' else 'A'
            window = configure(key)
            record(window)
            chord(42, 30)
            changed = save(window, 'Shift + ' + letter)
            activated(30, 42)
            passed('recorded-shifted-key-keeps-the-current-layout-base-symbol')

            window = configure(key)
            chord(69)
            record(window)
            chord(29, 30)
            changed = save(window, 'Ctrl + Num Lock + ' + letter)
            activated(30, 29)
            chord(69)
            passed('recording-preserves-ambient-num-lock-and-the-result-activates')

            window = configure(key)
            before = len(events)
            chord(69)
            record(window)
            chord(1)
            assert present(window)
            chord(69)
            changed = save(window, changed[0][1]['trigger_description'])
            assert not any(n in ('Activated', 'Deactivated') for n, v in events[before:])
            passed('escape-with-num-lock-cancels-recording-without-changing-the-draft')

            if args.layout == 'fr':
                window = configure(key)
                before = len(events)
                record(window)
                chord(100, 18)
                assert present(window)
                changed = save(window, changed[0][1]['trigger_description'])
                assert not any(n in ('Activated', 'Deactivated') for n, v in events[before:])
                passed('unsupported-altgr-recording-preserves-the-draft-and-consent')

            if args.xkb_options == 'caps:ctrl_modifier':
                window = configure(key)
                record(window)
                chord(58, 30)
                changed = save(window, 'Ctrl + ' + letter)
                activated(30, 58)
                passed('caps-remapped-to-control-records-a-working-chord')

                window = configure(key)
                record(window)
                chord(58)
                changed = save(window, 'Ctrl + Caps Lock')
                activated(58)
                passed('remapped-bare-caps-uses-actual-post-press-control-state')

            # Restore the original live binding for the existing cancellation,
            # persisted-history and registration-release checks below.
            window = configure(key)
            click(150, 187)
            keys('-M', 'ctrl', '-k', 'a', '-m', 'ctrl', 'F11')
            changed = save(window, 'F11')

        window = configure(key)
        click(150, 187)
        keys('-M', 'ctrl', '-k', 'a', '-m', 'ctrl', 'F12')
        keys('-k', 'Escape')
        gone(window)
        assert listed(key) == changed
        denied(lambda: call(outsider, FRONT, key, 'org.freedesktop.portal.Session', 'Close'))
        call(caller, FRONT, key, 'org.freedesktop.portal.Session', 'Close')
        passed('cancel-before-apply-preserves-current-bindings-and-foreign-close-is-denied')
        again = create()
        assert listed(again) == changed
        path, window = begin(again)
        session.capture('04-saved-unchecked-' + args.scheme + '.png')
        keys('-k', 'Escape')
        assert response(path, 1) == {}
        gone(window)
        assert listed(again) == []
        request('BindShortcuts', '(oa(sa{sv})sa{sv})', (again, [], '', {}), status=2)
        call(caller, FRONT, again, 'org.freedesktop.portal.Session', 'Close')
        passed('saved-successful-choices-are-history-only-and-canceled-bind-consumes-the-attempt')

        value, descriptors = observer.call_with_unix_fd_list_sync('org.canopa.Niri.GlobalShortcuts1', '/org/canopa/Niri/GlobalShortcuts1', 'org.canopa.Niri.GlobalShortcuts1', 'OpenShortcuts', GLib.Variant('()', ()), None, Gio.DBusCallFlags.NO_AUTO_START, 5000, None, None)
        replacement = Channel(socket.socket(fileno=descriptors.get(value.unpack()[0])))
        streams.append(replacement.stream)
        assert replacement.replace([(99, 'F11')])['error'] is None
        passed('closed-public-session-releases-its-compositor-registration')
        assert len(result['helper_socket_isolation']) == len(session.shortcut_helpers)
        passed('iced-helpers-never-inherit-backend-or-compositor-sockets')
    finally:
        for stream in streams:
            stream.close()
        for client in clients:
            client.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'niri', 'frontend'):
        parser.add_argument('--' + name, required=True, type=Path)
        parser.add_argument('--' + name + '-sha256', required=True)
    parser.add_argument('--scheme', choices=('dark', 'light'), default='dark')
    parser.add_argument('--scale', type=float, default=1.0)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--portal-descriptor', type=Path, help='Use an exact optional package descriptor')
    parser.add_argument('--check-editor-inhibition', action='store_true')
    parser.add_argument('--check-keyboard-recording', action='store_true')
    parser.add_argument('--layout', choices=('us', 'fr'), default='us')
    parser.add_argument('--xkb-options', choices=('', 'caps:ctrl_modifier'), default='')
    args = parser.parse_args()
    args.check_editor_inhibition |= args.check_keyboard_recording
    for name in ('binary', 'niri', 'frontend'):
        path = getattr(args, name).resolve(strict=True)
        setattr(args, name, path)
        assert sha256(path) == getattr(args, name + '_sha256')
    args.niri = freeze_niri(args.niri, args.niri_sha256)
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    paths = [Path(__file__).resolve(), args.binary, args.niri, args.frontend, ROOT / 'scripts/test-global-shortcuts-lease.py', ROOT / 'scripts/native_compositor.py', ROOT / 'scripts/validate-desktop.py', ROOT / 'packaging/portal/canopa.portal']
    if args.portal_descriptor:
        args.portal_descriptor = args.portal_descriptor.resolve(strict=True); paths.append(args.portal_descriptor)
    paths.extend([ROOT / 'scripts/desktop-fixture/portal-parent.c', Path('/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml'), Path('/usr/share/wayland-protocols/unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')])
    paths.extend((ROOT / 'crates/canopa-portal/src').rglob('*.rs'))
    pins = {str(path): sha256(path) for path in paths}
    result = dict(status='running', scope='Private canonical public frontend, real Rust/Iced UI and synthetic input on pinned Niri. No physical devices, real Flatpak, installed routing or distribution acceptance.', checks=[], pins=pins, scheme=args.scheme, scale=args.scale, keyboard_recording=args.check_keyboard_recording, layout=args.layout, xkb_options=args.xkb_options)
    started = time.monotonic()
    desktop = load('desktop', ROOT / 'scripts/validate-desktop.py')
    with tempfile.TemporaryDirectory(prefix='canopa-shortcuts-portal-') as directory:
        session = desktop.Session(Path(directory), args.output)
        session.shortcut_helpers = []
        session.env['PATH'] = str(args.niri.parent) + os.pathsep + session.env['PATH']
        try:
            verify(args, session, result)
            result['status'] = 'passed'
        except Exception as error:
            result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
            try:
                session.capture('failure.png')
            except Exception:
                pass
        finally:
            session.close()
            result['processes'] = [dict(pid=p.pid, exit_code=p.poll()) for p in session.processes]
            for window in session.shortcut_helpers:
                if window['pidfd'] is not None:
                    terminal = bool(select.select([window['pidfd']], [], [], 5)[0])
                    os.close(window['pidfd'])
                    result.setdefault('helpers', []).append(dict(pid=window['pid'], terminal=terminal))
            if not all(h['terminal'] for h in result.get('helpers', [])):
                result['status'] = 'failed'
    result.update(seconds=round(time.monotonic() - started, 2), changed_inputs=[p for p, h in pins.items() if sha256(Path(p)) != h])
    if result['changed_inputs'] or any(p['exit_code'] is None for p in result['processes']):
        result['status'] = 'failed'
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: result.get(k) for k in ('status', 'error', 'seconds')}))
    raise SystemExit(0 if result['status'] == 'passed' else 1)


if __name__ == '__main__':
    main()
