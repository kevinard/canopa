#!/usr/bin/env python3
"""Test D-Bus activation and the public Settings portal on a private bus."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from xml.sax.saxutils import escape

from gi.repository import Gio, GLib

ROOT = Path(__file__).resolve().parents[1]
NAME = 'org.freedesktop.impl.portal.desktop.canopa'
OBJECT = '/org/freedesktop/portal/desktop'
INTERFACE = 'org.freedesktop.portal.Settings'


def wait(predicate, description):
    deadline = time.monotonic() + 10
    last = None
    while time.monotonic() < deadline:
        try:
            value = predicate()
            if value:
                return value
        except GLib.Error as error:
            last = error
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)
        time.sleep(0.02)
    raise AssertionError(f'Timed out: {description}: {last}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--frontend', type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    frontend = args.frontend or next((p for p in [Path('/usr/libexec/xdg-desktop-portal'), Path('/usr/lib/xdg-desktop-portal')] if p.is_file()), None)
    if not binary.is_file() or not frontend:
        parser.error('build the Canopa backend and install xdg-desktop-portal')
    with tempfile.TemporaryDirectory(prefix='canopa-settings-frontend-') as directory:
        root = Path(directory)
        for part in ['runtime', 'services', 'portals', 'config/canopa', 'config/xdg-desktop-portal', 'data']:
            (root / part).mkdir(parents=True, mode=0o700)
        shutil.copy(ROOT / 'packaging/portal/canopa.portal', root / 'portals/canopa.portal')
        shutil.copy(ROOT / f'packaging/portal/{NAME}.service', root / f'services/{NAME}.service')
        base = root / 'base-portals.conf'
        base.write_bytes(b'# Keep the complete existing selection\r\n[preferred]\r\ndefault=none\r\norg.freedesktop.impl.portal.Settings=none\r\n[retained]\r\nvalue=untouched\r\n[preferred]\r\norg.freedesktop.impl.portal.Settings=none\r\n')
        subprocess.run([sys.executable, str(ROOT / 'scripts/portal-config.py'),
                        '--base', str(base), '--interface', 'Settings',
                        '--portal-dir', str(root / 'portals'),
                        '--output', str(root / 'portals/portals.conf')],
                       check=True, capture_output=True)
        config = root / 'config/canopa/config.kdl'
        config.write_text('schema-version 1\nappearance { color-scheme "dark"; accent "#123456"; }\n')
        bus_config = root / 'bus.conf'
        bus_config.write_text(f'<busconfig><type>session</type><listen>unix:tmpdir={escape(str(root))}</listen><auth>EXTERNAL</auth><servicedir>{escape(str(root / "services"))}</servicedir><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        env = {**os.environ, 'PATH': f'{binary.parent}:{os.environ.get("PATH", "")}',
               'XDG_CONFIG_HOME': str(root / 'config'), 'XDG_DATA_HOME': str(root / 'data'),
               'XDG_STATE_HOME': str(root / 'state'),
               'XDG_RUNTIME_DIR': str(root / 'runtime'), 'XDG_CURRENT_DESKTOP': 'Canopa',
               'XDG_DESKTOP_PORTAL_DIR': str(root / 'portals')}
        env.pop('NIRI_SOCKET', None)
        processes = []
        backend_pid = None
        connection = None
        try:
            with (root / 'bus.log').open('w') as log:
                bus = subprocess.Popen(['dbus-daemon', '--nofork', '--print-address=1', f'--config-file={bus_config}'], stdout=subprocess.PIPE, stderr=log, text=True, env=env)
                processes.append(bus)
                address = bus.stdout.readline().strip()
            env['DBUS_SESSION_BUS_ADDRESS'] = address
            connection = Gio.DBusConnection.new_for_address_sync(address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

            def call(destination, path, interface, method, parameters=None):
                return connection.call_sync(destination, path, interface, method, parameters, None, Gio.DBusCallFlags.NONE, 2000, None).unpack()

            with (root / 'frontend.log').open('w') as log:
                processes.append(subprocess.Popen([str(frontend), '--verbose'], env=env, stdout=log, stderr=subprocess.STDOUT))
            def read(key):
                return call('org.freedesktop.portal.Desktop', OBJECT, INTERFACE, 'ReadOne', GLib.Variant('(ss)', ('org.freedesktop.appearance', key)))[0]
            wait(lambda: read('color-scheme') == 1, 'public Settings ReadOne')
            assert f"Using portal configuration file '{root / 'portals/portals.conf'}'" in (root / 'frontend.log').read_text()
            backend_pid = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', GLib.Variant('(s)', (NAME,)))[0]
            assert Path(f'/proc/{backend_pid}/exe').resolve() == binary, 'frontend must activate the requested binary'
            # Private harnesses register these separately; verify the actual
            # activated backend also exposes their shared session controller.
            for interface, version in [('ScreenCast', 6), ('RemoteDesktop', 2), ('Clipboard', 1)]:
                properties = call(NAME, OBJECT, 'org.freedesktop.DBus.Properties', 'GetAll',
                                  GLib.Variant('(s)', (f'org.freedesktop.impl.portal.{interface}',)))[0]
                assert properties['version'] == version, (interface, properties)
                if interface == 'RemoteDesktop':
                    assert properties['AvailableDeviceTypes'] == 0, properties
            all_settings = call('org.freedesktop.portal.Desktop', OBJECT, INTERFACE, 'ReadAll', GLib.Variant('(as)', (['org.freedesktop.*'],)))[0]
            assert all_settings['org.freedesktop.appearance']['color-scheme'] == 1
            assert tuple(read('accent-color')) == (18 / 255, 52 / 255, 86 / 255)
            changes = []
            subscription = connection.signal_subscribe('org.freedesktop.portal.Desktop', INTERFACE, 'SettingChanged', OBJECT, None, Gio.DBusSignalFlags.NONE,
                                                       lambda conn, sender, path, iface, name, params: changes.append(params.unpack()))
            # Drain the subscription's AddMatch before triggering another process.
            call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetId')
            # Atomic file replacement matches common editor saves.
            pending = config.with_suffix('.new')
            pending.write_text('schema-version 1\nappearance { color-scheme "light"; accent "#abcdef"; }\n')
            pending.replace(config)
            os.kill(backend_pid, signal.SIGHUP)
            wait(lambda: len(changes) == 2, 'public SettingChanged signals')
            assert {row[1] for row in changes} == {'color-scheme', 'accent-color'}
            assert read('color-scheme') == 2
            assert tuple(read('accent-color')) == (171 / 255, 205 / 255, 239 / 255)
            connection.signal_unsubscribe(subscription)
            print(json.dumps({'passed': True, 'generated_backend_selection': True, 'dbus_activation': True, 'frontend_read_one': True, 'frontend_read_all': True, 'frontend_reload_signals': True, 'desktop_sharing_registered': True, 'unavailable_input_devices_denied': True}))
        except BaseException:
            for name in ['bus.log', 'frontend.log']:
                if (root / name).exists():
                    print((root / name).read_text())
            raise
        finally:
            if backend_pid:
                try:
                    os.kill(backend_pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
            if connection:
                connection.close_sync(None)
            for process in reversed(processes):
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


if __name__ == '__main__':
    main()
