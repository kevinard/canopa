#!/usr/bin/env python3
"""Real Flatpak ScreenCast: Iced approval, restricted media FD and session retirement.

Uses an already installed, fixture-owned runtime and probe. Never downloads an
SDK, changes the live desktop, or connects to hardware audio/video devices.
"""
import argparse
import ctypes
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import traceback

sys.dont_write_bytecode = True
from screencast_media import MediaSession, BACKEND, FRONTEND, digest

ROOT = Path(__file__).resolve().parents[1]
APP = 'org.canopa.PortalScreenCastProbe'
FIXTURE = ROOT / 'scripts/desktop-fixture'
BOUNDS = [(0, 0, 137, 93), (0, 93, 137, 300),
          (137, 0, 400, 181), (137, 181, 400, 300)]


def capacity(store, output):
    seen, allocated = set(), 0
    for root in [store, output.parent]:
        for path in root.rglob('*'):
            if path.is_symlink() or not path.is_file(): continue
            info = path.stat(); key = (info.st_dev, info.st_ino)
            if key not in seen:
                seen.add(key); allocated += info.st_blocks * 512
    assert allocated < 136 * 1024**2, allocated
    assert shutil.disk_usage(ROOT).free > 3 * 1024**3
    return allocated


def verify(args, result):
    from gi.repository import Gio, GLib
    store = args.flatpak_store
    result['fixture_allocated_bytes_before'] = capacity(store, args.output)
    build = json.loads(args.client_build.read_text())
    deployed = store / 'installation/app' / APP / 'x86_64/1/active'
    assert build['status'] == 'built'
    assert digest(deployed / 'files/bin/sharing-probe') == build['sha256']
    assert all(digest(Path(path)) == sha for path, sha in build['source_inputs'].items())
    metadata = (deployed / 'metadata').read_text()
    assert '[Context]' not in metadata, 'Probe must have no filesystem/display permissions'
    runtime_ref = next(line.split('=', 1)[1] for line in metadata.splitlines() if line.startswith('runtime='))
    runtime_tree = store / 'installation/runtime' / runtime_ref / 'active'
    runtime_commit = runtime_tree.resolve().name
    result.update(client_sha256=build['sha256'], runtime_ref=runtime_ref,
                  runtime_commit=runtime_commit, runtime_rebuilt=False, sdk_downloaded=False)
    flatpak = ['/usr/lib/ld-linux-x86-64.so.2', '--library-path', str(store / 'flatpak-tools/usr/lib'),
               str(store / 'flatpak-tools/usr/bin/flatpak')]
    with tempfile.TemporaryDirectory(prefix='canopa-flatpak-sharing-') as directory:
        base = Path(directory)
        result['namespaces'] = {}
        for kind in ['user', 'mnt']:
            parent = os.environ['CANOPA_SHARING_PARENT_' + kind.upper()]
            current = os.readlink('/proc/self/ns/' + kind)
            assert current != parent
            result['namespaces'][kind] = dict(parent=parent, fixture=current)
        runtime = Path('/run/user') / str(os.getuid())
        assert runtime.is_dir()
        libc = ctypes.CDLL(None, use_errno=True)
        if libc.mount(os.fsencode(base), os.fsencode(runtime), None, 4096, None):
            raise OSError(ctypes.get_errno(), 'Private runtime bind mount failed')
        session = MediaSession(base, args.output, args.niri, args.niri_sha256, reference_frozen=True)
        connection = None
        result['private_path'] = str(base)
        try:
            home = base / 'home'; home.mkdir()
            config = base / 'canopa.kdl'
            config.write_text('schema-version 1\nappearance { color-scheme "' + args.scheme + '"; }\n')
            portals = base / 'portals'; portals.mkdir()
            (portals / 'canopa.portal').write_text(
                '[portal]\nDBusName=' + BACKEND + '\nInterfaces=org.freedesktop.impl.portal.ScreenCast;org.freedesktop.impl.portal.Settings;\nUseIn=Canopa;\n')
            (portals / 'portals.conf').write_text('[preferred]\ndefault=none\norg.freedesktop.impl.portal.ScreenCast=canopa\norg.freedesktop.impl.portal.Settings=canopa\n')
            session.env.update(HOME=str(home), XDG_RUNTIME_DIR=str(runtime), XDG_CURRENT_DESKTOP='Canopa',
                XDG_DESKTOP_PORTAL_DIR=str(portals), XDG_DATA_DIRS=session.env['XDG_DATA_HOME'],
                GIO_USE_VFS='local', GSETTINGS_BACKEND='memory', GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas',
                FLATPAK_USER_DIR=str(store / 'installation'), FLATPAK_SYSTEM_DIR=str(store / 'empty-system'),
                FLATPAK_CONFIG_DIR=str(store / 'config'), FLATPAK_SYSTEM_CACHE_DIR=str(store / 'system-cache'))
            if not args.stock_wireplumber:
                assets = ROOT / 'packaging/portal/desktop-sharing'
                for target, source in [
                        (Path(session.env['XDG_DATA_HOME']) / 'wireplumber/scripts/canopa-screencast.lua', assets / 'canopa-screencast.lua'),
                        (Path(session.env['XDG_CONFIG_HOME']) / 'wireplumber/wireplumber.conf.d/90-canopa-screencast.conf', assets / '90-canopa-screencast.conf')]:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(source, target)
            if args.permission_diagnostics:
                session.env.update(PIPEWIRE_DEBUG='mod.portal:4,pw.client:4', WIREPLUMBER_DEBUG='s-client:4')
            session.start_niri('debug { dbus-interfaces-in-non-session-instances; }\nprefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; focus-ring { off; }; }\n')
            connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
                Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
            def owned(name):
                return connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
                    'org.freedesktop.DBus', 'NameHasOwner', GLib.Variant('(s)', (name,)),
                    None, Gio.DBusCallFlags.NONE, 1000, None).unpack()[0]
            session.start(['/usr/lib/xdg-permission-store'], 'permissions.log')
            session.wait_for(lambda: owned('org.freedesktop.impl.portal.PermissionStore'), 'private permission store')
            session.start([str(args.binary), '--canopa-config', str(config)], 'backend.log')
            session.wait_for(lambda: owned(BACKEND), 'normal backend')
            session.start(['dbus-monitor', '--session', "type='method_call',interface='org.freedesktop.impl.portal.ScreenCast'"], 'implementation.log')
            session.start([str(args.frontend), '--verbose'], 'frontend.log')
            session.wait_for(lambda: owned(FRONTEND), 'public frontend')
            # A persistent keyboard keeps one keymap across all approval windows.
            for name, xml in [('virtual-keyboard', FIXTURE / 'virtual-keyboard-unstable-v1.xml'),
                              ('xdg-shell', Path('/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml')),
                              ('xdg-foreign', Path('/usr/share/wayland-protocols/unstable/xdg-foreign/xdg-foreign-unstable-v2.xml'))]:
                for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
                    subprocess.run(['wayland-scanner', mode, str(xml), str(args.output / (name + suffix))], check=True, timeout=15)
            keyboard_binary = args.output / 'keyboard'
            subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(args.output), str(FIXTURE / 'keyboard.c'),
                str(args.output / 'virtual-keyboard.c'), '-lwayland-client', '-lxkbcommon', '-o', str(keyboard_binary)], check=True, timeout=30)
            with (args.output / 'keyboard.log').open('w') as log:
                keyboard = subprocess.Popen([str(keyboard_binary)], env=session.env, stdin=subprocess.PIPE,
                    stdout=log, stderr=log, text=True, start_new_session=True)
            session.processes.append(keyboard)
            session.wait_for(lambda: 'ready' in (args.output / 'keyboard.log').read_text(), 'persistent keyboard')
            key_count = 0
            def keys(*codes):
                nonlocal key_count
                for code in codes:
                    keyboard.stdin.write(str(code) + '\n'); keyboard.stdin.flush(); key_count += 1
                    session.wait_for(lambda: (args.output / 'keyboard.log').read_text().splitlines().count('done') == key_count, 'key delivery')
            pattern_binary = args.output / 'pattern'
            subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-DPORTAL_CAPTURE_PATTERN', '-I', str(args.output),
                str(FIXTURE / 'portal-parent.c'), str(args.output / 'xdg-shell.c'), str(args.output / 'xdg-foreign.c'),
                '-lwayland-client', '-o', str(pattern_binary)], check=True, timeout=30)
            pattern = session.start([str(pattern_binary)], 'pattern.log')
            def windows(): return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
            def action(*values): session.run(['niri', 'msg', 'action', *map(str, values)])
            source = session.wait_for(lambda: next((w for w in windows() if w['pid'] == pattern.pid), None), 'pattern window')
            if not source['is_floating']: action('move-window-to-floating', '--id', source['id'])
            action('set-window-width', '--id', source['id'], 400)
            action('set-window-height', '--id', source['id'], 300)
            session.wait_for(lambda: any(w['id'] == source['id'] and tuple(w['layout']['window_size']) == (400, 300) for w in windows()), 'pattern geometry')
            action('move-floating-window', '--id', source['id'], '--x', 850, '--y', 450)
            # Host-only node proves the portal's FD is narrower than the host registry.
            session.start(['pw-cli', '-m', 'create-node', 'spa-node-factory',
                '{ factory.name = support.node.driver node.name = canopa-sandbox-unshared object.linger = false }'], 'unshared-node.log')
            unshared = session.wait_for(lambda: next((entry for entry in session.registry()
                if entry.get('type') == 'PipeWire:Interface:Node'
                and entry.get('info', {}).get('props', {}).get('node.name') == 'canopa-sandbox-unshared'), None), 'unrelated private PipeWire node')
            forbidden = unshared['id']
            result['unrelated_node'] = dict(id=forbidden, serial=unshared['info']['props']['object.serial'])
            def wait(condition, description, timeout=30):
                end = time.monotonic() + timeout
                while time.monotonic() < end:
                    value = condition()
                    if value: return value
                    for process in session.processes:
                        if process.poll() is not None:
                            raise RuntimeError(f'{description}: process {process.args} exited {process.returncode}')
                    time.sleep(.05)
                raise RuntimeError('Timed out waiting for ' + description)
            for mode in args.modes:
                log_path = args.output / (mode + '.log')
                with log_path.open('w') as log:
                    client = subprocess.Popen(['setpriv', '--inh-caps=-all', '--ambient-caps=-all', *flatpak,
                        'run', '--user', '--no-a11y-bus', '--env=LC_ALL=C', APP, mode,
                        str(base / 'pipewire-0'), str(forbidden)], env=session.env, stdin=subprocess.PIPE,
                        stdout=log, stderr=log, text=True, start_new_session=True)
                session.processes.append(client)
                consent = wait(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.Portal'
                    and w['title'] == 'Share your screen — Canopa'), None), 'sandbox Iced consent')
                if not consent['is_floating']: action('move-window-to-floating', '--id', consent['id'])
                action('set-window-width', '--id', consent['id'], 680)
                action('set-window-height', '--id', consent['id'], 600)
                session.wait_for(lambda: any(w['id'] == consent['id'] and tuple(w['layout']['window_size']) == (680, 600) for w in windows()), 'consent geometry')
                action('move-floating-window', '--id', consent['id'], '--x', 20, '--y', 10)
                action('focus-window', '--id', consent['id']); time.sleep(.3)
                assert 'libgtk' not in Path(f"/proc/{consent['pid']}/maps").read_text()
                session.capture(mode + '-consent.png')
                if mode == 'cancel':
                    keys(28, 'ctrl 28'); time.sleep(.3)
                    assert 'RESPONSE Start' not in log_path.read_text(), 'unselected consent granted'
                    keys(1)
                else:
                    keys(108, 57, 'ctrl 28')
                    wait(lambda: 'FRAME_READY' in log_path.read_text(), 'sandbox pixels')
                    lines = log_path.read_text().splitlines()
                    node, serial = map(int, next(line.split()[1:] for line in lines if line.startswith('STREAM ')))
                    frame = next(line.split()[1:] for line in lines if line.startswith('FRAME '))
                    assert frame[:2] == ['400', '300'] and len(frame[2]) == 64, frame
                    colors = [list(map(int, line.split()[1:])) for line in lines if line.startswith('COLOR ')]
                    assert len(colors) == 4
                    for index, bounds in enumerate(BOUNDS):
                        expected = (bounds[2] - bounds[0]) * (bounds[3] - bounds[1])
                        assert colors[index] == [index, expected, *bounds], colors
                    visible = [int(line.split()[1]) for line in lines if line.startswith('VISIBLE_NODE ')]
                    assert node in visible and forbidden not in visible
                    assert 'REGISTRY_RESTRICTED' in lines
                    result['frames'].append(dict(mode=mode, node=node, serial=serial, sha256=frame[2], colors=colors, visible_nodes=visible))
                    controls = wait(lambda: next((w for w in windows() if w['app_id'] == 'org.canopa.Portal'
                        and w['title'] == 'Screen sharing — Canopa'), None), 'ongoing controls')
                    if mode == 'stop':
                        action('focus-window', '--id', controls['id']); time.sleep(.2); keys(1)
                    else:
                        client.stdin.write('exit\n'); client.stdin.flush()
                code = client.wait(timeout=15)
                client.stdin.close()
                session.processes.remove(client)
                result['clients'].append(dict(mode=mode, pid=client.pid, exit_code=code))
                text = log_path.read_text()
                assert code == 0, (mode, code, text)
                assert 'IDENTITY ' + APP in text and 'HOST_PIPEWIRE_DENIED' in text and 'PRIVATE_BACKEND_DENIED' in text, text
                session.wait_for(lambda: all(w['id'] != consent['id'] for w in windows()), 'consent retired')
                if mode == 'cancel':
                    assert 'CANCELLED_NO_STREAMS' in text and 'STREAM ' not in text
                else:
                    if mode == 'stop': assert 'SESSION_CLOSED' in text and 'CLOSED_REMOTE_DENIED' in text
                    else: assert 'EXIT_WITHOUT_SESSION_CLOSE' in text
                    session.wait_for(lambda: all(w['id'] != controls['id'] for w in windows()), 'controls retired')
                    session.wait_for(lambda: all(entry['id'] != node or
                        str(entry.get('info', {}).get('props', {}).get('object.serial')) != str(serial)
                        for entry in session.registry()), 'producer retired')
                result['checks'].append(mode + '-real-flatpak-iced-restricted-media-and-retirement')
                (args.output / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
                print(result['checks'][-1], flush=True)
            assert APP in (args.output / 'implementation.log').read_text()
            assert runtime_tree.resolve().name == runtime_commit
            result['checks'].append('authenticated-sandbox-app-id-forwarded-without-host-registration')
        except Exception:
            try: session.capture('failure.png')
            except Exception: pass
            raise
        finally:
            if connection: connection.close_sync(None)
            processes = list(session.processes)
            try:
                session.close()
                result['processes'] = [dict(pid=p.pid, command=p.args, exit_code=p.poll()) for p in processes]
            finally:
                if libc.umount2(os.fsencode(runtime), 0):
                    raise OSError(ctypes.get_errno(), 'Private runtime unmount failed')
                result['runtime_unmounted'] = True
    result['fixture_allocated_bytes_after'] = capacity(store, args.output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['binary', 'niri', 'frontend']:
        parser.add_argument('--' + name, required=True, type=Path)
        parser.add_argument('--' + name + '-sha256', required=True)
    parser.add_argument('--flatpak-store', required=True, type=Path)
    parser.add_argument('--client-build', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--scheme', choices=['dark', 'light'], default='dark')
    parser.add_argument('--modes', choices=['cancel', 'stop', 'exit'], nargs='+', default=['cancel', 'stop', 'exit'])
    parser.add_argument('--permission-diagnostics', action='store_true', help='Log only PipeWire portal/client and WirePlumber access decisions')
    parser.add_argument('--stock-wireplumber', action='store_true', help='Reproduce the upstream permission-widening baseline without the Canopa policy')
    parser.add_argument('--inside-runtime-namespace', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if not args.inside_runtime_namespace:
        env = dict(os.environ)
        for kind in ['user', 'mnt']:
            env['CANOPA_SHARING_PARENT_' + kind.upper()] = os.readlink('/proc/self/ns/' + kind)
        os.execvpe('unshare', ['unshare', '--user', '--map-current-user', '--mount', '--keep-caps',
            sys.executable, __file__, *sys.argv[1:], '--inside-runtime-namespace'], env)
    for name in ['binary', 'niri', 'frontend']:
        path = getattr(args, name).resolve(strict=True); setattr(args, name, path)
        if digest(path) != getattr(args, name + '_sha256'): parser.error(name + ' checksum mismatch')
    args.flatpak_store = args.flatpak_store.resolve(strict=True)
    args.client_build = args.client_build.resolve(strict=True)
    args.output = args.output.resolve()
    if not args.flatpak_store.is_relative_to(ROOT / 'target') or not args.output.is_relative_to(ROOT / 'target'):
        parser.error('Use fixture-owned store and output under root target/')
    args.output.mkdir(parents=True, exist_ok=False)
    paths = [Path(__file__), args.binary, args.niri, args.frontend, args.client_build,
             *[FIXTURE / name for name in ['screencast-flatpak.c', 'screencast-consumer.c', 'portal-parent.c', 'keyboard.c', 'virtual-keyboard-unstable-v1.xml']],
             *[ROOT / 'scripts' / name for name in ['screencast_media.py', 'validate-desktop.py', 'native_compositor.py']],
             *[ROOT / 'packaging/portal/desktop-sharing' / name for name in ['canopa-screencast.lua', '90-canopa-screencast.conf']],
             Path('/usr/lib/xdg-permission-store')]
    pins = {str(path): digest(path) for path in paths}
    shutil.copyfile(__file__, args.output / 'probe-source.py')
    shutil.copyfile(FIXTURE / 'screencast-flatpak.c', args.output / 'client-source.c')
    result = dict(status='running', scope=__doc__, checks=[], frames=[], clients=[], pins=pins,
                  wireplumber_policy='stock' if args.stock_wireplumber else 'canopa-screencast', modes=args.modes)
    try:
        verify(args, result); result['status'] = 'passed'
    except Exception as error:
        result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
    result['changed_inputs'] = [name for name, sha in pins.items() if digest(Path(name)) != sha]
    result['private_path_removed'] = not Path(result.get('private_path', '/nonexistent-canopa-fixture')).exists()
    if result['changed_inputs'] or not result['private_path_removed']: result['status'] = 'failed'
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({name: result.get(name) for name in ['status', 'error']}))
    if result['status'] != 'passed':
        print(result.get('traceback', 'Cleanup/input verification failed')); raise SystemExit(1)


if __name__ == '__main__': main()
