#!/usr/bin/env python3
"""Check production Screenshot PNG composition on a private two-output Sway."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def verify(args, session):
    from gi.repository import Gio, GLib
    from PIL import Image
    from urllib.parse import unquote, urlsplit
    session.start_private_bus()
    session.env.update(WLR_BACKENDS='headless', WLR_HEADLESS_OUTPUTS='2', WLR_RENDERER='pixman', WLR_LIBINPUT_NO_DEVICES='1')
    session.env.pop('SWAYSOCK', None)
    config = session.runtime / 'sway.conf'
    config.write_text('seat seat0 fallback true\noutput HEADLESS-1 mode 800x600 scale 1 pos 0 0 bg #b41428 solid_color\noutput HEADLESS-2 mode 800x600 scale 1.25 pos -720 0 bg #0a288c solid_color\n')
    session.start(['sway', '--unsupported-gpu', '-c', str(config)], 'sway.log')
    session.env['SWAYSOCK'] = str(session.wait_for(lambda: list(session.runtime.glob('sway-ipc.*.sock')), 'Sway IPC')[0])
    session.env['WAYLAND_DISPLAY'] = str(session.wait_for(lambda: [p for p in session.runtime.glob('wayland-*') if p.is_socket()], 'Sway display')[0])
    session.wait_for(lambda: len(json.loads(session.run(['swaymsg', '-r', '-t', 'get_outputs']))) == 2, 'two outputs')
    session.start([str(args.binary), '--defaults'], 'backend.log')
    flags = Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], flags, None, None)
    name = 'org.freedesktop.impl.portal.desktop.canopa'
    path = '/org/freedesktop/portal/desktop'
    def call(destination, object_path, interface, method, signature, values):
        return connection.call_sync(destination, object_path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 40000, None).unpack()
    session.wait_for(lambda: call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', '(s)', (name,))[0], 'backend')
    checks = []
    count = 0
    def capture():
        nonlocal count
        count += 1
        response, values = call(name, path, 'org.freedesktop.impl.portal.Screenshot', 'Screenshot', '(ossa{sv})', (f'/org/freedesktop/portal/desktop/request/composition/{count}', 'org.canopa.Test', '', {'permission_store_checked': GLib.Variant('b', True), 'target': GLib.Variant('u', 1)}))
        assert response == 0, (response, values)
        uri = values['uri']; uri = uri.unpack() if hasattr(uri, 'unpack') else uri
        file = Path(unquote(urlsplit(uri).path))
        assert file.is_relative_to(Path(session.env['XDG_CACHE_HOME']))
        image = Image.open(file).convert('RGBA'); image.save(session.output / f'composed-{count}.png')
        return image
    image = capture()
    assert image.size == (1900, 750), image.size
    for box, color in [((0, 0, 800, 600), (10, 40, 140, 255)), ((800, 0, 900, 750), (0, 0, 0, 0)), ((0, 600, 800, 750), (0, 0, 0, 0)), ((900, 0, 1900, 750), (180, 20, 40, 255))]:
        region = image.crop(box)
        assert region.getcolors(region.width * region.height) == [(region.width * region.height, color)], box
    checks.append('production PNG preserves two differently scaled outputs, negative origin and transparent layout gaps')
    session.run(['swaymsg', 'output', 'HEADLESS-2', 'disable'])
    session.wait_for(lambda: len([o for o in json.loads(session.run(['swaymsg', '-r', '-t', 'get_outputs'])) if o['active']]) == 1, 'one output')
    image = capture()
    assert image.size == (800, 600)
    assert image.getcolors(800 * 600) == [(800 * 600, (180, 20, 40, 255))]
    checks.append('subsequent production capture observes output removal and preserves single-output physical dimensions')
    return {'status': 'passed', 'checks': checks, 'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(); args.binary = args.binary.resolve(strict=True); args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-screenshot-composition-') as directory:
        session = desktop.Session(Path(directory), args.output)
        try:
            result = verify(args, session)
        except Exception as error:
            desktop.write_json(args.output / 'result.json', {'status': 'failed', 'error': str(error)})
            raise
        finally:
            session.close()
    desktop.write_json(args.output / 'result.json', result); print(json.dumps(result))


if __name__ == '__main__':
    main()
