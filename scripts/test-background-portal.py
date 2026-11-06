#!/usr/bin/env python3
"""Exercise Background questions/review in private Notify, Niri and D-Bus.

This stage checks backend presentation and lifetime. Synthetic sandbox observation
has its separate fixture; public Flatpak permission lifecycle is separate.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
PATH = '/org/freedesktop/portal/desktop'
BACKEND = 'org.freedesktop.impl.portal.desktop.canopa'
NOTIFY = 'org.freedesktop.impl.portal.desktop.canopa.notify'
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def verify(binary, notify, session, scheme, scale, shipped):
    from gi.repository import Gio, GLib
    from PIL import ImageChops
    home = session.runtime / 'home'
    home.mkdir()
    config = session.runtime / 'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\nnotifications {{ sound enabled=#false; }}\n')
    services = session.runtime / 'services'
    services.mkdir()
    for name in ['org.freedesktop.impl.portal.desktop.canopa.service', 'org.freedesktop.impl.portal.desktop.canopa.notify.service']:
        shutil.copy(ROOT / 'packaging/portal' / name, services / name)
    portals = session.runtime / 'portals'
    portals.mkdir()
    descriptor = (ROOT / 'packaging/portal/canopa.portal').read_text()
    if not shipped and 'org.freedesktop.impl.portal.Background;' not in descriptor:
        descriptor = descriptor.replace('Interfaces=', 'Interfaces=org.freedesktop.impl.portal.Background;')
    assert 'org.freedesktop.impl.portal.Background;' in descriptor
    (portals / 'canopa.portal').write_text(descriptor)
    shutil.copy(ROOT / 'packaging/portal/canopa-notify.portal', portals)
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\norg.freedesktop.impl.portal.Background=canopa\norg.freedesktop.impl.portal.Settings=canopa\n')
    session.env.update(HOME=str(home), WAYLAND_DEBUG='1', XDG_CURRENT_DESKTOP='Canopa', XDG_DESKTOP_PORTAL_DIR=str(portals),
                       PATH=str(binary.parent) + os.pathsep + str(notify.parent) + os.pathsep + session.env['PATH'])
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n',
                       width=round(1280 * scale), height=round(800 * scale), scale=scale, service_directories=[services])
    outputs = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))
    logical = next(item['logical'] for item in outputs.values() if item.get('logical'))
    width, height = logical['width'], logical['height']
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    flags = Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION
    def connect():
        return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], flags, None, None)
    connection = connect()
    def call(destination, path, interface, method, signature, values, client=connection):
        return client.call_sync(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()
    def bus(method, signature, values):
        return call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', method, signature, values)
    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)
    bus('UpdateActivationEnvironment', '(a{ss})', ({k: v for k, v in session.env.items()},))
    def start_backend():
        bus('StartServiceByName', '(su)', (BACKEND, 0))
        pid = bus('GetConnectionUnixProcessID', '(s)', (BACKEND,))[0]
        assert Path(f'/proc/{pid}/exe').resolve() == binary
        return pid
    backend_pid = start_backend()
    control = session.runtime / 'canopa-notify/control.sock'
    pointer = session.build_pointer()
    def active():
        if not control.exists():
            return []
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(5)
            client.connect(str(control))
            client.sendall(b'control {"command":"notification-center-projection"}\n')
            with client.makefile('rb') as stream:
                reply = json.loads(stream.readline(8 * 1024 * 1024))
        assert reply['reply'] == 'notification-center-projection', reply
        return [item for group in reply['value']['groups'] for item in group['items'] if item['status'] == 'active']
    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
    def click(x, y):
        session.run([str(pointer), str(round(x / width * 1600)), str(round(y / height * 1000))])
        time.sleep(.15)
    def empty():
        session.wait_for(lambda: not active(), 'question withdrawn')
    assert not bus('NameHasOwner', '(s)', (NOTIFY,))[0]
    baseline = session.capture('baseline.png')
    pending, callbacks, checks = {}, [], []
    subscription = connection.signal_subscribe(NOTIFY, 'org.freedesktop.impl.portal.Notification', 'ActionInvoked', PATH, None, Gio.DBusSignalFlags.NONE,
        lambda conn, sender, path, interface, name, params: callbacks.append(params.unpack()))
    count = 0
    def begin(client=connection):
        nonlocal count
        count += 1
        path = f'/org/freedesktop/portal/desktop/request/test/background{count}'
        def done(conn, result):
            try:
                pending[path] = conn.call_finish(result).unpack()
            except GLib.Error as error:
                pending[path] = error
        client.call(BACKEND, PATH, 'org.freedesktop.impl.portal.Background', 'NotifyBackground',
                    GLib.Variant('(oss)', (path, 'org.canopa.BackgroundFixture', 'Fixture background app')),
                    None, Gio.DBusCallFlags.NONE, 30000, None, done)
        session.wait_for(lambda: (drain(), active())[1], 'Iced background question')
        return path
    def result(path, decision):
        response = session.wait_for(lambda: (drain(), pending.get(path))[1], 'background decision')
        assert response[1] == {'result': decision}, response
        empty()
    def frame(name):
        def drawn():
            area = ImageChops.difference(baseline, session.capture(name)).getbbox()
            return area if area and area[2] - area[0] > 100 and area[3] - area[1] > 40 else None
        return tuple(value / scale for value in session.wait_for(drawn, 'drawn background banner'))
    def banner_action(area, decision):
        left, top, right, bottom = area
        if decision == 1:
            click((left + right) / 2, bottom - 44)
        else:
            click(left + (right - left) * (.25 if decision == 0 else .75), bottom - 82)
    def review(path, name):
        area = frame(name)
        click(area[0] + (area[2] - area[0]) * .45, area[1] + 32)
        window = session.wait_for(lambda: (drain(), next((w for w in windows() if w['app_id'] == 'org.canopa.Portal'), None))[1], 'user-opened review dialog')
        event = next((event for event in reversed(callbacks) if event[2] == 'canopa.background.review'), None)
        assert event and event[3][1].get('activation-token'), callbacks
        token = event[3][1]['activation-token']
        session.wait_for(lambda: f'.activate("{token}",' in (session.output / 'dbus.log').read_text(), 'review helper consumes input-bound token')
        # No parent is present in the Background contract; normalize the native
        # window explicitly without claiming compositor-enforced modality.
        if not window['is_floating']:
            session.run(['niri', 'msg', 'action', 'toggle-window-floating', '--id', str(window['id'])])
        session.run(['niri', 'msg', 'action', 'set-window-width', '--id', str(window['id']), '680'])
        session.run(['niri', 'msg', 'action', 'set-window-height', '--id', str(window['id']), '395'])
        time.sleep(.2)
        session.run(['niri', 'msg', 'action', 'move-floating-window', '--id', str(window['id']), '--x', '20', '--y', '10'])
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        time.sleep(.2)
        assert path not in pending
        return next(w for w in windows() if w['id'] == window['id'])
    def close_review(window):
        session.wait_for(lambda: all(w['id'] != window['id'] for w in windows()), 'review helper closes')

    for decision in [0, 2, 1]:
        path = begin()
        if decision == 0:
            notify_pid = bus('GetConnectionUnixProcessID', '(s)', (NOTIFY,))[0]
            assert Path(f'/proc/{notify_pid}/exe').resolve() == notify
            options = call(NOTIFY, PATH, 'org.freedesktop.DBus.Properties', 'Get', '(ss)', ('org.freedesktop.impl.portal.Notification', 'SupportedOptions'))[0]
            assert options['org.canopa.request-timeout-ms']
        assert not any(w['app_id'] == 'org.canopa.Portal' for w in windows())
        area = frame(f'01-banner-{decision}.png')
        banner_action(area, decision)
        result(path, decision)
    checks.append('Background activates a cold Notify service and three native banner actions return exact typed decisions without automatically opening a toplevel')

    path = begin()
    window = review(path, '02-review-source.png')
    session.run(['wtype', '-k', 'Return'])
    drain()
    assert path not in pending
    session.capture('03-review.png')
    session.run(['wtype', '-k', 'Escape'])
    result(path, 2)
    close_review(window)
    checks.append('explicit review consumes a fresh input token; initial Enter and Escape do not change permission')

    path = begin()
    window = review(path, '04-choice-source.png')
    click(20 + window['layout']['window_size'][0] * .82, 10 + window['layout']['window_size'][1] - 78)
    result(path, 1)
    close_review(window)
    checks.append('Iced review returns an explicit Allow decision')

    path = begin()
    stranger = connect()
    try:
        call(BACKEND, path, 'org.freedesktop.impl.portal.Request', 'Close', '()', (), client=stranger)
        raise AssertionError('unrelated caller closed the question')
    except GLib.Error as error:
        assert 'AccessDenied' in str(error), error
    finally:
        stranger.close_sync(None)
    call(BACKEND, path, 'org.freedesktop.impl.portal.Request', 'Close', '()', ())
    result(path, 2)
    checks.append('only the owning caller can Close; withdrawal leaves no permission decision beyond this instance')

    lost = connect()
    begin(client=lost)
    lost.close_sync(None)
    empty()
    checks.append('caller loss withdraws its question')

    path = begin()
    window = review(path, '05-crash-review-source.png')
    os.kill(backend_pid, signal.SIGKILL)
    close_review(window)
    empty()
    backend_pid = start_backend()
    checks.append('abrupt backend loss terminates its owned review helper')

    # The notification's independent expiry must work after its backend sender
    # is killed, including under DND where no display timeout starts.
    session.run([str(notify), 'ctl', '--socket', str(control), 'dnd', 'on'])
    begin()
    os.kill(backend_pid, signal.SIGKILL)
    session.wait_for(lambda: not active(), 'Notify-owned question expiry after backend crash', timeout=27)
    session.run([str(notify), 'ctl', '--socket', str(control), 'dnd', 'off'])
    checks.append('Notify-owned expiry withdraws a silenced question after sender crash without banner acknowledgement')
    connection.signal_unsubscribe(subscription)
    connection.close_sync(None)
    return checks


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
    with tempfile.TemporaryDirectory(prefix='canopa-background-ui-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        try:
            checks = verify(args.binary.resolve(), args.notify.resolve(), session, args.color_scheme, args.scale, args.shipped_descriptor)
            binaries = {str(path.resolve()): hashlib.sha256(path.read_bytes()).hexdigest() for path in [args.binary, args.notify]}
            desktop.write_json(args.output / 'result.json', dict(passed=True, checks=checks, binaries=binaries, scheme=args.color_scheme, scale=args.scale, shipped_descriptor=args.shipped_descriptor, scope='backend native presentation/lifetime; public frontend acceptance is a separate fixture'))
        finally:
            for process in reversed(session.processes):
                desktop.stop(process)
    print(f'Background presentation: {len(checks)} native checks passed')


if __name__ == '__main__':
    main()
