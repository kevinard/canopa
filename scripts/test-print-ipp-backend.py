#!/usr/bin/env python3
"""Check the actual CUPS backend entry point against an owned IPP receiver.

This exercises PPD option conversion, typed resolutions and default behavior.
It uses the backend ABI directly; Canopa consent, configured queues, filters and
physical/raster resolution require their separate acceptance checks.
"""
import argparse
import contextlib
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import select
import socket
import subprocess
import tempfile
import threading
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def json_value(value):
    if isinstance(value, datetime.datetime):
        return dict(iso8601=value.isoformat())
    raise TypeError(type(value).__name__)


def run(args):
    backend, receiver, library = [p.resolve(strict=True) for p in [args.backend, args.receiver, args.library]]
    out = args.out.resolve()
    out.mkdir(parents=True)
    (out / 'probe-source.py').write_bytes(Path(__file__).read_bytes())
    config = ROOT / 'scripts/desktop-fixture/print-advanced-ipp.conf'
    ppd = ROOT / 'scripts/desktop-fixture/print-pdf-advanced.ppd'
    source = ROOT / 'scripts/desktop-fixture/print-paper-document.py'
    pins = {str(p): sha(p) for p in [backend, receiver, library, config, ppd, source]}
    result = dict(status='running', scope=__doc__, pins=pins, processes=[], cases=[])
    processes, descriptors, observers = [], [], []

    def save():
        (out / 'summary.json').write_text(json.dumps(result, indent=2, default=json_value) + '\n')

    def start(command, env, name):
        with (out / (name + '.log')).open('wb') as log:
            p = subprocess.Popen(list(map(str, command)), env=env, stdout=log, stderr=log, start_new_session=True)
        processes.append(p)
        row = dict(role=name, pid=p.pid)
        result['processes'].append(row)
        try:
            row['start_ticks'] = int(Path(f'/proc/{p.pid}/stat').read_text().rsplit(')', 1)[1].split()[19])
            descriptors.append((os.pidfd_open(p.pid), row))
        except (FileNotFoundError, ProcessLookupError):
            assert p.poll() is not None
        save()
        return p, row

    def wait(ready, services, timeout=10):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if ready():
                return
            assert all(p.poll() is None for p in services), 'Private service exited'
            time.sleep(.01)
        raise TimeoutError('Private fixture condition not reached')

    def observe(p, row):
        while p.poll() is None:
            try:
                lines = [line for line in Path(f'/proc/{p.pid}/maps').read_text().splitlines()
                         if '/libcups.so' in line]
                if lines:
                    expected = library.stat().st_ino
                    assert all(int(line.split()[4]) == expected for line in lines), lines
                    row['matching_client_mapped'] = True
                    row['cups_paths'] = sorted({line.split(maxsplit=5)[5] for line in lines})
                    return
            except FileNotFoundError:
                return
            except BaseException as error:
                row['mapping_error'] = str(error)
                return
            time.sleep(.001)

    def stop():
        for p in reversed(processes):
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    p.kill(); p.wait()
        for thread in observers:
            thread.join(timeout=2)
            if thread.is_alive():
                result.update(status='failed', cleanup_error='Mapping observer remained alive')

    try:
        with tempfile.TemporaryDirectory(prefix='canopa-ipp-backend-') as directory, contextlib.ExitStack() as cleanup:
            cleanup.callback(stop)
            base = Path(directory)
            result['private_path'] = str(base)
            env = dict(os.environ, HOME=str(base), TMPDIR=str(base), LC_ALL='C',
                       CUPS_SERVER=str(base / 'no-scheduler.sock'), CUPS_SERVERROOT=str(base),
                       CUPS_CACHEDIR=str(base), CONTENT_TYPE='application/pdf',
                       FINAL_CONTENT_TYPE='application/pdf', PRINTER='CanopaBackendFixture',
                       PPD=str(ppd), AUTH_INFO_REQUIRED='none')
            for key in ['LD_LIBRARY_PATH', 'LD_PRELOAD', 'LD_AUDIT', 'CANOPA_PRINT_CUPS_LIBRARY']:
                env.pop(key, None)
            bus = base / 'bus.sock'
            bus_config = base / 'bus.conf'
            bus_config.write_text('<busconfig><type>session</type><listen>unix:path=' + str(bus)
                                  + '</listen><policy context="default"><allow send_destination="*"/>'
                                  '<allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
            discovery, _ = start(['dbus-daemon', '--config-file=' + str(bus_config), '--nofork'], env, 'discovery')
            wait(bus.is_socket, [discovery])
            env.update(DBUS_SYSTEM_BUS_ADDRESS='unix:path=' + str(bus), DBUS_SESSION_BUS_ADDRESS='unix:path=' + str(bus))
            recorder = base / 'record.py'
            recorder.write_text('''#!/usr/bin/python3
import json,os,sys
from pathlib import Path
p=Path(sys.argv[1]);base=Path(__file__).parent
row=dict(job_id=int(os.environ['IPP_JOB_ID']),name=os.environ['IPP_JOB_NAME'],bytes=p.stat().st_size)
with (base/'received.jsonl').open('a') as out:out.write(json.dumps(row)+'\\n')
sys.stdout.buffer.write(p.read_bytes())
''')
            recorder.chmod(0o700)
            for name in ['spool', 'output', 'tls']:
                (base / name).mkdir()
            with socket.socket() as reserve:
                reserve.bind(('127.0.0.1', 0))
                port = reserve.getsockname()[1]
            device = f'ipp://localhost:{port}/ipp/print'
            printer, _ = start([receiver, '-r', 'off', '-n', 'localhost', '-p', port, '-vv',
                                '-F', 'application/pdf', '-a', config, '-d', base / 'spool',
                                '-D', base / 'output', '-K', base / 'tls', '-c', recorder,
                                'Canopa owned backend fixture'], env, 'receiver')

            def listening():
                try:
                    with socket.create_connection(('127.0.0.1', port), timeout=.1):
                        return True
                except OSError:
                    return False

            wait(listening, [discovery, printer])
            spec = importlib.util.spec_from_file_location('paper', source)
            paper = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(paper)
            document = base / 'document.pdf'
            document.write_bytes(paper.document('print', 12513 * 72 / 2540, 18013 * 72 / 2540))
            tests = out / 'receiver-jobs.test'
            tests.write_text('''{
 NAME "actual-backend-jobs"
 OPERATION Get-Jobs
 GROUP operation-attributes-tag
 ATTR charset attributes-charset utf-8
 ATTR naturalLanguage attributes-natural-language en
 ATTR uri printer-uri $uri
 ATTR keyword which-jobs all
 ATTR keyword requested-attributes all
 STATUS successful-ok
}
''')
            choices = [('300dpi', (300, 300, 'dpi')), ('600dpi', (600, 600, 'dpi')),
                       ('600x1200dpi', (600, 1200, 'dpi')), ('118x236dpcm', (118, 236, 'dpcm')),
                       ('default', None)]
            choices = [item for item in choices if not args.case or item[0] in args.case]
            for index, (name, resolution) in enumerate(choices, 1):
                row = dict(name=name, expected_resolution=resolution, status='running')
                result['cases'].append(row)
                options = ('media=custom_canopa_125.13x180.13mm PageSize=Custom.354.69922x510.60474pt '
                           'media-source=manual InputSlot=Manual media-type=labels MediaType=Labels '
                           'print-quality=5 cupsPrintQuality=High sides=two-sided-long-edge')
                if resolution:
                    options += ' printer-resolution=' + name
                title = 'Canopa backend ' + name
                p, identity = start([backend, index, 'canopa-fixture', title, '1', options, document],
                                    dict(env, DEVICE_URI=device), 'backend-' + name)
                row['pid'] = p.pid
                thread = threading.Thread(target=observe, args=(p, identity), daemon=True)
                observers.append(thread)
                thread.start()
                try:
                    row['exit_code'] = p.wait(timeout=30)
                finally:
                    if p.poll() is None:
                        p.terminate()
                        try:
                            p.wait(timeout=3)
                        except subprocess.TimeoutExpired:
                            p.kill(); p.wait()
                    thread.join(timeout=2)
                assert row['exit_code'] == 0 and identity.get('matching_client_mapped') and 'mapping_error' not in identity, row
                report = out / (name + '.plist')
                query, _ = start(['ipptool', '-4', '-T', '3', '-tv', '-P', report, device, tests], env, 'query-' + name)
                assert query.wait(timeout=10) == 0
                data = plistlib.loads(report.read_bytes())
                assert data['Successful'] and data['Tests'][0]['Successful']
                jobs = [group for group in data['Tests'][0]['ResponseAttributes'] if 'job-id' in group]
                assert len(jobs) == index
                job = next(job for job in jobs if job['job-name'].endswith(title))
                row['received_job'] = job
                save()
                expected = None if resolution is None else dict(zip(['xres', 'yres', 'units'], resolution))
                assert job.get('printer-resolution') == expected, (name, expected, job.get('printer-resolution'))
                assert job['job-state'] == 9 and job['print-quality'] == 5
                media = job['media-col']
                assert media['media-size'] == {'x-dimension': 12513, 'y-dimension': 18013}
                assert all(media['media-' + side + '-margin'] == 318 for side in ['left', 'bottom', 'right', 'top'])
                assert media['media-source'] == 'manual' and media['media-type'] == 'labels'
                assert job['sides'] == 'two-sided-long-edge'
                row['status'] = 'passed'
                save()
            result['status'] = 'passed'
    except BaseException:
        result.update(status='failed', error=traceback.format_exc())
    finally:
        stop()
        for fd, row in descriptors:
            row['terminal'] = bool(select.select([fd], [], [], 0)[0])
            os.close(fd)
            if not row['terminal']:
                result.update(status='failed', cleanup_error='A recorded process remained alive')
        result['private_data_removed'] = 'base' not in locals() or not base.exists()
        if not result['private_data_removed'] or any(sha(Path(path)) != digest for path, digest in pins.items()):
            result.update(status='failed', cleanup_error='Private data remained or pinned inputs changed')
        save()
    print(json.dumps(dict(status=result['status'], cases=len(result['cases']), error=result.get('error'))))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', type=Path, required=True)
    parser.add_argument('--receiver', type=Path, required=True)
    parser.add_argument('--library', type=Path, required=True, help='Exact matching client for the selected backend')
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--case', choices=['300dpi', '600dpi', '600x1200dpi', '118x236dpcm', 'default'], action='append')
    raise SystemExit(run(parser.parse_args()))
