#!/usr/bin/env python3
"""Validate Rust frame ownership on private compositors; no portal/UI claim."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
CASE = 'capture::native_capture_fixture'


def verify(args, session):
    from PIL import Image, ImageChops
    checks = []
    if args.compositor == 'niri':
        name = session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n', scale=args.scale)
        def outputs():
            return {key: value for key, value in json.loads(session.run(['niri', 'msg', '--json', 'outputs'])).items() if value.get('logical')}
    else:
        session.start_private_bus()
        session.env.update(WLR_BACKENDS='headless', WLR_HEADLESS_OUTPUTS='2', WLR_RENDERER='pixman', WLR_LIBINPUT_NO_DEVICES='1')
        session.env.pop('SWAYSOCK', None)
        config = session.runtime / 'sway.conf'
        config.write_text('seat seat0 fallback true\noutput HEADLESS-1 mode 800x600 scale 1 pos 0 0 bg #b41428 solid_color\noutput HEADLESS-2 mode 800x600 scale 1.25 pos -640 0 bg #0a288c solid_color\n')
        session.start(['sway', '--unsupported-gpu', '-c', str(config)], 'sway.log')
        sockets = session.wait_for(lambda: list(session.runtime.glob('sway-ipc.*.sock')), 'private Sway IPC')
        session.env['SWAYSOCK'] = str(sockets[0])
        displays = session.wait_for(lambda: [p for p in session.runtime.glob('wayland-*') if p.is_socket()], 'private Sway display')
        session.env['WAYLAND_DISPLAY'] = str(displays[0])
        session.wait_for(lambda: len(json.loads(session.run(['swaymsg', '-r', '-t', 'get_outputs']))) == 2, 'two outputs')
        def outputs():
            return {o['name']: {'logical': {**o['rect'], 'scale': o['scale'], 'transform': o['transform']}, 'modes':[o['current_mode']], 'current_mode':0} for o in json.loads(session.run(['swaymsg', '-r', '-t', 'get_outputs'])) if o['active']}

    desktop.write_json(session.output / 'initial-outputs.json', outputs())
    if args.compositor == 'sway':
        session.run(['swaymsg', 'focus', 'output', 'HEADLESS-1'])
    protocols = Path('/usr/share/wayland-protocols')
    for name_, xml in [('xdg-shell', protocols / 'stable/xdg-shell/xdg-shell.xml'), ('xdg-foreign', protocols / 'unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for kind, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', kind, str(xml), str(session.output / (name_ + suffix))], check=True, timeout=15)
    parent = session.output / 'parent'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-DPORTAL_CAPTURE_PATTERN', '-I', str(session.output), str(ROOT / 'scripts/desktop-fixture/portal-parent.c'), str(session.output / 'xdg-shell.c'), str(session.output / 'xdg-foreign.c'), '-lwayland-client', '-o', str(parent)], check=True, timeout=30)
    session.start([str(parent)], 'parent.log')
    if args.compositor == 'niri':
        session.wait_for(lambda: json.loads(session.run(['niri', 'msg', '--json', 'windows'])), 'fixture window')
    else:
        session.wait_for(lambda: 'org.canopa.PortalTest' in session.run(['swaymsg', '-t', 'get_tree']), 'fixture window')
    session.wait_for(lambda: len(session.capture('baseline.png').getcolors(4_000_000) or []) > 1, 'nonuniform source pixels')

    def settle():
        previous = None
        def stable():
            nonlocal previous
            current = {name: hashlib.sha256(subprocess.check_output(['grim', '-o', name, '-t', 'ppm', '-'], env=session.env, timeout=5)).hexdigest() for name in outputs()}
            settled = current == previous
            previous = current
            return settled
        session.wait_for(stable, 'settled source frames')
    settle()

    session.start(['wl-copy', '--foreground', '--type', 'text/plain', 'Canopa capture fixture'], 'clipboard.log')
    def clipboard_ready():
        try:
            return session.run(['wl-paste', '--no-newline']) == 'Canopa capture fixture'
        except subprocess.CalledProcessError:
            return False
    session.wait_for(clipboard_ready, 'private clipboard marker')
    assert CASE + ': test' in subprocess.check_output([str(args.harness), '--list'], text=True, timeout=10)
    env = {**session.env, 'CANOPA_CAPTURE_NATIVE_FIXTURE': '1', 'CANOPA_CAPTURE_FIXTURE_OUTPUT': str(session.output)}
    with (session.output / 'capture.log').open('w') as log:
        helper = subprocess.Popen([str(args.harness), '--ignored', '--exact', CASE, '--nocapture'], env=env, stdin=subprocess.PIPE, stdout=log, stderr=log, text=True, start_new_session=True)
    session.processes.append(helper)

    def messages():
        return [json.loads(line.split('CAPTURE ', 1)[1]) for line in (session.output / 'capture.log').read_text().splitlines() if line.startswith('CAPTURE ')]

    def request(command='capture', **fields):
        before = len(messages())
        helper.stdin.write(json.dumps({'command': command, **fields}) + '\n')
        helper.stdin.flush()
        return session.wait_for(lambda: messages()[before] if len(messages()) > before else None, 'Rust capture result', timeout=20)

    def compare(label, result, reference_rounding=False):
        assert 'frames' in result, result
        current = outputs()
        assert {frame['name'] for frame in result['frames']} == set(current), (result, current)
        for index, frame in enumerate(result['frames']):
            logical = current[frame['name']]['logical']
            assert frame['logical'] == [logical['x'], logical['y'], logical['width'], logical['height']], (frame, current)
            target = session.output / f'{label}-reference-{index}.png'
            session.run(['grim', '-o', frame['name'], str(target)])
            actual = Image.open(session.output / frame['file']).convert('RGB')
            expected = Image.open(target).convert('RGB')
            if label.startswith('transform-') and frame['name'] == 'HEADLESS-1':
                assert len(actual.getcolors(actual.width * actual.height)) > 3, 'rotation source must contain the asymmetric pattern'
            mode = current[frame['name']]['modes'][current[frame['name']]['current_mode']]
            physical = (mode['width'], mode['height'])
            if logical['transform'] in ('90', '270', 'Flipped90', 'Flipped270', 'flipped-90', 'flipped-270'):
                physical = tuple(reversed(physical))
            assert actual.size == physical, (label, actual.size, physical)
            if reference_rounding:
                # grim composites at a scale inferred from rounded logical size.
                # It may resample/drop a physical row; retain our full native
                # frame and compare uniform interior source pixels separately.
                assert abs(actual.width - expected.width) <= 1 and abs(actual.height - expected.height) <= 1
                samples = 0
                for y in range(2, expected.height - 2, 8):
                    for x in range(2, expected.width - 2, 8):
                        colors = expected.crop((x - 1, y - 1, x + 2, y + 2)).getcolors(9)
                        if len(colors) == 1:
                            ax = round((x + .5) * actual.width / expected.width - .5)
                            ay = round((y + .5) * actual.height / expected.height - .5)
                            assert actual.getpixel((ax, ay)) == colors[0][1], (label, x, y)
                            samples += 1
                assert samples > 1000
            else:
                assert actual.size == expected.size, (label, frame, expected.size)
                assert ImageChops.difference(actual, expected).getbbox() is None, f'{label}: Rust pixels differ from screencopy reference'
        assert session.run(['wl-paste', '--no-newline']) == 'Canopa capture fixture'
        return result

    first = compare('initial', request(hold=True))
    initial_descriptors = len(list(Path(f'/proc/{helper.pid}/fd').iterdir()))
    compare('second', request(hold=True))
    assert request() == {'error': 'Screenshot busy'}
    assert request('release') == {'released': True}
    compare('released', request())
    checks.append('full physical pixels and logical geometry match reference; two retained snapshots bound admission; release permits capture; clipboard unchanged')

    if args.compositor == 'niri':
        compare('held-before-scale', request(hold=True))
        held_before = request('held')
        held_hashes = [hashlib.sha256((session.output / f['file']).read_bytes()).hexdigest() for f in held_before['frames']]
        session.run(['niri', 'msg', 'output', name, 'scale', '1.5'])
        session.wait_for(lambda: outputs()[name]['logical']['scale'] == 1.5, 'fractional output scale')
        settle()
        compare('scaled', request(), reference_rounding=True)
        held_after = request('held')
        assert [hashlib.sha256((session.output / f['file']).read_bytes()).hexdigest() for f in held_after['frames']] == held_hashes
        request('release')
        checks.append('fractional scaling and rounded logical geometry preserve full physical dimensions, reference interior colors and immutable retained pixels')
    else:
        assert any(f['logical'][0] < 0 for f in first['frames'])
        assert len(first['frames']) == 2
        for transform in ['normal', '90', '180', '270', 'flipped', 'flipped-90', 'flipped-180', 'flipped-270']:
            session.run(['swaymsg', 'output', 'HEADLESS-1', 'transform', transform])
            session.wait_for(lambda: outputs()['HEADLESS-1']['logical']['transform'] == transform, 'output transform')
            settle()
            compare('transform-' + transform, request())
        checks.append('all eight output transforms preserve real nonuniform source pixels')
        session.run(['swaymsg', 'output', 'HEADLESS-2', 'disable'])
        session.wait_for(lambda: len(outputs()) == 1, 'output removal')
        settle()
        compare('one-output', request())
        session.run(['swaymsg', 'output', 'HEADLESS-2', 'enable'])
        session.wait_for(lambda: len(outputs()) == 2, 'output addition')
        settle()
        compare('two-again', request())
        checks.append('two outputs retain negative origins and different fractional scales; subsequent requests observe output removal and addition')

    socket = Path(session.env['WAYLAND_DISPLAY'])
    displaced = socket.with_name(socket.name + '.unavailable')
    socket.rename(displaced)
    try:
        assert 'error' in request(), 'missing compositor socket returned stale pixels'
    finally:
        displaced.rename(socket)
    compare('reconnected', request(), reference_rounding=args.compositor == 'niri')
    assert len(list(Path(f'/proc/{helper.pid}/fd').iterdir())) <= initial_descriptors + 1, 'completed captures leaked descriptors'
    checks.append('each request owns a new connection; unavailable socket fails and restored compositor can be captured again')
    helper.stdin.write('{"command":"stop"}\n')
    helper.stdin.flush()
    assert helper.wait(timeout=5) == 0
    session.processes.remove(helper)
    return {'status': 'passed', 'compositor': args.compositor, 'scale': args.scale, 'checks': checks, 'harness_sha256': hashlib.sha256(args.harness.read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--harness', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--compositor', choices=['niri', 'sway'], default='niri')
    parser.add_argument('--scale', type=float, default=1)
    args = parser.parse_args()
    args.harness = args.harness.resolve(strict=True)
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-capture-native-') as directory:
        session = desktop.Session(Path(directory), args.output)
        try:
            result = verify(args, session)
        except Exception as error:
            desktop.write_json(args.output / 'result.json', {'status': 'failed', 'error': str(error)})
            raise
        finally:
            session.close()
    desktop.write_json(args.output / 'result.json', result)
    print(json.dumps(result))


if __name__ == '__main__':
    main()
