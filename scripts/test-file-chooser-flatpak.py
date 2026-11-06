#!/usr/bin/env python3
"""Real private Flatpak -> normal Iced FileChooser -> Documents access grants."""
import argparse
import ctypes
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

ROOT = Path(__file__).resolve().parents[1]
APP = 'org.canopa.PortalFileChooserProbe'
BACKEND = 'org.freedesktop.impl.portal.desktop.canopa'
SPEC = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(desktop)


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def capacity(store, output):
    seen, allocated = set(), 0
    for root in [store, output.parent]:
        for path in root.rglob('*'):
            if path.is_symlink() or not path.is_file(): continue
            info = path.stat(); key = (info.st_dev, info.st_ino)
            if key not in seen: seen.add(key); allocated += info.st_blocks * 512
    assert allocated < 136 * 1024**2, allocated
    assert shutil.disk_usage(ROOT).free > 3 * 1024**3
    return allocated


def verify(args, result):
    from gi.repository import Gio, GLib
    store = args.flatpak_store
    result['fixture_allocated_bytes'] = capacity(store, args.output)
    build = json.loads(args.client_build.read_text())
    client = store / 'installation/app' / APP / 'x86_64/1/active/files/bin/chooser-probe'
    assert build['status'] == 'built' and digest(client) == build['sha256']
    assert digest(ROOT / 'scripts/desktop-fixture/file-chooser-flatpak.c') == build['source_sha256']
    flatpak = ['/usr/lib/ld-linux-x86-64.so.2', '--library-path', str(store / 'flatpak-tools/usr/lib'), str(store / 'flatpak-tools/usr/bin/flatpak')]
    result['client_sha256'] = build['sha256']
    with tempfile.TemporaryDirectory(prefix='canopa-flatpak-chooser-') as directory:
        base = Path(directory)
        for kind in ['user', 'mnt']:
            assert os.readlink('/proc/self/ns/' + kind) != os.environ['CANOPA_CHOOSER_PARENT_' + kind.upper()]
        runtime = Path('/run/user') / str(os.getuid())
        assert runtime.is_dir()
        libc = ctypes.CDLL(None, use_errno=True)
        if libc.mount(os.fsencode(base), os.fsencode(runtime), None, 4096, None):
            raise OSError(ctypes.get_errno(), 'Private runtime bind mount failed')
        session = desktop.Session(base, args.output)
        connection = None
        mount = runtime / 'doc'
        result['private_path'] = str(base)
        try:
            home = base / 'host-home'; home.mkdir()
            sentinel = home / 'unshared.txt'; sentinel.write_bytes(b'Canopa sandbox file selection\n')
            save_folder = home / 'Portal folder' if args.new_folder else home
            saved = save_folder / 'sandbox-saved.txt'
            config = base / 'canopa.kdl'; config.write_text('schema-version 1\nappearance { color-scheme "' + args.scheme + '"; }\n')
            portals = base / 'portals'; portals.mkdir()
            shutil.copyfile(ROOT / 'packaging/portal/canopa.portal', portals / 'canopa.portal')
            (portals / 'portals.conf').write_text('[preferred]\ndefault=none\norg.freedesktop.impl.portal.FileChooser=canopa\norg.freedesktop.impl.portal.Settings=canopa\n')
            session.env.update(HOME=str(home), XDG_RUNTIME_DIR=str(runtime), PATH=str(args.niri.parent) + os.pathsep + os.environ['PATH'],
                XDG_CURRENT_DESKTOP='Canopa', XDG_DESKTOP_PORTAL_DIR=str(portals),
                FLATPAK_USER_DIR=str(store / 'installation'), FLATPAK_SYSTEM_DIR=str(store / 'empty-system'),
                FLATPAK_CONFIG_DIR=str(store / 'config'), FLATPAK_SYSTEM_CACHE_DIR=str(store / 'system-cache'))
            session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; focus-ring { off; }; }\nwindow-rule { open-floating true; }\n')
            session.start(['wtype', '-s', '600000'], 'keyboard.log')
            pointer = session.build_pointer()
            logical = next(v['logical'] for v in json.loads(session.run(['niri', 'msg', '--json', 'outputs'])).values() if v.get('logical'))
            connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
            def owned(name):
                return connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', GLib.Variant('(s)', (name,)), None, Gio.DBusCallFlags.NONE, 1000, None).unpack()[0]
            session.start(['/usr/lib/xdg-permission-store'], 'permissions.log')
            session.wait_for(lambda: owned('org.freedesktop.impl.portal.PermissionStore'), 'private permission store')
            session.start(['/usr/lib/xdg-document-portal', '--verbose'], 'documents.log')
            session.wait_for(lambda: owned('org.freedesktop.portal.Documents'), 'private Documents portal')
            mounted = connection.call_sync('org.freedesktop.portal.Documents', '/org/freedesktop/portal/documents', 'org.freedesktop.portal.Documents', 'GetMountPoint', None, None, Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
            assert bytes(mounted).rstrip(b'\0') == os.fsencode(mount) and os.path.ismount(mount)
            session.start([str(args.binary), '--canopa-config', str(config)], 'backend.log')
            session.wait_for(lambda: owned(BACKEND), 'normal backend')
            session.start(['dbus-monitor', '--session', "type='method_call',interface='org.freedesktop.impl.portal.FileChooser'"], 'implementation.log')
            session.start([str(args.frontend), '--verbose'], 'frontend.log')
            session.wait_for(lambda: owned('org.freedesktop.portal.Desktop'), 'public frontend')
            def windows(): return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
            def action(*values): session.run(['niri', 'msg', 'action', *map(str, values)])
            def click(x, y):
                session.run([str(pointer), str(round(x / logical['width'] * 1600)), str(round(y / logical['height'] * 1000))]); time.sleep(.15)
            for mode in args.modes:
                capacity(store, args.output)
                with (args.output / (mode + '.log')).open('w') as log:
                    process = subprocess.Popen(['setpriv', '--inh-caps=-all', '--ambient-caps=-all', *flatpak,
                        'run', '--user', '--no-a11y-bus', '--env=LC_ALL=C', APP, mode, str(sentinel)],
                        env=session.env, stdout=log, stderr=log, start_new_session=True)
                session.processes.append(process)
                chooser = session.wait_for(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.Portal'), None), 'real sandbox Iced chooser')
                action('focus-window', '--id', chooser['id'])
                action('move-floating-window', '--id', chooser['id'], '--x', 20, '--y', 10)
                time.sleep(.4)
                chooser = next(w for w in windows() if w['id'] == chooser['id'])
                width, height = chooser['layout']['window_size']
                session.capture(mode + '-review.png')
                assert 'libgtk' not in Path(f"/proc/{chooser['pid']}/maps").read_text()
                if mode == 'cancel':
                    session.run(['wtype', '-k', 'Escape'])
                else:
                    if mode == 'save' and args.new_folder:
                        session.run(['wtype', '-M', 'ctrl', '-M', 'shift', '-k', 'n', '-m', 'shift', '-m', 'ctrl'])
                        time.sleep(.4)
                        session.run(['wtype', 'Portal folder'])
                        session.run(['wtype', '-k', 'Return'])
                        session.wait_for(save_folder.is_dir, 'explicit New folder inside sandbox Save review')
                        time.sleep(.3)
                    if mode == 'open':
                        click(320, 150)
                        click(38, 10 + height - 65)  # Explicit Open read-only choice.
                    click(20 + width - 42, 10 + height - 30)
                actual = process.wait(timeout=15)
                session.processes.remove(process)
                text = (args.output / (mode + '.log')).read_text()
                assert actual == 0, (mode, actual, text)
                assert 'IDENTITY ' + APP in text and 'HOST_FILE inaccessible' in text, text
                session.wait_for(lambda: all(w['id'] != chooser['id'] for w in windows()), 'Iced chooser retired')
                if mode == 'open':
                    assert 'READ_OK ' + digest(sentinel) in text and 'READ_ONLY ' in text
                elif mode == 'save':
                    assert 'SAVE_OK' in text and saved.read_bytes() == b"Written through Canopa's sandbox file chooser\n"
                    if args.check_folder_history:
                        import sqlite3
                        database = Path(session.env['XDG_STATE_HOME']) / 'canopa/portal-file-chooser.db'
                        with sqlite3.connect('file:' + str(database) + '?mode=ro', uri=True) as db:
                            rows = db.execute('SELECT app,path FROM folders').fetchall()
                        assert rows == [(APP, os.fsencode(save_folder))], rows
                        assert database.stat().st_mode & 0o777 == 0o600
                        result['checks'].append('authenticated-flatpak-folder-recorded-without-the-selected-filename')
                else:
                    assert 'RESPONSE 1' in text and 'DOCUMENT_URI ' not in text
                result['checks'].append(mode + ('-new-folder' if mode == 'save' and args.new_folder else '') + '-real-flatpak-identity-documents-and-iced-approval')
                (args.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
                print(result['checks'][-1], flush=True)
            assert sentinel.read_bytes() == b'Canopa sandbox file selection\n'
            assert APP in (args.output / 'implementation.log').read_text()
            result['checks'].append('authenticated-app-id-forwarded-and-host-original-preserved')
        except Exception:
            try: session.capture('failure.png')
            except Exception: pass
            raise
        finally:
            if connection: connection.close_sync(None)
            processes = list(session.processes)
            session.close()
            if os.path.ismount(mount): subprocess.run(['fusermount3', '-u', str(mount)], check=True, timeout=10)
            result['documents_unmounted'] = not os.path.ismount(mount)
            result['processes'] = [dict(pid=p.pid, command=p.args, exit_code=p.poll()) for p in processes]
            if libc.umount2(os.fsencode(runtime), 0):
                raise OSError(ctypes.get_errno(), 'Private runtime unmount failed')
            result['runtime_unmounted'] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['binary', 'niri', 'frontend']:
        parser.add_argument('--' + name, required=True, type=Path); parser.add_argument('--' + name + '-sha256', required=True)
    parser.add_argument('--flatpak-store', required=True, type=Path)
    parser.add_argument('--client-build', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--scheme', choices=['dark', 'light'], default='dark')
    parser.add_argument('--modes', choices=['open', 'save', 'cancel'], nargs='+', default=['open', 'save', 'cancel'])
    parser.add_argument('--new-folder', action='store_true', help='Create a folder in Iced before accepting Save, then verify the sandbox writes through its document URI')
    parser.add_argument('--check-folder-history', action='store_true', help='Verify the accepted folder is stored privately under the authenticated sandbox app ID')
    parser.add_argument('--inside-runtime-namespace', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if not args.inside_runtime_namespace:
        env = dict(os.environ)
        for kind in ['user', 'mnt']:
            env['CANOPA_CHOOSER_PARENT_' + kind.upper()] = os.readlink('/proc/self/ns/' + kind)
        os.execvpe('unshare', ['unshare', '--user', '--map-current-user', '--mount', '--keep-caps', sys.executable,
                             __file__, *sys.argv[1:], '--inside-runtime-namespace'], env)
    for name in ['binary', 'niri', 'frontend']:
        path = getattr(args, name).resolve(strict=True); setattr(args, name, path)
        if digest(path) != getattr(args, name + '_sha256'): parser.error(name + ' checksum mismatch')
    args.flatpak_store = args.flatpak_store.resolve(strict=True)
    if not args.flatpak_store.is_relative_to(ROOT / 'target'): parser.error('Use a fixture-owned Flatpak store under target/')
    args.client_build = args.client_build.resolve(strict=True)
    args.output = args.output.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(__file__, args.output / 'probe-source.py')
    shutil.copyfile(ROOT / 'scripts/desktop-fixture/file-chooser-flatpak.c', args.output / 'client-source.c')
    pins = {str(p): digest(p) for p in [Path(__file__), args.binary, args.niri, args.frontend, args.client_build, ROOT / 'scripts/desktop-fixture/file-chooser-flatpak.c', Path('/usr/lib/xdg-document-portal'), Path('/usr/lib/xdg-permission-store')]}
    result = dict(status='running', checks=[], pins=pins, scope=__doc__, modes=args.modes, new_folder=args.new_folder, check_folder_history=args.check_folder_history)
    try:
        verify(args, result); result['status'] = 'passed'
    except Exception as error:
        result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
    result['changed_inputs'] = [name for name, sha in pins.items() if digest(Path(name)) != sha]
    result['private_path_removed'] = not Path(result.get('private_path', '/nonexistent-canopa-fixture')).exists()
    if result['changed_inputs']: result['status'] = 'failed'
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({name: result.get(name) for name in ['status', 'error']}))
    if result['status'] != 'passed': print(result['traceback']); raise SystemExit(1)


if __name__ == '__main__': main()
