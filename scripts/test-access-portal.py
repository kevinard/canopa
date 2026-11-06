#!/usr/bin/env python3
"""Exercise real Iced Access consent dialogs on a private backend and compositor."""
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

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def verify(binary, session, scheme, scale):
    from gi.repository import Gio, GLib
    home = session.runtime / 'home'
    home.mkdir()
    config = session.runtime / 'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n')
    session.env.update(HOME=str(home), WAYLAND_DEBUG='1')
    portals = session.runtime / 'portals'
    portals.mkdir()
    shutil.copy(ROOT / 'packaging/portal/canopa.portal', portals / 'canopa.portal')
    preferences = config.parent.parent / 'xdg-desktop-portal/portals.conf'
    preferences.parent.mkdir()
    preferences.write_text('[preferred]\ndefault=none\norg.freedesktop.impl.portal.Settings=canopa\norg.freedesktop.impl.portal.FileChooser=canopa\norg.freedesktop.impl.portal.Access=canopa\n')
    # XDG_DESKTOP_PORTAL_DIR also redirects the frontend's configuration lookup.
    shutil.copy(preferences, portals / 'portals.conf')
    services = session.runtime / 'services'
    services.mkdir()
    service = 'org.freedesktop.impl.portal.desktop.canopa.service'
    shutil.copy(ROOT / 'packaging/portal' / service, services / service)
    permission_service = 'org.freedesktop.impl.portal.PermissionStore.service'
    shutil.copy(Path('/usr/share/dbus-1/services') / permission_service, services / permission_service)
    session.env.update(PATH=str(binary.parent) + os.pathsep + session.env['PATH'], XDG_DESKTOP_PORTAL_DIR=str(portals), XDG_CURRENT_DESKTOP='Canopa')
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; focus-ring { off; }; }\n',
                       width=round(1280 * scale), height=round(800 * scale), scale=scale, service_directories=[services])
    outputs = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))
    logical = next(output['logical'] for output in outputs.values() if output.get('logical'))
    screen_width, screen_height = logical['width'], logical['height']
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

    def call(destination, path, interface, method, signature, values):
        return connection.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()

    # Activation inherits only this disposable bus's compositor/configuration.
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'UpdateActivationEnvironment', '(a{ss})', ({k: v for k, v in session.env.items()},))
    frontend = next(p for p in [Path('/usr/libexec/xdg-desktop-portal'), Path('/usr/lib/xdg-desktop-portal')] if p.is_file())
    session.env.update(PIPEWIRE_REMOTE='pipewire-0', PIPEWIRE_RUNTIME_DIR=str(session.runtime))
    session.start(['pipewire', '-c', '/usr/share/pipewire/pipewire.conf'], 'pipewire.log')
    session.wait_for(lambda: (session.runtime / 'pipewire-0').exists(), 'private PipeWire socket')
    session.start([str(frontend), '--verbose'], 'frontend.log')
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'StartServiceByName', '(su)', ('org.freedesktop.impl.portal.desktop.canopa', 0))
    backend_pid = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', '(s)', ('org.freedesktop.impl.portal.desktop.canopa',))[0]
    assert Path(f'/proc/{backend_pid}/exe').resolve() == binary

    # Native parent/export client uses only wayland-client and system protocol XMLs.
    protocols = Path('/usr/share/wayland-protocols')
    for name, xml in [('xdg-shell', protocols / 'stable/xdg-shell/xdg-shell.xml'), ('xdg-foreign', protocols / 'unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for kind, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', kind, str(xml), str(session.output / (name + suffix))], check=True)
    parent = session.output / 'portal-parent'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output), str(ROOT / 'scripts/desktop-fixture/portal-parent.c'), str(session.output / 'xdg-shell.c'), str(session.output / 'xdg-foreign.c'), '-lwayland-client', '-o', str(parent)], check=True)
    session.start([str(parent)], 'parent.log')

    def parent_handle():
        return next((line for line in (session.output / 'parent.log').read_text().splitlines() if line.startswith('wayland:')), None)
    exported = session.wait_for(parent_handle, 'exported Wayland parent')
    pointer = session.build_pointer()

    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))

    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)

    checks = []
    pending = {}
    count = 0
    def click(x, y):
        session.run([str(pointer), str(round(x / screen_width * 1600)), str(round(y / screen_height * 1000))])
        time.sleep(.2)
    def begin(options=None, body='Only this private fixture receives a decision.', client=connection):
        nonlocal count
        count += 1
        path = f'/org/freedesktop/portal/desktop/request/test/access{count}'
        before = {w['id'] for w in windows()}
        def done(conn, result):
            try:
                pending[path] = conn.call_finish(result).unpack()
            except GLib.Error as error:
                pending[path] = error
        client.call('org.freedesktop.impl.portal.desktop.canopa', '/org/freedesktop/portal/desktop', 'org.freedesktop.impl.portal.Access', 'AccessDialog', GLib.Variant('(osssssa{sv})', (path, 'org.canopa.PortalTest', exported, 'Camera access', 'Allow this application to use the camera?', body, options or {})), None, Gio.DBusCallFlags.NONE, 30000, None, done)
        window = session.wait_for(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.Portal' and w['id'] not in before), None), 'Iced Access window')
        assert window['is_floating'] and abs(window['layout']['window_size'][0] - 680) <= 1, window
        focus(window)
        return path, window
    def focus(window):
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        time.sleep(.5)
        session.run(['niri', 'msg', 'action', 'move-floating-window', '--id', str(window['id']), '--x', '20', '--y', '10'])
        time.sleep(.2)
    def result(path, status):
        response = session.wait_for(lambda: (drain(), pending.get(path))[1], 'Access response')
        assert response[0] == status, response
        return response[1]
    def action(window, grant):
        width, height = window['layout']['window_size']
        click(20 + width * (.75 if grant else .25), 10 + height - 36)
    def closed(window):
        session.wait_for(lambda: not any(w['id'] == window['id'] for w in windows()), 'Access helper closes')

    path, window = begin()
    session.run(['wtype', '-k', 'Shift_L'])
    session.run(['wtype', '-k', 'Return'])
    time.sleep(.2)
    drain()
    assert path not in pending
    session.capture('01-deny.png')
    action(window, False)
    assert result(path, 1) == {}
    closed(window)
    checks.append('opening and initial Enter do not grant; explicit Deny returns no grant')

    icon = Path(session.env['XDG_DATA_HOME']) / 'icons/hicolor/scalable/apps/fixture-lock-symbolic.svg'
    icon.parent.mkdir(parents=True)
    icon.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect x="10" y="26" width="44" height="30" rx="4" fill="currentColor"/><path d="M20 26 V18 a12 12 0 0 1 24 0 V26" fill="none" stroke="currentColor" stroke-width="5"/></svg>')
    options = {'deny_label': GLib.Variant('s', 'Refuse'), 'grant_label': GLib.Variant('s', 'Permit'), 'icon': GLib.Variant('s', 'fixture-lock-symbolic'), 'choices': GLib.Variant('a(ssa(ss)s)', [('remember', 'Remember this decision', [], 'false'), ('scope', 'Duration', [('once', 'Once'), ('session', 'This session')], 'once')])}
    path, window = begin(options)
    session.capture('02-grant-choices.png')
    click(48, 233)
    click(100, 300)
    click(100, 267)
    session.capture('04-changed-choices.png')
    action(window, True)
    assert result(path, 0)['choices'] == [('remember', 'true'), ('scope', 'session')]
    closed(window)
    checks.append('explicit Grant returns edited bool/combo choices with custom labels and a symbolic icon')

    path, window = begin(body='\n'.join(f'Permission explanation paragraph {i}: the frontend retains policy.' for i in range(80)))
    session.capture('03-long-body.png')
    action(window, False)
    result(path, 1)
    closed(window)
    checks.append('long body scrolls while Deny and Grant remain reachable')

    path, window = begin()
    session.run(['wtype', '-k', 'Shift_L'])
    session.run(['wtype', '-k', 'Escape'])
    result(path, 1)
    closed(window)
    checks.append('Escape denies without a grant')
    path, window = begin()
    call('org.freedesktop.impl.portal.desktop.canopa', path, 'org.freedesktop.impl.portal.Request', 'Close', '()', ())
    result(path, 1)
    closed(window)
    checks.append('Request.Close destroys its owned helper without granting')

    first, first_window = begin()
    second, second_window = begin()
    assert call('org.freedesktop.impl.portal.desktop.canopa', '/org/freedesktop/portal/desktop', 'org.freedesktop.impl.portal.Settings', 'ReadAll', '(as)', ([],))[0]
    action(second_window, True)
    result(second, 0)
    closed(second_window)
    assert any(w['id'] == first_window['id'] for w in windows())
    focus(first_window)
    action(first_window, False)
    result(first, 1)
    closed(first_window)
    checks.append('concurrent requests retain independent decisions and responsive Settings')

    lost = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    _, window = begin(client=lost)
    lost.close_sync(None)
    closed(window)
    checks.append('caller loss destroys its helper without granting')
    # Exercise a real public-frontend consumer. Media flow is a separate test.
    session.wait_for(lambda: call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', '(s)', ('org.freedesktop.portal.Desktop',))[0], 'public frontend ready', timeout=40)
    responses = {}
    subscription = connection.signal_subscribe('org.freedesktop.portal.Desktop', 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE,
        lambda conn, sender, path, interface, name, params: responses.__setitem__(path, params.unpack()))
    before = {window['id'] for window in windows()}
    path = call('org.freedesktop.portal.Desktop', '/org/freedesktop/portal/desktop', 'org.freedesktop.portal.Camera', 'AccessCamera', '(a{sv})', ({'handle_token': GLib.Variant('s', 'camera')},))[0]
    window = session.wait_for(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.Portal' and w['id'] not in before), None), 'public Camera consent question')
    # Camera has no parent-window option. Normalize this independent test window
    # explicitly, without claiming automatic transient placement for that case.
    session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
    if not window['is_floating']:
        session.run(['niri', 'msg', 'action', 'toggle-window-floating', '--id', str(window['id'])])
    time.sleep(.5)
    session.run(['niri', 'msg', 'action', 'set-window-width', '680'])
    session.run(['niri', 'msg', 'action', 'set-window-height', '475'])
    time.sleep(.2)
    focus(window)
    window = next(w for w in windows() if w['id'] == window['id'])
    session.capture('05-public-camera.png')
    action(window, True)
    response = session.wait_for(lambda: (drain(), responses.get(path))[1], 'public Camera permission response')
    assert response[0] == 0, response
    closed(window)
    checks.append('public Camera frontend displays Canopa consent and receives explicit grant; media flow unverified')
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
    with tempfile.TemporaryDirectory(prefix='canopa-access-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        try:
            checks = verify(args.binary.resolve(), session, args.color_scheme, args.scale)
            desktop.write_json(args.output / 'result.json', dict(passed=True, checks=checks, scheme=args.color_scheme, scale=args.scale))
        finally:
            for process in reversed(session.processes):
                desktop.stop(process)
    print(f'Access: {len(checks)} native checks passed')


if __name__ == '__main__':
    main()
