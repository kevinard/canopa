#!/usr/bin/env python3
"""Exercise Account sharing and its embedded picker in a private Niri session."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import time
from urllib.parse import unquote_to_bytes, urlsplit
import zlib

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def png(color):
    def chunk(kind, data):
        return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!IIBBBBB', 32, 32, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress((b'\0' + bytes(color) * 32) * 32)) + chunk(b'IEND', b''))


def verify(binary, session, scheme, scale):
    from gi.repository import Gio, GLib
    from PIL import Image
    home = session.runtime / 'home'
    pictures = home / 'Pictures'
    pictures.mkdir(parents=True)
    profile = home / '.face'
    source = png((25, 105, 200, 255))
    profile.write_bytes(source + b'private source metadata')
    (pictures / 'new.png').write_bytes(png((190, 65, 120, 255)))
    config = session.runtime / 'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n')
    (config.parent.parent / 'user-dirs.dirs').write_text('XDG_PICTURES_DIR="$HOME/Pictures"\n')
    data = Path(session.env['XDG_DATA_HOME'])
    (data / 'applications').mkdir(parents=True)
    (data / 'applications/org.canopa.PortalTest.desktop').write_text('[Desktop Entry]\nType=Application\nName=Profile Fixture\nExec=true\n')
    (data / 'mime').mkdir()
    shutil.copy('/usr/share/mime/globs2', data / 'mime/globs2')
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home), XDG_DATA_DIRS=str(data), WAYLAND_DEBUG='1',
                       GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas')
    portals = session.runtime / 'portals'
    portals.mkdir()
    descriptor = (ROOT / 'packaging/portal/canopa.portal').read_text()
    # Permit development before the real descriptor advertises this interface.
    # Final acceptance must use --shipped-descriptor.
    if not session.account_shipped:
        descriptor = descriptor.replace('Interfaces=', 'Interfaces=org.freedesktop.impl.portal.Account;')
    assert 'org.freedesktop.impl.portal.Account;' in descriptor
    (portals / 'canopa.portal').write_text(descriptor)
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\norg.freedesktop.impl.portal.Settings=canopa\norg.freedesktop.impl.portal.Account=canopa\n')
    services = session.runtime / 'services'
    services.mkdir()
    shutil.copy(ROOT / 'packaging/portal/org.freedesktop.impl.portal.desktop.canopa.service', services)
    session.env.update(PATH=str(binary.parent) + os.pathsep + session.env['PATH'], XDG_DESKTOP_PORTAL_DIR=str(portals), XDG_CURRENT_DESKTOP='Canopa')
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; focus-ring { off; }; }\n',
                       width=round(1280 * scale), height=round(800 * scale), scale=scale, service_directories=[services])
    session.env['DBUS_SYSTEM_BUS_ADDRESS'] = session.env['DBUS_SESSION_BUS_ADDRESS']
    outputs = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))
    logical = next(output['logical'] for output in outputs.values() if output.get('logical'))
    screen_width, screen_height = logical['width'], logical['height']
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    flags = Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION
    def connect():
        return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], flags, None, None)
    connection = connect()
    def call(destination, path, interface, method, signature, values, client=connection):
        return client.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()
    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)

    # Only fixture identity can enter captures; never contact the host system bus.
    xml = '<node><interface name="org.freedesktop.Accounts.User"><property name="UserName" type="s" access="read"/><property name="RealName" type="s" access="read"/><property name="IconFile" type="s" access="read"/></interface></node>'
    node = Gio.DBusNodeInfo.new_for_xml(xml)
    reads = []
    def property_value(conn, sender, path, interface, name):
        reads.append(name)
        return GLib.Variant('s', {'UserName': 'fixture-user', 'RealName': 'Fixture Name', 'IconFile': str(profile)}[name])
    registration = connection.register_object(f'/org/freedesktop/Accounts/User{os.geteuid()}', node.interfaces[0], None, property_value, None)
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'RequestName', '(su)', ('org.freedesktop.Accounts', 0))
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'UpdateActivationEnvironment', '(a{ss})', ({k: v for k, v in session.env.items()},))
    frontend = next(p for p in [Path('/usr/libexec/xdg-desktop-portal'), Path('/usr/lib/xdg-desktop-portal')] if p.is_file())
    session.start([str(frontend), '--verbose'], 'frontend.log')
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'StartServiceByName', '(su)', ('org.freedesktop.impl.portal.desktop.canopa', 0))
    backend_pid = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', '(s)', ('org.freedesktop.impl.portal.desktop.canopa',))[0]
    assert Path(f'/proc/{backend_pid}/exe').resolve() == binary
    protocols = Path('/usr/share/wayland-protocols')
    for name, xml_path in [('xdg-shell', protocols / 'stable/xdg-shell/xdg-shell.xml'), ('xdg-foreign', protocols / 'unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for kind, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', kind, str(xml_path), str(session.output / (name + suffix))], check=True)
    parent = session.output / 'portal-parent'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output), str(ROOT / 'scripts/desktop-fixture/portal-parent.c'), str(session.output / 'xdg-shell.c'), str(session.output / 'xdg-foreign.c'), '-lwayland-client', '-o', str(parent)], check=True)
    session.start([str(parent)], 'parent.log')
    exported = session.wait_for(lambda: next((line for line in (session.output / 'parent.log').read_text().splitlines() if line.startswith('wayland:')), None), 'exported parent')
    pointer = session.build_pointer()
    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
    def focus(window):
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        time.sleep(.5)
        session.run(['niri', 'msg', 'action', 'move-floating-window', '--id', str(window['id']), '--x', '20', '--y', '10'])
        time.sleep(.2)
    def click(x, y):
        session.run([str(pointer), str(round(x / screen_width * 1600)), str(round(y / screen_height * 1000))])
        time.sleep(.2)
    def keys(key):
        session.run(['wtype', '-k', key])
        time.sleep(.15)
    def edit(x, y, value):
        click(x, y)
        session.run(['wtype', '-M', 'ctrl', '-k', 'a', '-m', 'ctrl', value])
        time.sleep(.15)
    def closed(window):
        session.wait_for(lambda: not any(w['id'] == window['id'] for w in windows()), 'Account helper closes')
    def action(window, share):
        width, height = window['layout']['window_size']
        click(20 + width * (.75 if share else .25), 10 + height - 36)

    checks, pending, responses = [], {}, {}
    subscription = connection.signal_subscribe('org.freedesktop.portal.Desktop', 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE,
        lambda conn, sender, path, interface, name, params: responses.__setitem__(path, params.unpack()))
    count = 0
    def begin(public=False, client=connection):
        nonlocal count
        count += 1
        before = {w['id'] for w in windows()}
        prior_reads = len(reads)
        options = {'reason': GLib.Variant('s', 'Review the information shared with this fixture.')}
        if public:
            options['handle_token'] = GLib.Variant('s', f'account{count}')
            path = call('org.freedesktop.portal.Desktop', '/org/freedesktop/portal/desktop', 'org.freedesktop.portal.Account', 'GetUserInformation', '(sa{sv})', (exported, options))[0]
        else:
            path = f'/org/freedesktop/portal/desktop/request/test/account{count}'
            def done(conn, result):
                try:
                    pending[path] = conn.call_finish(result).unpack()
                except GLib.Error as error:
                    pending[path] = error
            client.call('org.freedesktop.impl.portal.desktop.canopa', '/org/freedesktop/portal/desktop', 'org.freedesktop.impl.portal.Account', 'GetUserInformation', GLib.Variant('(ossa{sv})', (path, 'org.canopa.PortalTest', exported, options)), None, Gio.DBusCallFlags.NONE, 30000, None, done)
        window = session.wait_for(lambda: (drain(), next((w for w in windows() if w['app_id'] == 'org.canopa.Portal' and w['id'] not in before), None))[1], 'Account window')
        session.wait_for(lambda: (drain(), len(reads) >= prior_reads + 3)[1], 'fixture profile query')
        assert window['is_floating'] and abs(window['layout']['window_size'][0] - 680) <= 1, window
        focus(window)
        return path, window
    def result(path, status, public=False):
        response = session.wait_for(lambda: (drain(), (responses if public else pending).get(path))[1], 'Account response')
        assert response[0] == status, response
        return response[1]

    path, window = begin()
    keys('Return')
    drain()
    assert path not in pending
    session.capture('01-profile.png')
    action(window, False)
    assert result(path, 1) == {}
    closed(window)
    checks.append('fixture-only profile loads; initial Enter does not share; explicit Cancel returns no fields')

    path, window = begin()
    edit(180, 390, 'reviewed-user')
    edit(180, 468, 'Reviewed Name')
    # Returning from the same-window picker must keep edited fields and choices.
    click(225, 225)
    time.sleep(.6)
    assert len([w for w in windows() if w['app_id'] == 'org.canopa.Portal']) == 1
    session.capture('02-embedded-picker.png')
    keys('Escape')
    time.sleep(.3)
    session.capture('03-returned-draft.png')
    profile.unlink()
    action(window, True)
    fields = result(path, 0)
    assert fields['id'] == 'reviewed-user' and fields['name'] == 'Reviewed Name', fields
    image = Path(os.fsdecode(unquote_to_bytes(urlsplit(fields['image']).path)))
    assert image.is_file() and image.stat().st_mode & 0o777 == 0o600
    shared = image.read_bytes()
    assert shared.startswith(b'\x89PNG') and b'private source metadata' not in shared
    with Image.open(image) as decoded:
        assert decoded.size == (32, 32) and decoded.convert('RGBA').getpixel((0, 0)) == (25, 105, 200, 255)
    (session.output / 'shared-profile.png').write_bytes(shared)
    closed(window)
    assert image.read_bytes() == shared
    profile.write_bytes(source)
    checks.append('editable draft survives embedded picker Cancel; reviewed PNG survives source removal and helper exit')

    path, window = begin()
    click(225, 225)
    time.sleep(.5)
    session.run(['wtype', '-M', 'ctrl', '-k', 'l', '-k', 'a', '-m', 'ctrl', str(pictures), '-k', 'Return'])
    time.sleep(.5)
    # The filtered picker contains just new.png. Its explicit Open selects it.
    click(655, 185)
    time.sleep(.7)
    session.capture('04-new-avatar.png')
    action(window, True)
    changed = result(path, 0)
    assert changed['image'] != fields['image'], changed
    changed_image = Path(os.fsdecode(unquote_to_bytes(urlsplit(changed['image']).path)))
    with Image.open(changed_image) as decoded:
        assert decoded.size == (32, 32) and decoded.convert('RGBA').getpixel((0, 0)) == (190, 65, 120, 255)
    (session.output / 'shared-chosen.png').write_bytes(changed_image.read_bytes())
    closed(window)
    checks.append('embedded image selection returns to the same draft and publishes the chosen pixels')

    path, window = begin()
    click(48, 320)
    click(48, 355)
    click(48, 435)
    session.capture('05-excluded-fields.png')
    action(window, True)
    assert result(path, 0) == {'id': '', 'name': '', 'image': ''}
    closed(window)
    checks.append('explicit field exclusions return complete empty-string result shape')

    path, window = begin()
    click(225, 275)
    action(window, True)
    cleared = result(path, 0)
    assert cleared == {'id': 'fixture-user', 'name': 'Fixture Name', 'image': ''}, cleared
    assert profile.read_bytes() == source
    closed(window)
    checks.append('Clear removes only the shared image draft and leaves the local profile source unchanged')

    path, window = begin()
    click(225, 225)
    time.sleep(.4)
    session.run(['niri', 'msg', 'action', 'close-window', '--id', str(window['id'])])
    assert result(path, 1) == {}
    closed(window)
    checks.append('closing the owning window from its embedded picker cancels the Account request')

    path, window = begin()
    stranger = connect()
    try:
        call('org.freedesktop.impl.portal.desktop.canopa', path, 'org.freedesktop.impl.portal.Request', 'Close', '()', (), client=stranger)
        raise AssertionError('stranger closed an owned request')
    except GLib.Error as error:
        assert 'AccessDenied' in str(error), error
    finally:
        stranger.close_sync(None)
    call('org.freedesktop.impl.portal.desktop.canopa', path, 'org.freedesktop.impl.portal.Request', 'Close', '()', ())
    assert result(path, 1) == {}
    closed(window)
    checks.append('only the owning caller can Close its request')

    first, first_window = begin()
    edit(180, 468, 'First draft')
    second, second_window = begin()
    edit(180, 468, 'Second draft')
    assert call('org.freedesktop.impl.portal.desktop.canopa', '/org/freedesktop/portal/desktop', 'org.freedesktop.impl.portal.Settings', 'ReadAll', '(as)', ([],))[0]
    action(second_window, True)
    assert result(second, 0)['name'] == 'Second draft'
    closed(second_window)
    focus(first_window)
    action(first_window, True)
    assert result(first, 0)['name'] == 'First draft'
    closed(first_window)
    checks.append('concurrent requests retain independent edits and Settings stays responsive')

    lost = connect()
    _, window = begin(client=lost)
    lost.close_sync(None)
    closed(window)
    checks.append('caller loss destroys its owned helper')

    path, window = begin(public=True)
    session.capture('06-public-frontend.png')
    action(window, True)
    public_fields = result(path, 0, public=True)
    assert public_fields['id'] == 'fixture-user' and public_fields['name'] == 'Fixture Name', public_fields
    assert Path(os.fsdecode(unquote_to_bytes(urlsplit(public_fields['image']).path))).is_file()
    closed(window)
    checks.append('public Account frontend receives explicit profile and readable local image URI; sandbox export unverified')
    connection.signal_unsubscribe(subscription)
    connection.unregister_object(registration)
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
    with tempfile.TemporaryDirectory(prefix='canopa-account-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        session.account_shipped = args.shipped_descriptor
        try:
            checks = verify(args.binary.resolve(), session, args.color_scheme, args.scale)
            desktop.write_json(args.output / 'result.json', dict(passed=True, checks=checks, scheme=args.color_scheme, scale=args.scale, shipped_descriptor=args.shipped_descriptor))
        finally:
            for process in reversed(session.processes):
                desktop.stop(process)
    print(f'Account: {len(checks)} native checks passed')


if __name__ == '__main__':
    main()
