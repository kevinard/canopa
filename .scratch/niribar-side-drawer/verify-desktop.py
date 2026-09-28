"""Task-local native acceptance: private compositor, never the user's desktop."""
import importlib.util
import json
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import time
from PIL import Image, ImageChops

sys.dont_write_bytecode = True
repo = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('desktop', repo / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
output = Path(sys.argv[1])
output.mkdir(parents=True, exist_ok=False)
fixture = repo / 'scripts/desktop-fixture'
protocol = fixture / 'wlr-virtual-pointer-unstable-v1.xml'
subprocess.run(['wayland-scanner', 'client-header', str(protocol), str(output / 'virtual-pointer.h')], check=True)
subprocess.run(['wayland-scanner', 'private-code', str(protocol), str(output / 'virtual-pointer.c')], check=True)
flags = shlex.split(subprocess.check_output(['pkg-config', '--cflags', '--libs', 'wayland-client'], text=True))
subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I' + str(output), str(fixture / 'click.c'), str(output / 'virtual-pointer.c'), '-o', str(output / 'click'), *flags], check=True)
results = []
for edge, scheme, scale in [('left', 'dark', 1.0), ('right', 'light', 1.25)]:
    if len(sys.argv) > 2 and edge != sys.argv[2]:
        continue
    case = output / f'{edge}-{scheme}'
    case.mkdir()
    with tempfile.TemporaryDirectory(prefix='canopa-drawer-') as tmp:
        runtime = Path(tmp)
        session = desktop.Session(runtime, case)
        try:
            niri = runtime / 'niri.kdl'
            niri.write_text('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n')
            bus = runtime / 'bus.conf'
            bus.write_text(f'<busconfig><type>session</type><listen>unix:tmpdir={runtime}</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
            session.start(['dbus-daemon', f'--config-file={bus}', '--nofork', '--print-address=1'], 'dbus.log')
            session.env['DBUS_SESSION_BUS_ADDRESS'] = session.wait_for(lambda: (case / 'dbus.log').read_text().strip(), 'private bus').splitlines()[0]
            session.start(['weston', '--backend=headless', '--renderer=pixman', '--fake-seat', '--width=1280', '--height=800', '--socket=wayland-parent', '--no-config', '--idle-time=0'], 'weston.log')
            session.wait_for(lambda: (runtime / 'wayland-parent').is_socket(), 'Weston')
            session.env['WAYLAND_DISPLAY'] = 'wayland-parent'
            session.start(['niri', '-c', str(niri)], 'niri.log')
            session.env['NIRI_SOCKET'] = str(session.wait_for(lambda: list(runtime.glob('niri.*.sock')), 'Niri IPC')[0])
            session.env['WAYLAND_DISPLAY'] = str(session.wait_for(lambda: [p for p in runtime.glob('wayland-*') if p.is_socket() and p.name != 'wayland-parent'], 'Niri socket')[0])
            outputs = json.loads(session.run(['niri', 'msg', '--json', 'outputs']))
            name = next(iter(outputs))
            session.run(['niri', 'msg', 'output', name, 'scale', str(scale)])
            config = runtime / 'config/canopa/config.kdl'
            config.parent.mkdir(parents=True)
            config.write_text(f'''schema-version 1
appearance {{ color-scheme "{scheme}"; }}
shell {{
  bar "main" {{
    edge "{edge}"
    modules {{ start "launcher"; end "clock"; }}
    panel {{ presentation "drawer"; end "clock"; }}
  }}
  menus {{ menu "Applications" {{ item "Terminal" {{ launch "kitty"; }}; item "Browser" {{ launch "firefox"; }}; }} }}
  modules {{
    module "launcher" {{ type "button"; label "Apps"; icon "≡"; actions {{ click {{ open-menu "Applications"; }} }} }}
    module "clock" {{ actions {{ click {{ open-panel; }} }} }}
  }}
}}
''')
            session.start([str(repo / 'target/debug/niribar')], 'niribar.log')
            session.wait_for_layers(1)
            # Layer registration can precede the first GPU frame by several seconds.
            expected_bar = (30, 30, 46) if scheme == 'dark' else (239, 241, 245)
            def bar_visible():
                session.screenshot('settling.png')
                frame = Image.open(case / 'settling.png').convert('RGB')
                px = 5 if edge == 'left' else frame.width - 5
                return frame.getpixel((px, frame.height // 2)) == expected_bar
            session.wait_for(bar_visible, 'first bar frame', timeout=30)
            session.screenshot('closed.png')
            base = Image.open(case / 'closed.png').convert('RGB')
            w, h = base.size
            def click(x, y):
                session.run([str(output / 'click'), str(round(x * 1600 / w)), str(round(y * 1000 / h))])
            rail = round(48 * scale)
            x = rail / 2 if edge == 'left' else w - rail / 2
            click(x, h - 22 * scale)
            session.wait_for_layers(2)
            crop = (rail, 0, w, h) if edge == 'left' else (0, 0, w - rail, h)
            expected = (0, 0, round(360 * scale), h) if edge == 'left' else (w - rail - round(360 * scale), 0, w - rail, h)
            def drawer_visible():
                session.screenshot('calendar.png')
                try:
                    frame = Image.open(case / 'calendar.png').convert('RGB')
                    return ImageChops.difference(base.crop(crop), frame.crop(crop)).getbbox() == expected
                except OSError:
                    return False
            session.wait_for(drawer_visible, 'drawer first frame', timeout=60)
            calendar = Image.open(case / 'calendar.png').convert('RGB')
            clock_bounds = ImageChops.difference(base.crop(crop), calendar.crop(crop)).getbbox()
            assert clock_bounds == expected, (clock_bounds, expected)
            # Repeat-click, then replacement by a different module, then Escape.
            click(x, h - 22 * scale)
            session.wait_for_layers(1)
            click(x, h - 22 * scale)
            session.wait_for_layers(2)
            click(x, 15 * scale)
            session.wait_for_layers(2)
            def menu_visible():
                session.screenshot('applications.png')
                try:
                    frame = Image.open(case / 'applications.png').convert('RGB')
                    diff = ImageChops.difference(calendar.crop(crop), frame.crop(crop)).convert('L')
                    return diff.width * diff.height - diff.histogram()[0] > 1000
                except OSError:
                    return False
            session.wait_for(menu_visible, 'replacement menu frame', timeout=60)
            menu = Image.open(case / 'applications.png').convert('RGB')
            assert ImageChops.difference(base.crop(crop), menu.crop(crop)).getbbox() == expected
            session.run(['wtype', '-k', 'Escape'])
            session.wait_for_layers(1)
            click(x, h - 22 * scale)
            session.wait_for_layers(2)
            time.sleep(.3)
            left = rail if edge == 'left' else w - rail - 360 * scale
            click(left + 315 * scale, 29 * scale)
            session.wait_for_layers(1)
            results.append({'edge': edge, 'scheme': scheme, 'scale': scale, 'drawer_bounds_in_desktop_crop': clock_bounds, 'repeat_click': True, 'replacement': True, 'escape': True, 'close_button': True})
            desktop.write_json(output / 'result.json', results)
        finally:
            session.close()
print(json.dumps(results, indent=2))
