#!/usr/bin/env python3
"""Verify Wallpaper through a private frontend, Iced review and actual Shell owner."""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import ssl
import subprocess
import sys
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
BACKEND = 'org.freedesktop.impl.portal.desktop.canopa'
FRONTEND = 'org.freedesktop.portal.Desktop'
PATH = '/org/freedesktop/portal/desktop'
IMPL = 'org.freedesktop.impl.portal.Wallpaper'
PUBLIC = 'org.freedesktop.portal.Wallpaper'
OWNER = 'org.canopa.Shell.Wallpaper'


def certificates(runtime):
    folder = runtime / 'tls'
    folder.mkdir()
    def run(*args):
        subprocess.run(['openssl', *map(str, args)], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
    run('req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', folder / 'ca.key',
        '-out', folder / 'ca.pem', '-subj', '/CN=Canopa private fixture CA', '-days', '1')
    run('req', '-new', '-newkey', 'rsa:2048', '-nodes', '-keyout', folder / 'server.key',
        '-out', folder / 'server.csr', '-subj', '/CN=localhost')
    (folder / 'extensions.cnf').write_text('subjectAltName=DNS:localhost,IP:127.0.0.1\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n')
    run('x509', '-req', '-in', folder / 'server.csr', '-CA', folder / 'ca.pem', '-CAkey', folder / 'ca.key',
        '-CAcreateserial', '-out', folder / 'server.pem', '-days', '1', '-extfile', folder / 'extensions.cnf')
    run('req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', folder / 'untrusted.key',
        '-out', folder / 'untrusted.pem', '-subj', '/CN=localhost', '-days', '1',
        '-addext', 'subjectAltName=DNS:localhost,IP:127.0.0.1')
    return folder


def verify(binary, shell_binary, session, scheme, scale, shipped):
    from gi.repository import Gio, GLib
    from PIL import Image, ImageChops
    home = session.runtime / 'home'
    home.mkdir()
    tls = certificates(session.runtime)
    config = session.runtime / 'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n')
    applications = Path(session.env['XDG_DATA_HOME']) / 'applications'
    applications.mkdir(parents=True)
    (applications / 'org.canopa.WallpaperTest.desktop').write_text('[Desktop Entry]\nType=Application\nName=Wallpaper Fixture\nExec=true\n')
    bins = session.runtime / 'bin'
    bins.mkdir()
    locker = bins / 'swaylock'
    locker.write_text('#!/usr/bin/python3\nimport hashlib,json,sys\n' +
        'print(json.dumps({"args":sys.argv[1:],"hash":hashlib.sha256(open(sys.argv[2],"rb").read()).hexdigest() if len(sys.argv)>1 else None}))\n')
    locker.chmod(0o700)
    services, portals = session.runtime / 'services', session.runtime / 'portals'
    services.mkdir()
    portals.mkdir()
    shutil.copy(ROOT / 'packaging/portal/org.freedesktop.impl.portal.desktop.canopa.service', services)
    descriptor = (ROOT / 'packaging/portal/canopa.portal').read_text()
    if not shipped and IMPL + ';' not in descriptor:
        descriptor = descriptor.replace('Interfaces=', 'Interfaces=' + IMPL + ';')
    assert IMPL + ';' in descriptor
    (portals / 'canopa.portal').write_text(descriptor)
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\n' + IMPL + '=canopa\norg.freedesktop.impl.portal.Settings=canopa\norg.freedesktop.impl.portal.Access=canopa\n')
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home), WAYLAND_DEBUG='1', XDG_CURRENT_DESKTOP='Canopa', SSL_CERT_FILE=str(tls / 'ca.pem'),
        XDG_DESKTOP_PORTAL_DIR=str(portals), XDG_DATA_DIRS=session.env['XDG_DATA_HOME'],
        GIO_USE_VFS='local', GSETTINGS_BACKEND='memory', NO_PROXY='*', no_proxy='*',
        GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas',
        PATH=str(bins) + os.pathsep + str(binary.parent) + os.pathsep + session.env['PATH'])
    output = session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
        'hotkey-overlay { skip-at-startup; }\nlayout { gaps 8; border { off; }; focus-ring { off; }; }\n',
        width=round(1280 * scale), height=round(800 * scale), scale=scale, service_directories=[services])
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    flags = Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION

    def connect():
        return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], flags, None, None)
    connection = connect()

    def call(destination, path, interface, method, signature, values, client=connection):
        return client.call_sync(destination, path, interface, method, GLib.Variant(signature, values),
            None, Gio.DBusCallFlags.NONE, 15000, None).unpack()

    def bus(method, signature, values):
        return call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', method, signature, values)

    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)

    def start_shell(log):
        # Keep locker discovery under fixture control; other test commands keep
        # their ordinary PATH and never launch the host desktop's locker.
        previous = session.env['PATH']
        try:
            session.env['PATH'] = str(bins)
            return session.start([str(shell_binary)], log)
        finally:
            session.env['PATH'] = previous
    shell = start_shell('shell-01.log')
    session.wait_for(lambda: bus('NameHasOwner', '(s)', (OWNER,))[0], 'Shell wallpaper owner', timeout=45)
    bus('UpdateActivationEnvironment', '(a{ss})', ({k: v for k, v in session.env.items()},))
    frontend = next(p for p in [Path('/usr/libexec/xdg-desktop-portal'), Path('/usr/lib/xdg-desktop-portal')] if p.is_file())
    session.start([str(frontend), '--verbose'], 'frontend.log')
    bus('StartServiceByName', '(su)', (BACKEND, 0))
    backend_pid = bus('GetConnectionUnixProcessID', '(s)', (BACKEND,))[0]
    assert Path(f'/proc/{backend_pid}/exe').resolve() == binary
    session.wait_for(lambda: bus('NameHasOwner', '(s)', (FRONTEND,))[0], 'public frontend')
    call(FRONTEND, PATH, 'org.freedesktop.host.portal.Registry', 'Register', '(sa{sv})', ('org.canopa.WallpaperTest', {}))
    xml = call(BACKEND, PATH, 'org.freedesktop.DBus.Introspectable', 'Introspect', '()', ())[0]
    interface = xml.split(f'<interface name="{IMPL}">')[1].split('</interface>')[0]
    assert '<property' not in interface and 'name="response" type="u" direction="out"' in interface

    protocols = Path('/usr/share/wayland-protocols')
    for name, path in [('xdg-shell', protocols / 'stable/xdg-shell/xdg-shell.xml'),
                       ('xdg-foreign', protocols / 'unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', mode, str(path), str(session.output / (name + suffix))], check=True, timeout=15)
    parent_binary = session.output / 'portal-parent'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output), str(ROOT / 'scripts/desktop-fixture/portal-parent.c'),
        str(session.output / 'xdg-shell.c'), str(session.output / 'xdg-foreign.c'), '-lwayland-client', '-o', str(parent_binary)], check=True, timeout=30)
    parent = session.start([str(parent_binary)], 'parent.log')
    exported = session.wait_for(lambda: next((line for line in (session.output / 'parent.log').read_text().splitlines() if line.startswith('wayland:')), None), 'exported parent')
    pointer = session.build_pointer()
    logical = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))[output]['logical']
    width, height = logical['width'], logical['height']
    responses, pending, checks = {}, {}, []
    subscription = connection.signal_subscribe(FRONTEND, 'org.freedesktop.portal.Request', 'Response', None, None,
        Gio.DBusSignalFlags.NONE, lambda c, s, p, i, n, v: responses.__setitem__(p, v))
    count = 0
    state = Path(session.env['XDG_STATE_HOME']) / 'canopa/shell/wallpaper'
    source = session.runtime / 'caller.png'
    red, blue, green = (180, 20, 40), (10, 40, 190), (10, 140, 60)

    def image(color):
        Image.new('RGB', (2048, 1024), color).save(source)

    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))

    def done(client, result, path):
        try:
            pending[path] = client.call_finish(result)
        except GLib.Error as error:
            pending[path] = error

    def begin(public=True, uri=None, target='both', preview=True, parent_window='', client=connection, show=True):
        nonlocal count
        count += 1
        before = {w['id'] for w in windows()}
        options = {'show-preview': GLib.Variant('b', preview)}
        if target is not None:
            options['set-on'] = GLib.Variant('s', target)
        if public:
            options['handle_token'] = GLib.Variant('s', f'wallpaper{count}')
            if uri is None:
                descriptor = os.open(source, os.O_RDONLY)
                descriptors = Gio.UnixFDList.new()
                try:
                    index = descriptors.append(descriptor)
                    reply, _ = client.call_with_unix_fd_list_sync(FRONTEND, PATH, PUBLIC, 'SetWallpaperFile',
                        GLib.Variant('(sha{sv})', (parent_window, index, options)), None, Gio.DBusCallFlags.NONE, 15000, descriptors, None)
                finally:
                    os.close(descriptor)
                path = reply.unpack()[0]
            else:
                path = call(FRONTEND, PATH, PUBLIC, 'SetWallpaperURI', '(ssa{sv})', (parent_window, uri, options), client)[0]
        else:
            path = f'/org/freedesktop/portal/desktop/request/test/wallpaper{count}'
            client.call(BACKEND, PATH, IMPL, 'SetWallpaperURI', GLib.Variant('(osssa{sv})',
                (path, 'org.canopa.WallpaperTest', parent_window, uri or source.as_uri(), options)),
                None, Gio.DBusCallFlags.NONE, 60000, None, lambda c, r: done(c, r, path))
        if not show:
            return path, None
        window = session.wait_for(lambda: (drain(), next((w for w in windows() if w['app_id'] == 'org.canopa.Portal'
            and w['id'] not in before and 'Set wallpaper' in w['title']), None))[1], 'Iced wallpaper review', timeout=30)
        if parent_window:
            assert window['is_floating'], window
        if not window['is_floating']:
            # An independent portal window may legitimately tile. Arrange it
            # explicitly so pointer assertions use the same review geometry.
            session.run(['niri', 'msg', 'action', 'move-window-to-floating', '--id', str(window['id'])])
            session.run(['niri', 'msg', 'action', 'set-window-width', '--id', str(window['id']), '700'])
            session.run(['niri', 'msg', 'action', 'set-window-height', '--id', str(window['id']), '600'])
            window = session.wait_for(lambda: next((w for w in windows() if w['id'] == window['id']
                and abs(w['layout']['window_size'][0] - 700) <= 1
                and abs(w['layout']['window_size'][1] - 600) <= 1), None), 'independent review geometry')
        assert abs(window['layout']['window_size'][0] - 700) <= 1, window
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        session.run(['niri', 'msg', 'action', 'move-floating-window', '--id', str(window['id']), '--x', '20', '--y', '10'])
        time.sleep(.2)
        return path, window

    def action(window, accept):
        w, h = window['layout']['window_size']
        session.run([str(pointer), str(round((20 + w * (.75 if accept else .25)) / width * 1600)),
            str(round((10 + h - 36) / height * 1000))])

    def result(path, status, public=True):
        response = session.wait_for(lambda: (drain(), (responses if public else pending).get(path))[1], 'wallpaper response', timeout=45)
        assert isinstance(response, GLib.Variant), response
        assert response.get_child_value(0).unpack() == status, response
        return response

    def closed(window):
        session.wait_for(lambda: not any(w['id'] == window['id'] for w in windows()), 'review closes')

    def selection():
        return json.loads((state / 'selection.json').read_text())

    def painted(name, color):
        def capture():
            frame = session.capture(name)
            return frame if frame.getpixel((frame.width - 3, frame.height - 3)) == color else None
        session.wait_for(capture, 'applied desktop pixels')

    image(red)
    path, window = begin(parent_window=exported)
    assert not state.exists()
    def preview_ready():
        frame = session.capture('01-parented-review.png')
        difference = ImageChops.difference(frame, Image.new('RGB', frame.size, red))
        channels = difference.split()
        distance = ImageChops.lighter(ImageChops.lighter(channels[0], channels[1]), channels[2])
        box = distance.point(lambda value: 255 if value <= 2 else 0).getbbox()
        if not box:
            return False
        assert abs((box[2] - box[0]) / (box[3] - box[1]) - 2) < .03, box
        return True
    session.wait_for(preview_ready, 'aspect-preserving preview pixels')
    action(window, False)
    result(path, 1)
    closed(window)
    assert not state.exists()
    session.retire(parent)
    checks.append('real public FD input creates a parented Iced review; Cancel installs nothing')

    image(red)
    path, window = begin()
    source.unlink()
    action(window, True)
    result(path, 0)
    closed(window)
    first = selection()
    assert first['background'] == first['lockscreen']
    assert Image.open(state / (first['background'] + '.png')).size == (2048, 1024)
    painted('02-frozen-both.png', red)
    assert json.loads(session.run([str(shell_binary), 'lock']))['hash'] == first['lockscreen']
    checks.append('Apply uses the reviewed frozen image after caller deletion for actual desktop and locker')

    image(blue)
    path, window = begin(target='lockscreen')
    session.capture('03-lock-review.png')
    action(window, True)
    result(path, 0)
    closed(window)
    locked = selection()
    assert locked['background'] == first['background'] and locked['lockscreen'] != first['lockscreen']
    assert json.loads(session.run([str(shell_binary), 'lock']))['hash'] == locked['lockscreen']
    painted('03-lock-only.png', red)
    image(green)
    path, window = begin(target=None)
    action(window, True)
    result(path, 0)
    closed(window)
    desktop_only = selection()
    assert desktop_only['lockscreen'] == locked['lockscreen']
    painted('04-default-background.png', green)
    checks.append('explicit lockscreen and omitted-destination desktop policy preserve the other selection')

    original = (state / 'selection.json').read_bytes()
    path, window = begin(public=False, preview=False)
    call(BACKEND, path, 'org.freedesktop.impl.portal.Request', 'Close', '()', ())
    result(path, 1, False)
    closed(window)
    assert (state / 'selection.json').read_bytes() == original
    path, window = begin()
    call(FRONTEND, path, 'org.freedesktop.portal.Request', 'Close', '()', ())
    closed(window)
    assert path not in responses or responses[path].get_child_value(0).unpack() != 0
    stranger = connect()
    path, window = begin(public=False, client=stranger)
    stranger.close_sync(None)
    closed(window)
    assert (state / 'selection.json').read_bytes() == original
    checks.append('backend Close, public Close and caller loss cannot install; preview remains explicit when false')

    encoded = io.BytesIO()
    Image.new('RGB', (1600, 900), red).save(encoded, format='PNG')
    body, requested = [encoded.getvalue()], []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_GET(self):
            requested.append(self.path)
            if self.path == '/oversize':
                self.send_response(200)
                self.send_header('Content-Length', str(32 * 1024 * 1024 + 1))
                self.end_headers()
                return
            self.send_response(200)
            self.send_header('Content-Length', str(len(body[0])))
            self.end_headers()
            self.wfile.write(body[0])

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        uri = f'http://127.0.0.1:{server.server_address[1]}/image'
        path, window = begin(uri=uri, target='background')
        body[0] = b'changed after preview'
        session.capture('05-remote-review.png')
        action(window, True)
        result(path, 0)
        closed(window)
        assert requested == ['/image'], requested
        painted('06-remote-frozen.png', red)
        path, _ = begin(uri=uri.replace('/image', '/oversize'), show=False)
        result(path, 2)
        checks.append('remote public URI is fetched once before review; oversized HTTP input fails')
    finally:
        server.shutdown()
        server.server_close()

    encoded = io.BytesIO()
    Image.new('RGB', (1600, 900), green).save(encoded, format='PNG')
    body[0] = encoded.getvalue()
    for trusted in [True, False]:
        secure = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(tls / ('server.pem' if trusted else 'untrusted.pem'),
            tls / ('server.key' if trusted else 'untrusted.key'))
        secure.socket = context.wrap_socket(secure.socket, server_side=True)
        threading.Thread(target=secure.serve_forever, daemon=True).start()
        try:
            previous = (state / 'selection.json').read_bytes()
            uri = f'https://127.0.0.1:{secure.server_address[1]}/secure'
            path, window = begin(uri=uri, target='background', show=trusted)
            if trusted:
                action(window, True)
                result(path, 0)
                closed(window)
                painted('06-trusted-https.png', green)
            else:
                result(path, 2)
                assert (state / 'selection.json').read_bytes() == previous
        finally:
            secure.shutdown()
            secure.server_close()
    checks.append('HTTPS works with the private trusted CA and rejects an untrusted certificate without publication')

    previous = (state / 'selection.json').read_bytes()
    disabled = locker.with_suffix('.disabled')
    locker.rename(disabled)
    try:
        assert call(OWNER, '/org/canopa/Shell/Wallpaper', 'org.freedesktop.DBus.Properties', 'Get',
            '(ss)', ('org.canopa.Shell.Wallpaper1', 'Destinations')) == (1,)
        path, _ = begin(show=False)
        result(path, 2)
        assert (state / 'selection.json').read_bytes() == previous
    finally:
        disabled.rename(locker)
    checks.append('both fails without changing either selection when the configured locker capability is unavailable')

    original = (state / 'selection.json').read_bytes()
    source.write_bytes(b'not an image')
    path, _ = begin(show=False)
    result(path, 2)
    image(blue)
    path, window = begin()
    session.retire(shell)
    session.wait_for(lambda: not bus('NameHasOwner', '(s)', (OWNER,))[0], 'owner gone')
    action(window, True)
    result(path, 2)
    closed(window)
    assert (state / 'selection.json').read_bytes() == original
    path, _ = begin(show=False)
    result(path, 2)
    checks.append('malformed source, lost owner after review and absent owner fail without publication')

    shell = start_shell('shell-02.log')
    session.wait_for(lambda: bus('NameHasOwner', '(s)', (OWNER,))[0], 'restarted owner', timeout=45)
    session.run(['niri', 'msg', 'output', output, 'scale', '1.5'])
    painted('07-restart-scale.png', green)
    assert selection()['lockscreen'] == locked['lockscreen']
    checks.append('accepted desktop and lockscreen persist across owner restart and output scaling')
    trace = (session.output / 'dbus.log').read_text()
    assert '.import_toplevel(' in trace and '.set_parent_of(' in trace
    checks.append('foreign-parent import and compositor relationship use the actual Wayland adapter')
    connection.signal_unsubscribe(subscription)
    connection.close_sync(None)
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--shell-binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--color-scheme', choices=['dark', 'light'], default='dark')
    parser.add_argument('--scale', type=float, default=1.0)
    parser.add_argument('--shipped-descriptor', action='store_true')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-wallpaper-portal-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        try:
            checks = verify(args.binary.resolve(), args.shell_binary.resolve(), session, args.color_scheme, args.scale, args.shipped_descriptor)
            desktop.write_json(args.output / 'result.json', dict(passed=True, checks=checks, scheme=args.color_scheme,
                scale=args.scale, shipped_descriptor=args.shipped_descriptor,
                binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                shell_sha256=hashlib.sha256(args.shell_binary.read_bytes()).hexdigest()))
        finally:
            for process in reversed(session.processes):
                desktop.stop(process)
    print(f'Wallpaper portal: {len(checks)} native checks passed')


if __name__ == '__main__':
    main()
