#!/usr/bin/env python3
"""Exercise the private Print draft and discovery helpers; no Print wire claims."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import select
import socket
import subprocess
import sys
import tempfile
import threading
import time
import traceback

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from native_compositor import freeze_niri, sha256


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def request(title):
    return dict(purpose='prepare', prepare=dict(
        context=dict(app_id='org.canopa.PrintFixture', parent_window='', title=title, modal=True),
        accept_label='Print', formats=['pdf', 'ps', 'svg'],
        page_choices=dict(current=False, selection=False),
        settings=dict(layout=dict(orientation='portrait', copies=1, collate=False,
            reverse=False, scale=100., pages='all', ranges=[], page_set='all',
            number_up=1, number_up_layout='lrtb'), advisory={}),
        page_setup={key: None for key in ['ppd_name', 'name', 'display_name', 'orientation',
            'width', 'height', 'top', 'bottom', 'left', 'right']}))


def verify(args, session, result):
    def group(name, **evidence):
        result['checks'].append(dict(name=name, **evidence))
    session.env['PATH'] = str(args.niri.parent) + os.pathsep + session.env['PATH']
    session.env['HOME'] = str(session.runtime / 'home')
    Path(session.env['HOME']).mkdir()
    preferences = Path(session.env['HOME']) / '.cups'
    preferences.mkdir()
    (preferences / 'lpoptions').write_text(f'Default {args.queue}\nDest {args.queue}/Short sides=two-sided-short-edge media=iso_a4_210x297mm\n')
    session.env.update(CUPS_SERVER=str(args.cups_server), CUPS_SERVERROOT=str(session.runtime / 'cups-options'))
    config = session.runtime / 'appearance.kdl'
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.scheme}"; }}\n')

    def worker(operation):
        read, write = os.pipe2(os.O_CLOEXEC)
        process = None
        record = {}
        try:
            process = subprocess.Popen([str(args.binary), '--print-worker'], env=session.env,
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                pass_fds=(read,), text=True, start_new_session=True)
            record['pid'] = process.pid
            result.setdefault('workers', []).append(record)
            out, err = process.communicate(json.dumps(dict(cancel=read, operation=operation)) + '\n', timeout=15)
            record['stderr'] = err
            assert process.returncode == 0, err
            return [json.loads(line) for line in out.splitlines()]
        finally:
            os.close(read)
            os.close(write)
            if process is not None:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
                record['exit_code'] = process.returncode

    listed = worker(dict(operation='printers'))
    assert len(listed) == 1 and listed[0]['event'] == 'printers', listed
    printers = listed[0]['printers']
    destination = dict(queue=args.queue, instance=None)
    caps = {}
    if args.file_only:
        assert printers == [], printers
        group('healthy-empty-scheduler-keeps-file-output-available')
    else:
        assert any(p['destination'] == destination and p['accepting'] for p in printers), printers
        capabilities = worker(dict(operation='capabilities', destination=destination))
        assert len(capabilities) == 1 and capabilities[0]['event'] == 'capabilities', capabilities
        caps = capabilities[0]['capabilities']
        assert caps['destination'] == destination and caps['copies'][0] <= 1 <= caps['copies'][1], caps
        assert any(abs(p['width_mm'] - 210) < .01 and abs(p['height_mm'] - 297) < .01 for p in caps['media']), caps
        group('real-cups-catalog-and-geometry', printers=printers, capabilities=caps)
        instance = worker(dict(operation='capabilities', destination=dict(queue=args.queue, instance='Short')))
        assert instance[0]['event'] == 'capabilities' and instance[0]['capabilities']['default_sides'] == 'short_edge', instance
        assert instance[0]['capabilities']['default_media'] == 'iso_a4_210x297mm', instance
        group('saved-instance-options-are-validated-and-preserved', capabilities=instance[0]['capabilities'])
        missing = worker(dict(operation='capabilities', destination=dict(queue='NotConfigured', instance=None)))
        assert len(missing) == 1 and missing[0]['event'] == 'failed', missing
        group('unknown-queue-is-an-explicit-failure', frames=missing)
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; focus-ring { off; }; }\n',
        width=round(1280 * args.scale), height=round(800 * args.scale), scale=args.scale)
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    session.start([str(args.parent)], 'parent.log')
    exported = session.wait_for(lambda: next((line for line in (args.output / 'parent.log').read_text().splitlines() if line.startswith('wayland:')), None), 'exported parent')
    pointer = session.build_pointer()
    logical = next(output['logical'] for output in json.loads(session.run(['niri', 'msg', '--json', 'outputs'])).values() if output.get('logical'))
    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
    def click(x, y):
        session.run([str(pointer), str(round(x / logical['width'] * 1600)), str(round(y / logical['height'] * 1000))])
        time.sleep(.15)
    def focus(window):
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        if window['is_floating']:
            session.run(['niri', 'msg', 'action', 'move-floating-window', '--id', str(window['id']), '--x', '20', '--y', '10'])
        time.sleep(.3)
    def keys(key):
        session.run(['wtype', '-k', key])
        time.sleep(.15)
    def begin(title, server=None, parent=None, reject=False):
        before = {window['id'] for window in windows()}
        index = len(result.setdefault('dialogs', [])) + 1
        output = args.output / f'dialog-{index:02}.json'
        errors = args.output / f'dialog-{index:02}.err'
        env = dict(session.env)
        if server is not None:
            env['CUPS_SERVER'] = str(server)
        packet = request(title)
        packet['prepare']['context']['parent_window'] = exported if parent is None else parent
        input_path = session.runtime / f'request-{index:02}.json'
        input_path.write_text(json.dumps(packet))
        output.touch()
        env.update(CANOPA_PRINT_REVIEW_INPUT=str(input_path), CANOPA_PRINT_REVIEW_OUTPUT=str(output),
            CANOPA_PRINT_REVIEW_BINARY=str(args.binary), CANOPA_PRINT_REVIEW_CONFIG=str(config))
        env.pop('CANOPA_PRINT_REVIEW_EXPECT_REJECT', None)
        if reject:
            env['CANOPA_PRINT_REVIEW_EXPECT_REJECT'] = '1'
        with (args.output / f'dialog-{index:02}-harness.out').open('w') as out, errors.open('w') as err:
            process = subprocess.Popen([str(args.harness), 'print::presentation::tests::native_review_fixture', '--exact', '--ignored', '--nocapture'],
                stdin=subprocess.DEVNULL, stdout=out, stderr=err, env=env, cwd=ROOT / 'crates/canopa-portal', start_new_session=True, text=True)
        session.print_dialogs.append(process)
        result['dialogs'].append(dict(pid=process.pid, title=title, output=str(output), errors=str(errors)))
        def owned_window():
            for window in windows():
                if window['id'] in before or window['app_id'] != 'org.canopa.Portal' or not window.get('pid'):
                    continue
                pid = window['pid']
                status = Path(f'/proc/{pid}/status').read_text().splitlines()
                if f'PPid:\t{process.pid}' not in status:
                    continue
                assert Path(f'/proc/{pid}/exe').resolve() == args.binary
                return window
            return None
        window = session.wait_for(owned_window, 'owned Print dialog window')
        result['dialogs'][-1]['ui_pid'] = window['pid']
        result['dialogs'][-1]['window'] = window
        session.print_helpers.append((window['pid'], os.pidfd_open(window['pid'])))
        if packet['prepare']['context']['parent_window']:
            assert window['is_floating'], window
        focus(window)
        time.sleep(.6)
        window = next(w for w in windows() if w['id'] == window['id'])
        return process, window, output
    def finish(dialog, expected):
        process, window, output = dialog
        code = process.wait(timeout=18)
        assert code == 0, (args.output / f"dialog-{session.print_dialogs.index(process)+1:02}.err").read_text()
        assert not any(w['id'] == window['id'] for w in windows())
        packet = json.loads(output.read_text())
        value = packet['grant']
        assert packet['file_pin'] == (expected in ('file', 'file-existing')), packet
        if expected == 'rejected':
            assert packet.get('rejected') and value is None, packet
        elif expected is None:
            assert value is None, value
        elif expected in ('file', 'file-existing'):
            target = value['target']['file']
            assert target['format'] == 'pdf', value
            assert bytes(target['path']) == os.fsencode(Path(session.env['HOME']) / 'File review.pdf'), value
            path = Path(os.fsdecode(bytes(target['path'])))
            assert target['selection']['path'] == target['path'], value
            assert (target['selection']['existing'] is not None) == (expected == 'file-existing'), value
            if expected == 'file':
                assert not path.exists(), value
            else:
                assert path.read_bytes() == b'reviewed original bytes', value
            assert value['page_setup']['name'] == 'canopa-custom' and value['page_setup']['width'] == 210. and value['page_setup']['height'] == 297., value
        else:
            assert value['target'] == {'printer': destination}, value
            assert value['device']['copies'] == value['layout']['copies'] == 1, value
            assert value['layout']['pages'] == 'all' and value['layout']['ranges'] == [], value
            if caps['default_media']:
                assert value['device']['media'] == caps['default_media'], value
        result['dialogs'][session.print_dialogs.index(process)].update(exit_code=code, grant=value, file_pin=packet['file_pin'], rejected=packet.get('rejected', False))
        return value
    def approve(dialog):
        _, window, _ = dialog
        width, height = window['layout']['window_size']
        click(20 + width - 55, 10 + height - 37)

    if args.file_only:
        dialog = begin('File review')
        session.capture('01-file-draft.png')
        approve(dialog)
        assert dialog[0].poll() is None and dialog[2].read_text() == '', 'file approved without a picker target'
        def choose_file():
            width, _ = dialog[1]['layout']['window_size']
            click(20 + width - 85, 10 + 244)
            session.wait_for(lambda: any(w['id'] == dialog[1]['id'] and w['title'] == 'Save printed document — Canopa' for w in windows()), 'embedded Files picker')
            time.sleep(.8)
        choose_file()
        session.capture('02-embedded-file-picker.png')
        keys('Escape')
        session.wait_for(lambda: any(w['id'] == dialog[1]['id'] and w['title'] == 'Print — Canopa' for w in windows()), 'return to file draft')
        assert dialog[0].poll() is None and dialog[2].read_text() == ''
        group('embedded-picker-cancellation-preserves-request-without-approval')
        choose_file()
        approve(dialog)
        session.wait_for(lambda: any(w['id'] == dialog[1]['id'] and w['title'] == 'Print — Canopa' for w in windows()), 'selected file returns to draft')
        session.capture('03-selected-file-draft.png')
        approve(dialog)
        value = finish(dialog, 'file')
        group('explicit-file-draft-preserves-selected-path-without-creating-output', grant=value)
        path = Path(session.env['HOME']) / 'File review.pdf'
        dialog = begin('File review', reject=True)
        choose_file()
        approve(dialog)
        session.wait_for(lambda: any(w['id'] == dialog[1]['id'] and w['title'] == 'Print — Canopa' for w in windows()), 'new-file draft selected')
        path.write_bytes(b'newer work after selection')
        approve(dialog)
        finish(dialog, 'rejected')
        assert path.read_bytes() == b'newer work after selection'
        group('late-file-appearance-does-not-acquire-replacement-authority')
        path.unlink()
        path.write_bytes(b'reviewed original bytes')
        dialog = begin('File review')
        choose_file()
        approve(dialog)
        time.sleep(.6)
        session.capture('04-overwrite-review.png')
        assert any(w['id'] == dialog[1]['id'] and w['title'] == 'Save printed document — Canopa' for w in windows())
        width, height = dialog[1]['layout']['window_size']
        click(20 + width / 2 + 160, 10 + height / 2 + 49)
        session.wait_for(lambda: any(w['id'] == dialog[1]['id'] and w['title'] == 'Print — Canopa' for w in windows()), 'confirmed overwrite draft')
        approve(dialog)
        finish(dialog, 'file-existing')
        group('explicit-overwrite-review-transfers-existing-entry-without-changing-bytes')
        return

    first = begin('Enter cannot print')
    session.capture('01-printer-draft.png')
    keys('Return')
    assert first[0].poll() is None and first[2].read_text() == ''
    keys('Escape')
    finish(first, None)
    group('enter-does-not-approve-escape-cancels')

    second = begin('Explicit printer approval')
    approve(second)
    grant = finish(second, True)
    group('explicit-approval-returns-supported-printer-draft', grant=grant)

    first = begin('Independent first document')
    second = begin('Independent second document')
    keys('Escape')
    finish(second, None)
    focus(first[1])
    approve(first)
    finish(first, True)
    group('concurrent-drafts-and-cancellation-are-independent')

    unavailable = begin('Unavailable scheduler', session.runtime / 'absent-cups.sock')
    keys('Escape')
    finish(unavailable, None)
    group('scheduler-failure-does-not-prevent-dialog-cancellation')

    parentless = begin('Parentless cancellation', parent='')
    keys('Escape')
    finish(parentless, None)
    group('parentless-dialog-can-be-cancelled-without-assuming-compositor-placement')

    # Force a real worker to remain in metadata I/O; an already-failed query
    # cannot establish cancellation/reaping of an in-flight child.
    server = session.runtime / 'stalled-cups.sock'
    accepted, stop = threading.Event(), threading.Event()
    listener = socket.socket(socket.AF_UNIX)
    listener.bind(str(server))
    listener.listen(2)
    listener.settimeout(.2)
    def stalled_scheduler():
        while not stop.is_set():
            try:
                connection, _ = listener.accept()
            except socket.timeout:
                continue
            with connection:
                accepted.set()
                stop.wait(20)
            return
    thread = threading.Thread(target=stalled_scheduler, daemon=True)
    thread.start()
    pidfds = []
    try:
        stalled = begin('Cancel in-flight metadata', server)
        assert accepted.wait(3), 'metadata worker did not contact the stalled scheduler'
        children = set()
        for path in Path(f"/proc/{stalled[1]['pid']}/task").glob('*/children'):
            children.update(map(int, path.read_text().split()))
        assert children, 'no live metadata helper at cancellation'
        for pid in children:
            assert b'--print-worker' in Path(f'/proc/{pid}/cmdline').read_bytes()
            pidfds.append(os.pidfd_open(pid))
        keys('Escape')
        finish(stalled, None)
        assert all(select.select([fd], [], [], 5)[0] for fd in pidfds)
        group('cancelling-live-metadata-reaps-owned-helper-before-ui-success', retired_helpers=sorted(children))
    finally:
        stop.set()
        thread.join(timeout=2)
        listener.close()
        for fd in pidfds:
            os.close(fd)
        assert not thread.is_alive()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'niri', 'parent', 'harness'):
        parser.add_argument('--' + name, type=Path, required=True)
        parser.add_argument('--' + name + '-sha256', required=True)
    parser.add_argument('--cups-server', type=Path, required=True, help='Socket of a disposable scheduler; no queue is changed by this script')
    parser.add_argument('--queue', default='CanopaFixture')
    parser.add_argument('--file-only', action='store_true', help='Expect an empty private scheduler and exercise the embedded Save picker')
    parser.add_argument('--scheme', choices=('dark', 'light'), default='dark')
    parser.add_argument('--scale', type=float, default=1.)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    for name in ('binary', 'niri', 'parent', 'harness'):
        path = getattr(args, name).resolve(strict=True)
        assert sha256(path) == getattr(args, name + '_sha256')
        setattr(args, name, path)
    args.niri = freeze_niri(args.niri, args.niri_sha256)
    args.cups_server = args.cups_server.resolve(strict=True)
    assert args.cups_server.is_socket() and str(args.cups_server).startswith(('/tmp/', str(ROOT / 'target/'))), 'Use a private test scheduler'
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    paths = [Path(__file__), args.binary, args.niri, args.parent, args.harness, ROOT / 'scripts/validate-desktop.py', ROOT / 'scripts/native_compositor.py']
    paths.extend((ROOT / 'crates/canopa-portal/src').rglob('*.rs'))
    pins = {str(path): sha256(path) for path in paths}
    result = dict(status='running', scope='Private CUPS catalog and actual Iced draft; no public Print interface, document rendering, submission, physical printer or installed routing acceptance', scheme=args.scheme, scale=args.scale, file_only=args.file_only, checks=[], pins=pins)
    desktop = load('desktop', ROOT / 'scripts/validate-desktop.py')
    with tempfile.TemporaryDirectory(prefix='canopa-print-dialog-') as directory:
        session = desktop.Session(Path(directory), args.output)
        session.print_dialogs = []
        session.print_helpers = []
        try:
            verify(args, session, result)
            result['status'] = 'passed'
        except BaseException:
            result.update(status='failed', error=traceback.format_exc())
            try:
                session.capture('failure.png')
            except Exception:
                pass
        finally:
            for process in reversed(session.print_dialogs):
                desktop.stop(process)
            session.close()
            result['processes'] = [dict(pid=p.pid, exit_code=p.poll()) for p in session.processes + session.print_dialogs]
            result['ui_helpers'] = []
            for pid, pidfd in session.print_helpers:
                terminal = bool(select.select([pidfd], [], [], 5)[0])
                os.close(pidfd)
                result['ui_helpers'].append(dict(pid=pid, terminal=terminal))
            if not all(helper['terminal'] for helper in result['ui_helpers']):
                result['status'] = 'failed'
    result['changed_inputs'] = [path for path, checksum in pins.items() if sha256(Path(path)) != checksum]
    if result['changed_inputs'] or any(p['exit_code'] is None for p in result['processes']):
        result['status'] = 'failed'
    (args.output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({key: value for key, value in result.items() if key not in ('pins', 'checks')}, indent=2))
    raise SystemExit(0 if result['status'] == 'passed' else 1)


if __name__ == '__main__':
    main()
