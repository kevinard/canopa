#!/usr/bin/env python3
"""Exercise the public OpenURI frontend and real Iced AppChooser in private Niri."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
from urllib.parse import quote_from_bytes

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def verify(binary, session, scheme, scale):
    from gi.repository import Gio, GLib
    home = session.runtime / 'home'
    home.mkdir()
    fixture = home / 'Documents'
    fixture.mkdir()
    (fixture / 'a.txt').write_text('original content')
    (fixture / 'b.txt').write_text('second file')
    (fixture / 'az.png').write_bytes(b'not opened by the chooser')
    (fixture / 'folder').mkdir()
    config = session.runtime / 'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n')
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home), WAYLAND_DEBUG='1')
    portals = session.runtime / 'portals'
    portals.mkdir()
    shutil.copy(ROOT / 'packaging/portal/canopa.portal', portals / 'canopa.portal')
    preferences = config.parent.parent / 'xdg-desktop-portal/portals.conf'
    preferences.parent.mkdir()
    preferences.write_text('[preferred]\ndefault=none\norg.freedesktop.impl.portal.Settings=canopa\norg.freedesktop.impl.portal.FileChooser=canopa\norg.freedesktop.impl.portal.AppChooser=canopa\n')
    # XDG_DESKTOP_PORTAL_DIR also redirects the frontend's configuration lookup.
    shutil.copy(preferences, portals / 'portals.conf')
    services = session.runtime / 'services'
    services.mkdir()
    service = 'org.freedesktop.impl.portal.desktop.canopa.service'
    shutil.copy(ROOT / 'packaging/portal' / service, services / service)
    session.env.update(PATH=str(binary.parent) + os.pathsep + session.env['PATH'], XDG_DESKTOP_PORTAL_DIR=str(portals), XDG_CURRENT_DESKTOP='Canopa')
    # Only disposable desktop entries and MIME defaults can be launched.
    data = session.runtime / 'data'
    applications = data / 'applications'
    applications.mkdir(parents=True)
    empty_data = session.runtime / 'empty-data'
    empty_data.mkdir()
    session.env['XDG_DATA_DIRS'] = str(empty_data)
    session.env['GSETTINGS_SCHEMA_DIR'] = '/usr/share/glib-2.0/schemas'
    recorded = session.output / 'launched.json'
    recorder = session.runtime / 'record-app'
    chosen = session.output / 'portal-chosen'
    recorder.write_text('#!/usr/bin/python3\nimport json, os, sys\nfrom pathlib import Path\n'
        + f'Path({str(recorded)!r}).write_text(json.dumps(dict(app=sys.argv[1], uri=sys.argv[2], token=bool(os.environ.get("XDG_ACTIVATION_TOKEN")))))\n'
        + f'os.execl({str(chosen)!r}, {str(chosen)!r})\n')
    recorder.chmod(0o755)
    for label in ['Alpha', 'Beta']:
        (applications / f'{label.lower()}.desktop').write_text(
            f'[Desktop Entry]\nType=Application\nName={label} Viewer\nGenericName=Fixture application\nExec={recorder} {label} %u\nIcon=fixture-{label.lower()}\nMimeType=x-scheme-handler/canopa-test;\nStartupNotify=true\n')
    (applications / 'mimeinfo.cache').write_text('[MIME Cache]\nx-scheme-handler/canopa-test=alpha.desktop;beta.desktop;\n')
    (config.parent.parent / 'mimeapps.list').write_text('[Default Applications]\nx-scheme-handler/canopa-test=alpha.desktop;\n[Added Associations]\nx-scheme-handler/canopa-test=alpha.desktop;beta.desktop;\n')
    icons = data / 'icons/hicolor/scalable/apps'
    icons.mkdir(parents=True)
    for label, color in [('alpha', '#4a8ad4'), ('beta', '#d48a4a')]:
        (icons / f'fixture-{label}.svg').write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect x="4" y="4" width="56" height="56" rx="12" fill="{color}"/></svg>')
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; focus-ring { off; }; }\n',
                       width=round(1280 * scale), height=round(800 * scale), scale=scale, service_directories=[services])
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

    def call(destination, path, interface, method, signature, values):
        return connection.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()

    # Activation inherits only this disposable bus's compositor/configuration.
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'UpdateActivationEnvironment', '(a{ss})', ({k: v for k, v in session.env.items()},))
    frontend = next(p for p in [Path('/usr/libexec/xdg-desktop-portal'), Path('/usr/lib/xdg-desktop-portal')] if p.is_file())
    session.start([str(frontend), '--verbose'], 'frontend.log')
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'StartServiceByName', '(su)', ('org.freedesktop.impl.portal.desktop.canopa', 0))
    backend_pid = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', '(s)', ('org.freedesktop.impl.portal.desktop.canopa',))[0]
    assert Path(f'/proc/{backend_pid}/exe').resolve() == binary

    # Native parent/export client uses only wayland-client and system protocol XMLs.
    protocols = Path('/usr/share/wayland-protocols')
    for name, xml in [('xdg-shell', protocols / 'stable/xdg-shell/xdg-shell.xml'), ('xdg-foreign', protocols / 'unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for kind, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', kind, str(xml), str(session.output / (name + suffix))], check=True)
    xml = protocols / 'staging/xdg-activation/xdg-activation-v1.xml'
    for kind, suffix in [('client-header', '.h'), ('private-code', '.c')]:
        subprocess.run(['wayland-scanner', kind, str(xml), str(session.output / ('xdg-activation' + suffix))], check=True)
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-DPORTAL_CHOSEN', '-I', str(session.output), str(ROOT / 'scripts/desktop-fixture/portal-parent.c'), str(session.output / 'xdg-shell.c'), str(session.output / 'xdg-foreign.c'), str(session.output / 'xdg-activation.c'), '-lwayland-client', '-o', str(chosen)], check=True)
    parent = session.output / 'portal-parent'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output), str(ROOT / 'scripts/desktop-fixture/portal-parent.c'), str(session.output / 'xdg-shell.c'), str(session.output / 'xdg-foreign.c'), '-lwayland-client', '-o', str(parent)], check=True)
    session.start([str(parent)], 'parent.log')

    def parent_handle():
        return next((line for line in (session.output / 'parent.log').read_text().splitlines() if line.startswith('wayland:')), None)
    exported = session.wait_for(parent_handle, 'exported Wayland parent')

    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))

    def chooser():
        return next((w for w in windows() if w['app_id'] == 'org.canopa.Portal'), None)

    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)

    responses = {}
    subscription = connection.signal_subscribe('org.freedesktop.portal.Desktop', 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE,
        lambda conn, sender, path, interface, name, params: responses.__setitem__(path, params.unpack()))
    def keys(*names):
        for name in names:
            session.run(['wtype', '-k', name])
            time.sleep(.1)

    def chord(key):
        session.run(['wtype', '-M', 'ctrl', '-k', key, '-m', 'ctrl'])
        time.sleep(.15)

    checks = []
    def focus():
        window = session.wait_for(chooser, 'Iced AppChooser')
        assert window['is_floating'] and abs(window['layout']['window_size'][0] - 620) <= 1, window
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        time.sleep(.5)
        session.run(['niri', 'msg', 'action', 'move-floating-window', '--id', str(window['id']), '--x', '20', '--y', '10'])
        time.sleep(.2)

    session.wait_for(lambda: call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', '(s)', ('org.freedesktop.portal.Desktop',))[0], 'public frontend bus name', timeout=40)
    uri = 'canopa-test:requested-item'
    handle = call('org.freedesktop.portal.Desktop', '/org/freedesktop/portal/desktop', 'org.freedesktop.portal.OpenURI', 'OpenURI', '(ssa{sv})', (exported, uri, {'ask': GLib.Variant('b', True), 'handle_token': GLib.Variant('s', 'openuri')}))[0]
    focus()
    session.capture('01-applications.png')
    chord('f')
    session.run(['wtype', 'Beta'])
    time.sleep(.3)
    session.capture('02-filtered.png')
    keys('Return')
    def received():
        drain()
        return responses.get(handle)
    response = session.wait_for(received, 'public OpenURI response')
    assert response[0] == 0, response
    session.wait_for(lambda: chooser() is None, 'chooser closed')
    launched = session.wait_for(lambda: json.loads(recorded.read_text()) if recorded.exists() else None, 'fixture application launch')
    assert launched == dict(app='Beta', uri=uri, token=True), launched
    selected = session.wait_for(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.PortalChosen'), None), 'chosen Wayland client')
    session.wait_for(lambda: next(w for w in windows() if w['id'] == selected['id'])['is_focused'], 'fresh token focuses chosen client')
    checks.extend(['public OpenURI chooses only fixture Beta and preserves URI', 'fresh outgoing activation token reaches launched application', 'chosen native Wayland client consumes token and receives focus'])

    # The same direct caller owns the live choice update and Close operations.
    pending = {}
    def begin(number, choices):
        path = f'/org/freedesktop/portal/desktop/request/test/native{number}'
        def done(conn, result):
            try:
                pending[path] = conn.call_finish(result).unpack()
            except GLib.Error as error:
                pending[path] = error
        connection.call('org.freedesktop.impl.portal.desktop.canopa', '/org/freedesktop/portal/desktop', 'org.freedesktop.impl.portal.AppChooser', 'ChooseApplication', GLib.Variant('(ossasa{sv})', (path, 'org.canopa.Test', exported, choices, {'activation_token': GLib.Variant('s', 'fixture-stale-incoming')})), None, Gio.DBusCallFlags.NONE, 15000, None, done)
        focus()
        return path

    path = begin(2, ['alpha.desktop', 'beta.desktop'])
    call('org.freedesktop.impl.portal.desktop.canopa', '/org/freedesktop/portal/desktop', 'org.freedesktop.impl.portal.AppChooser', 'UpdateChoices', '(oas)', (path, ['beta.desktop']))
    time.sleep(.4)
    session.capture('03-updated.png')
    # A distinct virtual-keyboard map also primes newly connected clients;
    # Niri may retain the last wtype map across keyboard replacement.
    chord('f')
    keys('Return')
    drain()
    assert path not in pending, pending
    chord('f')
    session.run(['wtype', 'Beta'])
    keys('Return')
    session.wait_for(lambda: (drain(), pending.get(path))[1], 'updated choice response')
    status, result = pending[path]
    assert status == 0 and result['choice'] == 'beta.desktop' and result.get('activation_token') and result['activation_token'] != 'fixture-stale-incoming', pending[path]
    session.wait_for(lambda: chooser() is None, 'updated chooser closes')
    checks.append('native UpdateChoices removes initial selection; explicit current choice gets a fresh token')
    path = begin(3, ['alpha.desktop', 'beta.desktop'])
    call('org.freedesktop.impl.portal.desktop.canopa', path, 'org.freedesktop.impl.portal.Request', 'Close', '()', ())
    session.wait_for(lambda: chooser() is None, 'Close retires helper')
    session.wait_for(lambda: (drain(), pending.get(path))[1], 'Close response')
    assert pending[path][0] == 1, pending[path]
    checks.append('native Request.Close destroys the helper')
    trace = (session.output / 'dbus.log').read_text()
    assert 'set_serial' in trace and 'get_activation_token' in trace
    assert 'activate("fixture-stale-incoming"' in trace
    checks.append('incoming activation is applied during native helper startup')
    checks.append('outgoing xdg-activation request carries user input serial')
    connection.signal_unsubscribe(subscription)
    connection.close_sync(None)
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--color-scheme', choices=['dark', 'light'], default='dark')
    parser.add_argument('--scale', type=float, default=1.0)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-app-chooser-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        try:
            checks = verify(args.binary.resolve(), session, args.color_scheme, args.scale)
            desktop.write_json(args.output / 'result.json', dict(passed=True, checks=checks, scheme=args.color_scheme, scale=args.scale))
        finally:
            for process in reversed(session.processes):
                desktop.stop(process)
    print(f'AppChooser: {len(checks)} native checks passed')


if __name__ == '__main__':
    main()
