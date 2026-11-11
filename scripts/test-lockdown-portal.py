#!/usr/bin/env python3
"""Verify live Lockdown enforcement with a private frontend and Rust/Iced dialogs."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import sqlite3
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
BACKEND = 'org.freedesktop.impl.portal.desktop.canopa'
FRONTEND = 'org.freedesktop.portal.Desktop'
PATH = '/org/freedesktop/portal/desktop'
IMPL = 'org.freedesktop.impl.portal.Lockdown'
PROPERTIES = 'org.freedesktop.DBus.Properties'
KEYS = ['disable-printing', 'disable-save-to-disk', 'disable-application-handlers',
        'disable-location', 'disable-camera', 'disable-microphone', 'disable-sound-output']


def verify(binary, session, scheme, scale, shipped):
    from gi.repository import Gio, GLib
    home = session.runtime / 'home'
    folder = home / 'Files'
    folder.mkdir(parents=True)
    (folder / 'fixture.txt').write_text('Disposable policy fixture\n')
    config = session.runtime / 'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n')
    data = Path(session.env['XDG_DATA_HOME'])
    applications = data / 'applications'
    applications.mkdir(parents=True)
    recorded = session.runtime / 'launched.json'
    recorder = session.runtime / 'record-handler'
    recorder.write_text('#!/usr/bin/python3\nimport json,sys\nfrom pathlib import Path\n' + f'Path({str(recorded)!r}).write_text(json.dumps(sys.argv[1:]))\n')
    recorder.chmod(0o700)
    (applications / 'org.canopa.PolicyTest.desktop').write_text('[Desktop Entry]\nType=Application\nName=Policy Fixture\nExec=true\n')
    (applications / 'handler.desktop').write_text(f'[Desktop Entry]\nType=Application\nName=Private Handler\nExec={recorder} %u\nMimeType=x-scheme-handler/https;\n')
    (applications / 'mimeinfo.cache').write_text('[MIME Cache]\nx-scheme-handler/https=handler.desktop;\n')
    (config.parent.parent / 'mimeapps.list').write_text('[Default Applications]\nx-scheme-handler/https=handler.desktop;\n[Added Associations]\nx-scheme-handler/https=handler.desktop;\n')
    portals, services = session.runtime / 'portals', session.runtime / 'services'
    portals.mkdir()
    services.mkdir()
    descriptor = (ROOT / 'packaging/portal/canopa.portal').read_text()
    if not shipped and IMPL + ';' not in descriptor:
        descriptor = descriptor.replace('Interfaces=', 'Interfaces=' + IMPL + ';')
    assert IMPL + ';' in descriptor
    (portals / 'canopa.portal').write_text(descriptor)
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\n' + ''.join(
        f'org.freedesktop.impl.portal.{name}=canopa\n' for name in ['Settings', 'Lockdown', 'FileChooser', 'AppChooser']))
    shutil.copy(ROOT / 'packaging/portal/org.freedesktop.impl.portal.desktop.canopa.service', services)
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home), WAYLAND_DEBUG='1', XDG_DATA_DIRS=str(data),
        GIO_USE_VFS='local', GSETTINGS_BACKEND='memory', GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas',
        XDG_CURRENT_DESKTOP='Canopa', XDG_DESKTOP_PORTAL_DIR=str(portals),
        PATH=str(binary.parent) + os.pathsep + session.env['PATH'])
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n',
        width=round(1280 * scale), height=round(800 * scale), scale=scale, service_directories=[services])
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

    def call(destination, path, interface, method, signature, values):
        return connection.call_sync(destination, path, interface, method, GLib.Variant(signature, values),
            None, Gio.DBusCallFlags.NONE, 10000, None).unpack()

    def bus(method, signature, values):
        return call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', method, signature, values)

    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)

    bus('UpdateActivationEnvironment', '(a{ss})', ({k: v for k, v in session.env.items()},))
    frontend = next(p for p in [Path('/usr/libexec/xdg-desktop-portal'), Path('/usr/lib/xdg-desktop-portal')] if p.is_file())
    session.start([str(frontend), '--verbose'], 'frontend.log')

    def activate():
        bus('StartServiceByName', '(su)', (BACKEND, 0))
        pid = bus('GetConnectionUnixProcessID', '(s)', (BACKEND,))[0]
        assert Path(f'/proc/{pid}/exe').resolve() == binary
        return pid
    backend_pid = activate()
    session.wait_for(lambda: bus('NameHasOwner', '(s)', (FRONTEND,))[0], 'public frontend')
    call(FRONTEND, PATH, 'org.freedesktop.host.portal.Registry', 'Register', '(sa{sv})', ('org.canopa.PolicyTest', {}))
    state = Path(session.env['XDG_STATE_HOME']) / 'canopa/portal/lockdown.sqlite3'
    assert call(BACKEND, PATH, PROPERTIES, 'GetAll', '(s)', (IMPL,)) == ({key: False for key in KEYS},)
    assert not state.exists()
    changes, responses, checks = [], {}, []
    subscription = connection.signal_subscribe(BACKEND, PROPERTIES, 'PropertiesChanged', PATH, IMPL,
        Gio.DBusSignalFlags.NONE, lambda c, s, p, i, n, v: changes.append(v.unpack()))
    response_subscription = connection.signal_subscribe(FRONTEND, 'org.freedesktop.portal.Request', 'Response', None, None,
        Gio.DBusSignalFlags.NONE, lambda c, s, p, i, n, v: responses.__setitem__(p, v.unpack()))
    bus('GetId', '()', ())

    def set_policy(key, value):
        before = len(changes)
        call(BACKEND, PATH, PROPERTIES, 'Set', '(ssv)', (IMPL, key, GLib.Variant('b', value)))
        session.wait_for(lambda: (drain(), len(changes) > before)[1], 'committed property signal')
        assert changes[-1] == (IMPL, {key: value}, []), changes[-1]

    for key in KEYS:
        set_policy(key, True)
        set_policy(key, False)
    assert state.is_file() and state.stat().st_mode & 0o777 == 0o600
    before = len(changes)
    call(BACKEND, PATH, PROPERTIES, 'Set', '(ssv)', (IMPL, KEYS[0], GLib.Variant('b', False)))
    time.sleep(.1)
    drain()
    assert len(changes) == before
    checks.append('all seven booleans persist and publish exact standard changes; no-op has no signal')

    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))

    def helpers():
        return [window for window in windows() if window['app_id'] == 'org.canopa.Portal']

    uri = 'https://example.test/only-private-handler?fixture=lockdown'

    def denied(interface, method, signature, values):
        before = {window['id'] for window in helpers()}
        try:
            call(FRONTEND, PATH, interface, method, signature, values)
            raise AssertionError(f'{method} bypassed the disabled policy')
        except GLib.Error as error:
            assert 'NotAllowed' in str(error), error
        assert {window['id'] for window in helpers()} == before
        assert not recorded.exists()

    def save_disabled():
        for method in ['SaveFile', 'SaveFiles']:
            denied('org.freedesktop.portal.FileChooser', method, '(ssa{sv})', ('', 'Private save', {}))

    count = 0

    def save_dialog(capture):
        nonlocal count
        count += 1
        path = call(FRONTEND, PATH, 'org.freedesktop.portal.FileChooser', 'SaveFile', '(ssa{sv})',
            ('', 'Private save', {'handle_token': GLib.Variant('s', f'policy{count}'),
                'current_folder': GLib.Variant('ay', os.fsencode(folder) + b'\0'), 'current_name': GLib.Variant('s', 'policy.txt')}))[0]
        window = session.wait_for(lambda: (drain(), next(iter(helpers()), None))[1], 'real Iced save dialog')
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        time.sleep(.3)
        session.capture(capture)
        session.run(['wtype', '-k', 'Escape'])
        result = session.wait_for(lambda: (drain(), responses.get(path))[1], 'cancelled public save')
        assert result[0] == 1 and result[1].get('uris', []) == [], result
        session.wait_for(lambda: not helpers(), 'Iced helper retirement')

    set_policy('disable-save-to-disk', True)
    save_disabled()
    checks.append('live frontend cache blocks SaveFile and SaveFiles before creating a helper')
    set_policy('disable-save-to-disk', False)
    save_dialog('01-saving-restored.png')
    checks.append('clearing save policy restores the actual Rust/Iced chooser without frontend restart')
    set_policy('disable-application-handlers', True)
    denied('org.freedesktop.portal.OpenURI', 'OpenURI', '(ssa{sv})', ('', uri, {}))
    checks.append('live handler policy rejects public OpenURI before launching any fixture app')
    set_policy('disable-application-handlers', False)
    path = call(FRONTEND, PATH, 'org.freedesktop.portal.OpenURI', 'OpenURI', '(ssa{sv})', ('', uri, {}))[0]

    def launched():
        try:
            return json.loads(recorded.read_text()) == [uri]
        except (FileNotFoundError, json.JSONDecodeError):
            return False
    session.wait_for(lambda: (drain(), launched())[1], 'only the disposable handler runs')
    result = session.wait_for(lambda: (drain(), responses.get(path))[1], 'public OpenURI response')
    assert result[0] == 0
    recorded.unlink()
    checks.append('clearing handler policy restores only the private recording app with the exact URI')

    set_policy('disable-save-to-disk', True)
    set_policy('disable-application-handlers', True)
    os.kill(backend_pid, signal.SIGTERM)
    session.wait_for(lambda: not Path(f'/proc/{backend_pid}').exists(), 'backend shutdown')
    replacement_pid = activate()
    assert replacement_pid != backend_pid
    backend_pid = replacement_pid
    session.wait_for(lambda: call(BACKEND, PATH, PROPERTIES, 'Get', '(ss)', (IMPL, 'disable-save-to-disk')) == (True,), 'durable restart snapshot')
    time.sleep(.2)
    save_disabled()
    denied('org.freedesktop.portal.OpenURI', 'OpenURI', '(ssa{sv})', ('', uri, {}))
    checks.append('new activated backend restores durable policy and the existing frontend enforces it')

    with sqlite3.connect(state) as db:
        db.execute('PRAGMA user_version=99')
    previous = (session.output / 'dbus.log').read_text().count('event=policy_reload_rejected')
    os.kill(backend_pid, signal.SIGHUP)
    session.wait_for(lambda: (session.output / 'dbus.log').read_text().count('event=policy_reload_rejected') > previous, 'rejected policy reload')
    save_disabled()
    assert call(BACKEND, PATH, PROPERTIES, 'Get', '(ss)', (IMPL, 'disable-save-to-disk')) == (True,)
    checks.append('invalid source reload preserves the last valid restrictive values')
    with sqlite3.connect(state) as db:
        db.execute('PRAGMA user_version=1')
        db.execute('UPDATE policy SET save_to_disk=0,application_handlers=0,revision=revision+1')
    before = len(changes)
    os.kill(backend_pid, signal.SIGHUP)
    session.wait_for(lambda: (drain(), len(changes) > before)[1], 'external transaction reload publication')
    assert changes[-1] == (IMPL, {'disable-save-to-disk': False, 'disable-application-handlers': False}, []), changes[-1]
    save_dialog('02-reload-restored.png')
    checks.append('valid source transaction and SIGHUP publish one coherent changed-value snapshot')
    state.chmod(0o400)
    try:
        call(BACKEND, PATH, PROPERTIES, 'Set', '(ssv)', (IMPL, 'disable-printing', GLib.Variant('b', True)))
        raise AssertionError('unwritable policy accepted a change')
    except GLib.Error as error:
        assert 'AccessDenied' in str(error), error
    finally:
        state.chmod(0o600)
    assert call(BACKEND, PATH, PROPERTIES, 'Get', '(ss)', (IMPL, 'disable-printing')) == (False,)
    checks.append('unwritable selected source fails explicitly and retains its prior value')
    connection.signal_unsubscribe(subscription)
    connection.signal_unsubscribe(response_subscription)
    connection.close_sync(None)
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--color-scheme', choices=['dark', 'light'], default='dark')
    parser.add_argument('--scale', type=float, default=1.0)
    parser.add_argument('--shipped-descriptor', action='store_true')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-lockdown-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        try:
            checks = verify(args.binary.resolve(), session, args.color_scheme, args.scale, args.shipped_descriptor)
            desktop.write_json(args.output / 'result.json', dict(passed=True, checks=checks, scheme=args.color_scheme,
                scale=args.scale, shipped_descriptor=args.shipped_descriptor,
                binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest()))
        finally:
            for process in reversed(session.processes):
                desktop.stop(process)
    print(f'Lockdown: {len(checks)} native checks passed')


if __name__ == '__main__':
    main()
