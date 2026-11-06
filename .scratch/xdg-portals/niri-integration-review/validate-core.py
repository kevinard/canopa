#!/usr/bin/env python3
"""Check the review config/user descriptor search on an isolated session bus."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time
import traceback
from xml.sax.saxutils import escape

from gi.repository import Gio, GLib

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
BACKEND = 'org.freedesktop.impl.portal.desktop.canopa'
PUBLIC = 'org.freedesktop.portal.Desktop'
OBJECT = '/org/freedesktop/portal/desktop'


def main():
    validation = OUT / 'validation'
    binary = ROOT / 'target/debug/xdg-desktop-portal-canopa'
    frontend = ROOT / 'target/native-test-snapshots/frontend-by-sha256/277568bfecc9e75c1a155a370a69f7a992c335bf6745344601bac54cea132233/xdg-desktop-portal'
    identities = json.loads((ROOT / 'target/portal-window-screenshot-95/delivery-identities-01.json').read_text())['identities']
    for path, member in [(binary, 'bin/xdg-desktop-portal-canopa'), (frontend, 'libexec/canopa/desktop-sharing/xdg-desktop-portal')]:
        with path.open('rb') as stream:
            assert hashlib.file_digest(stream, 'sha256').hexdigest() == identities['canopa-x86_64-unknown-linux-gnu/' + member]['sha256']
    processes = []
    connection = None
    result = dict(status='running',checks=[],activation_scope='Backend starts explicitly; installed systemd activation is not exercised.')
    with tempfile.TemporaryDirectory(prefix='canopa-niri-review-') as folder:
        runtime = Path(folder)
        env = {**os.environ, 'XDG_RUNTIME_DIR': folder, 'XDG_CONFIG_HOME': folder + '/config',
               'XDG_DATA_HOME': folder + '/data', 'XDG_STATE_HOME': folder + '/state',
               'XDG_CACHE_HOME': folder + '/cache', 'XDG_CURRENT_DESKTOP': 'niri',
               'DBUS_SYSTEM_BUS_ADDRESS': 'unix:path=' + folder + '/no-system-bus', 'GIO_USE_VFS': 'local'}
        for key in ['XDG_DESKTOP_PORTAL_DIR', 'NIRI_SOCKET', 'WAYLAND_SOCKET', 'WAYLAND_DISPLAY', 'DISPLAY']:
            env.pop(key, None)
        configuration = runtime / 'config/xdg-desktop-portal/niri-portals.conf'
        configuration.parent.mkdir(parents=True)
        shutil.copyfile(OUT / 'core-niri-portals.conf', configuration)
        descriptors = runtime / 'data/xdg-desktop-portal/portals'
        shutil.copytree(OUT / 'payload/share/xdg-desktop-portal/portals', descriptors)
        bus_config = runtime / 'bus.conf'
        bus_config.write_text('<busconfig><type>session</type><listen>unix:tmpdir=' + escape(folder)
                              + '</listen><policy context="default"><allow send_destination="*"/>'
                              + '<allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        def start(command, log):
            with (validation / log).open('w') as stream:
                process = subprocess.Popen(command, env=env, stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
            processes.append(process)
            return process
        def wait(predicate, description):
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                assert all(p.poll() is None for p in processes), 'Private service exited'
                try:
                    value = predicate()
                    if value:
                        return value
                except GLib.Error:
                    pass
                time.sleep(.05)
            raise TimeoutError(description)
        try:
            start(['dbus-daemon', '--nofork', '--print-address=1', '--config-file=' + str(bus_config)], 'core-bus.log')
            address = wait(lambda: (validation / 'core-bus.log').read_text().strip(), 'private bus address')
            env['DBUS_SESSION_BUS_ADDRESS'] = address
            connection = Gio.DBusConnection.new_for_address_sync(address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
            def call(dest, path, interface, method, signature='()', values=()):
                return connection.call_sync(dest, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NO_AUTO_START, 2000, None).unpack()
            def owner(name):
                return call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', '(s)', (name,))[0]
            backend = start([str(binary), '--canopa-config', '/home/kevinard/.config/canopa/config.kdl'], 'core-backend.log')
            wait(lambda: owner(BACKEND), 'backend with existing shared configuration')
            start([str(frontend), '--verbose'], 'core-frontend.log')
            wait(lambda: owner(PUBLIC), 'public frontend')
            public = call(PUBLIC, OBJECT, 'org.freedesktop.portal.Settings', 'ReadOne', '(ss)', ('org.freedesktop.appearance', 'color-scheme'))[0]
            direct = call(BACKEND, OBJECT, 'org.freedesktop.impl.portal.Settings', 'Read', '(ss)', ('org.freedesktop.appearance', 'color-scheme'))[0]
            assert public == direct
            pid = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', '(s)', (BACKEND,))[0]
            assert pid == backend.pid
            assert call(PUBLIC, OBJECT, 'org.freedesktop.DBus.Properties', 'Get', '(ss)', ('org.freedesktop.portal.FileChooser', 'version'))[0] == 4
            log = (validation / 'core-frontend.log').read_text()
            assert "Using portal configuration file '" + str(configuration) + "'" in log
            for interface in ['Settings', 'FileChooser']:
                assert 'Using canopa.portal for org.freedesktop.impl.portal.' + interface in log
            result.update(status='passed', checks=['actual shared config starts the tested backend',
                          'niri-specific configuration selected', 'user-local descriptors found without XDG_DESKTOP_PORTAL_DIR',
                          'Settings and FileChooser explicitly select Canopa', 'public Settings matches backend appearance',
                          'public FileChooser v4 available'], backend_pid=backend.pid)
        except Exception as error:
            result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
        finally:
            if connection:
                connection.close_sync(None)
            for process in reversed(processes):
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGTERM)
                    try:
                        process.wait(timeout=6)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
            result['processes'] = [dict(pid=p.pid, command=p.args, exit_code=p.poll()) for p in processes]
    result['temporary_runtime_removed'] = not runtime.exists()
    (validation / 'core-results.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
