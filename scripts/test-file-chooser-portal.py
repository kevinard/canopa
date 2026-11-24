#!/usr/bin/env python3
"""Exercise the public FileChooser frontend and real Iced dialogs in private Niri."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import traceback
from urllib.parse import quote_from_bytes

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
import native_accessibility


def verify(binary, session, scheme, scale, folder_history=False):
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
    bookmarked = home / 'Bookmarked folder'
    bookmarked.mkdir()
    recent = bookmarked / 'recent.txt'
    recent.write_text('opened recently')
    data = Path(session.env['XDG_DATA_HOME'])
    data.mkdir(parents=True, exist_ok=True)
    (data / 'recently-used.xbel').write_text(
        f'<xbel version="1.0"><bookmark href="{recent.as_uri()}" '
        'visited="2026-01-01T00:00:00Z"/></xbel>')
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\n'
                      f'files {{ places {{ place "recent"; bookmark "Fixture bookmark" path={json.dumps(str(bookmarked))}; place "home"; }}; }}\n')
    # Use the existing native GIO fixture so the chooser never lists host devices.
    modules = session.output / 'gio-modules'
    modules.mkdir()
    gio_flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'gio-2.0'], text=True).split()
    subprocess.run(['cc', '-shared', '-fPIC', '-Wall', '-Wextra', '-Werror',
                    '-DCANOPA_FIXTURE_NATIVE_MONITOR',
                    str(ROOT / 'scripts/desktop-fixture/files-volume-monitor.c'),
                    '-o', str(modules / 'libcanopa-volume-fixture.so'), *gio_flags], check=True)
    media = session.runtime / 'volumes'
    mounted = media / 'Fixture USB'
    mounted.mkdir(parents=True)
    (mounted / 'Drive notes.txt').write_text('mounted file')
    session.env.update(GIO_EXTRA_MODULES=str(modules), GIO_USE_VOLUME_MONITOR='canopa-private-volume-fixture',
                       GIO_USE_VFS='local', CANOPA_FIXTURE_VOLUMES=str(media))
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home), WAYLAND_DEBUG='1')
    portals = session.runtime / 'portals'
    portals.mkdir()
    shutil.copy(ROOT / 'packaging/portal/canopa.portal', portals / 'canopa.portal')
    preferences = config.parent.parent / 'xdg-desktop-portal/portals.conf'
    preferences.parent.mkdir()
    preferences.write_text('[preferred]\ndefault=none\norg.freedesktop.impl.portal.Settings=canopa\norg.freedesktop.impl.portal.FileChooser=canopa\n')
    # XDG_DESKTOP_PORTAL_DIR also redirects the frontend's configuration lookup.
    shutil.copy(preferences, portals / 'portals.conf')
    if folder_history:
        applications = Path(session.env['XDG_DATA_HOME']) / 'applications'
        applications.mkdir(parents=True, exist_ok=True)
        for app in ['org.canopa.HistoryA', 'org.canopa.HistoryB']:
            (applications / (app + '.desktop')).write_text('[Desktop Entry]\nType=Application\nName=Chooser history fixture\nExec=true\n')
    services = session.runtime / 'services'
    services.mkdir()
    service = 'org.freedesktop.impl.portal.desktop.canopa.service'
    shutil.copy(ROOT / 'packaging/portal' / service, services / service)
    session.env.update(PATH=str(binary.parent) + os.pathsep + session.env['PATH'], XDG_DESKTOP_PORTAL_DIR=str(portals), XDG_CURRENT_DESKTOP='Canopa')
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; focus-ring { off; }; }\n',
                       width=round(1280 * scale), height=round(800 * scale), scale=scale, service_directories=[services])
    outputs = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))
    logical = next(output['logical'] for output in outputs.values() if output.get('logical'))
    screen_width, screen_height = logical['width'], logical['height']
    native_accessibility.start(session)
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

    def call(destination, path, interface, method, signature, values, client=connection):
        return client.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()

    # Activation inherits only this disposable bus's compositor/configuration.
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'UpdateActivationEnvironment', '(a{ss})', ({k: v for k, v in session.env.items()},))
    frontend = next(p for p in [Path('/usr/libexec/xdg-desktop-portal'), Path('/usr/lib/xdg-desktop-portal')] if p.is_file())
    session.start([str(frontend), '--verbose'], 'frontend.log')
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'StartServiceByName', '(su)', ('org.freedesktop.impl.portal.desktop.canopa', 0))
    backend_pid = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', '(s)', ('org.freedesktop.impl.portal.desktop.canopa',))[0]
    assert Path(f'/proc/{backend_pid}/exe').resolve() == binary
    session.backend_pid = backend_pid
    session.backend_pids = [backend_pid]

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

    def chooser():
        return next((w for w in windows() if w['app_id'] == 'org.canopa.Portal'), None)

    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)

    responses = {}
    subscription = connection.signal_subscribe('org.freedesktop.portal.Desktop', 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE,
        lambda conn, sender, path, interface, name, params: responses.__setitem__(path, params.unpack()))
    count = 0

    def begin(method, options, parent=exported, expect_parent=True, client=connection, initial_folder=fixture):
        nonlocal count
        count += 1
        options = {**options, 'handle_token': GLib.Variant('s', f'test{count}')}
        if initial_folder is not None: options['current_folder'] = GLib.Variant('ay', os.fsencode(initial_folder) + b'\0')
        handle = call('org.freedesktop.portal.Desktop', '/org/freedesktop/portal/desktop', 'org.freedesktop.portal.FileChooser', method, '(ssa{sv})', (parent, method, options), client)[0]
        window = session.wait_for(chooser, f'{method} Iced window')
        if expect_parent:
            assert window['is_floating'], window
            # Parenting before initial configure retains the requested dimensions.
            assert abs(window['layout']['window_size'][0] - 1000) <= 1, window
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        time.sleep(.5)
        # Wait for first-frame CSD geometry before placing the window for input.
        if expect_parent:
            session.run(['niri', 'msg', 'action', 'move-floating-window', '--id', str(window['id']), '--x', '20', '--y', '10'])
            time.sleep(.15)
        return handle

    def picker_pid():
        children = {int(pid) for task in Path(f'/proc/{backend_pid}/task').iterdir()
                    for pid in (task / 'children').read_text().split()}
        for pid in children:
            try:
                if (Path(f'/proc/{pid}/exe').resolve() == binary.with_name('canopa-files')
                        and b'--internal-picker' in Path(f'/proc/{pid}/cmdline').read_bytes()):
                    return pid
            except FileNotFoundError:
                pass
        return None

    def keys(*names):
        for name in names:
            session.run(['wtype', '-k', name])
            time.sleep(.1)

    def chord(key):
        session.run(['wtype', '-M', 'ctrl', '-k', key, '-m', 'ctrl'])
        time.sleep(.15)

    def result(handle, status=0):
        def received():
            drain()
            return responses.get(handle)
        response = session.wait_for(received, 'public FileChooser Response')
        assert response[0] == status, response
        session.wait_for(lambda: chooser() is None, 'Iced helper closed')
        return response[1]

    def uri(path):
        return 'file://' + quote_from_bytes(os.fsencode(path), safe='/')

    def click_footer(window, label_offset):
        # Fit the dialog on the smaller logical output at fractional scale.
        width, height = window['layout']['window_size']
        click(20 + width - label_offset, 10 + height - 30)
        time.sleep(.2)

    def click(x, y):
        session.run([str(pointer), str(round(x / screen_width * 1600)), str(round(y / screen_height * 1000))])

    session.wait_for(lambda: call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', '(s)', ('org.freedesktop.portal.Desktop',))[0], 'public frontend bus name', timeout=40)
    checks = []
    session.completed_checks = checks
    handle = begin('OpenFile', {'multiple': GLib.Variant('b', True), 'filters': GLib.Variant('a(sa(us))', [('Text', [(0, '*.txt')])]), 'choices': GLib.Variant('a(ssa(ss)s)', [('encoding', 'Encoding', [('utf8', 'UTF-8'), ('latin1', 'Latin-1')], 'latin1')])})
    session.wait_for(picker_pid, 'the portal opened a Canopa Files picker process')
    checks.append('public FileChooser launches Canopa Files in picker mode')
    session.capture('01-open.png')
    keys('Down', 'Down')
    session.run(['wtype', '-M', 'shift', '-k', 'Down', '-m', 'shift'])
    time.sleep(.15)
    click_footer(chooser(), 42)
    response = result(handle)
    assert response['uris'] == [uri(fixture / 'a.txt'), uri(fixture / 'b.txt')], response
    assert response['choices'] == [('encoding', 'latin1')], response
    assert response['current_filter'] == ('Text', [(0, '*.txt')]), response
    checks.append('public filtered multi-open, keyboard range selection and choices')

    raw = Path(os.fsdecode(os.fsencode(fixture) + b'/raw-\xff.txt'))
    raw.write_text('non Unicode name')
    handle = begin('OpenFile', {})
    chord('f')
    session.run(['wtype', 'raw-'])
    time.sleep(.3)
    keys('Down', 'Return')
    response = result(handle)
    assert response['uris'] == [uri(raw)], response
    checks.append('Files search opens the selected result and preserves non-UTF-8 bytes')

    handle = begin('OpenFile', {'directory': GLib.Variant('b', True)})
    click_footer(chooser(), 42)
    assert result(handle)['uris'] == [uri(fixture)]
    checks.append('directory selection')

    # Browse away from the app's starting folder, then check the real selected
    # directory returned after traversing the chooser's navigation history.
    for forward, expected in [(False, fixture), (True, home)]:
        handle = begin('OpenFile', {'directory': GLib.Variant('b', True)})
        for direction in ['Up', 'Left', *(['Right'] if forward else [])]:
            session.run(['wtype', '-M', 'alt', '-k', direction, '-m', 'alt'])
            time.sleep(.25)
        click_footer(chooser(), 42)
        assert result(handle)['uris'] == [uri(expected)]
    checks.append('Alt+Up, Back and Forward navigate the chooser without granting a different folder')

    for place, filename, expected in [
        ('Fixture bookmark', 'recent.txt', recent),
        ('Recent', 'recent.txt', recent),
        ('Fixture USB', 'Drive notes.txt', mounted / 'Drive notes.txt'),
    ]:
        handle = begin('OpenFile', {})
        native_accessibility.request(session, '--window', 'OpenFile — Canopa Files', '--name', place, '--click')
        native_accessibility.request(session, '--window', 'OpenFile — Canopa Files', '--name', filename, '--click')
        click_footer(chooser(), 42)
        assert result(handle)['uris'] == [uri(expected)]
        checks.append(f'Files navigation opens an entry from {place}')
    assert (media / 'events').read_text().splitlines().count('private provider initialized') >= 1
    assert not (Path(session.env['XDG_STATE_HOME']) / 'canopa-files/operations.sqlite3').exists()
    checks.append('picker mode leaves the resident Files operations store unopened')

    handle = begin('SaveFile', {'current_name': GLib.Variant('s', 'new.txt')})
    click_footer(chooser(), 42)
    assert result(handle)['uris'] == [uri(fixture / 'new.txt')]
    assert not (fixture / 'new.txt').exists()
    checks.append('save returns a path without writing')

    handle = begin('SaveFile', {'current_name': GLib.Variant('s', 'report.txt')})
    session.run(['wtype', '-M', 'ctrl', '-M', 'shift', '-k', 'n', '-m', 'shift', '-m', 'ctrl'])
    time.sleep(.4)
    session.capture('03-new-folder.png')
    session.run(['wtype', '../escape'])
    keys('Return')
    drain()
    assert handle not in responses and not (home / 'escape').exists()
    chord('a'); session.run(['wtype', 'folder']); keys('Return')
    drain()
    assert handle not in responses and (fixture / 'folder').is_dir()
    chord('a'); session.run(['wtype', 'Created here']); keys('Return')
    created = fixture / 'Created here'
    session.wait_for(created.is_dir, 'explicitly created folder')
    time.sleep(.3)
    click_footer(chooser(), 42)
    assert result(handle)['uris'] == [uri(created / 'report.txt')]
    assert not (created / 'report.txt').exists()
    checks.append('New folder rejects traversal/duplicates, enters the child and preserves the SaveFile name')

    handle = begin('SaveFile', {'current_name': GLib.Variant('s', 'after-cancel.txt')})
    session.run(['wtype', '-M', 'ctrl', '-M', 'shift', '-k', 'n', '-m', 'shift', '-m', 'ctrl'])
    time.sleep(.4)
    session.run(['wtype', 'Cancelled folder'])
    keys('Escape')
    drain()
    assert handle not in responses and not (fixture / 'Cancelled folder').exists()
    click_footer(chooser(), 42)
    assert result(handle)['uris'] == [uri(fixture / 'after-cancel.txt')]
    assert not (fixture / 'after-cancel.txt').exists()
    checks.append('Escape cancels only the New folder prompt without a filesystem mutation')

    handle = begin('SaveFiles', {'files': GLib.Variant('aay', [b'one.txt\0', b'two.txt\0'])})
    session.run(['wtype', '-M', 'ctrl', '-M', 'shift', '-k', 'n', '-m', 'shift', '-m', 'ctrl'])
    time.sleep(.4)
    session.run(['wtype', 'Batch été']); keys('Return')
    batch = fixture / 'Batch été'
    session.wait_for(batch.is_dir, 'SaveFiles New folder action')
    time.sleep(.3)
    click_footer(chooser(), 42)
    assert result(handle)['uris'] == [uri(batch / 'one.txt'), uri(batch / 'two.txt')]
    assert not (batch / 'one.txt').exists() and not (batch / 'two.txt').exists()
    checks.append('SaveFiles creates a Unicode folder and preserves ordered unwritten destinations')

    handle = begin('SaveFile', {'current_name': GLib.Variant('s', 'a.txt')})
    click_footer(chooser(), 42)
    time.sleep(.3)
    drain()
    assert handle not in responses
    session.capture('02-replace-confirmation.png')
    keys('Escape')  # dismiss confirmation, keeping chooser open
    assert chooser() is not None
    keys('Escape')
    result(handle, 1)
    assert (fixture / 'a.txt').read_text() == 'original content'
    checks.append('overwrite requires confirmation and can be cancelled')

    handle = begin('SaveFile', {'current_name': GLib.Variant('s', 'a.txt')})
    click_footer(chooser(), 42)
    time.sleep(.3)
    width, height = chooser()['layout']['window_size']
    click(20 + width / 2 + 165, 10 + height / 2 + 60)
    assert result(handle)['uris'] == [uri(fixture / 'a.txt')]
    assert (fixture / 'a.txt').read_text() == 'original content'
    checks.append('confirmed replacement returns its path without modifying bytes')

    handle = begin('SaveFiles', {'files': GLib.Variant('aay', [b'one.txt\0', b'two.txt\0'])})
    click_footer(chooser(), 42)
    assert result(handle)['uris'] == [uri(fixture / 'one.txt'), uri(fixture / 'two.txt')]
    checks.append('SaveFiles preserves order')

    handle = begin('OpenFile', {})
    call('org.freedesktop.portal.Desktop', handle, 'org.freedesktop.portal.Request', 'Close', '()', ())
    session.wait_for(lambda: chooser() is None, 'Request.Close destroys helper')
    checks.append('public Request.Close destroys the Iced window')
    handle = begin('OpenFile', {}, parent='wayland:expired-handle', expect_parent=False)
    keys('Escape')
    result(handle, 1)
    checks.append('expired parent handle leaves a usable cancellable chooser')
    trace = (session.output / 'dbus.log').read_text()
    assert 'import_toplevel' in trace and 'set_parent_of' in trace
    if '"xdg_wm_dialog_v1"' in trace:
        assert 'set_modal' in trace
        checks.append('xdg-dialog modal request')
    else:
        checks.append('optional xdg-dialog unavailable on this compositor')
    checks.append('xdg-foreign parenting before initial configure')
    if folder_history:
        clients = []
        subscriptions = []
        try:
            for app in ['org.canopa.HistoryA', 'org.canopa.HistoryB']:
                client = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
                clients.append(client)
                call('org.freedesktop.portal.Desktop', '/org/freedesktop/portal/desktop', 'org.freedesktop.host.portal.Registry', 'Register', '(sa{sv})', (app, {}), client)
                subscriptions.append(client.signal_subscribe('org.freedesktop.portal.Desktop', 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE,
                    lambda c, s, p, i, n, values: responses.__setitem__(p, values.unpack())))
            a, b = fixture / 'History A', fixture / 'History B'
            a.mkdir(); b.mkdir()
            def save(client, name, expected, folder=None, current_file=None):
                options = {'current_name': GLib.Variant('s', name)}
                if current_file is not None: options['current_file'] = GLib.Variant('ay', os.fsencode(current_file) + b'\0')
                pending = begin('SaveFile', options, client=client, initial_folder=folder)
                click_footer(chooser(), 42)
                assert result(pending)['uris'] == [uri(expected)]
                assert not expected.exists(), 'Chooser must not write the document'
            save(clients[0], 'a.txt', a / 'a.txt', a)
            save(clients[1], 'b.txt', b / 'b.txt', b)
            os.kill(backend_pid, 15)
            session.wait_for(lambda: not call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', '(s)', ('org.freedesktop.impl.portal.desktop.canopa',))[0], 'backend stopped before history restart')
            call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'StartServiceByName', '(su)', ('org.freedesktop.impl.portal.desktop.canopa', 0))
            backend_pid = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', '(s)', ('org.freedesktop.impl.portal.desktop.canopa',))[0]
            assert Path(f'/proc/{backend_pid}/exe').resolve() == binary
            session.backend_pid = backend_pid; session.backend_pids.append(backend_pid)
            save(clients[0], 'again-a.txt', a / 'again-a.txt')
            save(clients[1], 'again-b.txt', b / 'again-b.txt')
            checks.append('identified applications retain separate accepted folders across backend restart')
            pending = begin('SaveFile', {'current_name': GLib.Variant('s', 'cancelled.txt')}, client=clients[0], initial_folder=b)
            keys('Escape'); result(pending, 1)
            save(clients[0], 'after-cancel.txt', a / 'after-cancel.txt')
            checks.append('cancelled folder hints do not change the remembered application folder')
            save(clients[0], 'ignored.txt', b / 'explicit.txt', current_file=b / 'explicit.txt')
            save(clients[0], 'after-hint.txt', b / 'after-hint.txt')
            checks.append('explicit current_file overrides memory and successful selection updates it')
            b.rmdir()
            save(clients[0], 'fallback.txt', home / 'fallback.txt')
            checks.append('deleted remembered folder falls back to Home without writing a document')
            import sqlite3
            database = Path(session.env['XDG_STATE_HOME']) / 'canopa/portal-file-chooser.db'
            with sqlite3.connect('file:' + str(database) + '?mode=ro', uri=True) as db:
                rows = db.execute('SELECT app,path FROM folders ORDER BY app').fetchall()
            assert rows == [('org.canopa.HistoryA', os.fsencode(home)), ('org.canopa.HistoryB', os.fsencode(b))], rows
            assert database.stat().st_mode & 0o777 == 0o600
            checks.append('private history contains only identified application folders, with no filenames or anonymous entry')
        finally:
            for client, registered in zip(clients, subscriptions): client.signal_unsubscribe(registered)
            for client in clients: client.close_sync(None)
    begin('OpenFile', {})
    os.kill(backend_pid, 15)
    session.wait_for(lambda: chooser() is None, 'backend shutdown destroys helper')
    checks.append('backend shutdown destroys pending helpers')
    connection.signal_unsubscribe(subscription)
    connection.close_sync(None)
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--color-scheme', choices=['dark', 'light'], default='dark')
    parser.add_argument('--scale', type=float, default=1.0)
    parser.add_argument('--folder-history', action='store_true', help='Also verify per-app remembered folders, explicit hints, cancellation and persistence across backend restart')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    args.binary = args.binary.resolve(strict=True)
    def digest(path):
        with path.open('rb') as stream: return hashlib.file_digest(stream, 'sha256').hexdigest()
    pins = {str(path): digest(path) for path in [Path(__file__).resolve(), args.binary, args.binary.with_name('canopa-files'),
        ROOT / 'crates/canopa-files/src/iced_desktop/picker_ui.rs',
        ROOT / 'crates/canopa-files/src/iced_desktop/view.rs',
        ROOT / 'crates/canopa-files/src/picker/folders.rs',
        ROOT / 'crates/canopa-files/src/picker/mod.rs',
        ROOT / 'crates/canopa-files/src/browser_ui.rs',
        ROOT / 'crates/canopa-files/src/browser_ui/breadcrumb.rs', ROOT / 'crates/canopa-files/src/browser_ui/entry.rs',
        ROOT / 'crates/canopa-files/src/browser_ui/text_layout.rs',
        ROOT / 'crates/canopa-portal/src/file_chooser.rs', ROOT / 'crates/canopa-portal/src/file_chooser/history.rs',
        ROOT / 'scripts/validate-desktop.py',
        ROOT / 'scripts/native_accessibility.py',
        ROOT / 'scripts/desktop-fixture/files-volume-monitor.c']}
    result = dict(passed=False, checks=[], scheme=args.color_scheme, scale=args.scale, pins=pins, folder_history=args.folder_history)
    with tempfile.TemporaryDirectory(prefix='canopa-files-native-chooser-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        try:
            result['checks'] = verify(args.binary, session, args.color_scheme, args.scale, args.folder_history)
            result['passed'] = True
        except Exception as error:
            result.update(error=repr(error), traceback=traceback.format_exc())
            result['completed_checks_before_failure'] = getattr(session, 'completed_checks', [])
            try: session.capture('failure.png')
            except Exception: pass
        finally:
            session.close()
            result['processes'] = [dict(pid=p.pid, exit_code=p.poll()) for p in session.processes]
            result['backend_pid'] = getattr(session, 'backend_pid', None)
            result['backend_pids'] = getattr(session, 'backend_pids', [])
    result['private_path_removed'] = not Path(directory).exists()
    result['changed_inputs'] = [name for name, sha in pins.items() if digest(Path(name)) != sha]
    if result['changed_inputs'] or not result['private_path_removed']: result['passed'] = False
    desktop.write_json(args.output / 'result.json', result)
    print(f"FileChooser: {len(result['checks'])} native checks passed; passed={result['passed']}")
    if not result['passed']:
        print(result.get('traceback', 'Source/cleanup verification failed')); raise SystemExit(1)


if __name__ == '__main__':
    main()
