#!/usr/bin/env python3
"""Verify ordinary toolkit file-dialog routing to Iced in a private Niri session.

Uses installed toolkit development files. Caches one tiny client per build recipe;
does not rebuild Rust, install a runtime, or modify the live session.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec); spec.loader.exec_module(desktop)


def digest(path):
    with path.open('rb') as stream: return hashlib.file_digest(stream, 'sha256').hexdigest()


def source_for(toolkit):
    return ROOT / ('scripts/desktop-fixture/file-chooser-' + toolkit + ('.cpp' if toolkit == 'qt' else '.c'))


def client_build(toolkit):
    package = 'Qt6Widgets' if toolkit == 'qt' else 'gtk4'
    driver = 'c++' if toolkit == 'qt' else 'cc'
    source = source_for(toolkit)
    version = subprocess.check_output(['pkg-config', '--modversion', package], text=True).strip()
    flags = shlex.split(subprocess.check_output(['pkg-config', '--cflags', '--libs', package], text=True))
    compiler = subprocess.check_output([driver, '--version'], text=True).splitlines()[0]
    recipe = dict(source_sha256=digest(source), compiler=compiler, flags=flags,
                  options=['-std=c++20' if toolkit == 'qt' else '-std=c11', '-O1', '-g0', '-Wall', '-Wextra', '-Werror', '-fPIC'])
    recipe[toolkit + '_version'] = version
    key = hashlib.sha256(json.dumps(recipe, sort_keys=True).encode()).hexdigest()
    store = ROOT / ('target/portal-filechooser-' + toolkit + '-client');store.mkdir(exist_ok=True)
    program, receipt = store / ('client-' + key[:16]), store / ('build-' + key[:16] + '.json')
    if receipt.exists():
        result = json.loads(receipt.read_text())
        assert result['recipe'] == recipe and result['sha256'] == digest(program)
    else:
        assert not program.exists(), 'Existing unreceipted client must be inspected before rebuilding'
        command = [driver, *recipe['options'], str(source), *flags, '-o', str(program)]
        subprocess.run(command, check=True, timeout=60)
        result = dict(recipe=recipe, program=str(program), sha256=digest(program), bytes=program.stat().st_size, command=command)
        receipt.write_text(json.dumps(result, indent=2) + '\n')
    return receipt, result


def verify(args, session, result, build):
    from gi.repository import Gio, GLib
    label = 'Qt' if args.toolkit == 'qt' else 'GTK'
    original_contents = label + ' chooser original\n'
    saved_contents = 'Written by an ordinary ' + label + ' file dialog caller\n'
    home = session.runtime / 'home';home.mkdir()
    folder = home / (label + ' files');folder.mkdir()
    (folder / 'folder').mkdir()
    original = folder / 'a.txt';original.write_text(original_contents)
    (folder / 'hidden-by-filter.png').write_text('Must be hidden by the text filter')
    saved = folder / (label + ' saved été.txt')
    config = Path(session.env['XDG_CONFIG_HOME']) / 'canopa/config.kdl';config.parent.mkdir(parents=True)
    config.write_text('schema-version 1\nappearance { color-scheme "dark"; }\n')
    portals = session.runtime / 'portals';portals.mkdir()
    shutil.copyfile(ROOT / 'packaging/portal/canopa.portal', portals / 'canopa.portal')
    base = session.runtime / 'base-portals.conf';base.write_text('[preferred]\ndefault=none\n')
    subprocess.run([sys.executable, str(ROOT / 'scripts/portal-config.py'), '--base', str(base),
                    '--portal-dir', str(portals), '--output', str(portals / 'portals.conf')], check=True, capture_output=True)
    services = session.runtime / 'services';services.mkdir()
    service = 'org.freedesktop.impl.portal.desktop.canopa.service'
    shutil.copyfile(ROOT / 'packaging/portal' / service, services / service)
    (services / 'org.freedesktop.portal.Desktop.service').write_text('[D-BUS Service]\nName=org.freedesktop.portal.Desktop\nExec=' + shlex.quote(str(args.frontend)) + '\n')
    session.env.update(HOME=str(home), PATH=str(args.binary.parent) + os.pathsep + session.env['PATH'],
                       XDG_DESKTOP_PORTAL_DIR=str(portals), XDG_CURRENT_DESKTOP='Canopa',
                       WAYLAND_DEBUG='1', GIO_USE_VFS='local', GSETTINGS_BACKEND='memory')
    if args.toolkit == 'qt':
        session.env.update(QT_QPA_PLATFORM='wayland', QT_QPA_PLATFORMTHEME='xdgdesktopportal')
        session.env.pop('QT_STYLE_OVERRIDE', None)
    else:
        for name in ['GTK_USE_PORTAL', 'GDK_DEBUG', 'QT_QPA_PLATFORM', 'QT_QPA_PLATFORMTHEME']:
            session.env.pop(name, None)
        session.env.update(GDK_BACKEND='wayland', GSK_RENDERER='cairo', GTK_A11Y='none')
    niri = Path(shutil.which('niri', path=session.env['PATH'])).resolve(strict=True)
    result['pins'][str(niri)] = digest(niri)
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; focus-ring { off; }; }\n', service_directories=[services])
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    def call(destination, path, interface, method, signature, values):
        return connection.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'UpdateActivationEnvironment', '(a{ss})', ({k:v for k,v in session.env.items()},))
    session.start([str(args.frontend), '--verbose'], 'frontend.log')
    session.start(['dbus-monitor', '--session', "type='method_call',interface='org.freedesktop.portal.FileChooser'", "type='method_call',interface='org.freedesktop.impl.portal.FileChooser'"], 'portal-calls.log')
    call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'StartServiceByName', '(su)', ('org.freedesktop.impl.portal.desktop.canopa', 0))
    backend = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', '(s)', ('org.freedesktop.impl.portal.desktop.canopa',))[0]
    result['backend_pid'] = backend
    assert Path(f'/proc/{backend}/exe').resolve() == args.binary
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    pointer = session.build_pointer()
    def windows(): return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
    def chooser(): return next((w for w in windows() if w['app_id'] == 'org.canopa.Portal'), None)
    def click(x, y): session.run([str(pointer), str(round(x / 1280 * 1600)), str(round(y / 800 * 1000))])
    def key(name): session.run(['wtype', '-k', name]);time.sleep(.15)
    def complete(process):
        session.wait_for(lambda: process.poll() is not None, label + ' file dialog result', timeout=15)
        assert process.returncode == 0, process.returncode
        session.wait_for(lambda: chooser() is None, 'Iced chooser closes')
        desktop.stop(process)
    clients = result.setdefault('clients', [])
    try:
        version = session.wait_for(lambda: call('org.freedesktop.portal.Desktop', '/org/freedesktop/portal/desktop', 'org.freedesktop.DBus.Properties', 'Get', '(ss)', ('org.freedesktop.portal.FileChooser', 'version'))[0], 'public FileChooser version')
        result['public_file_chooser_version'] = version
        assert version == 4, version
        activatable = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'ListActivatableNames', '()', ())[0]
        result['frontend_activatable'] = 'org.freedesktop.portal.Desktop' in activatable
        assert result['frontend_activatable'], 'GTK requires standard frontend activation metadata'
        for mode in ['open', 'save', 'cancel']:
            before = (session.output / 'portal-calls.log').read_text()
            process = session.start([build['program'], mode, str(folder)], mode + '.log')
            clients.append(dict(pid=process.pid, mode=mode))
            session.wait_for(lambda: any(w['title'] == label + ' file chooser parent' for w in windows()), label + ' parent window')
            if args.toolkit == 'qt':
                clients[-1]['portal_theme_loaded'] = 'libqxdgdesktopportal.so' in Path(f'/proc/{process.pid}/maps').read_text()
                assert clients[-1]['portal_theme_loaded'], 'Qt did not load the requested portal platform theme'
            window = session.wait_for(chooser, label + ' must open an Iced portal window')
            assert window['is_floating'], window
            session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
            session.run(['niri', 'msg', 'action', 'move-floating-window', '--id', str(window['id']), '--x', '20', '--y', '10'])
            time.sleep(.6)
            caller_maps = Path(f'/proc/{process.pid}/maps').read_text()
            if args.toolkit == 'qt':
                assert 'libqxdgdesktopportal.so' in caller_maps and 'libQt6Widgets.so' in caller_maps
                assert 'libgtk-' not in caller_maps
                libraries = sorted({line.split()[-1] for line in caller_maps.splitlines() if '/libQt6' in line or '/libqxdgdesktopportal.so' in line})
            else:
                assert 'libgtk-4.so' in caller_maps
                libraries = sorted({line.split()[-1] for line in caller_maps.splitlines() if '/libgtk-4.so' in line or '/libgio-2.0.so' in line})
            result.setdefault(args.toolkit + '_loaded_libraries', libraries)
            for path in libraries:
                if path not in result['pins']:result['pins'][path] = digest(Path(path))
            children = {int(pid) for task in Path(f'/proc/{backend}/task').iterdir() for pid in (task / 'children').read_text().split()}
            helpers = [pid for pid in children if b'--file-chooser' in Path(f'/proc/{pid}/cmdline').read_bytes()]
            assert len(helpers) == 1, helpers
            clients[-1]['helper_pid'] = helpers[0]
            helper_maps = Path(f'/proc/{helpers[0]}/maps').read_text()
            assert 'libgtk-' not in helper_maps
            if mode == 'open':
                session.capture('01-' + args.toolkit + '-opens-iced.png')
                click(320, 218)
            if mode == 'cancel': key('Escape')
            else:
                width, height = chooser()['layout']['window_size']
                assert not saved.exists(), 'Iced must not write the application document'
                click(20 + width - 42, 10 + height - 30)
            # Session.wait_for treats exited tracked clients as service failures;
            # remove the caller while awaiting its expected normal exit.
            session.processes.remove(process)
            try: complete(process)
            finally:
                if process not in session.processes and process.poll() is None: session.processes.append(process)
            lines = (session.output / (mode + '.log')).read_text().splitlines()
            if args.toolkit == 'qt':
                evidence = next(json.loads(line) for line in lines if line.startswith('{') and '"qt_version"' in line)
            else:
                payload = next(line.removeprefix('GTK_RESULT ') for line in lines if line.startswith('GTK_RESULT '))
                evidence = GLib.Variant.parse(None, payload, None, None).unpack()
            assert evidence['mode'] == mode and evidence[args.toolkit + '_version'] == build['recipe'][args.toolkit + '_version']
            trace = (session.output / 'portal-calls.log').read_text()[len(before):]
            method = 'SaveFile' if mode == 'save' else 'OpenFile'
            assert f'interface=org.freedesktop.portal.FileChooser; member={method}' in trace
            assert f'interface=org.freedesktop.impl.portal.FileChooser; member={method}' in trace
            if mode == 'cancel':
                assert evidence['dialog_result'] == 0 and evidence['paths'] == [] and evidence['contents'] == ''
            else:
                assert evidence['dialog_result'] == 1
                assert evidence['paths'] == [str(saved if mode == 'save' else original)], evidence
                if args.toolkit == 'qt':assert evidence['selected_filter'] == 'Text documents (*.txt)', evidence
                else:assert 'string "Text documents"' in trace and 'string "*.txt"' in trace
                assert evidence['contents'] == (saved_contents if mode == 'save' else original_contents)
            assert original.read_text() == original_contents
            clients[-1].update(exit_code=process.returncode, evidence=evidence)
            result['checks'].append('ordinary-' + label + '-' + mode + '-uses-public-portal-and-GTK-free-Iced')
        assert saved.read_text() == saved_contents
    finally:
        connection.close_sync(None)
        try:
            if os.fsencode(session.runtime) in Path(f'/proc/{backend}/environ').read_bytes():os.kill(backend, 15)
        except ProcessLookupError:pass
        except FileNotFoundError:pass


def main(toolkit):
    assert toolkit in ['qt', 'gtk']
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--binary-sha256', required=True)
    parser.add_argument('--frontend', required=True, type=Path)
    parser.add_argument('--frontend-sha256', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    args.toolkit = toolkit
    for name in ['binary', 'frontend']:
        path = getattr(args, name).resolve(strict=True);setattr(args, name, path)
        if digest(path) != getattr(args, name + '_sha256'):parser.error(name + ' checksum mismatch')
    args.output = args.output.resolve();args.output.mkdir(parents=True, exist_ok=False)
    assert shutil.disk_usage(ROOT).free > 3 * 1024**3
    source = source_for(toolkit)
    receipt, build = client_build(toolkit)
    pins = {str(p):digest(p) for p in [Path(__file__), Path(sys.argv[0]).resolve(), source, args.binary, args.frontend, Path(build['program']), ROOT / 'scripts/portal-config.py', ROOT / 'scripts/validate-desktop.py']}
    result = dict(status='running', checks=[], pins=pins, client_build=str(receipt), client=build,
                  toolkit=toolkit, sandbox=False)
    if toolkit == 'qt':result.update(qt_platform='wayland', qt_platform_theme='xdgdesktopportal')
    else:result.update(gdk_backend='wayland', portal_forcing_env=False)
    shutil.copyfile(source, args.output / ('client-source' + source.suffix))
    with tempfile.TemporaryDirectory(prefix='canopa-chooser-' + toolkit + '-') as directory:
        result['private_path'] = directory
        session = desktop.Session(Path(directory), args.output)
        try:
            verify(args, session, result, build);result['status'] = 'passed'
        except Exception as error:
            result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
            try:session.capture('failure.png')
            except Exception:pass
        finally:
            session.close()
            result['processes'] = [dict(pid=p.pid, exit_code=p.poll()) for p in session.processes]
    result['private_path_removed'] = not Path(directory).exists()
    result['changed_inputs'] = [p for p,h in pins.items() if digest(Path(p)) != h]
    if result['changed_inputs'] or not result['private_path_removed']:result['status'] = 'failed'
    desktop.write_json(args.output / 'result.json', result)
    print(json.dumps({k:result.get(k) for k in ['status','checks','error']}))
    if result['status'] != 'passed':print(result.get('traceback','Input or cleanup changed'));raise SystemExit(1)
