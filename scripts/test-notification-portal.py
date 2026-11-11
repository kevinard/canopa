#!/usr/bin/env python3
"""Exercise the Rust Notification backend and public frontend on a private desktop."""
import argparse
import importlib.util
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
APP = 'org.canopa.PortalNotificationFixture'
BACKEND = 'org.freedesktop.impl.portal.desktop.canopa.notify'
PATH = '/org/freedesktop/portal/desktop'
sys.dont_write_bytecode = True


def application(mode, output):
    from gi.repository import Gio, GLib
    events = []
    children = []

    def save(kind, parameter=None, platform=None):
        events.append(dict(kind=kind, parameter_type=parameter.get_type_string() if parameter else None,
                           parameter=parameter.unpack() if parameter else None,
                           platform=platform or {}))
        output.write_text(json.dumps(events, indent=2) + '\n')
        if kind == 'default' and os.environ.get('CANOPA_PORTAL_NOTIFICATION_PEER'):
            environment = os.environ.copy()
            environment['XDG_ACTIVATION_TOKEN'] = platform.get('activation-token', '')
            with output.with_suffix('.wayland.log').open('w') as log:
                children.append(subprocess.Popen([environment['CANOPA_PORTAL_NOTIFICATION_PEER']],
                    env=environment, stdout=log, stderr=log))

    platform = {}
    class RecordingApplication(Gio.Application):
        def do_before_emit(self, values):
            platform.clear()
            platform.update(values.unpack())

    flags = Gio.ApplicationFlags.NON_UNIQUE if mode == 'send' else Gio.ApplicationFlags.DEFAULT_FLAGS
    app = RecordingApplication(application_id=APP, flags=flags)
    for name, signature in [('default', '(si)'), ('reply', 's')]:
        action = Gio.SimpleAction.new(name, GLib.VariantType.new(signature))
        action.connect('activate', lambda action, parameter: save(action.get_name(), parameter, dict(platform)))
        app.add_action(action)
    app.connect('activate', lambda app: save('activate', platform=dict(platform)))
    app.register(None)
    assert not app.get_is_remote(), 'fixture notification sender must remain a local application instance'
    if mode == 'serve':
        app.hold()
        GLib.MainLoop().run()
    connection = app.get_dbus_connection()
    connection.call_sync('org.freedesktop.portal.Desktop', PATH, 'org.freedesktop.host.portal.Registry',
                         'Register', GLib.Variant('(sa{sv})', (APP, {})), None,
                         Gio.DBusCallFlags.NONE, 5000, None)
    notification = Gio.Notification.new('Canopa portal notification')
    notification.set_body('Literal <b>body</b> — sender may exit')
    notification.set_default_action_and_target('app.default', GLib.Variant('(si)', ('payload', 7)))
    notification.add_button_with_target('Reply to fixture', 'app.reply', GLib.Variant('s', 'answer'))
    svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32'><rect width='32' height='32' fill='#468ac2'/></svg>"
    notification.set_icon(Gio.BytesIcon.new(GLib.Bytes.new(svg)))
    app.send_notification('native-fixture', notification)
    # Flush asynchronous public-portal delivery before this sending process exits.
    loop = GLib.MainLoop()
    GLib.timeout_add(1500, lambda: (loop.quit(), False)[1])
    loop.run()
    connection.flush_sync(None)


def verify(binary, session, scheme, scale, shell_binary):
    from gi.repository import Gio, GLib
    from PIL import ImageChops
    home = session.runtime / 'home'
    home.mkdir()
    config = session.runtime / 'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\nnotifications {{ sound enabled=#false; }}\n')
    if shell_binary:
        with config.open('a') as stream:
            stream.write('shell {\nbar "test" { edge "top"; collapsed-width 44; modules { start "notifications"; }; }\nmodules { module "notifications"; }\n}\n')
    services = session.runtime / 'services'
    services.mkdir()
    permission = 'org.freedesktop.impl.portal.PermissionStore.service'
    shutil.copy(Path('/usr/share/dbus-1/services') / permission, services / permission)
    activation = 'org.freedesktop.impl.portal.desktop.canopa.notify.service'
    shutil.copy(ROOT / 'packaging/portal' / activation, services / activation)
    portals = session.runtime / 'portals'
    portals.mkdir()
    shutil.copy(ROOT / 'packaging/portal/canopa-notify.portal', portals / 'canopa-notify.portal')
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\norg.freedesktop.impl.portal.Notification=canopa-notify\n')
    applications = session.runtime / 'data/applications'
    applications.mkdir(parents=True)
    (applications / (APP + '.desktop')).write_text('[Desktop Entry]\nType=Application\nName=Canopa portal fixture\nExec=/usr/bin/false\nDBusActivatable=true\n')
    direct_app = APP + '.Direct'
    (applications / (direct_app + '.desktop')).write_text('[Desktop Entry]\nType=Application\nName=Direct portal fixture\nExec=/usr/bin/false\n')
    events = session.output / 'application-events.json'
    (services / (APP + '.service')).write_text(f'[D-BUS Service]\nName={APP}\nExec={sys.executable} {Path(__file__).resolve()} --serve {events}\n')
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home), XDG_CURRENT_DESKTOP='Canopa', XDG_DESKTOP_PORTAL_DIR=str(portals),
                       GIO_USE_PORTALS='1', GTK_USE_PORTAL='1', WAYLAND_DEBUG='1')
    session.env['PATH'] = str(binary.parent) + os.pathsep + session.env['PATH']
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n',
                       width=round(1280 * scale), height=round(800 * scale), scale=scale,
                       service_directories=[services])
    outputs = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))
    logical = next(item['logical'] for item in outputs.values() if item.get('logical'))
    screen_width, screen_height = logical['width'], logical['height']
    socket = session.runtime / 'canopa-notify/control.sock'
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

    protocols = Path('/usr/share/wayland-protocols')
    for name, xml in [('xdg-shell', protocols / 'stable/xdg-shell/xdg-shell.xml'),
                      ('xdg-foreign', protocols / 'unstable/xdg-foreign/xdg-foreign-unstable-v2.xml'),
                      ('xdg-activation', protocols / 'staging/xdg-activation/xdg-activation-v1.xml')]:
        for kind, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', kind, str(xml), str(session.output / (name + suffix))], check=True)
    peer = session.output / 'activated-application'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-DPORTAL_CHOSEN', '-I', str(session.output),
                    str(ROOT / 'scripts/desktop-fixture/portal-parent.c'),
                    *[str(session.output / (name + '.c')) for name in ('xdg-shell', 'xdg-foreign', 'xdg-activation')],
                    '-lwayland-client', '-o', str(peer)], check=True)
    session.env['CANOPA_PORTAL_NOTIFICATION_PEER'] = str(peer)

    def call(destination, interface, method, signature, values):
        return connection.call_sync(destination, '/org/freedesktop/DBus' if destination == 'org.freedesktop.DBus' else PATH, interface, method, GLib.Variant(signature, values),
                                    None, Gio.DBusCallFlags.NONE, 5000, None).unpack()

    call('org.freedesktop.DBus', 'org.freedesktop.DBus', 'UpdateActivationEnvironment', '(a{ss})', ({k: v for k, v in session.env.items()},))
    def backend_ready():
        try:
            return call(BACKEND, 'org.freedesktop.DBus.Properties', 'Get', '(ss)', ('org.freedesktop.impl.portal.Notification', 'version'))[0] == 2
        except GLib.Error:
            return False
    session.wait_for(backend_ready, 'ready Notification backend interface', timeout=30)
    backend_pid = call('org.freedesktop.DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', '(s)', (BACKEND,))[0]
    assert Path(f'/proc/{backend_pid}/exe').resolve() == binary
    session.wait_for(socket.exists, 'activated Notify control socket')
    frontend = next(path for path in [Path('/usr/lib/xdg-desktop-portal'), Path('/usr/libexec/xdg-desktop-portal')] if path.is_file())
    session.start([str(frontend), '--verbose'], 'frontend.log')
    def frontend_ready():
        try:
            return call('org.freedesktop.portal.Desktop', 'org.freedesktop.DBus.Properties', 'Get', '(ss)', ('org.freedesktop.portal.Notification', 'version'))[0] == 2
        except GLib.Error:
            return False
    session.wait_for(frontend_ready, 'public Notification frontend')
    pointer = session.build_pointer()
    baseline = session.capture('baseline.png')

    def active():
        return json.loads(session.run([str(binary), 'ctl', '--socket', str(socket), 'active', '--json']))

    checks = []
    tokens = []
    def send():
        result = subprocess.run([sys.executable, str(Path(__file__).resolve()), '--send', str(events)],
                                env=session.env, capture_output=True, text=True, timeout=20)
        (session.output / 'sender.log').write_text(result.stdout + result.stderr)
        assert result.returncode == 0, result.stderr
        return session.wait_for(lambda: active(), 'accepted public notification')

    def frame(name):
        def drawn():
            image = session.capture(name)
            area = ImageChops.difference(baseline, image).getbbox()
            return area if area and area[2] - area[0] > 100 and area[3] - area[1] > 30 else None
        area = session.wait_for(drawn, 'drawn Iced notification banner')
        return tuple(coordinate / scale for coordinate in area)

    def click(x, y):
        session.run([str(pointer), str(round(x / screen_width * 1600)), str(round(y / screen_height * 1000))])

    send()
    # The public sender process has exited. Survive longer than Notify's normal
    # five-second sender expiry before invoking the callback from actual input.
    time.sleep(6)
    rows = active()
    assert len(rows) == 1, rows
    left, top, right, bottom = frame('sender-exited.png')
    click(left + (right-left) * .45, top + 32)
    def recorded(kind):
        return next((event for event in json.loads(events.read_text()) if event['kind'] == kind), None) if events.exists() else None
    event = session.wait_for(lambda: recorded('default'), 'D-Bus reactivated typed default action')
    assert event['parameter_type'] == '(si)' and event['parameter'] == ['payload', 7], event
    assert event['platform'].get('activation-token'), event
    tokens.append(event['platform']['activation-token'])
    checks.append('public typed default action survives sender exit with native input token')
    def activated_window():
        return next((window for window in json.loads(session.run(['niri', 'msg', '--json', 'windows']))
                     if window['app_id'] == 'org.canopa.PortalChosen' and window['is_focused']), None)
    window = session.wait_for(activated_window, 'activated Wayland application window')
    token = event['platform']['activation-token']
    assert f'.activate("{token}",' in events.with_suffix('.wayland.log').read_text()
    checks.append('reactivated application consumes the token on its focused native surface')
    session.run(['niri', 'msg', 'action', 'close-window', '--id', str(window['id'])])
    session.wait_for(lambda: not any(item['id'] == window['id'] for item in json.loads(session.run(['niri', 'msg', '--json', 'windows']))), 'fixture application window closed')
    session.wait_for(lambda: not active(), 'default action retires its notification')
    send()
    time.sleep(.5)
    left, top, right, bottom = frame('button.png')
    click(left + (right-left) * .5, bottom - 44)
    event = session.wait_for(lambda: recorded('reply'), 'typed named application action')
    assert event['parameter_type'] == 's' and event['parameter'] == 'answer', event
    assert event['platform'].get('activation-token'), event
    assert event['platform']['activation-token'] not in tokens, event
    tokens.append(event['platform']['activation-token'])
    checks.append('native button preserves typed target and fresh token')
    session.wait_for(lambda: not active(), 'button action retires its notification')
    direct = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    def public(method, signature, values):
        return direct.call_sync('org.freedesktop.portal.Desktop', PATH, 'org.freedesktop.portal.Notification',
                                method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 5000, None)
    direct.call_sync('org.freedesktop.portal.Desktop', PATH, 'org.freedesktop.host.portal.Registry',
                     'Register', GLib.Variant('(sa{sv})', (direct_app, {})), None,
                     Gio.DBusCallFlags.NONE, 5000, None)
    signals = []
    direct.signal_subscribe('org.freedesktop.portal.Desktop', 'org.freedesktop.portal.Notification',
        'ActionInvoked', PATH, None, Gio.DBusSignalFlags.NONE,
        lambda connection, sender, path, interface, signal, parameters: signals.append(parameters))
    public('AddNotification', '(sa{sv})', ('signal-fixture', {
        'title': GLib.Variant('s', 'Portal signal action'),
        'body': GLib.Variant('s', 'A non-exported action retains its typed target.'),
        'default-action': GLib.Variant('s', 'chosen'),
        'default-action-target': GLib.Variant('(si)', ('direct-target', 19)),
    }))
    session.wait_for(active, 'direct public notification')
    time.sleep(.5)
    left, top, right, bottom = frame('signal-action.png')
    click(left + (right-left) * .45, top + 32)
    def drain():
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)
    signal = session.wait_for(lambda: (drain(), signals[0] if signals else None)[1], 'public non-exported action signal')
    identifier, action, parameters = signal.unpack()
    assert (identifier, action) == ('signal-fixture', 'chosen'), signal
    target = signal.get_child_value(2).get_child_value(0).get_variant()
    assert target.get_type_string() == '(si)' and target.unpack() == ('direct-target', 19), signal
    assert parameters[1].get('activation-token'), signal
    assert parameters[1]['activation-token'] not in tokens, signal
    tokens.append(parameters[1]['activation-token'])
    checks.append('public non-exported signal preserves target type and platform data')
    session.wait_for(lambda: not active(), 'signal action retires its notification')
    public('AddNotification', '(sa{sv})', ('withdraw-fixture', {'title': GLib.Variant('s', 'Withdraw this notification')}))
    session.wait_for(active, 'notification before withdrawal')
    public('RemoveNotification', '(s)', ('withdraw-fixture',))
    session.wait_for(lambda: not active(), 'public notification withdrawal')
    checks.append('public withdrawal closes the correct application notification')
    if shell_binary:
        session.env.update(RUST_LOG='canopa_shell=debug', NO_AT_BRIDGE='1')
        session.start([str(shell_binary), '--canopa-config', str(config)], 'shell.log')
        def shell_layers(count):
            layers = json.loads(session.run(['niri', 'msg', '--json', 'layers']))
            (session.output / 'shell-layers.json').write_text(json.dumps(layers, indent=2))
            return len([layer for layer in layers if layer['namespace'].startswith('canopa-shell')]) == count
        session.wait_for(lambda: shell_layers(1), 'private Shell bar', timeout=30)
        public('AddNotification', '(sa{sv})', ('center-fixture', {
            'title': GLib.Variant('s', 'Portal action from Notification Center'),
            'body': GLib.Variant('s', 'This callback belongs to the retained Center card.'),
            'default-action': GLib.Variant('s', 'chosen-center'),
            'default-action-target': GLib.Variant('(si)', ('center-target', 27)),
        }))
        session.wait_for(active, 'notification before opening Center')
        click(25, 20)
        session.run([str(pointer), '800', '500', 'move'])
        def center_drawn():
            image = session.capture('center.png')
            region = (0, round(60 * scale), round(496 * scale), round(screen_height * scale))
            area = ImageChops.difference(image.crop(region), baseline.crop(region)).getbbox()
            return area and area[3] - area[1] > round(screen_height * scale * .6)
        session.wait_for(center_drawn, 'drawn Notification Center reading panel')
        # Center geometry is fixed by the small private shell configuration;
        # keep a capture for reviewing the native target before accepting it.
        click(108, 296)
        def center_signal():
            drain()
            return next((signal for signal in signals if signal.get_child_value(1).unpack() == 'chosen-center'), None)
        signal = session.wait_for(center_signal, 'native Center default action signal')
        identifier, action, parameters = signal.unpack()
        assert identifier == 'center-fixture' and parameters[0] == ('center-target', 27), signal
        assert parameters[1].get('activation-token'), signal
        token = parameters[1]['activation-token']
        assert token not in tokens, signal
        trace = (session.output / 'shell.log').read_text()
        request = re.search(r'(xdg_activation_token_v1#\d+)\.done\("' + re.escape(token) + r'"\)', trace)
        assert request, 'Center token did not originate in Shell'
        assert request.group(1) + '.set_serial(' in trace and request.group(1) + '.set_surface(' in trace, 'Center token lacks native source/serial context'
        checks.append('native Notification Center invokes the captured portal action with fresh platform data')
    direct.close_sync(None)
    # Leave the live desktop untouched; every process and resource is fixture-owned.
    session.env['WAYLAND_DEBUG'] = '0'
    return dict(passed=True, checks=checks, scheme=scheme, scale=scale, daemon_pid=backend_pid,
                shipped_activation=True)


def main():
    if len(sys.argv) == 3 and sys.argv[1] in ('--serve', '--send'):
        application(sys.argv[1][2:], Path(sys.argv[2]))
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/canopa-notify')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--scheme', choices=['light', 'dark'], default='dark')
    parser.add_argument('--scale', type=float, default=1.0)
    parser.add_argument('--shell-binary', type=Path, help='Also verify native Notification Center input')
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
    desktop = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(desktop)
    with tempfile.TemporaryDirectory(prefix='canopa-notification-native-') as temp:
        session = desktop.Session(Path(temp), args.output)
        try:
            result = verify(args.binary.resolve(), session, args.scheme, args.scale,
                            args.shell_binary.resolve() if args.shell_binary else None)
            desktop.write_json(args.output / 'result.json', result)
            print(json.dumps(result))
        finally:
            session.close()


if __name__ == '__main__':
    main()
