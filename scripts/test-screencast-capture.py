#!/usr/bin/env python3
"""Verify owned worker failure/cleanup on private Niri26.04 (no media success claim)."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec); spec.loader.exec_module(desktop)
SERVICE = 'org.gnome.Mutter.ScreenCast'


def verify(args, session):
    from gi.repository import Gio, GLib
    session.env['RUST_LOG'] = 'niri=debug'
    monitor = session.start_niri('debug { dbus-interfaces-in-non-session-instances; }\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n')
    flags = Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION
    backend = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], flags, None, None)
    def call(destination, path, interface, method, signature, values):
        return backend.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()
    owner = call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetNameOwner', '(s)', (SERVICE,))[0]
    assert call(SERVICE, '/org/gnome/Mutter/ScreenCast', 'org.freedesktop.DBus.Properties', 'Get', '(ss)', (SERVICE, 'Version'))[0] == 4
    closures = []
    subscription = backend.signal_subscribe(owner, SERVICE + '.Session', 'Closed', None, None, Gio.DBusSignalFlags.NONE, lambda c,s,p,i,n,v:closures.append(p))
    def drain():
        while GLib.MainContext.default().pending(): GLib.MainContext.default().iteration(False)
    def retired(path):
        try:
            xml = call(owner, path, 'org.freedesktop.DBus.Introspectable', 'Introspect', '()', ())[0]
            return 'interface name="org.gnome.Mutter.ScreenCast.Session"' not in xml
        except GLib.Error as error:
            assert Gio.DBusError.get_remote_error(error) == 'org.freedesktop.DBus.Error.UnknownObject', error
            return True
    def run(label, source, expected_owner=owner, creates=True):
        before = len(closures)
        request = {'compositor':expected_owner,'backend':backend.get_unique_name(),'caller':backend.get_unique_name(),'sources':[source],'cursor':'Hidden'}
        worker = subprocess.Popen([str(args.binary), '--screencast-worker'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=session.env, text=True, start_new_session=True)
        session.processes.append(worker)
        try:
            worker.stdin.write(json.dumps(request)+'\n'); worker.stdin.flush()
            # Keep the lease open while waiting for a failed startup. communicate()
            # would close stdin and test cancellation before session creation.
            worker.wait(timeout=40)
        finally:
            worker.stdin.close()
            if worker.poll() is None: desktop.stop(worker)
            session.processes.remove(worker)
        result = subprocess.CompletedProcess(worker.args, worker.returncode, worker.stdout.read(), worker.stderr.read())
        (session.output/(label+'.stderr')).write_text(result.stderr)
        assert result.returncode != 0 and result.stdout == '', (label, result.returncode, result.stdout)
        if creates:
            session.wait_for(lambda:(drain(),len(closures)>before)[1], 'compositor session Closed')
            for path in closures[before:]: assert retired(path), path
        else:
            drain(); assert len(closures) == before
    checks = []
    run('missing-monitor', {'Monitor':'Canopa-missing-output'})
    checks.append('RecordMonitor failure sends Stop and retires the actual compositor session without exposing nodes')
    run('monitor-no-gbm', {'Monitor':monitor})
    assert 'no GBM device available' in (session.output/'niri.log').read_text()
    checks.append('installed nested compositor startup failure publishes no streams and leaves no session object')
    for name, xml in [('xdg-shell','stable/xdg-shell/xdg-shell.xml'),('xdg-foreign','unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for mode, suffix in [('client-header','.h'),('private-code','.c')]:
            subprocess.run(['wayland-scanner',mode,str(Path('/usr/share/wayland-protocols')/xml),str(session.output/(name+suffix))],check=True,timeout=15)
    parent = session.output/'parent'
    subprocess.run(['cc','-Wall','-Wextra','-Werror','-DPORTAL_CAPTURE_PATTERN','-I',str(session.output),str(ROOT/'scripts/desktop-fixture/portal-parent.c'),str(session.output/'xdg-shell.c'),str(session.output/'xdg-foreign.c'),'-lwayland-client','-o',str(parent)],check=True,timeout=30)
    window = session.start([str(parent)], 'parent.log')
    selected = session.wait_for(lambda:next((w for w in json.loads(session.run(['niri','msg','--json','windows'])) if w['pid']==window.pid),None), 'owned window')
    run('window-no-gbm', {'Window':selected['id']})
    log = (session.output/'niri.log').read_text()
    assert 'record_window' in log and log.count('no GBM device available') >= 2
    checks.append('real owned window ID reaches RecordWindow and failed startup retires its session without placeholder geometry')
    ghost = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], flags, None, None)
    stale = ghost.get_unique_name(); ghost.close_sync(None)
    run('stale-owner', {'Monitor':monitor}, stale, creates=False)
    checks.append('a stale pinned owner cannot start capture against the current compositor')
    backend.signal_unsubscribe(subscription)
    return {'status':'passed','checks':checks,'media_flow_verified':False,'expected_limitation':'Niri26.04 nested Winit has no GBM capture device','binary_sha256':hashlib.sha256(args.binary.read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(); args.binary = args.binary.resolve(strict=True); args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-screencast-capture-') as directory:
        session = desktop.Session(Path(directory), args.output)
        try: result = verify(args, session)
        except Exception as error:
            desktop.write_json(args.output/'result.json', {'status':'failed','error':str(error)}); raise
        finally: session.close()
    desktop.write_json(args.output/'result.json', result); print(json.dumps(result))


if __name__ == '__main__': main()
