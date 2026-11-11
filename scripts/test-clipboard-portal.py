#!/usr/bin/env python3
"""Private standard-frontend Clipboard v1, real Iced consent and descriptor lifetimes."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import fcntl
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import traceback
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
from screencast_media import MediaSession, digest
ROOT = Path(__file__).resolve().parents[1]
FRONT = 'org.freedesktop.portal.Desktop'
BACK = 'org.freedesktop.impl.portal.desktop.canopa'
PATH = '/org/freedesktop/portal/desktop'
REMOTE = 'org.freedesktop.portal.RemoteDesktop'
CLIP = 'org.freedesktop.portal.Clipboard'
IMPL = 'org.freedesktop.impl.portal.Clipboard'
APP = 'org.canopa.ClipboardFixture'
TEXT = 'text/plain;charset=utf-8'
BINARY = 'application/octet-stream'


def verify(args, session, result):
    from gi.repository import Gio, GLib
    out = session.output
    config = session.runtime / 'canopa.kdl'
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.scheme}"; }}\n')
    portals = session.runtime / 'portals'; portals.mkdir()
    names = ['org.freedesktop.impl.portal.' + name for name in ['RemoteDesktop', 'ScreenCast', 'Clipboard', 'Settings']]
    descriptor = (args.portal_descriptor.read_text() if args.portal_descriptor else
                  '[portal]\nDBusName=' + BACK + '\nInterfaces=' + ';'.join(names) + ';\nUseIn=Canopa;\n')
    assert all(name + ';' in descriptor for name in names)
    (portals / 'canopa.portal').write_text(descriptor)
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\n' + ''.join(name + '=canopa\n' for name in names))
    applications = Path(session.env['XDG_DATA_HOME']) / 'applications'; applications.mkdir(parents=True)
    for app in [APP, APP + '.Other']:
        (applications / (app + '.desktop')).write_text('[Desktop Entry]\nType=Application\nName=Clipboard fixture\nExec=true\n')
    home = session.runtime / 'home'; home.mkdir()
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(CANOPA_PORTAL_CONFIG=str(config), CANOPA_REMOTE_DESKTOP_NATIVE='1', CANOPA_PORTAL_BINARY=str(args.portal),
                       XDG_CURRENT_DESKTOP='Canopa', XDG_DESKTOP_PORTAL_DIR=str(portals), XDG_DATA_DIRS=session.env['XDG_DATA_HOME'], HOME=str(home),
                       GIO_USE_VFS='local', GSETTINGS_BACKEND='memory', GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas')
    for path in [config, portals / 'canopa.portal', portals / 'portals.conf']:
        shutil.copy2(path, out / path.name)
    output = session.start_niri('debug { dbus-interfaces-in-non-session-instances; }\nprefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { border { off; }; focus-ring { off; }; }\n', scale=args.scale)
    logical = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))[output]['logical']
    pointer = session.build_pointer()
    def click(x, y): session.run([str(pointer), str(round(x / logical['width'] * 1600)), str(round(y / logical['height'] * 1000))])
    def keys(*values): session.run(['wtype', *values])
    def windows(): return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
    def action(*values): return session.run(['niri', 'msg', 'action', *map(str, values)])
    def drain():
        while GLib.MainContext.default().pending(): GLib.MainContext.default().iteration(False)
    def connect(app=None):
        connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        if app:
            call(connection, FRONT, PATH, 'org.freedesktop.host.portal.Registry', 'Register', '(sa{sv})', (app, {}))
        return connection
    def call(connection, destination, path, interface, method, signature='()', values=()):
        return connection.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 10000, None).unpack()
    observer = connect()
    call(observer, 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'UpdateActivationEnvironment', '(a{ss})', ({**session.env},))
    command = ([str(args.portal), '--canopa-config', str(config)] if args.normal_backend else
               [str(args.harness), '--exact', 'screencast::remote::production::tests::native_backend_fixture', '--ignored', '--nocapture', '--test-threads=1'])
    ready = 'component=settings_portal event=ready' if args.normal_backend else 'remote-backend-ready '
    with (out / 'backend.log').open('w') as log:
        backend = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=log, stderr=log, env=session.env, start_new_session=True)
    session.processes.append(backend)
    session.wait_for(lambda: ready in (out / 'backend.log').read_text(), 'private production backend')
    session.start([str(args.frontend), '--verbose'], 'frontend.log')
    session.wait_for(lambda: call(observer, 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', '(s)', (FRONT,))[0], 'standard frontend')
    def passed(name, **details):
        result['checks'].append(dict(name=name, **details)); (out / 'progress.json').write_text(json.dumps(result, indent=2) + '\n'); print(name, flush=True)
    xml = call(observer, BACK, PATH, 'org.freedesktop.DBus.Introspectable', 'Introspect')[0]
    interface = next(item for item in ET.fromstring(xml).findall('interface') if item.get('name') == IMPL)
    methods = {item.get('name'): (''.join(arg.get('type') for arg in item.findall('arg') if arg.get('direction') != 'out'), ''.join(arg.get('type') for arg in item.findall('arg') if arg.get('direction') == 'out')) for item in interface.findall('method')}
    assert methods == {'RequestClipboard': ('oa{sv}', ''), 'SetSelection': ('oa{sv}', ''), 'SelectionRead': ('os', 'h'), 'SelectionWrite': ('ou', 'h'), 'SelectionWriteDone': ('oub', '')}, methods
    assert call(observer, FRONT, PATH, 'org.freedesktop.DBus.Properties', 'Get', '(ss)', (CLIP, 'version'))[0] == 1
    passed('exact-backend-v1-signatures-and-public-frontend-registration')
    responses, closed, offers, transfers = {}, set(), {}, []
    clients = []
    def client(app=APP):
        caller = connect(app); clients.append(caller)
        caller.signal_subscribe(FRONT, 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE, lambda c,s,p,i,n,v: responses.__setitem__(p, v.unpack()))
        caller.signal_subscribe(FRONT, 'org.freedesktop.portal.Session', 'Closed', None, None, Gio.DBusSignalFlags.NONE, lambda c,s,p,i,n,v: closed.add(p))
        caller.signal_subscribe(FRONT, CLIP, 'SelectionOwnerChanged', None, None, Gio.DBusSignalFlags.NONE, lambda c,s,p,i,n,v: offers.__setitem__(v.unpack()[0], v.unpack()[1]))
        caller.signal_subscribe(FRONT, CLIP, 'SelectionTransfer', None, None, Gio.DBusSignalFlags.NONE, lambda c,s,p,i,n,v: transfers.append(v.unpack()))
        return caller
    counter = 0
    def options(**values):
        nonlocal counter
        counter += 1; return {'handle_token': GLib.Variant('s', f'clipboard{counter}'), **values}
    def response(path, status=0):
        value = session.wait_for(lambda: (drain(), responses.get(path))[1], 'portal response', timeout=35)
        assert value[0] == status, value
        return value[1]
    def request(caller, interface, method, signature, values): return call(caller, FRONT, PATH, interface, method, signature, values)[0]
    def create(caller, devices=0, clipboard=True):
        opts = options(); opts['session_handle_token'] = GLib.Variant('s', f'session{counter}')
        key = response(request(caller, REMOTE, 'CreateSession', '(a{sv})', (opts,)))['session_handle']
        response(request(caller, REMOTE, 'SelectDevices', '(oa{sv})', (key, options(types=GLib.Variant('u', devices)))))
        if clipboard: call(caller, FRONT, PATH, CLIP, 'RequestClipboard', '(oa{sv})', (key, {}))
        return key
    def start(caller, key): return request(caller, REMOTE, 'Start', '(osa{sv})', (key, '', options()))
    def popup():
        window = session.wait_for(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.Portal' and w['title'].startswith('Allow ') and w['title'].endswith(' — Canopa')), None), 'Iced permission dialog')
        if not window['is_floating']: action('move-window-to-floating', '--id', window['id'])
        action('set-window-width', '--id', window['id'], 580); action('set-window-height', '--id', window['id'], 340 if window['title'].startswith('Allow clipboard') else 420)
        action('move-floating-window', '--id', window['id'], '--x', 20, '--y', 10); action('focus-window', '--id', window['id'])
        time.sleep(.15)
        return window
    def gone(window): session.wait_for(lambda: not any(w['id'] == window['id'] for w in windows()), 'Iced surface retired')
    def worker_count():
        count = 0
        for path in Path(f'/proc/{backend.pid}/task').glob('*/comm'):
            try: count += path.read_text().strip() == 'portal-clipboar'
            except FileNotFoundError: pass
        return count
    def fd(caller, key, method, signature, values):
        reply, descriptors = caller.call_with_unix_fd_list_sync(FRONT, PATH, CLIP, method, GLib.Variant(signature, (key, *values)), GLib.VariantType.new('(h)'), Gio.DBusCallFlags.NONE, 10000, None, None)
        owned = descriptors.steal_fds(); assert reply.unpack() == (0,) and len(owned) == 1
        return owned[0]
    def read_fd(descriptor):
        os.set_blocking(descriptor, False); result = bytearray(); deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            try: chunk = os.read(descriptor, 65536)
            except BlockingIOError: drain(); time.sleep(.01); continue
            if not chunk: return bytes(result)
            result.extend(chunk)
        raise AssertionError('Readable clipboard pipe did not reach EOF')
    def rejected(operation):
        try: operation()
        except GLib.Error as error:
            name = Gio.DBusError.get_remote_error(error)
            assert name and name not in ['org.freedesktop.DBus.Error.NoReply', 'org.freedesktop.DBus.Error.Timeout', 'org.freedesktop.DBus.Error.TimedOut'], str(error)
            assert not error.matches(Gio.io_error_quark(), Gio.IOErrorEnum.TIMED_OUT), str(error)
            return
        raise AssertionError('Unapproved clipboard/input operation succeeded')
    def source(payload=b'local clipboard bytes'):
        with (out / f'copy-{counter}.log').open('w') as log:
            child = subprocess.Popen(['/usr/bin/wl-copy', '--foreground', '--type', TEXT], stdin=subprocess.PIPE, stdout=log, stderr=log, env=session.env, start_new_session=True)
        session.processes.append(child); child.stdin.write(payload); child.stdin.close()
        session.wait_for(lambda: TEXT.encode() in subprocess.run(['/usr/bin/wl-paste', '--list-types'], env=session.env, capture_output=True, timeout=5).stdout.splitlines(), 'independent local clipboard source')
        return child
    def finished(process, expected=0):
        actual = process.wait(timeout=10)
        assert actual == expected, (process.args, actual, expected)
        session.processes.remove(process)
        result.setdefault('retired', []).append({'pid': process.pid, 'command': process.args, 'exit_code': actual})
    def controls():
        return session.wait_for(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.Portal' and w['title'] == 'Clipboard sharing — Canopa'), None), 'ongoing Clipboard controls')
    # Cancel proves the permission stays unchecked and no privileged bridge opens.
    caller = client(); key = create(caller); path = start(caller, key); window = popup()
    session.capture('01-unchecked.png'); assert worker_count() == 0
    keys('-k', 'Return', '-M', 'ctrl', '-k', 'Return', '-m', 'ctrl'); drain(); assert path not in responses
    keys('-k', 'Escape'); response(path, 1); gone(window)
    passed('initial-enter-cannot-grant-clipboard-and-cancel-opens-no-bridge')
    # Clipboard refusal leaves independently selected keyboard permission intact.
    caller = client(); key = create(caller, devices=1); path = start(caller, key); window = popup()
    click(120, 125); keys('-M', 'ctrl', '-k', 'Return', '-m', 'ctrl')
    values = response(path); gone(window); assert values['devices'] == 1 and not values.get('clipboard_enabled', False)
    assert worker_count() == 0
    rejected(lambda: fd(caller, key, 'SelectionRead', '(os)', (TEXT,)))
    call(caller, FRONT, key, 'org.freedesktop.portal.Session', 'Close');
    session.wait_for(lambda: not any(w['app_id'] == 'org.canopa.Portal' for w in windows()), 'denied Clipboard session controls retired')
    passed('keyboard-consent-does-not-imply-clipboard-permission')
    # A selection already present before Start must be announced after its reply.
    caller = client(); local_source = source(); key = create(caller); path = start(caller, key); window = popup()
    rejected(lambda: fd(caller, key, 'SelectionRead', '(os)', (TEXT,)))
    keys('-M', 'alt', 'c', '-m', 'alt'); session.capture('02-selected.png')
    keys('-M', 'ctrl', '-k', 'Return', '-m', 'ctrl'); values = response(path); gone(window)
    assert values['devices'] == 0 and values['clipboard_enabled'] and not values.get('streams')
    status = controls(); session.capture('03-controls.png')
    session.wait_for(lambda: (drain(), TEXT in offers.get(key, {}).get('mime_types', []))[1], 'initial clipboard metadata after Start')
    assert not offers[key]['session_is_owner'] and worker_count() == 1
    rejected(lambda: call(caller, FRONT, PATH, REMOTE, 'NotifyKeyboardKeycode', '(oa{sv}iu)', (key, {}, 30, 1)))
    other = client(APP + '.Other'); rejected(lambda: fd(other, key, 'SelectionRead', '(os)', (TEXT,)))
    descriptor = fd(caller, key, 'SelectionRead', '(os)', (TEXT,)); assert fcntl.fcntl(descriptor, fcntl.F_GETFL) & os.O_ACCMODE == os.O_RDONLY
    try: assert read_fd(descriptor) == b'local clipboard bytes'
    finally: os.close(descriptor)
    passed('unchecked-independent-clipboard-only-grant-announces-existing-selection-and-passes-readable-fd')
    call(caller, FRONT, PATH, CLIP, 'SetSelection', '(oa{sv})', (key, {'mime_types': GLib.Variant('as', [TEXT, BINARY])}))
    finished(local_source)
    session.wait_for(lambda: (drain(), offers.get(key, {}).get('session_is_owner', False))[1], 'session clipboard ownership')
    assert set(offers[key]['mime_types']) == {TEXT, BINARY}
    with (out / 'paste.log').open('w') as log:
        peer = subprocess.Popen(['/usr/bin/wl-paste', '--no-newline', '--type', BINARY], stdout=subprocess.PIPE, stderr=log, env=session.env, start_new_session=True)
    session.processes.append(peer)
    transfer = session.wait_for(lambda: (drain(), next((item for item in transfers if item[0] == key and item[1] == BINARY), None))[1], 'public SelectionTransfer')
    serial = transfer[2]
    rejected(lambda: fd(caller, key, 'SelectionWrite', '(ou)', (serial + 10000,)))
    writer = fd(caller, key, 'SelectionWrite', '(ou)', (serial,)); assert fcntl.fcntl(writer, fcntl.F_GETFL) & os.O_ACCMODE == os.O_WRONLY
    rejected(lambda: fd(caller, key, 'SelectionWrite', '(ou)', (serial,)))
    payload = bytes(index % 251 for index in range(2 * 1024 * 1024))
    with ThreadPoolExecutor(max_workers=1) as pool:
        receiving = pool.submit(peer.communicate)
        try:
            remaining = memoryview(payload)
            while remaining: remaining = remaining[os.write(writer, remaining):]
            call(caller, FRONT, PATH, CLIP, 'SelectionWriteDone', '(oub)', (key, serial, True))
        finally: os.close(writer)
        data, _ = receiving.result(timeout=10)
    assert peer.returncode == 0 and data == payload
    finished(peer)
    passed('public-transfer-serial-writable-fd-and-two-megabyte-independent-paste')
    # Hold both app descriptor ends open across revocation of a self-read transfer.
    def held(caller, key):
        before = len(transfers)
        reader = fd(caller, key, 'SelectionRead', '(os)', (BINARY,))
        transfer = session.wait_for(lambda: (drain(), next((item for item in transfers[before:] if item[0] == key and item[1] == BINARY), None))[1], 'held transfer')
        writer = fd(caller, key, 'SelectionWrite', '(ou)', (transfer[2],))
        return reader, writer
    def revoked(reader, writer):
        try:
            try: os.write(writer, b'late bytes')
            except BrokenPipeError: pass
            else: raise AssertionError('Application writer stayed active after revocation')
            assert read_fd(reader) == b''
        finally: os.close(reader); os.close(writer)
    reader, writer = held(caller, key)
    action('focus-window', '--id', status['id']); keys('-k', 'Escape')
    session.wait_for(lambda: (drain(), key in closed)[1], 'public Clipboard session closure'); gone(status); revoked(reader, writer)
    session.wait_for(lambda: worker_count() == 0, 'Clipboard worker retirement')
    passed('iced-stop-closes-public-session-and-revokes-both-app-held-descriptors')
    for ending in ['caller-loss', 'backend-kill']:
        caller = client(); key = create(caller); path = start(caller, key); window = popup()
        keys('-M', 'alt', 'c', '-m', 'alt', '-M', 'ctrl', '-k', 'Return', '-m', 'ctrl'); assert response(path)['clipboard_enabled']; gone(window); status = controls()
        call(caller, FRONT, PATH, CLIP, 'SetSelection', '(oa{sv})', (key, {'mime_types': GLib.Variant('as', [BINARY])}))
        session.wait_for(lambda: (drain(), offers.get(key, {}).get('session_is_owner', False))[1], 'owned source before loss')
        reader, writer = held(caller, key)
        if ending == 'caller-loss': caller.close_sync(None)
        else:
            assert backend.poll() is None
            backend.kill(); finished(backend, -signal.SIGKILL)
            session.wait_for(lambda: (drain(), key in closed)[1], 'public backend-loss Closed')
        gone(status)
        if ending == 'caller-loss': session.wait_for(lambda: worker_count() == 0, 'caller-loss worker retired')
        revoked(reader, writer)
        passed(ending + '-retires-controls-and-revokes-held-clipboard-pipes')
    for caller in clients:
        if not caller.is_closed(): caller.close_sync(None)
    observer.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--normal-backend', action='store_true', help='Run the normal executable instead of the unit-test fixture')
    parser.add_argument('--portal-descriptor', type=Path, help='Use the exact optional package descriptor')
    for name in ['portal', 'harness', 'niri', 'frontend']:
        parser.add_argument('--' + name, type=Path, required=name != 'harness'); parser.add_argument('--' + name + '-sha256', required=name != 'harness')
    parser.add_argument('--scheme', choices=['dark', 'light'], default='dark'); parser.add_argument('--scale', type=float, default=1.25); parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not args.normal_backend and (args.harness is None or args.harness_sha256 is None): parser.error('provide --harness and --harness-sha256, or --normal-backend')
    binaries = ['portal', 'niri', 'frontend'] + ([] if args.normal_backend else ['harness'])
    for name in binaries:
        path = getattr(args, name).resolve(strict=True); setattr(args, name, path)
        if digest(path) != getattr(args, name + '_sha256'): parser.error(name + ' checksum mismatch')
    args.output = args.output.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    sources = [Path(__file__), ROOT / 'scripts/screencast_media.py', ROOT / 'scripts/validate-desktop.py', *[getattr(args, name) for name in binaries]]
    if args.portal_descriptor:
        args.portal_descriptor = args.portal_descriptor.resolve(strict=True); sources.append(args.portal_descriptor)
    pins = {str(path): digest(path) for path in sources}
    result = {'status': 'running', 'normal_backend': args.normal_backend, 'checks': [], 'pins': pins, 'scheme': args.scheme, 'scope': 'Private bus and canonical frontend; real Iced Clipboard consent/controls and public FD transfer. Not InputCapture, Flatpak or full distribution acceptance.'}; started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='canopa-clipboard-portal-') as directory:
        session = MediaSession(Path(directory), args.output, args.niri, args.niri_sha256, reference_frozen=True)
        try: verify(args, session, result); result['status'] = 'passed'
        except Exception as error:
            result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
            try: session.capture('failure.png')
            except Exception: pass
        finally:
            processes = list(session.processes); session.close(); result['processes'] = [{'pid': process.pid, 'command': process.args, 'exit_code': process.poll()} for process in processes]
    result.update(seconds=round(time.monotonic() - started, 2), changed_inputs=[path for path, value in pins.items() if digest(Path(path)) != value])
    if result['changed_inputs'] or any(row['exit_code'] is None for row in result['processes']): result['status'] = 'failed'
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n'); print(json.dumps({key: result.get(key) for key in ['status', 'error', 'seconds']})); raise SystemExit(0 if result['status'] == 'passed' else 1)


if __name__ == '__main__': main()
