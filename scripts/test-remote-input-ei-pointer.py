#!/usr/bin/env python3
"""Real libei pointer devices on a private Niri desktop; never the live session."""
import argparse
import array
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / 'scripts'))
from screencast_media import MediaSession, digest


def verify(args, session, result):
    from gi.repository import Gio, GLib
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
layout { border { off; }; focus-ring { off; }; }
''', scale=1.25)
    out = session.output
    for mode, suffix in [('client-header', '.h'), ('private-code', '.c')]:
        subprocess.run(['wayland-scanner', mode, '/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml', str(out / ('xdg-shell' + suffix))], check=True)
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(out), str(ROOT / 'scripts/desktop-fixture/remote-input-pointer-receiver.c'), str(out / 'xdg-shell.c'), '-lwayland-client', '-lxkbcommon', '-o', str(out / 'receiver')], check=True)
    flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'libei-1.0'], text=True).split()
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', str(ROOT / 'scripts/desktop-fixture/remote-input-ei-pointer.c'), '-o', str(out / 'client'), *flags], check=True)
    receiver = session.start([str(out / 'receiver')], 'receiver.log')
    log = out / 'receiver.log'
    session.wait_for(lambda: 'focused' in log.read_text(), 'receiver focus')
    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
    wid = str(session.wait_for(lambda: next((w['id'] for w in windows() if w['pid'] == receiver.pid), None), 'receiver window'))
    def action(*values):
        return session.run(['niri', 'msg', 'action', *map(str, values)])
    action('move-window-to-floating', '--id', wid)
    def resize(width, height):
        for dimension, value in [('width', width), ('height', height)]:
            action('set-window-' + dimension, '--id', wid, value)
        session.wait_for(lambda: next(w for w in windows() if str(w['id']) == wid)['layout']['window_size'] == [width, height], 'committed size')
    resize(400, 300)
    action('move-floating-window', '--id', wid, '--x', 100, '--y', 100)
    def passed(name, **details):
        result['checks'].append(dict(name=name, **details))
        (out / 'progress.json').write_text(json.dumps(result, indent=2) + '\n')
        print(name, flush=True)
    def events(path, prefix=''):
        return [json.loads(line.split(prefix, 1)[1] if prefix else line) for line in path.read_text().splitlines() if (prefix and prefix in line) or (not prefix and line.startswith('{'))]
    fixture_log = out / 'capture.log'
    env = {**session.env, 'CANOPA_POINTER_NATIVE': '1', 'CANOPA_POINTER_WINDOW': wid, 'CANOPA_PORTAL_BINARY': str(args.portal)}
    with fixture_log.open('w') as stream:
        fixture = subprocess.Popen([str(args.harness), '--exact', 'remote_input::pointer::tests::native_pointer_fixture', '--ignored', '--nocapture', '--test-threads=1'], stdin=subprocess.PIPE, stdout=stream, stderr=stream, env=env, text=True, start_new_session=True)
    session.processes.append(fixture)
    def fixture_command(value):
        before = len(events(fixture_log, 'POINTER '))
        fixture.stdin.write(json.dumps(value) + '\n'); fixture.stdin.flush()
        return session.wait_for(lambda: events(fixture_log, 'POINTER ')[before] if len(events(fixture_log, 'POINTER ')) > before else None, 'capture fixture reply', timeout=20)
    ready = session.wait_for(lambda: next((e for e in events(fixture_log, 'POINTER ') if e.get('ready')), None), 'capture ready', timeout=40)
    assert fixture_command({'close': True}) == {'closed': True}
    binding = ready['binding']; streams = ready['streams']
    assert len(binding['regions']) == len(streams) == 2
    window_index = next(i for i, s in enumerate(streams) if s['source'] == {'Window': int(wid)})
    import gi
    gi.require_version('Gst', '1.0'); gi.require_version('GstVideo', '1.0')
    from gi.repository import Gst, GstVideo
    from PIL import Image
    for key in ['XDG_RUNTIME_DIR', 'PIPEWIRE_RUNTIME_DIR', 'PIPEWIRE_REMOTE']:
        os.environ[key] = session.env[key]
    Gst.init(None)
    def video(width, height, scale, label):
        node = streams[window_index]['node']
        serial = next(int(v['info']['props']['object.serial']) for v in session.registry() if v['id'] == node)
        pipeline = Gst.parse_launch(f'pipewiresrc target-object={serial} ! videoconvert ! video/x-raw,format=RGBA ! appsink name=frames sync=false max-buffers=2 drop=true')
        sink = pipeline.get_by_name('frames'); expected = (round(width*scale), round(height*scale))
        try:
            pipeline.set_state(Gst.State.PLAYING); deadline = time.monotonic()+10; image = None
            while time.monotonic() < deadline:
                sample = sink.emit('try-pull-sample', Gst.SECOND)
                if sample is None:
                    continue
                info = GstVideo.VideoInfo.new_from_caps(sample.get_caps()); buffer = sample.get_buffer(); ok, mapping = buffer.map(Gst.MapFlags.READ); assert ok
                try:
                    image = Image.frombytes('RGBA', (info.width, info.height), bytes(mapping.data)[info.offset[0]:], 'raw', 'RGBA', info.stride[0])
                finally:
                    buffer.unmap(mapping)
                if image.size == expected:
                    break
            assert image is not None and image.size == expected, (image.size if image else None, expected)
            for (x, y), color in zip([(1,1),(3,1),(1,3),(3,3)], [(208,32,32),(32,176,80),(48,79,208),(224,176,32)]):
                pixel = image.getpixel((image.width*x//4, image.height*y//4))[:3]
                assert max(abs(a-b) for a,b in zip(pixel,color)) <= 8, (pixel,color)
            image.save(out / (label+'.png'))
            result.setdefault('frames', []).append({'node':node,'serial':serial,'size':expected,'logical_size':[width,height],'scale':scale,'file':label+'.png'})
        finally:
            pipeline.set_state(Gst.State.NULL); pipeline.get_state(5*Gst.SECOND)
    def connection():
        return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    backend, caller = connection(), connection()
    input_owner = backend.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetNameOwner', GLib.Variant('(s)', ('org.canopa.Niri.RemoteInput1',)), None, Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
    # The capture fixture and candidate worker each own their own bus connection.
    # Bindings pin the same compositor process and exact source/mapping identity.
    def launch(label, grants=3, capture=binding):
        parent, child = socket.socketpair(); parent.settimeout(30)
        request = dict(compositor=input_owner, compositor_pid=next(p.pid for p in session.processes if Path(p.args[0]).name == 'niri'), ipc_socket=session.env['NIRI_SOCKET'], backend=backend.get_unique_name(), caller=caller.get_unique_name(), grants=grants)
        if capture is not None:
            request['capture'] = capture
        with (out / (label + '-worker.log')).open('w') as stream:
            worker = subprocess.Popen([str(args.portal), '--remote-input-worker'], stdin=subprocess.PIPE, stdout=child, stderr=stream, env=session.env, start_new_session=True)
        child.close(); session.processes.append(worker)
        worker.stdin.write(json.dumps(request).encode() + b'\n'); worker.stdin.flush()
        payload, ancillary, flags, _ = parent.recvmsg(1, socket.CMSG_SPACE(4), socket.MSG_CMSG_CLOEXEC)
        fds = array.array('i')
        for level, kind, data in ancillary:
            assert (level, kind) == (socket.SOL_SOCKET, socket.SCM_RIGHTS)
            fds.frombytes(data)
        assert payload == b'\x01' and len(fds) == 1 and not flags & ~socket.MSG_CMSG_CLOEXEC, (payload, fds, flags, (out / (label + '-worker.log')).read_text())
        control_read, control_write = os.pipe()
        client_log = out / (label + '-client.log')
        with client_log.open('w') as stream:
            client = subprocess.Popen([str(out / 'client'), str(control_read), str(grants), str(int(capture is not None))], stdin=fds[0], stdout=stream, stderr=stream, env=session.env, pass_fds=(control_read,), start_new_session=True)
        os.close(fds[0]); os.close(control_read); parent.close(); session.processes.append(client)
        def client_events():
            return events(client_log)
        def command(text):
            before = len(client_events()); os.write(control_write, (text + '\n').encode())
            session.wait_for(lambda: any(e.get('sent') == text for e in client_events()[before:]), 'libei sent ' + text)
        session.wait_for(lambda: any(e.get('resumed') == 'pointer' for e in client_events()), 'EI pointer resumed')
        if grants & 1:
            session.wait_for(lambda: any(e.get('resumed') == 'keyboard' for e in client_events()), 'EI keyboard resumed')
        return worker, client, control_write, client_events, command
    def line(text, before):
        session.wait_for(lambda: any(text in e for e in log.read_text().splitlines()[before:]), text)
    def count(text):
        return log.read_text().splitlines().count(text)
    def finish(process, timeout=20, expected=0):
        status = process.wait(timeout=timeout)
        assert status == expected, (process.args, status, expected)
        session.processes.remove(process)
        result.setdefault('retired_processes', []).append({'pid': process.pid, 'command': process.args, 'exit_code': process.returncode})
    worker, client, control, client_events, command = launch('paired')
    def region_generation():
        return [e for e in client_events() if e.get('added') == 'pointer'][-1]
    announced = region_generation()['regions']
    assert [r['id'] for r in announced] == [r['mapping_id'] for r in binding['regions']]
    assert [r['size'] for r in announced] == [r['size'] for r in binding['regions']]
    assert all(r['scale'] == 1.25 for r in announced)
    assert announced[1]['offset'][0] == announced[0]['size'][0] and announced[0]['offset'] == [0, 0]
    passed('libei-keyboard-and-pointer-have-independent-devices-and-paired-nonoverlapping-regions', regions=announced)
    def motion(x, y):
        before = len(log.read_text().splitlines()); command(f'absolute {window_index} {x} {y}')
        session.wait_for(lambda: any((m := re.fullmatch(r'(?:enter|motion) ([-\d.]+) ([-\d.]+)', e)) and abs(float(m[1])-x) < .12 and abs(float(m[2])-y) < .12 for e in log.read_text().splitlines()[before:]), 'actual window coordinate')
    motion(30, 40)
    video(400, 300, 1.25, 'paired-before')
    before = len(log.read_text().splitlines()); command('relative 7.5 -3.25'); line('motion 37.50 36.75', before)
    passed('real-libei-absolute-and-relative-motion-reach-selected-client-coordinates')
    before = len(log.read_text().splitlines())
    command('scroll 1.5 -2.25'); line('axis 0 -2.25', before); line('axis 1 1.50', before)
    command('stop 1 1'); line('axis-stop 0', before); line('axis-stop 1', before)
    command('wheel 120 -240'); line('axis-discrete 0 -2', before); line('axis-discrete 1 1', before)
    command('scroll 0 0'); command('wheel 0 0')
    command('scroll 0 1'); command('cancel 0 1')
    passed('smooth-scroll-discrete-v120-zero-deltas-and-axis-stop-cancel-delivered')
    before = len(log.read_text().splitlines()); command('key 42 1'); line('key 42 1 Shift_L', before)
    for index, (width, height, scale) in enumerate([(420, 320, 1.25), (420, 320, 1.5), (400, 300, 1.5), (400, 300, 1.25)]):
        previous = region_generation()['generation']; released = count('button 272 0')
        command('button 272 1'); session.wait_for(lambda: count('button 272 1') > released, 'held pointer button')
        if index % 2 == 0:
            resize(width, height)
        else:
            session.run(['niri', 'msg', 'output', 'winit', 'scale', str(scale)])
        session.wait_for(lambda: region_generation()['generation'] > previous, 'replacement EI device')
        current = region_generation(); generation = current['generation']
        session.wait_for(lambda: any(e.get('resumed') == 'pointer' and e.get('generation') == generation for e in client_events()), 'replacement resumed')
        session.wait_for(lambda: count('button 272 0') > released, 'old held button released')
        assert current['regions'][window_index]['size'] == [width, height] and current['regions'][window_index]['scale'] == scale
        assert [r['id'] for r in current['regions']] == [r['mapping_id'] for r in binding['regions']]
        assert count('key 42 0 Shift_L') == 0 and not any(e.get('removed') == 'keyboard' for e in client_events())
        motion(31 + index, 41 + index)
        video(width, height, scale, 'paired-change-'+str(index))
        command('button 272 1'); command('button 272 0')
        passed('capture-change-replaces-EI-pointer-after-held-cleanup-with-keyboard-continuity-' + str(index), generation=generation, regions=current['regions'])
    released = count('button 272 0'); command('button 272 1'); command('restart')
    session.wait_for(lambda: count('button 272 0') > released, 'emulation stop cleanup'); motion(38, 48)
    passed('stop-emulating-cleans-held-input-before-next-sequence')
    previous = region_generation()['generation']; command('close')
    session.wait_for(lambda: any(e.get('removed') == 'pointer' and e.get('generation') == previous for e in client_events()), 'client device release')
    command('request'); session.wait_for(lambda: region_generation()['generation'] > previous, 'requested replacement')
    session.wait_for(lambda: any(e.get('resumed') == 'pointer' and e.get('generation') == region_generation()['generation'] for e in client_events()), 'requested pointer resumed'); motion(39, 49)
    passed('client-can-request-a-fresh-pointer-without-replacing-keyboard')
    before = len(log.read_text().splitlines()); command('button 272 1'); line('button 272 1', before)
    before = len(log.read_text().splitlines()); worker.stdin.close()
    finish(worker); finish(client)
    line('button 272 0', before); line('key 42 0 Shift_L', before); os.close(control)
    passed('parent-cancellation-retires-both-devices-and-releases-held-input')
    worker, client, control, ce, command = launch('relative-only', grants=2, capture=None)
    assert [e for e in ce() if e.get('added') == 'pointer'][0]['regions'] == []
    before = len(log.read_text().splitlines()); command('relative 1 1'); line('motion 40.00 50.00', before)
    worker.stdin.close(); finish(worker); finish(client); os.close(control)
    passed('input-only-pointer-omits-absolute-regions-and-keyboard')
    for ending_index, ending in enumerate(['client-eof', 'client-kill', 'worker-kill', 'caller-loss', 'backend-loss']):
        worker, client, control, ce, command = launch(ending)
        x, y = 45 + ending_index, 55 + ending_index
        before = len(log.read_text().splitlines()); command(f'absolute {window_index} {x} {y}'); line(f'motion {x:.2f} {y:.2f}', before)
        before = len(log.read_text().splitlines()); command('key 42 1'); command('button 272 1'); line('key 42 1 Shift_L', before); line('button 272 1', before)
        before = len(log.read_text().splitlines())
        if ending == 'client-eof':
            os.close(control); control = None
        elif ending == 'client-kill':
            client.kill()
        elif ending == 'worker-kill':
            worker.kill()
        elif ending == 'caller-loss':
            caller.close_sync(None)
        else:
            backend.close_sync(None)
        finish(worker, expected=-9 if ending == 'worker-kill' else 0); finish(client, expected=-9 if ending == 'client-kill' else 0)
        line('button 272 0', before); line('key 42 0 Shift_L', before)
        if control is not None:
            os.close(control)
        if ending == 'caller-loss':
            caller = connection()
        elif ending == 'backend-loss':
            backend = connection()
        passed(ending+'-cleans-both-compositor-input-sources')
    worker, client, control, ce, command = launch('flood')
    before = len(log.read_text().splitlines()); os.write(control, b'flood\n')
    finish(worker, expected=1); finish(client); os.close(control)
    assert 'unframed' in (out / 'flood-worker.log').read_text()
    assert not any(re.fullmatch(r'(?:enter|motion) .*', e) for e in log.read_text().splitlines()[before:])
    passed('unframed-pointer-flood-rejected-before-native-motion')
    worker, client, control, ce, command = launch('capture-loss')
    before = len(log.read_text().splitlines()); command(f'absolute {window_index} 54 64'); line('motion 54.00 64.00', before)
    before = len(log.read_text().splitlines()); command('key 42 1'); command('button 272 1'); line('key 42 1 Shift_L', before); line('button 272 1', before)
    before = len(log.read_text().splitlines())
    # These endings are intentional; reap them before the shared desktop wait
    # helper checks the remaining long-lived processes.
    fixture.stdin.write('{"stop":true}\n'); fixture.stdin.flush(); finish(fixture)
    assert events(fixture_log, 'POINTER ')[-1] == {'stopped': True}
    finish(worker); finish(client); os.close(control)
    line('button 272 0', before); line('key 42 0 Shift_L', before)
    passed('capture-loss-retires-EI-and-held-keyboard-and-pointer-sources')
    backend.close_sync(None); caller.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['portal', 'harness', 'niri', 'output']:
        parser.add_argument('--' + name, required=True, type=Path)
    for name in ['portal', 'harness', 'niri']:
        parser.add_argument('--' + name + '-sha256', required=True)
    args = parser.parse_args()
    for name in ['portal', 'harness', 'niri']:
        path = getattr(args, name).resolve(); setattr(args, name, path)
        assert digest(path) == getattr(args, name + '_sha256')
    args.output = args.output.resolve(); args.output.mkdir(exist_ok=False)
    sources = [Path(__file__), ROOT / 'scripts/desktop-fixture/remote-input-ei-pointer.c', ROOT / 'scripts/desktop-fixture/remote-input-pointer-receiver.c', ROOT / 'scripts/screencast_media.py', ROOT / 'scripts/validate-desktop.py']
    pins = {str(p): digest(p) for p in sources}
    archive = args.output / 'sources'; archive.mkdir()
    for path in sources:
        shutil.copy2(path, archive / path.name)
    result = {'status': 'running', 'scope': 'Private real libei keyboard/pointer worker and production capture bindings; no public pointer consent/advertisement or compositor adoption', 'checks': [], 'pins': pins}
    started = time.monotonic(); code = 1
    with tempfile.TemporaryDirectory(prefix='canopa-ei-pointer-') as folder:
        session = MediaSession(Path(folder), args.output, args.niri, args.niri_sha256, reference_frozen=True)
        try:
            verify(args, session, result); result['status'] = 'passed'; code = 0
        except Exception as error:
            result.update(status='failed', error=repr(error), traceback=traceback.format_exc())
        finally:
            processes = list(session.processes); session.close()
            result['processes'] = [{'pid': p.pid, 'command': p.args, 'exit_code': p.poll()} for p in processes]
    result.update(seconds=round(time.monotonic()-started, 2), changed_sources=[p for p, h in pins.items() if digest(Path(p)) != h], exit_code=code)
    result['binaries_unchanged'] = all(digest(getattr(args, n)) == getattr(args, n + '_sha256') for n in ['portal', 'harness', 'niri'])
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: result.get(k) for k in ['status', 'seconds', 'error']}), flush=True)
    if code:
        print(result['traceback'])
    raise SystemExit(code)


if __name__ == '__main__':
    main()
