#!/usr/bin/env python3
"""Real compositor/host smoke check. Requires Niri, Sway, grim and Python GI/AT-SPI."""
import argparse
import json
import os
from pathlib import Path
import signal
import shlex
import socket
import subprocess
import sys
import time


def eventually(probe, description, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = probe()
        if value:
            return value
        time.sleep(.05)
    raise RuntimeError(f'Timed out waiting for {description}')


def run(args):
    import gi
    gi.require_version('Gio', '2.0')
    gi.require_version('Atspi', '2.0')
    from gi.repository import Gio, GLib, Atspi
    root = args.artifacts
    env = os.environ.copy()
    children = []
    logs = []
    def start(name, command):
        log = (root / f'{name}.log').open('w')
        logs.append(log)
        child = subprocess.Popen(command, env=env, stdout=log, stderr=log, start_new_session=True)
        children.append(child)
        return child
    def command(*argv):
        return subprocess.check_output(argv, env=env, text=True).strip()
    def find(name, obj=None, depth=0):
        if obj is None:
            obj = Atspi.get_desktop(0)
        if depth > 12:
            return None
        try:
            if obj.get_name() == name:
                return obj
            for index in range(min(obj.get_child_count(), 256)):
                result = find(name, obj.get_child_at_index(index), depth + 1)
                if result:
                    return result
        except GLib.Error:
            pass
        return None
    try:
        source = Path(__file__).resolve().parent / 'desktop-fixture'
        protocol = source / 'wlr-virtual-pointer-unstable-v1.xml'
        subprocess.check_call(['wayland-scanner', 'client-header', str(protocol), str(root / 'virtual-pointer.h')])
        subprocess.check_call(['wayland-scanner', 'private-code', str(protocol), str(root / 'virtual-pointer.c')])
        flags = shlex.split(command('pkg-config', '--cflags', '--libs', 'wayland-client'))
        subprocess.check_call(['cc', '-Wall', '-Wextra', '-Werror', '-I'+str(root),
            str(source / 'click.c'), str(root / 'virtual-pointer.c'), '-o', str(root / 'click'), *flags])
        atspi_roots = ['/usr/lib', '/usr/libexec', '/usr/lib/at-spi2-core']
        launcher = next((str(Path(root) / 'at-spi-bus-launcher') for root in atspi_roots if (Path(root) / 'at-spi-bus-launcher').is_file()), None)
        registry = next((str(Path(root) / 'at-spi2-registryd') for root in atspi_roots if (Path(root) / 'at-spi2-registryd').is_file()), None)
        if not launcher or not registry:
            raise RuntimeError('AT-SPI launcher and registry are required')
        bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        def call(destination, path, interface, method, parameters=None, result=None):
            return bus.call_sync(destination, path, interface, method, parameters,
                                 GLib.VariantType.new(result) if result else None,
                                 Gio.DBusCallFlags.NONE, 5000, None)
        def owned(name):
            return call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
                        'NameHasOwner', GLib.Variant('(s)', (name,)), '(b)').unpack()[0]
        start('a11y-launcher', [launcher, '--launch-immediately', '--a11y=1'])
        eventually(lambda: owned('org.a11y.Bus'), 'accessibility bus')
        start('a11y-registry', [registry])
        start('sway', ['sway', '-c', str(root / 'sway.conf')])
        sway_socket = eventually(lambda: next(iter((root / 'runtime').glob('sway-ipc.*.sock')), None), 'Sway IPC')
        outer = eventually(lambda: next((p for p in (root / 'runtime').glob('wayland-*') if p.is_socket()), None), 'outer Wayland socket')
        env['WAYLAND_DISPLAY'] = str(outer)
        env['SWAYSOCK'] = str(sway_socket)
        start('niri', ['niri', '-c', str(root / 'niri.kdl')])
        niri_socket = eventually(lambda: next(iter((root / 'runtime').glob('niri.*.sock')), None), 'Niri IPC')
        env['NIRI_SOCKET'] = str(niri_socket)
        inner = eventually(lambda: next((p for p in (root / 'runtime').glob('wayland-*') if p.is_socket() and p != outer), None), 'nested Wayland socket')
        env['WAYLAND_DISPLAY'] = str(inner)
        notify_socket = root / 'runtime' / 'notify.sock'
        start('notify', [str(args.notify), 'daemon', '--service', '--socket', str(notify_socket)])
        start('niribar', [str(args.niribar)])
        def ready():
            try:
                with socket.socket(socket.AF_UNIX) as client:
                    client.settimeout(.2)
                    client.connect(str(notify_socket))
                    client.sendall(b'control {"command":"status"}\n')
                    reply = bytearray()
                    while part := client.recv(4096):
                        reply.extend(part)
                        if len(reply) > 16384:
                            return False
                    return json.loads(reply).get('reply') == 'status'
            except (OSError, ValueError):
                return False
        eventually(ready, 'ready daemon control endpoint')
        seen = []
        bus.signal_subscribe('org.freedesktop.Notifications', 'org.freedesktop.Notifications',
            'ActionInvoked', '/org/freedesktop/Notifications', None, Gio.DBusSignalFlags.NONE,
            lambda _bus, _sender, _path, _iface, _signal, values, _data: seen.append(values.unpack()), None)
        ident, = call('org.freedesktop.Notifications', '/org/freedesktop/Notifications',
            'org.freedesktop.Notifications', 'Notify', GLib.Variant('(susssasa{sv}i)',
            ('Desktop fixture', 0, '', 'Desktop acceptance',
             'Before <b>styled</b> <img src="/missing-fixture.png" alt="Image alternative"/> After',
             ['one','First action','two','Second action','three','Third action','four','Fourth action'],
             {'resident': GLib.Variant('b', True)}, 0)), '(u)').unpack()
        fourth = eventually(lambda: find('Fourth action'), 'rendered banner accessibility')
        assert fourth.get_action_iface().do_action(0), 'native banner action rejected'
        def action_received():
            context = GLib.MainContext.default()
            while context.pending():
                context.iteration(False)
            return (ident, 'four') in seen
        eventually(action_received, 'real sender action from banner')
        subprocess.check_call(
            ['grim', '-o', 'HEADLESS-1', str(root / 'before-center.png')], env={**env, 'WAYLAND_DISPLAY': str(outer)})
        # Inject into the inner compositor: outer headless seats may have no pointer device.
        command(str(root / 'click'), '25', '20')
        subprocess.check_call(
            ['grim', '-o', 'HEADLESS-1', str(root / 'after-click.png')], env={**env, 'WAYLAND_DISPLAY': str(outer)})
        clear = eventually(lambda: find('Clear active notifications'), 'real Niribar Center')
        center = clear.get_application()
        eventually(lambda: find('Image alternative', center), 'Center image fallback')
        center_action = eventually(lambda: find('Fourth action', center), 'Center sender action')
        assert center_action.get_action_iface().do_action(0), 'native Center action rejected'
        def center_action_received():
            action_received()
            return seen.count((ident, 'four')) == 2
        eventually(center_action_received, 'real sender action from Center')
        subprocess.check_call(
            ['grim', '-o', 'HEADLESS-1', str(root / 'desktop.png')], env={**env, 'WAYLAND_DISPLAY': str(outer)})
        (root / 'result.json').write_text(json.dumps({'passed': True, 'sender_actions': seen,
            'niri': command('niri', '--version'), 'sway': command('sway', '--version'),
            'scope': 'real compositor, banner native activation, host opening and image fallback; Orca is separate'}, indent=2))
        print(f'Desktop smoke check passed; artifacts: {root}')
    finally:
        for child in reversed(children):
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        for child in reversed(children):
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
        for log in logs:
            log.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--notify', type=Path, required=True)
    parser.add_argument('--niribar', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--inside-private-bus', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    args.notify = args.notify.resolve(strict=True)
    args.niribar = args.niribar.resolve(strict=True)
    args.artifacts = args.artifacts.resolve()
    if args.inside_private_bus:
        run(args)
    else:
        if args.artifacts.exists():
            parser.error('artifacts directory must be new, to prevent reusing private state')
        args.artifacts.mkdir(parents=True, mode=0o700)
        root = args.artifacts
        env = os.environ.copy()
        for key in ['DISPLAY','WAYLAND_DISPLAY','WAYLAND_SOCKET','SWAYSOCK','NIRI_SOCKET','AT_SPI_BUS_ADDRESS','NO_AT_BRIDGE']:
            env.pop(key, None)
        for key in ['runtime','state','config','data','cache']:
            (root / key).mkdir(mode=0o700)
            env['XDG_RUNTIME_DIR' if key == 'runtime' else f'XDG_{key.upper()}_HOME'] = str(root / key)
        env.update(WLR_BACKENDS='headless', WLR_HEADLESS_OUTPUTS='1', WLR_RENDERER='pixman',
                   LIBGL_ALWAYS_SOFTWARE='1', GTK_A11Y='always', DCONF_BACKEND='memory')
        (root / 'alsa.conf').write_text('pcm.!default { type null }\n')
        env['ALSA_CONFIG_PATH'] = str(root / 'alsa.conf')
        env['PULSE_SERVER'] = 'unix:' + str(root / 'runtime' / 'no-pulse')
        env['PIPEWIRE_REMOTE'] = 'canopa-fixture-no-audio'
        (root / 'bus.conf').write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth><policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>')
        (root / 'sway.conf').write_text('output HEADLESS-1 mode 1600x1000\noutput * bg #20242c solid_color\nseat seat0 fallback true\nxwayland disable\ndefault_border none\nfor_window [app_id="niri"] fullscreen enable\n')
        (root / 'niri.kdl').write_text('prefer-no-csd\nhotkey-overlay { skip-at-startup; }\nanimations { off; }\nlayout { gaps 0; focus-ring { off; }; border { off; }; }\n')
        (root / 'config' / 'canopa-notify').mkdir()
        (root / 'config' / 'canopa-notify' / 'config.kdl').write_text('sound enabled=#false\n')
        (root / 'config' / 'niribar').mkdir()
        (root / 'config' / 'niribar' / 'config.kdl').write_text('''panel-providers {
 provider "notifications" {
  protocol "canopa-v4"
  command { argv %s "panel-provider" "--socket" %s; }
 }
}
bar "test" { panel { height 720; }; edge "top"; collapsed-width 44; modules { start "notifications"; }; }
modules { module "notifications" { type "button"; label "Notifications"; icon "N"; format "{label}"; horizontal-format "{label}"; actions { click { open-provider "notifications"; }; }; }; }
''' % (json.dumps(str(args.notify)), json.dumps(str(root / 'runtime' / 'notify.sock'))))
        sys.exit(subprocess.call(['dbus-run-session', '--config-file='+str(root/'bus.conf'), '--', sys.executable,
            str(Path(__file__).resolve()), '--notify', str(args.notify), '--niribar', str(args.niribar),
            '--artifacts', str(root), '--inside-private-bus'], env=env))
