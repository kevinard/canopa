#!/usr/bin/env python3
"""Exercise Shell's private wallpaper owner on a disposable bus and Niri output."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
BUS = 'org.canopa.Shell.Wallpaper'
OBJECT = '/org/canopa/Shell/Wallpaper'
INTERFACE = 'org.canopa.Shell.Wallpaper1'


def verify(binary, session, scale, bar_layer):
    from gi.repository import Gio, GLib
    from PIL import Image
    checks = []
    runtime = session.runtime
    session.env['HOME'] = str(runtime / 'home')
    Path(session.env['HOME']).mkdir()
    bins = runtime / 'bin'
    bins.mkdir()
    locker = bins / 'swaylock'
    locker.write_text('#!/usr/bin/python3\nimport hashlib,json,sys\n' +
        'print(json.dumps({"args":sys.argv[1:],"hash":hashlib.sha256(open(sys.argv[2],"rb").read()).hexdigest() if len(sys.argv)>1 else None}))\n')
    locker.chmod(0o700)
    session.env['PATH'] = str(bins) + os.pathsep + session.env['PATH']
    output = session.start_niri('animations { off; }\nxwayland-satellite { off; }\n'
        'hotkey-overlay { skip-at-startup; }\n', width=round(1280 * scale), height=round(800 * scale), scale=scale)
    config = runtime / 'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    shell = (ROOT / 'crates/canopa-shell/config/default.kdl').read_text()
    shell = shell.replace('// layer "top"', f'layer "{bar_layer}"')
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "dark"; }}\nshell {{\n{shell}\n}}\n')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)

    def owned():
        return connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', 'NameHasOwner', GLib.Variant('(s)', (BUS,)),
            None, Gio.DBusCallFlags.NONE, 2000, None).unpack()[0]

    def layers():
        return [layer for layer in json.loads(session.run(['niri', 'msg', '--json', 'layers']))
            if layer['namespace'] == 'canopa-shell-wallpaper']

    process = session.start([str(binary)], 'shell-01.log')
    session.wait_for(owned, 'wallpaper owner', timeout=45)
    session.wait_for_layers(1)
    assert layers() == []
    state = Path(session.env['XDG_STATE_HOME']) / 'canopa/shell/wallpaper'
    assert not state.exists()
    bar_point = (round(44 * scale), round(200 * scale))
    bar_color = (27, 39, 43)
    session.wait_for(lambda: session.capture('01-empty.png').getpixel(bar_point) == bar_color,
        'first bar frame')

    def apply(path, targets):
        descriptor = os.open(path, os.O_RDONLY)
        try:
            descriptors = Gio.UnixFDList.new()
            index = descriptors.append(descriptor)
            result, _ = connection.call_with_unix_fd_list_sync(BUS, OBJECT, INTERFACE,
                'Apply', GLib.Variant('(hu)', (index, targets)), None,
                Gio.DBusCallFlags.NONE, 15000, descriptors, None)
            assert result.unpack() == ()
        finally:
            os.close(descriptor)

    def selected():
        return json.loads((state / 'selection.json').read_text())

    def color(name, expected):
        def painted():
            frame = session.capture(name)
            return frame if frame.getpixel((frame.width // 2, frame.height // 2)) == expected else None
        frame = session.wait_for(painted, f'{name} desktop pixels')
        for point in [(2, 2), (frame.width - 2, 2), (2, frame.height - 2),
                      (frame.width - 2, frame.height - 2)]:
            assert frame.getpixel(point) == expected, 'wallpaper did not cover the complete output'
        session.wait_for(lambda: session.capture(name).getpixel(bar_point) == bar_color,
            'bar above wallpaper')
        return frame

    source = runtime / 'caller.png'
    red, blue, green = (180, 20, 40), (10, 40, 190), (10, 140, 60)
    Image.new('RGB', (2048, 1024), red).save(source)
    apply(source, 3)
    first = selected()
    assert first['background'] == first['lockscreen']
    assert Image.open(state / (first['background'] + '.png')).size == (2048, 1024)
    assert len(layers()) == 1
    color('02-both.png', red)
    source.unlink()
    color('03-source-deleted.png', red)
    lock = json.loads(session.run([str(binary), 'lock']))
    assert lock['hash'] == first['lockscreen']
    assert lock['args'][0] == '--image' and lock['args'][1].startswith('/proc/self/fd/')
    checks.append('both destinations own an immutable full-resolution image and survive caller deletion')

    Image.new('RGB', (1024, 2048), blue).save(source)
    apply(source, 2)
    second = selected()
    assert second['background'] == first['background'] and second['lockscreen'] != first['lockscreen']
    assert json.loads(session.run([str(binary), 'lock']))['hash'] == second['lockscreen']
    color('04-lock-only.png', red)
    checks.append('lockscreen-only selection reaches the locker while retaining the desktop')

    Image.new('RGB', (1536, 1536), green).save(source)
    apply(source, 1)
    third = selected()
    assert third['background'] != second['background'] and third['lockscreen'] == second['lockscreen']
    color('05-background-only.png', green)
    checks.append('background-only selection changes actual pixels and preserves the lock image')

    original = (state / 'selection.json').read_bytes()
    source.write_bytes(b'not a PNG')
    for target in [1, 0, 4]:
        try:
            apply(source, target)
            raise AssertionError('invalid input accepted')
        except GLib.Error:
            pass
        assert (state / 'selection.json').read_bytes() == original
    color('06-invalid-retains.png', green)
    checks.append('malformed PNG and destination masks fail without changing the selection')

    session.run(['niri', 'msg', 'output', output, 'scale', '1.5'])
    bar_point = (round(44 * 1.5), round(200 * 1.5))
    color('07-scale.png', green)
    checks.append('background resizes with the real output scale and keeps the bar visible')

    session.retire(process)
    session.wait_for(lambda: not owned(), 'owner gone')
    process = session.start([str(binary)], 'shell-02.log')
    session.wait_for(owned, 'restarted wallpaper owner', timeout=45)
    session.wait_for(lambda: len(layers()) == 1, 'restored wallpaper surface')
    color('08-restart.png', green)
    assert json.loads(session.run([str(binary), 'lock']))['hash'] == third['lockscreen']
    assert len(list(state.glob('*.png'))) == 2
    checks.append('Shell restart restores desktop and locker selections; obsolete images are reclaimed')
    # The real locker only connects to this disposable compositor. Its debug log
    # verifies that the inherited image descriptor survives exec and image loading.
    locker.unlink()
    locker_config = runtime / 'config/swaylock/config'
    locker_config.parent.mkdir(parents=True)
    locker_config.write_text('debug\n')
    actual_locker = session.start([str(binary), 'lock'], 'swaylock.log')
    try:
        session.wait_for(lambda: 'Loaded image /proc/self/fd/' in (session.output / 'swaylock.log').read_text(),
            'real Swaylock image load on the private compositor', timeout=30)
    except Exception:
        process = Path('/proc') / str(actual_locker.pid)
        diagnostics = {}
        for name in ['wchan', 'status']:
            if (process / name).exists():
                diagnostics[name] = (process / name).read_text()
        if (process / 'fd').exists():
            diagnostics['fds'] = {p.name: os.readlink(p) for p in (process / 'fd').iterdir()}
        desktop.write_json(session.output / 'swaylock-diagnostic.json', diagnostics)
        raise
    checks.append('installed Swaylock loads the pinned image on the disposable compositor')
    session.retire(actual_locker)
    connection.close_sync(None)
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--scale', type=float, default=1.25)
    parser.add_argument('--bar-layer', choices=['background', 'bottom', 'top', 'overlay'], default='top')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-wallpaper-owner-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        try:
            checks = verify(args.binary.resolve(), session, args.scale, args.bar_layer)
            desktop.write_json(args.output / 'result.json', dict(passed=True, checks=checks,
                scale=args.scale, bar_layer=args.bar_layer,
                binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest()))
        finally:
            for process in reversed(session.processes):
                desktop.stop(process)
    print(f'Wallpaper owner: {len(checks)} native checks passed')


if __name__ == '__main__':
    main()
