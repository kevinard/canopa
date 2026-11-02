#!/usr/bin/env python3
"""Private compositor/host appearance check. Requires Niri, Sway, grim, wtype, Pillow and GI/AT-SPI."""
import argparse
import html
import json
import os
import re
from pathlib import Path
import signal
import shlex
import socket
import subprocess
import sys
import tempfile
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
    from PIL import Image
    import gi
    gi.require_version('Gio', '2.0')
    gi.require_version('Atspi', '2.0')
    from gi.repository import Gio, GLib, Atspi
    root = args.artifacts
    env = os.environ.copy()
    runtime = Path(env['XDG_RUNTIME_DIR'])
    children = []
    logs = []
    def start(name, command, input_text=None):
        log = (root / f'{name}.log').open('w')
        logs.append(log)
        child = subprocess.Popen(command, env=env, stdout=log, stderr=log, start_new_session=True,
                                 stdin=subprocess.PIPE if input_text is not None else None)
        children.append(child)
        if input_text is not None:
            child.stdin.write(input_text.encode())
            child.stdin.close()
        return child
    def command(*argv):
        return subprocess.check_output(argv, env=env, text=True).strip()
    def find(name, obj=None, depth=0):
        if depth == 0:
            # Deliver native tree updates before inspecting cached AT-SPI nodes.
            context = GLib.MainContext.default()
            while context.pending():
                context.iteration(False)
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
        # Distinct solid colors let screenshots prove the real asset adapters drew
        # pixels, independently of accessibility labels and fallback text.
        raster_color = (40, 210, 80)
        icon_color = (240, 120, 20)
        svg_color = (234, 17, 203)
        body_image = root / 'body.png'
        Image.new('RGB', (32, 32), raster_color).save(body_image)
        icon_theme = root / 'data' / 'icons' / 'hicolor'
        (icon_theme / '22x22/apps').mkdir(parents=True)
        (icon_theme / 'scalable/apps').mkdir(parents=True)
        (icon_theme / 'index.theme').write_text(
            '[Icon Theme]\nDirectories=22x22/apps,scalable/apps\n'
            '[22x22/apps]\nSize=22\nType=Fixed\n'
            '[scalable/apps]\nSize=22\nType=Scalable\nMinSize=16\nMaxSize=128\n')
        Image.new('RGB', (22, 22), icon_color).save(icon_theme / '22x22/apps/canopa-fixture-raster.png')
        (icon_theme / 'scalable/apps/canopa-fixture-svg.svg').write_text(
            '<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22">'
            '<rect width="22" height="22" fill="#ea11cb"/></svg>')
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
        env['GSETTINGS_BACKEND'] = 'memory'
        start('a11y-launcher', [launcher, '--launch-immediately'])
        eventually(lambda: owned('org.a11y.Bus'), 'accessibility bus')
        call('org.a11y.Bus', '/org/a11y/bus', 'org.freedesktop.DBus.Properties',
             'Set', GLib.Variant('(ssv)', ('org.a11y.Status', 'IsEnabled', GLib.Variant('b', True))))
        start('a11y-registry', [registry])
        start('sway', ['sway', '-c', str(root / 'sway.conf')])
        sway_socket = eventually(lambda: next(iter(runtime.glob('sway-ipc.*.sock')), None), 'Sway IPC')
        outer = eventually(lambda: next((p for p in runtime.glob('wayland-*') if p.is_socket()), None), 'outer Wayland socket')
        env['WAYLAND_DISPLAY'] = str(outer)
        env['SWAYSOCK'] = str(sway_socket)
        start('niri', ['niri', '-c', str(root / 'niri.kdl')])
        niri_socket = eventually(lambda: next(iter(runtime.glob('niri.*.sock')), None), 'Niri IPC')
        env['NIRI_SOCKET'] = str(niri_socket)
        inner = eventually(lambda: next((p for p in runtime.glob('wayland-*') if p.is_socket() and p != outer), None), 'nested Wayland socket')
        env['WAYLAND_DISPLAY'] = str(inner)
        output = next(iter(json.loads(command('niri', 'msg', '--json', 'outputs'))))
        command('niri', 'msg', 'output', output, 'scale', str(args.scale))
        eventually(lambda: json.loads(command('niri', 'msg', '--json', 'outputs'))[output]['logical']['scale'] == args.scale, 'output scale')
        # Keep keyboard capability present between short input commands. A
        # headless seat otherwise loses its last keyboard when wtype exits,
        # legitimately clearing native window focus before AT-SPI can inspect it.
        start('keyboard', ['wtype', '-s', '600000'])
        notify_socket = runtime / 'canopa-notify' / 'control.sock'
        notify = start('notify', [str(args.notify), 'daemon', '--service'])
        shell_process = start('canopa-shell', [str(args.shell)])
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
            ('Desktop fixture', 0, 'canopa-fixture-raster', 'Desktop acceptance',
             'Before <b>styled</b> <img src="/missing-fixture.png" alt="Image alternative"/> After '
             f'<img src="{html.escape(str(body_image), quote=True)}" alt="Green image"/>',
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
        clear = eventually(lambda: find('Clear active notifications'), 'real Canopa Shell Center')
        center = clear.get_application()
        output_height = json.loads(command('niri', 'msg', '--json', 'outputs'))[output]['logical']['height']
        def center_geometry(title, application):
            node = find(title, application)
            if node is None:
                return False
            rect = node.get_component_iface().get_extents(Atspi.CoordType.WINDOW)
            # The 448px reading body plus the shared 24px padding on each side.
            return rect.width == 496 and rect.height == output_height - 44
        eventually(lambda: center_geometry('Notifications', center), 'wide, full-height Center')
        assert find('Delivery', center) is None, 'delivery rules crowd the reading view'
        settings = eventually(lambda: find('Settings', center), 'Center settings navigation')
        assert settings.get_action_iface().do_action(0), 'settings navigation rejected'
        eventually(lambda: find('Delivery', center), 'delivery settings view')
        eventually(lambda: find('Delete notification history', center), 'history management')
        eventually(lambda: center_geometry('Notification settings', center), 'stable settings geometry')
        # Native state can precede the compositor's next submitted frame.
        time.sleep(.25)
        subprocess.check_call(
            ['grim', '-o', 'HEADLESS-1', str(root / 'center-settings.png')],
            env={**env, 'WAYLAND_DISPLAY': str(outer)})
        settings_frame = Image.open(root / 'center-settings.png').convert('RGB')
        settings_crop = settings_frame.crop((0, 80, settings_frame.width // 2, settings_frame.height))
        assert sum(count for count, rgb in settings_crop.getcolors(settings_crop.width * settings_crop.height)
                   if rgb == icon_color) > 100, 'Settings must identify the app with its source icon'
        back = eventually(lambda: find('Back to notifications', center), 'back to reading')
        assert back.get_action_iface().do_action(0), 'return from settings rejected'
        eventually(lambda: find('Desktop acceptance', center), 'notification content after settings')
        time.sleep(.25)
        subprocess.check_call(
            ['grim', '-o', 'HEADLESS-1', str(root / 'center-reading.png')],
            env={**env, 'WAYLAND_DISPLAY': str(outer)})
        center_frame = Image.open(root / 'center-reading.png').convert('RGB')
        center_crop = center_frame.crop((0, 80, center_frame.width // 2, center_frame.height))
        assert sum(count for count, rgb in center_crop.getcolors(center_crop.width * center_crop.height)
                   if rgb == icon_color) > 100, 'Center must show the sender icon, separately from body images'
        card = eventually(lambda: find('Desktop acceptance', center), 'Center card source')
        assert find('Desktop fixture', card) is not None, 'card must retain an accessible source label'
        eventually(lambda: find('Image alternative', center), 'Center image fallback')
        assert find('Status: Read', center) is None, 'acknowledged cards must not show status text'
        assert find('Unread', center) is None, 'acknowledged cards must not show an attention dot'
        # The decoded image can push the last action below the native viewport at
        # fractional scale. Offscreen actions appear in AT-SPI after scrolling.
        center_action_scrolled = find('Fourth action', center) is None
        if center_action_scrolled:
            command('wtype', '-k', 'Next')
        center_action = eventually(lambda: find('Fourth action', center), 'Center sender action')
        assert center_action.get_action_iface().do_action(0), 'native Center action rejected'
        def center_action_received():
            action_received()
            return seen.count((ident, 'four')) == 2
        eventually(center_action_received, 'real sender action from Center')
        def focused(obj, depth=0):
            if obj is None or depth > 12:
                return None
            try:
                if (obj.get_state_set().contains(Atspi.StateType.FOCUSED)
                        and obj.get_role() in (Atspi.Role.PUSH_BUTTON, Atspi.Role.LINK)):
                    return obj.get_name()
                for index in range(min(obj.get_child_count(), 256)):
                    value = focused(obj.get_child_at_index(index), depth + 1)
                    if value:
                        return value
            except GLib.Error:
                pass # A provider revision can retire nodes while this walk is in progress.
            return None
        def focus_after_update():
            # Sender publication precedes the Center's replacement snapshot.
            # A replacement intentionally clears keyboard selection; navigate
            # again if that update retired the selection before AT-SPI observed it.
            current = focused(center)
            if current:
                return current
            command('wtype', '-k', 'Tab')
            return focused(center)
        command('wtype', '-k', 'Tab')
        focus = eventually(focus_after_update, 'keyboard focus in Center accessibility tree')
        subprocess.check_call(
            ['grim', '-o', 'HEADLESS-1', str(root / 'desktop.png')], env={**env, 'WAYLAND_DISPLAY': str(outer)})
        initial = (32, 45, 49) if args.color_scheme == 'dark' else (251, 252, 248)
        changed = (251, 252, 248) if args.color_scheme == 'dark' else (32, 45, 49)
        def palette_visible(path, left, right):
            frame = Image.open(path).convert('RGB')
            halves = [frame.crop((0, 0, frame.width // 2, frame.height)),
                      frame.crop((frame.width // 2, 80, frame.width, frame.height))]
            return all(sum(count for count, rgb in part.getcolors(part.width * part.height) if rgb == color) > 3000
                       for part, color in zip(halves, (left, right)))
        def initial_palette():
            subprocess.check_call(['grim', '-o', 'HEADLESS-1', str(root / 'desktop.png')],
                                  env={**env, 'WAYLAND_DISPLAY': str(outer)})
            frame = Image.open(root / 'desktop.png').convert('RGB')
            halves = [frame.crop((0, 80, frame.width // 2, frame.height)),
                      frame.crop((frame.width // 2, 80, frame.width, frame.height))]
            colors = [dict((rgb, count) for count, rgb in part.getcolors(part.width * part.height))
                      for part in halves]
            return (palette_visible(root / 'desktop.png', initial, initial)
                    and all(counts.get(raster_color, 0) > 100 for counts in colors)
                    and colors[1].get(icon_color, 0) > 100)
        eventually(initial_palette, 'selected palette in both consumers')
        common = root / 'config' / 'canopa' / 'config.kdl'
        alternate = 'light' if args.color_scheme == 'dark' else 'dark'
        common.write_text(common.read_text().replace(f'color-scheme "{args.color_scheme}"', f'color-scheme "{alternate}"'))
        command(str(args.notify), 'ctl', '--socket', str(notify_socket), 'reload')
        # Live Shell reload also installs appearance and retires the open panel.
        # Wait for that commit, then reopen the Center for the pixel comparison.
        eventually(lambda: 'configuration reloaded' in
                   re.sub(r'\x1b\[[0-9;]*m', '', (root / 'canopa-shell.log').read_text()),
                   'Shell live configuration reload')
        command(str(root / 'click'), '25', '20')
        eventually(lambda: find('Clear active notifications'), 'Center after appearance reload')
        def repainted():
            subprocess.check_call(['grim', '-o', 'HEADLESS-1', str(root / 'reloaded.png')],
                                  env={**env, 'WAYLAND_DISPLAY': str(outer)})
            return palette_visible(root / 'reloaded.png', changed, changed)
        eventually(repainted, 'live appearance repaint in both consumers')
        command('wtype', '-k', 'Escape')
        def shell_summary(log, available, count, dnd):
            records = re.findall(r'available=(true|false).*requires_attention=(\d+).*dnd=(true|false)',
                                 re.sub(r'\x1b\[[0-9;]*m', '', (root / log).read_text()))
            return records and records[-1] == (str(available).lower(), str(count), str(dnd).lower())
        # The resident action above acknowledged its item. A new retained item
        # supplies pending attention for both independent restart checks.
        call('org.freedesktop.Notifications', '/org/freedesktop/Notifications',
             'org.freedesktop.Notifications', 'Notify', GLib.Variant('(susssasa{sv}i)',
             ('Desktop fixture', 0, '', 'Retained across restarts', 'Restart fixture', [], {}, 0)), '(u)')
        eventually(lambda: shell_summary('canopa-shell.log', True, 1, False), 'shell attention count')
        command(str(root / 'click'), '25', '20', 'right')
        eventually(lambda: shell_summary('canopa-shell.log', True, 1, True), 'built-in DND toggle')
        os.killpg(shell_process.pid, signal.SIGTERM)
        shell_process.wait(timeout=10)
        assert ready(), 'Notify stopped with Canopa Shell'
        # Receipt continues while the shell is stopped, even with DND enabled.
        call('org.freedesktop.Notifications', '/org/freedesktop/Notifications',
             'org.freedesktop.Notifications', 'Notify', GLib.Variant('(susssasa{sv}i)',
             ('Desktop fixture', 0, '', 'Received without shell', 'Independent daemon', [], {}, 0)), '(u)')
        shell_process = start('canopa-shell-restarted', [str(args.shell)])
        eventually(lambda: shell_summary('canopa-shell-restarted.log', True, 2, True), 'shell restart fresh snapshot')
        os.killpg(notify.pid, signal.SIGTERM)
        notify.wait(timeout=20)
        eventually(lambda: shell_summary('canopa-shell-restarted.log', False, 0, False), 'unavailable shell state')
        subprocess.check_call(['grim', '-o', 'HEADLESS-1', str(root / 'unavailable.png')],
                              env={**env, 'WAYLAND_DISPLAY': str(outer)})
        notify = start('notify-restarted', [str(args.notify), 'daemon', '--service'])
        eventually(ready, 'restarted daemon')
        eventually(lambda: shell_summary('canopa-shell-restarted.log', True, 2, True), 'reconnected summary with retained history and DND')
        command(str(root / 'click'), '25', '20')
        clear = eventually(lambda: find('Clear active notifications'), 'Center after independent restarts')
        eventually(lambda: find('Retained across restarts', clear.get_application()), 'retained Center item')
        eventually(lambda: find('Received without shell', clear.get_application()), 'notification received without shell')
        eventually(lambda: find('Unread', clear.get_application()), 'accessible attention indicator')
        time.sleep(.25)
        def recovered_frame():
            subprocess.check_call(['grim', '-o', 'HEADLESS-1', str(root / 'recovered.png')],
                                  env={**env, 'WAYLAND_DISPLAY': str(outer)})
            frame = Image.open(root / 'recovered.png').convert('RGB')
            center_area = frame.crop((0, 80, frame.width // 2, frame.height))
            return sum(count for count, rgb in center_area.getcolors(center_area.width * center_area.height)
                       if rgb == changed) > 3000
        eventually(recovered_frame, 'recovered Center pixels beyond the bar')
        retained_frame = Image.open(root / 'recovered.png').convert('RGB')
        retained_crop = retained_frame.crop((0, 80, retained_frame.width // 2, retained_frame.height))
        assert sum(count for count, rgb in retained_crop.getcolors(retained_crop.width * retained_crop.height)
                   if rgb == icon_color) > 100, 'retained source icon was lost after daemon restart'
        # Exercise a genuinely overflowing list at the automatic full height.
        for index in range(8):
            call('org.freedesktop.Notifications', '/org/freedesktop/Notifications',
                 'org.freedesktop.Notifications', 'Notify', GLib.Variant('(susssasa{sv}i)',
                 ('Mail fixture' if index % 2 else 'Chat fixture', 0, '', f'Overflow notification {index + 1}',
                  'A longer message should wrap inside its card, leaving the right edge clear. ' * 3,
                  [], {}, 0)), '(u)')
        newest = eventually(lambda: find('Overflow notification 8'), 'overflowing Center list')
        center = newest.get_application()
        eventually(lambda: center_geometry('Notifications', center), 'stable overflow geometry')
        command('wtype', '-k', 'Home')
        time.sleep(.25)
        first = eventually(lambda: find('Overflow notification 8', center), 'newest card at top')
        second = eventually(lambda: find('Overflow notification 7', center), 'next card from another app')
        assert first.get_parent() == second.get_parent(), 'apps must share one reading list'
        assert first.get_index_in_parent() < second.get_index_in_parent(), \
            'native reading order must remain newest first across apps'
        subprocess.check_call(['grim', '-o', 'HEADLESS-1', str(root / 'center-overflow.png')],
                              env={**env, 'WAYLAND_DISPLAY': str(outer)})
        command('wtype', '-k', 'End')
        eventually(lambda: find('Desktop acceptance', center), 'last card after scrolling')
        command('wtype', '-k', 'Escape')
        # Rich dmenu entries use the shell's icon lookup and its SVG renderer.
        menu = start('svg-menu', [str(args.shell), '--dmenu', '--json', '--no-custom'],
                     json.dumps({'value': 'fixture', 'label': 'SVG asset fixture',
                                 'icon': 'canopa-fixture-svg'}) + '\n')
        def svg_frame():
            subprocess.check_call(['grim', '-o', 'HEADLESS-1', str(root / 'svg-menu.png')],
                                  env={**env, 'WAYLAND_DISPLAY': str(outer)})
            frame = Image.open(root / 'svg-menu.png').convert('RGB')
            return sum(count for count, rgb in frame.getcolors(frame.width * frame.height)
                       if rgb == svg_color) > 50
        eventually(svg_frame, 'shell SVG icon pixels')
        command('wtype', '-k', 'Escape')
        menu.wait(timeout=10)
        (root / 'result.json').write_text(json.dumps({'passed': True, 'sender_actions': seen,
            'built_in_notifications': True, 'dnd_control': True,
            'center_settings_navigation': True, 'center_reading_hierarchy': True,
            'center_full_height': True, 'center_width': 496, 'center_overflow': True,
            'center_source_icon': True, 'center_source_accessibility': True,
            'center_settings_icon': True, 'center_chronological_order': True,
            'shell_restart': True, 'daemon_restart': True, 'retained_attention': 2,

            'color_scheme': args.color_scheme, 'scale': args.scale, 'keyboard_focus': focus,
            'appearance_reload': True, 'persistent_keyboard': True,
            'shared_raster_pixels': True, 'indexed_banner_icon': True, 'shell_svg_icon': True,
            'center_action_scrolled': center_action_scrolled,
            'niri': command('niri', '--version'), 'sway': command('sway', '--version'),
            'scope': 'real compositor, banner native activation, host opening and image fallback; Orca is separate'}, indent=2))
        print(f'Desktop smoke check passed; artifacts: {root}')
    except Exception:
        # Preserve the native tree and the last rendered frame before cleanup.
        # These describe the actual failing seam, without production debug hooks.
        def describe(obj, depth=0):
            if obj is None or depth > 12:
                return None
            try:
                return {'name': obj.get_name(), 'role': obj.get_role_name(),
                        'focused': obj.get_state_set().contains(Atspi.StateType.FOCUSED),
                        'children': [describe(obj.get_child_at_index(i), depth + 1)
                                     for i in range(min(obj.get_child_count(), 256))]}
            except GLib.Error:
                return {'retired': True}
        (root / 'failure-accessibility.json').write_text(json.dumps(describe(Atspi.get_desktop(0)), indent=2))
        if 'outer' in locals():
            subprocess.run(['grim', '-o', 'HEADLESS-1', str(root / 'failure.png')],
                           env={**env, 'WAYLAND_DISPLAY': str(outer)}, check=False)
        raise
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
    parser.add_argument('--shell', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--color-scheme', choices=['dark', 'light'], default='dark')
    parser.add_argument('--scale', type=float, choices=[1.0, 1.25, 1.5, 2.0], default=1.0)
    parser.add_argument('--inside-private-bus', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    args.notify = args.notify.resolve(strict=True)
    args.shell = args.shell.resolve(strict=True)
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
        env['RUST_LOG'] = 'canopa_shell::notifications=debug,canopa_shell::configuration=info'
        env.update(WLR_BACKENDS='headless', WLR_HEADLESS_OUTPUTS='1', WLR_RENDERER='pixman',
                   LIBGL_ALWAYS_SOFTWARE='1', GTK_A11Y='always', DCONF_BACKEND='memory')
        (root / 'alsa.conf').write_text('pcm.!default { type null }\n')
        env['ALSA_CONFIG_PATH'] = str(root / 'alsa.conf')
        env['PULSE_SERVER'] = 'unix:' + str(root / 'runtime' / 'no-pulse')
        env['PIPEWIRE_REMOTE'] = 'canopa-fixture-no-audio'
        (root / 'bus.conf').write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth><policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>')
        (root / 'sway.conf').write_text('output HEADLESS-1 mode 1600x1000\noutput * bg #20242c solid_color\nseat seat0 fallback true\nxwayland disable\ndefault_border none\nfor_window [app_id="niri"] fullscreen enable\n')
        (root / 'niri.kdl').write_text('prefer-no-csd\nhotkey-overlay { skip-at-startup; }\nanimations { off; }\nlayout { gaps 0; focus-ring { off; }; border { off; }; }\n')
        (root / 'config' / 'canopa').mkdir()
        # Exercise built-in module, provider, actions and default socket discovery.
        # Custom placement keeps pointer coordinates deterministic across scales.
        shell = '''bar "test" { edge "top"; collapsed-width 44; modules { start "notifications"; }; }
modules { module "notifications"; }
'''
        (root / 'config' / 'canopa' / 'config.kdl').write_text(
            'schema-version 1\nappearance { color-scheme "' + args.color_scheme + '"; }\n'
            'notifications { sound enabled=#false; }\nshell {\n' + shell + '\n}\n')
        # Unix socket names (Sway/Niri/AT-SPI/Notify) must fit sun_path even
        # when the requested evidence directory has a long absolute name.
        with tempfile.TemporaryDirectory(prefix='canopa-desktop-runtime-') as runtime:
            env['XDG_RUNTIME_DIR'] = runtime
            env['PULSE_SERVER'] = 'unix:' + str(Path(runtime) / 'no-pulse')
            sys.exit(subprocess.call(['dbus-run-session', '--config-file='+str(root/'bus.conf'), '--', sys.executable,
                str(Path(__file__).resolve()), '--notify', str(args.notify), '--shell', str(args.shell),
                '--artifacts', str(root), '--color-scheme', args.color_scheme, '--scale', str(args.scale), '--inside-private-bus'], env=env))
