#!/usr/bin/env python3
"""Exercise the installed Background frontend with disposable bubblewrap instances.

The instance records, permission store, bus, compositor and processes all belong
to this fixture. This is synthetic Flatpak metadata, not a real Flatpak install.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
PATH = '/org/freedesktop/portal/desktop'
BACKEND = 'org.freedesktop.impl.portal.desktop.canopa'
NOTIFY = 'org.freedesktop.impl.portal.desktop.canopa.notify'
STORE = 'org.freedesktop.impl.portal.PermissionStore'
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def verify(binary, notify, session, clients, scheme, scale, shipped):
    from gi.repository import Gio, GLib
    from PIL import ImageChops
    home = session.runtime / 'home'
    home.mkdir()
    config = session.runtime / 'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\nnotifications {{ sound enabled=#false; }}\n')
    services, portals = session.runtime / 'services', session.runtime / 'portals'
    services.mkdir()
    portals.mkdir()
    for name in ['org.freedesktop.impl.portal.desktop.canopa.service', 'org.freedesktop.impl.portal.desktop.canopa.notify.service']:
        shutil.copy(ROOT / 'packaging/portal' / name, services)
    shutil.copy(Path('/usr/share/dbus-1/services') / (STORE + '.service'), services)
    descriptor = (ROOT / 'packaging/portal/canopa.portal').read_text()
    if not shipped and 'org.freedesktop.impl.portal.Background;' not in descriptor:
        descriptor = descriptor.replace('Interfaces=', 'Interfaces=org.freedesktop.impl.portal.Background;')
    assert 'org.freedesktop.impl.portal.Background;' in descriptor
    (portals / 'canopa.portal').write_text(descriptor)
    shutil.copy(ROOT / 'packaging/portal/canopa-notify.portal', portals)
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\norg.freedesktop.impl.portal.Background=canopa\norg.freedesktop.impl.portal.Access=canopa\norg.freedesktop.impl.portal.Settings=canopa\n')
    instances = session.runtime / '.flatpak'
    instances.mkdir()
    session.env.update(HOME=str(home), WAYLAND_DEBUG='1', XDG_CURRENT_DESKTOP='Canopa', XDG_DESKTOP_PORTAL_DIR=str(portals),
                       PATH=str(binary.parent) + os.pathsep + str(notify.parent) + os.pathsep + session.env['PATH'])
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n',
                       width=round(1280 * scale), height=round(800 * scale), scale=scale, service_directories=[services])
    outputs = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))
    logical = next(item['logical'] for item in outputs.values() if item.get('logical'))
    width, height = logical['width'], logical['height']
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    def call(destination, path, interface, method, signature, values):
        timeout = 15000 if method == 'StartServiceByName' else 5000
        return connection.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, timeout, None).unpack()
    def bus(method, signature, values):
        return call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', method, signature, values)
    bus('UpdateActivationEnvironment', '(a{ss})', ({k: v for k, v in session.env.items()},))
    frontend = next(p for p in [Path('/usr/libexec/xdg-desktop-portal'), Path('/usr/lib/xdg-desktop-portal')] if p.is_file())
    session.start([str(frontend), '--verbose'], 'frontend.log')
    for name, executable in [(BACKEND, binary), (NOTIFY, notify)]:
        bus('StartServiceByName', '(su)', (name, 0))
        pid = bus('GetConnectionUnixProcessID', '(s)', (name,))[0]
        assert Path(f'/proc/{pid}/exe').resolve() == executable
    session.wait_for(lambda: bus('NameHasOwner', '(s)', ('org.freedesktop.portal.Desktop',))[0], 'public frontend')
    xml = call('org.freedesktop.portal.Desktop', PATH, 'org.freedesktop.DBus.Introspectable', 'Introspect', '()', ())[0]
    assert 'org.freedesktop.portal.Background' in xml
    session.wait_for(lambda: call(BACKEND, PATH, 'org.freedesktop.impl.portal.Background', 'GetAppState', '()', ()) == ({},), 'complete empty native observation')
    control = session.runtime / 'canopa-notify/control.sock'
    session.wait_for(control.exists, 'Notify control socket')
    def active():
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(5)
            client.connect(str(control))
            client.sendall(b'control {"command":"notification-center-projection"}\n')
            with client.makefile('rb') as stream:
                reply = json.loads(stream.readline(8 * 1024 * 1024))
        assert reply['reply'] == 'notification-center-projection', reply
        return [item for group in reply['value']['groups'] for item in group['items'] if item['status'] == 'active']
    def permission(app):
        try:
            permissions, _ = call(STORE, '/org/freedesktop/impl/portal/PermissionStore', STORE, 'Lookup', '(ss)', ('background', 'background'))
            return permissions.get(app)
        except GLib.Error as error:
            if 'NotFound' not in str(error):
                raise
            return None
    def set_permission(app, value):
        call(STORE, '/org/freedesktop/impl/portal/PermissionStore', STORE, 'SetPermission', '(sbssas)', ('background', True, 'background', app, [value]))
    pointer = session.build_pointer()
    def click(x, y):
        session.run([str(pointer), str(round(x / width * 1600)), str(round(y / height * 1000))])
        time.sleep(.15)
    baseline = session.capture('baseline.png')
    def choose(decision, name):
        session.wait_for(lambda: active(), 'frontend-generated background question', timeout=25)
        def drawn():
            area = ImageChops.difference(baseline, session.capture(name)).getbbox()
            return area if area and area[2] - area[0] > 100 and area[3] - area[1] > 40 else None
        left, top, right, bottom = (value / scale for value in session.wait_for(drawn, 'drawn frontend question'))
        if decision == 1:
            click((left + right) / 2, bottom - 44)
        else:
            click(left + (right - left) * (.25 if decision == 0 else .75), bottom - 82)
        session.wait_for(lambda: not active(), 'frontend question withdrawal')
    def start(app, name, program=None):
        metadata = f'[Application]\nname={app}\n[Instance]\ninstance-id={name}\n'
        info = session.runtime / (name + '.info')
        info.write_text(metadata)
        command = ['bwrap', '--unshare-user', '--uid', str(os.getuid()), '--gid', str(os.getgid()), '--die-with-parent', '--tmpfs', '/', '--ro-bind', '/usr', '/usr']
        for library in ['/lib', '/lib64']:
            if Path(library).exists():
                command += ['--ro-bind', library, library]
        command += ['--proc', '/proc', '--dev', '/dev', '--bind', str(session.runtime), str(session.runtime), '--ro-bind', str(info), '/.flatpak-info', '--chdir', '/', '--', '/usr/bin/sh', '-c',
                    'printf "CLIENT_PID:%s\\n" "$$"; exec ' + (program or '/usr/bin/sleep 180')]
        with (session.output / (name + '.log')).open('w') as log:
            process = subprocess.Popen(command, env=session.env, stdin=subprocess.PIPE, stdout=log, stderr=log, text=True, start_new_session=True)
        clients.append(process)
        def main_pid():
            assert process.poll() is None, (session.output / (name + '.log')).read_text()
            return next((int(line.split(':', 1)[1]) for line in (session.output / (name + '.log')).read_text().splitlines() if line.startswith('CLIENT_PID:')), None)
        child = session.wait_for(main_pid, 'sandbox main PID')
        assert child > 1 and child != os.getpid()
        # Publish a complete instance record atomically. Both PIDs refer solely
        # to processes launched here; frontend refusal cannot target the session.
        record = session.runtime / ('record-' + name)
        record.mkdir()
        (record / 'info').write_text(metadata)
        (record / 'pid').write_text(str(process.pid))
        (record / 'bwrapinfo.json').write_text(json.dumps({'child-pid': child}))
        record.rename(instances / name)
        return process, child, instances / name
    def retire(process, record):
        desktop.stop(process)
        shutil.rmtree(record)
    checks = []
    for name, decision, expected in [('once', 2, None), ('allow', 1, ['yes']), ('forbid', 0, ['no'])]:
        app = 'org.canopa.BackgroundFixture.' + name
        process, child, record = start(app, name)
        assert permission(app) is None
        choose(decision, name + '.png')
        if expected is None:
            assert permission(app) is None
        else:
            session.wait_for(lambda: permission(app) == expected, 'frontend-owned permission update')
        if decision == 0:
            assert process.wait(timeout=5) != 0
            assert not Path(f'/proc/{child}').exists()
        else:
            assert process.poll() is None
        retire(process, record)
    checks.append('real frontend discovers synthetic sandbox instances; once leaves permission unset, Allow stores yes, Block stores no and terminates only the fixture child')
    app = 'org.canopa.BackgroundFixture.Ask'
    set_permission(app, 'ask')
    process, _, record = start(app, 'ask')
    choose(1, 'ask.png')
    assert permission(app) == ['ask'] and process.poll() is None
    retire(process, record)
    checks.append('an existing Ask policy survives an explicit Allow as specified by the frontend')

    # Connect from an actual namespace with /.flatpak-info. Do not use the
    # frontend's test identity environment overrides or a host client identity.
    client_source = session.runtime / 'request-background.py'
    client_source.write_text('''import json, os, sys, time
from gi.repository import Gio, GLib
request = json.loads(sys.stdin.readline())
connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
responses = []
connection.signal_subscribe('org.freedesktop.portal.Desktop', 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE, lambda c,s,p,i,n,v: responses.append(v.unpack()))
options = {'handle_token': GLib.Variant('s', 'background_fixture'), 'reason': GLib.Variant('s', 'Only this disposable sandbox receives the decision.'), 'autostart': GLib.Variant('b', request.get('autostart', False))}
if request.get('autostart'):
    options['commandline'] = GLib.Variant('as', ['fixture-noop', '--argument', 'value'])
connection.call_sync('org.freedesktop.portal.Desktop', '/org/freedesktop/portal/desktop', 'org.freedesktop.portal.Background', 'RequestBackground', GLib.Variant('(sa{sv})', ('', options)), None, Gio.DBusCallFlags.NONE, 5000, None)
deadline = time.monotonic() + 30
while not responses and time.monotonic() < deadline:
    while GLib.MainContext.default().pending():
        GLib.MainContext.default().iteration(False)
    time.sleep(.02)
assert responses, 'no public response'
print('PUBLIC_RESPONSE:' + json.dumps(responses[0]), flush=True)
sys.stdin.readline()
''')
    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
    for name, grant in [('request-deny', False), ('request-allow', True)]:
        app = 'org.canopa.BackgroundFixture.' + name
        set_permission(app, 'ask')
        process, _, record = start(app, name, '/usr/bin/python3 ' + str(client_source))
        process.stdin.write('{}\n')
        process.stdin.flush()
        window = session.wait_for(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.Portal'), None), 'public Background Access consent')
        if not window['is_floating']:
            session.run(['niri', 'msg', 'action', 'toggle-window-floating', '--id', str(window['id'])])
        for action, size in [('set-window-width', '680'), ('set-window-height', '475')]:
            session.run(['niri', 'msg', 'action', action, '--id', str(window['id']), size])
        session.run(['niri', 'msg', 'action', 'move-floating-window', '--id', str(window['id']), '--x', '20', '--y', '10'])
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        time.sleep(.4)
        session.capture(name + '.png')
        if grant:
            window = next(w for w in windows() if w['id'] == window['id'])
            click(20 + window['layout']['window_size'][0] - 60, 10 + window['layout']['window_size'][1] - 42)
        else:
            session.run(['wtype', '-k', 'Escape'])
        def response():
            return next((json.loads(line.split(':', 1)[1]) for line in (session.output / (name + '.log')).read_text().splitlines() if line.startswith('PUBLIC_RESPONSE:')), None)
        result = session.wait_for(response, 'public Background response')
        assert result == [0 if grant else 1, {'background': grant, 'autostart': False}], result
        assert permission(app) == ['ask']
        assert process.poll() is None
        assert not list((config.parent.parent / 'autostart').glob('*.desktop'))
        retire(process, record)
        session.wait_for(lambda: not windows(), 'Access helper closure')
    checks.append('actual sandbox public RequestBackground uses Iced Access for Ask; explicit grant/cancel return booleans without changing Ask or publishing autostart')
    app = 'org.canopa.BackgroundFixture.Autostart'
    set_permission(app, 'yes')
    autostart = config.parent.parent / 'autostart' / (app + '.desktop')
    for enabled in [True, False]:
        name = 'autostart-' + str(enabled).lower()
        process, _, record = start(app, name, '/usr/bin/python3 ' + str(client_source))
        process.stdin.write(json.dumps({'autostart': enabled}) + '\n')
        process.stdin.flush()
        def response():
            return next((json.loads(line.split(':', 1)[1]) for line in (session.output / (name + '.log')).read_text().splitlines() if line.startswith('PUBLIC_RESPONSE:')), None)
        result = session.wait_for(response, 'frontend-owned autostart response')
        assert result == [0, {'background': True, 'autostart': enabled}], result
        assert permission(app) == ['yes']
        assert autostart.exists() == enabled
        if enabled:
            contents = autostart.read_text()
            assert 'X-XDP-Autostart=' + app in contents
            assert 'Exec=flatpak run --command=fixture-noop ' + app + ' --argument value' in contents, contents
        retire(process, record)
    checks.append('public autostart enable/disable is validated and published by the installed frontend; the deprecated backend method is not needed and no command is launched')
    connection.close_sync(None)
    return checks, {'path': str(frontend), 'version': session.run([str(frontend), '--version']).strip(), 'sha256': hashlib.sha256(frontend.read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--notify', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--color-scheme', choices=['dark', 'light'], default='dark')
    parser.add_argument('--scale', type=float, default=1.0)
    parser.add_argument('--shipped-descriptor', action='store_true')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    clients = []
    with tempfile.TemporaryDirectory(prefix='canopa-background-frontend-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        try:
            checks, frontend = verify(args.binary.resolve(), args.notify.resolve(), session, clients, args.color_scheme, args.scale, args.shipped_descriptor)
            binaries = {str(path.resolve()): hashlib.sha256(path.read_bytes()).hexdigest() for path in [args.binary, args.notify]}
            desktop.write_json(args.output / 'result.json', dict(passed=True, checks=checks, binaries=binaries, frontend=frontend, scheme=args.color_scheme, scale=args.scale, shipped_descriptor=args.shipped_descriptor, scope='installed public frontend with real bubblewrap roots and synthetic Flatpak metadata; not a real Flatpak installation'))
        finally:
            for process in reversed(clients + session.processes):
                desktop.stop(process)
    print(f'Background frontend: {len(checks)} native checks passed')


if __name__ == '__main__':
    main()
