#!/usr/bin/env python3
"""Exercise Rust/Iced launcher review and real frontend publication in a private desktop."""
import argparse
import hashlib
import importlib.util
import io
import json
import os
import re
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
BACKEND = 'org.freedesktop.impl.portal.desktop.canopa'
FRONTEND = 'org.freedesktop.portal.Desktop'
PATH = '/org/freedesktop/portal/desktop'
PUBLIC = 'org.freedesktop.portal.DynamicLauncher'
IMPL = 'org.freedesktop.impl.portal.DynamicLauncher'


def verify(binary, session, scheme, scale, shipped):
    from gi.repository import Gio, GLib
    from PIL import Image
    home = session.runtime / 'home'
    home.mkdir()
    config = session.runtime / 'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n')
    data = Path(session.env['XDG_DATA_HOME'])
    applications = data / 'applications'
    applications.mkdir(parents=True)
    for app_id in ['org.canopa.PortalTest', 'org.canopa.OtherTest']:
        (applications / f'{app_id}.desktop').write_text('[Desktop Entry]\nType=Application\nName=Launcher Fixture\nExec=true\n')
    services, portals = session.runtime / 'services', session.runtime / 'portals'
    services.mkdir()
    portals.mkdir()
    shutil.copy(ROOT / 'packaging/portal/org.freedesktop.impl.portal.desktop.canopa.service', services)
    descriptor = (ROOT / 'packaging/portal/canopa.portal').read_text()
    if not shipped and IMPL + ';' not in descriptor:
        descriptor = descriptor.replace('Interfaces=', 'Interfaces=' + IMPL + ';')
    assert IMPL + ';' in descriptor
    (portals / 'canopa.portal').write_text(descriptor)
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\n' + IMPL + '=canopa\norg.freedesktop.impl.portal.Settings=canopa\n')
    session.env.update(HOME=str(home), WAYLAND_DEBUG='1', XDG_CURRENT_DESKTOP='Canopa',
                       XDG_DESKTOP_PORTAL_DIR=str(portals), XDG_DATA_DIRS=str(data),
                       GIO_USE_VFS='local', GSETTINGS_BACKEND='memory',
                       GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas',
                       PATH=str(binary.parent) + os.pathsep + session.env['PATH'])
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; focus-ring { off; }; }\n',
                       width=round(1280 * scale), height=round(800 * scale), scale=scale, service_directories=[services])
    outputs = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))
    logical = next(item['logical'] for item in outputs.values() if item.get('logical'))
    width, height = logical['width'], logical['height']
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    flags = Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION

    def connect():
        return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], flags, None, None)
    connection = connect()

    def raw(destination, path, interface, method, signature, values, client=connection):
        return client.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 15000, None)

    def call(destination, path, interface, method, signature, values, client=connection):
        return raw(destination, path, interface, method, signature, values, client).unpack()

    def bus(method, signature, values):
        return call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', method, signature, values)

    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)

    bus('UpdateActivationEnvironment', '(a{ss})', ({k: v for k, v in session.env.items()},))
    frontend = next(p for p in [Path('/usr/libexec/xdg-desktop-portal'), Path('/usr/lib/xdg-desktop-portal')] if p.is_file())
    session.start([str(frontend), '--verbose'], 'frontend.log')
    bus('StartServiceByName', '(su)', (BACKEND, 0))
    backend_pid = bus('GetConnectionUnixProcessID', '(s)', (BACKEND,))[0]
    assert Path(f'/proc/{backend_pid}/exe').resolve() == binary
    session.wait_for(lambda: bus('NameHasOwner', '(s)', (FRONTEND,))[0], 'public frontend')
    call(FRONTEND, PATH, 'org.freedesktop.host.portal.Registry', 'Register', '(sa{sv})', ('org.canopa.PortalTest', {}))
    stranger = connect()
    call(FRONTEND, PATH, 'org.freedesktop.host.portal.Registry', 'Register', '(sa{sv})', ('org.canopa.OtherTest', {}), stranger)
    assert call(FRONTEND, PATH, 'org.freedesktop.DBus.Properties', 'Get', '(ss)', (PUBLIC, 'SupportedLauncherTypes')) == (3,)
    assert call(BACKEND, PATH, 'org.freedesktop.DBus.Properties', 'Get', '(ss)', (IMPL, 'version')) == (1,)
    protocols = Path('/usr/share/wayland-protocols')
    for name, xml in [('xdg-shell', protocols / 'stable/xdg-shell/xdg-shell.xml'), ('xdg-foreign', protocols / 'unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', mode, str(xml), str(session.output / (name + suffix))], check=True, timeout=15)
    parent = session.output / 'portal-parent'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output), str(ROOT / 'scripts/desktop-fixture/portal-parent.c'), str(session.output / 'xdg-shell.c'), str(session.output / 'xdg-foreign.c'), '-lwayland-client', '-o', str(parent)], check=True, timeout=30)
    session.start([str(parent)], 'parent.log')
    exported = session.wait_for(lambda: next((line for line in (session.output / 'parent.log').read_text().splitlines() if line.startswith('wayland:')), None), 'exported parent')
    pointer = session.build_pointer()

    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))

    def focus(window):
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        time.sleep(.3)
        session.run(['niri', 'msg', 'action', 'move-floating-window', '--id', str(window['id']), '--x', '20', '--y', '10'])
        time.sleep(.2)

    def click(x, y):
        session.run([str(pointer), str(round(x / width * 1600)), str(round(y / height * 1000))])
        time.sleep(.15)

    def keys(key):
        session.run(['wtype', '-k', key])
        time.sleep(.15)

    def edit(name):
        click(260, 200)
        session.run(['wtype', '-M', 'ctrl', '-k', 'a', '-m', 'ctrl', '-k', 'BackSpace', name])
        time.sleep(.15)

    def action(window, accept):
        w, h = window['layout']['window_size']
        click(20 + w * (.75 if accept else .25), 10 + h - 36)

    def closed(window):
        session.wait_for(lambda: not any(w['id'] == window['id'] for w in windows()), 'launcher helper closes')

    icons = {}
    for image_format in ['png', 'jpeg']:
        encoded = io.BytesIO()
        Image.new('RGB', (32, 32), (20, 80, 190)).save(encoded, format=image_format)
        icons[image_format] = encoded.getvalue()
    icons['svg'] = b'<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="#1450be"/></svg>'

    def serialized(image_format):
        return Gio.BytesIcon.new(GLib.Bytes.new(icons[image_format])).serialize()

    responses, pending, checks = {}, {}, []
    subscription = connection.signal_subscribe(FRONTEND, 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE,
        lambda c, s, p, i, n, v: responses.__setitem__(p, v))
    count = 0

    def begin(image_format='png', public=False, options=None, client=connection):
        nonlocal count
        count += 1
        before = {w['id'] for w in windows()}
        options = dict(options or {})
        if public:
            options['handle_token'] = GLib.Variant('s', f'launcher{count}')
            created = {}
            started = time.monotonic()

            def prepared(conn, reply):
                try:
                    created['path'] = conn.call_finish(reply).unpack()[0]
                except GLib.Error as error:
                    created['error'] = error
            client.call(FRONTEND, PATH, PUBLIC, 'PrepareInstall', GLib.Variant('(ssva{sv})', (exported, 'Original launcher', serialized(image_format), options)), None, Gio.DBusCallFlags.NONE, 45000, None, prepared)
            session.wait_for(lambda: (drain(), created)[1], 'frontend icon validation and request creation', timeout=45)
            if 'error' in created:
                raise created['error']
            path = created['path']
            print(f'Public PrepareInstall({image_format}): {time.monotonic() - started:.2f}s', flush=True)
        else:
            path = f'/org/freedesktop/portal/desktop/request/test/launcher{count}'

            def done(conn, result):
                try:
                    pending[path] = conn.call_finish(result)
                except GLib.Error as error:
                    pending[path] = error
            client.call(BACKEND, PATH, IMPL, 'PrepareInstall', GLib.Variant('(osssva{sv})', (path, 'org.canopa.PortalTest', exported, 'Original launcher', serialized(image_format), options)), None, Gio.DBusCallFlags.NONE, 30000, None, done)
        window = session.wait_for(lambda: (drain(), next((w for w in windows() if w['app_id'] == 'org.canopa.Portal' and w['id'] not in before), None))[1], 'launcher review window')
        assert window['is_floating'] and abs(window['layout']['window_size'][0] - 680) <= 1, window
        focus(window)
        return path, window

    def result(path, status, public=False):
        response = session.wait_for(lambda: (drain(), (responses if public else pending).get(path))[1], 'launcher response')
        assert isinstance(response, GLib.Variant), response
        assert response.get_child_value(0).unpack() == status, response
        return response.get_child_value(1)

    def assert_icon(result_dict, image_format):
        icon = result_dict.lookup_value('icon', None)
        assert icon.get_type_string() == 'v', icon
        assert icon.get_variant().get_type_string() == '(sv)', icon
        assert icon.get_variant().get_child_value(0).unpack() == 'bytes'
        assert bytes(icon.get_variant().get_child_value(1).get_variant().unpack()) == icons[image_format]

    path, window = begin(options={'launcher_type': GLib.Variant('u', 2), 'target': GLib.Variant('s', 'https://example.test/review?q=one#two'), 'editable_icon': GLib.Variant('b', True)})
    keys('Return')
    drain()
    assert path not in pending
    frame = session.capture('01-webapp-review.png')
    pixel = frame.getpixel((round(96 * scale), round(161 * scale)))
    assert all(abs(a - b) <= 15 for a, b in zip(pixel, (20, 80, 190))), pixel
    edit('')
    session.capture('01-blank-name.png')
    action(window, True)
    drain()
    assert path not in pending
    edit('Reviewed launcher')
    session.capture('01-edited-name.png')
    action(window, True)
    reviewed = result(path, 0)
    assert reviewed.lookup_value('name', None).unpack() == 'Reviewed launcher', reviewed
    assert_icon(reviewed, 'png')
    closed(window)
    checks.append('real Iced preview, editable webapp name, target, disabled blank Install and no initial Enter acceptance')

    path, window = begin()
    keys('Escape')
    assert result(path, 1).unpack() == {}
    closed(window)
    path, window = begin()
    try:
        call(BACKEND, path, 'org.freedesktop.impl.portal.Request', 'Close', '()', (), stranger)
        raise AssertionError('stranger closed owned request')
    except GLib.Error as error:
        assert 'AccessDenied' in str(error), error
    call(BACKEND, path, 'org.freedesktop.impl.portal.Request', 'Close', '()', ())
    assert result(path, 1).unpack() == {}
    closed(window)
    lost = connect()
    _, window = begin(client=lost)
    lost.close_sync(None)
    closed(window)
    checks.append('Escape, owner-only Close and caller loss retire request-owned helpers')

    path, window = begin(public=True)
    call(FRONTEND, path, 'org.freedesktop.portal.Request', 'Close', '()', ())
    closed(window)
    drain()
    assert path not in responses or responses[path].get_child_value(0).unpack() != 0
    checks.append('public Request.Close forwards cancellation without issuing an install token')

    recorded = session.runtime / 'launched.json'
    recorder = session.runtime / 'record-launch'
    recorder.write_text('#!/usr/bin/python3\nimport json,sys\nfrom pathlib import Path\n' + f'Path({str(recorded)!r}).write_text(json.dumps(sys.argv[1:]))\n')
    recorder.chmod(0o700)
    for image_format in icons:
        editable = image_format == 'png'
        path, window = begin(image_format, public=True, options={'editable_name': GLib.Variant('b', editable), 'modal': GLib.Variant('b', False)})
        chosen = 'Approved public launcher' if editable else 'Original launcher'
        if editable:
            edit(chosen)
        frame = session.capture(f'02-public-{image_format}.png')
        pixel = frame.getpixel((round(96 * scale), round(161 * scale)))
        assert all(abs(a - b) <= 15 for a, b in zip(pixel, (20, 80, 190))), (image_format, pixel)
        action(window, True)
        accepted = result(path, 0, public=True)
        assert_icon(accepted, image_format)
        assert accepted.lookup_value('name', None).unpack() == chosen
        token = accepted.lookup_value('token', None).unpack()
        assert token
        closed(window)
        desktop_id = f'org.canopa.PortalTest.{image_format}.desktop'
        entry = f'[Desktop Entry]\nType=Application\nName=Unapproved name\nIcon=/unapproved-icon\nExec={recorder} {image_format}\nTryExec=/usr/bin/true\n'
        try:
            call(FRONTEND, PATH, PUBLIC, 'Install', '(sssa{sv})', (token, f'org.canopa.OtherTest.{image_format}.desktop', entry, {}), stranger)
            raise AssertionError('another sender redeemed the token')
        except GLib.Error as error:
            assert 'Token given is invalid' in str(error), error
        call(FRONTEND, PATH, PUBLIC, 'Install', '(sssa{sv})', (token, desktop_id, entry, {}))
        try:
            call(FRONTEND, PATH, PUBLIC, 'Install', '(sssa{sv})', (token, desktop_id, entry, {}))
            raise AssertionError('install token was reused')
        except GLib.Error as error:
            assert 'Token given is invalid' in str(error), error
        published = call(FRONTEND, PATH, PUBLIC, 'GetDesktopEntry', '(s)', (desktop_id,))[0]
        keyfile = GLib.KeyFile.new()
        keyfile.load_from_data(published, len(published.encode()), GLib.KeyFileFlags.NONE)
        assert keyfile.get_string('Desktop Entry', 'Name') == chosen
        icon_file = Path(keyfile.get_string('Desktop Entry', 'Icon'))
        assert icon_file.resolve().is_relative_to(data.resolve())
        assert icon_file.read_bytes() == icons[image_format]
        assert (applications / desktop_id).is_symlink()
        returned_icon, returned_format, _ = call(FRONTEND, PATH, PUBLIC, 'GetIcon', '(s)', (desktop_id,))
        assert returned_format == image_format
        assert bytes(returned_icon[1]) == icons[image_format]
        call(FRONTEND, PATH, PUBLIC, 'Launch', '(sa{sv})', (desktop_id, {}))
        session.wait_for(lambda: recorded.is_file() and json.loads(recorded.read_text()) == [image_format], 'only fixture launcher executes')
        recorded.unlink()
        call(FRONTEND, PATH, PUBLIC, 'Uninstall', '(sa{sv})', (desktop_id, {}))
        assert not icon_file.exists() and not (applications / desktop_id).exists()
        assert not (data / 'xdg-desktop-portal/applications' / desktop_id).exists()
        checks.append(f'public {image_format} review preserves approved name and original bytes; token scope/replay, install/read/launch/uninstall pass')

    try:
        call(FRONTEND, PATH, PUBLIC, 'RequestInstallToken', '(sva{sv})', ('Unattended', serialized('png'), {}))
        raise AssertionError('resolved app received an unattended token')
    except GLib.Error as error:
        assert 'NotAllowed' in str(error), error
    checks.append('resolved host app identity receives explicit unattended-install denial')
    trace = (session.output / 'dbus.log').read_text()
    imported = next(line for line in trace.splitlines() if '.import_toplevel(' in line)
    assert '"' + exported.removeprefix('wayland:') + '"' in imported, imported
    configure = re.search(r'xdg_surface#\d+\.configure\(', trace)
    assert configure and trace.index('.set_parent_of(') < configure.start()
    checks.append('actual exported-parent import and foreign set_parent_of precede initial configure')
    connection.signal_unsubscribe(subscription)
    stranger.close_sync(None)
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
    with tempfile.TemporaryDirectory(prefix='canopa-launcher-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        try:
            checks = verify(args.binary.resolve(), session, args.color_scheme, args.scale, args.shipped_descriptor)
            desktop.write_json(args.output / 'result.json', dict(passed=True, checks=checks, scheme=args.color_scheme,
                scale=args.scale, shipped_descriptor=args.shipped_descriptor,
                binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest()))
        finally:
            for process in reversed(session.processes):
                desktop.stop(process)
    print(f'DynamicLauncher: {len(checks)} native checks passed')


if __name__ == '__main__':
    main()
