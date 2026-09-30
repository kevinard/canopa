#!/usr/bin/env python3
"""Check drawer input and rendered scrolling in a private Weston/Niri desktop.

Requires the validate-desktop.py dependencies, wtype, grim, Pillow, a C compiler,
and wayland-scanner. No live desktop services or installed apps are used.
"""
import argparse
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from PIL import Image, ImageChops, ImageStat

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/canopa-shell')
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
out = args.output.resolve()
out.mkdir(parents=True)
print(f'Artifacts: {out}', flush=True)
binary = args.binary.resolve()
with tempfile.TemporaryDirectory(prefix='canopa-scroll-') as directory:
    runtime = Path(directory)
    session = desktop.Session(runtime, out)
    try:
        bus = runtime / 'bus.conf'
        bus.write_text(f'<busconfig><type>session</type><listen>unix:tmpdir={runtime}</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        session.start(['dbus-daemon', f'--config-file={bus}', '--nofork', '--print-address=1'], 'dbus.log')
        session.env['DBUS_SESSION_BUS_ADDRESS'] = session.wait_for(lambda: (out / 'dbus.log').read_text().strip(), 'bus').splitlines()[0]
        cfg = runtime / 'niri.kdl'
        cfg.write_text('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n')
        session.start(['weston', '--backend=headless', '--renderer=pixman', '--fake-seat', '--width=1280', '--height=800', '--socket=wayland-parent', '--no-config', '--idle-time=0'], 'weston.log')
        session.wait_for(lambda: (runtime / 'wayland-parent').is_socket(), 'Weston')
        session.env['WAYLAND_DISPLAY'] = 'wayland-parent'
        session.start(['niri', '-c', str(cfg)], 'niri.log')
        session.env['NIRI_SOCKET'] = str(session.wait_for(lambda: list(runtime.glob('niri.*.sock')), 'Niri IPC')[0])
        session.env['WAYLAND_DISPLAY'] = str(session.wait_for(lambda: [p for p in runtime.glob('wayland-*') if p.is_socket() and p.name != 'wayland-parent'], 'Niri')[0])
        name = next(iter(json.loads(session.run(['niri', 'msg', '--json', 'outputs']))))
        session.run(['niri', 'msg', 'output', name, 'scale', '1'])
        session.start(['wtype', '-s', '600000'], 'keyboard.log')
        config = runtime / 'config/niribar/config.kdl'
        config.parent.mkdir(parents=True)
        config.write_text('''menus { menu "apps" show-icons=#true { source "installed-apps"; } }
bar "main" {
 edge "left"
 collapsed-width 48
 modules { start "button"; }
 panel { presentation "drawer"; width 360; }
 actions { Super+Space { toggle-menu "apps"; } }
}
modules { module "button" { label "Apps"; icon "A"; actions { click { toggle-menu "apps"; } } } }
''')
        apps = runtime / 'data/applications'
        apps.mkdir(parents=True)
        icons = runtime / 'data/icons/hicolor/22x22/apps'
        icons.mkdir(parents=True)
        (icons.parent.parent / 'index.theme').write_text('[Icon Theme]\nName=hicolor\nDirectories=22x22/apps\n[22x22/apps]\nSize=22\nType=Fixed\n')
        for i in range(200):
            if i % 2:
                (icons / f'app-{i:03}.svg').write_text('<svg xmlns="http://www.w3.org/2000/svg" width="22" height="22"><rect width="22" height="22" rx="3" fill="#0088ff"/></svg>')
            else:
                Image.new('RGBA', (22, 22), (0, 136, 255, 255)).save(icons / f'app-{i:03}.png')
            (apps / f'app-{i:03}.desktop').write_text(f'[Desktop Entry]\nType=Application\nName=App {i:03}\nIcon=app-{i:03}\nExec=true\n')
        session.env['XDG_DATA_DIRS'] = str(runtime / 'empty')
        session.start([str(binary)], 'canopa-shell.log')
        session.wait_for_layers(1)
        session.wait_for(lambda: (runtime / 'canopa-shell.sock').is_socket(), 'IPC')
        time.sleep(1)
        # Use an actual pointer click on the launcher.
        generated = out / 'virtual-pointer.h'
        subprocess.run(['wayland-scanner', 'client-header', str(ROOT / 'scripts/desktop-fixture/wlr-virtual-pointer-unstable-v1.xml'), str(generated)], check=True)
        protocol = out / 'virtual-pointer.c'
        subprocess.run(['wayland-scanner', 'private-code', str(ROOT / 'scripts/desktop-fixture/wlr-virtual-pointer-unstable-v1.xml'), str(protocol)], check=True)
        subprocess.run(['cc', '-I', str(out), str(ROOT / 'scripts/desktop-fixture/click.c'), str(protocol), '-lwayland-client', '-o', str(out / 'click')], check=True)
        session.run([str(out / 'click'), '25', '16'])
        session.wait_for_layers(2)
        time.sleep(1)
        def capture(name):
            raw = subprocess.check_output(['grim', '-t', 'ppm', '-'], env=session.env, stderr=subprocess.PIPE, timeout=5)
            frame = Image.open(io.BytesIO(raw)).convert('RGB')
            frame.save(out / name)
            return frame
        opened = capture('open.png')
        assert any(b > 240 and 100 < g < 160 and r < 15 for r, g, b in opened.crop((48, 100, 150, 700)).getdata()), 'prepared icons never appeared'
        for _ in range(14):
            session.run(['wtype', '-k', 'Down'])
            time.sleep(.08)
        time.sleep(1)
        # Unfocus the search input so caret blinking cannot hide a missing redraw.
        session.run([str(out / 'click'), '125', '35'])
        time.sleep(1)
        # Capture time includes software rendering; this checks liveness, not FPS.
        results = []
        for i in range(6):
            before = capture(f'key-{i}-before.png').crop((80, 125, 330, 300))
            started = time.monotonic()
            session.run(['wtype', '-k', 'Down'])
            frames = 0
            while time.monotonic() - started < 1.5:
                after = capture(f'key-{i}-{frames}.png').crop((80, 125, 330, 300))
                frames += 1
                if ImageChops.difference(before, after).getbbox():
                    break
            assert ImageChops.difference(before, after).getbbox(), f'key {i}: scroll never presented while idle'
            result = {'key': i, 'capture_elapsed_ms': (time.monotonic() - started) * 1000, 'samples': frames}
            print(json.dumps(result), flush=True)
            results.append(result)
            time.sleep(.65)
        subprocess.run(['cc', '-I', str(out), str(ROOT / 'scripts/desktop-fixture/scroll.c'), str(protocol), '-lwayland-client', '-lm', '-o', str(out / 'scroll')], check=True)
        touchpad = []
        for name, delta, samples, interval in [('steady', '0.5', '120', '8333'),
                                               ('bursty', '1.5', '40', '25000')]:
            before = capture(f'touchpad-{name}-before.png')
            session.run([str(out / 'scroll'), '250', '375', delta, samples, interval])
            after = capture(f'touchpad-{name}-after.png')
            scores = []
            for dy in range(301):
                diff = ImageChops.difference(before.crop((100, 350 + dy, 350, 450 + dy)), after.crop((100, 350, 350, 450)))
                scores.append(sum(ImageStat.Stat(diff).mean))
            displacement = min(range(len(scores)), key=scores.__getitem__)
            result = {'gesture': name, 'touchpad_input_distance': 60,
                      'rendered_displacement': displacement, 'difference': scores[displacement]}
            print(json.dumps(result), flush=True)
            touchpad.append(result)
            assert displacement == 180 and scores[displacement] < 0.1, result
        # Cross several complete rendered windows, then return to the same pixels.
        crop = (100, 350, 350, 650)
        before = capture('boundary-before.png').crop(crop)
        session.run([str(out / 'scroll'), '250', '375', '5', '240', '8333'])
        after = capture('boundary-after.png').crop(crop)
        assert ImageChops.difference(before, after).getbbox(), 'long gesture did not advance'
        session.run([str(out / 'scroll'), '250', '375', '-5', '240', '8333'])
        returned = capture('boundary-returned.png').crop(crop)
        assert not ImageChops.difference(before, returned).getbbox(), 'cached-row crossings changed scroll geometry'
        (out / 'result.json').write_text(json.dumps({'keyboard': results, 'touchpad': touchpad, 'boundary_round_trip_pixels': 3600}, indent=2))
        print('PASS: icons, idle keyboard redraw, touchpad travel and cached-row crossings', flush=True)
    finally:
        session.close()
