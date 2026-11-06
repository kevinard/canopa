#!/usr/bin/env python3
"""Validation-only custom media requests to an owned IPP receiver."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import plistlib
import select
import socket
import subprocess
import tempfile
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / 'scripts/desktop-fixture/print-custom-ipp.conf'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def plist_value(value):
    if isinstance(value, datetime.datetime):
        return dict(plist_type='date', iso8601=value.isoformat())
    if isinstance(value, bytes):
        return dict(plist_type='data', hex=value.hex())
    raise TypeError(type(value).__name__)


def test(name, media, expected='successful-ok', fidelity='true'):
    return f'''{{
 NAME "{name}"
 OPERATION Validate-Job
 GROUP operation-attributes-tag
 ATTR charset attributes-charset utf-8
 ATTR naturalLanguage attributes-natural-language en
 ATTR uri printer-uri $uri
 ATTR mimeMediaType document-format application/pdf
 ATTR boolean ipp-attribute-fidelity {fidelity}
 GROUP job-attributes-tag
 ATTR collection media-col {{ {media} }}
 STATUS {expected}
 {'EXPECT !media-col' if expected == 'successful-ok' else ''}
}}
'''


def size(x, y):
    return f'MEMBER collection media-size {{ MEMBER integer x-dimension {x} MEMBER integer y-dimension {y} }}'


def main(out, binary, library):
    out.mkdir(parents=True)
    result = dict(status='running', scope=__doc__, binary=str(binary), binary_sha256=sha(binary),
                  config_sha256=sha(CONFIG), source_sha256=sha(Path(__file__)), processes=[], prints_submitted=0)
    (out / 'probe-source.py').write_bytes(Path(__file__).read_bytes())
    services = []
    def start(args, env, name):
        with (out / (name + '.log')).open('w') as log:
            process = subprocess.Popen(list(map(str, args)), env=env, stdout=log, stderr=log)
        row = dict(pid=process.pid, role=name)
        row['start_ticks'] = int(Path(f'/proc/{process.pid}/stat').read_text().rsplit(')', 1)[1].split()[19])
        result['processes'].append(row)
        services.append((process, row))
        return process
    def wait(ready):
        until = time.monotonic() + 5
        while time.monotonic() < until:
            if ready():
                return
            assert all(p.poll() is None for p, _row in services)
            time.sleep(.02)
        raise TimeoutError('Private receiver was not ready')
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-receiver-validation-') as directory:
            base = Path(directory)
            try:
                bus = base / 'bus.sock'
                configuration = base / 'bus.conf'
                configuration.write_text('<busconfig><type>session</type><listen>unix:path=' + str(bus) + '</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
                env = dict(os.environ, HOME=str(base), TMPDIR=str(base), DBUS_SYSTEM_BUS_ADDRESS='unix:path=' + str(bus))
                env.pop('LD_LIBRARY_PATH', None)
                start(['dbus-daemon', '--config-file=' + str(configuration), '--nofork'], env, 'private-bus')
                wait(bus.is_socket)
                for name in ['spool', 'tls']:
                    (base / name).mkdir()
                with socket.socket() as reserve:
                    reserve.bind(('127.0.0.1', 0))
                    port = reserve.getsockname()[1]
                printer_env = dict(env)
                if library:
                    result['selected_library_sha256'] = sha(library)
                    printer_env['LD_LIBRARY_PATH'] = str(library.parent)
                printer = start([binary, '-r', 'off', '-n', 'localhost', '-p', str(port),
                                 '-F', 'application/pdf', '-a', str(CONFIG), '-d', base / 'spool',
                                 '-K', base / 'tls', 'Canopa validation-only receiver'], printer_env, 'receiver')
                def listening():
                    try:
                        with socket.create_connection(('127.0.0.1', port), timeout=.1):
                            return True
                    except OSError:
                        return False
                wait(listening)
                listeners = subprocess.check_output(['ss', '-H', '-ltnp', 'sport', '=', ':' + str(port)], text=True)
                assert listeners.strip()
                for line in listeners.splitlines():
                    assert line.split()[3].rsplit(':', 1)[0].strip('[]') in ['127.0.0.1', '::1']
                result['loopback_only'] = True
                result['mapped_cups'] = sorted({line.split()[-1] for line in Path(f'/proc/{printer.pid}/maps').read_text().splitlines() if '/libcups.so' in line})
                if library:
                    assert result['mapped_cups'] == [str(library)]
                cases = [('named-a4-control', 'MEMBER keyword media-size-name iso_a4_210x297mm', 'successful-ok')]
                for name, x, y, valid in [('interior', 12500, 22500, True), ('minimum', 7620, 12700, True),
                                          ('maximum', 21590, 35560, True), ('fractional', 12501, 22502, True),
                                          ('outside-x', 7619, 22500, False), ('outside-y', 12500, 12699, False),
                                          ('outside-both', 7619, 12699, False)]:
                    cases.append((name, size(x, y), 'successful-ok' if valid else 'client-error-attributes-or-values-not-supported'))
                document = ''.join(test(*case) for case in cases)
                document += test('interior-without-fidelity', size(12500, 22500), fidelity='false')
                document += '''{
 NAME "strict-printer-capabilities"
 OPERATION Get-Printer-Attributes
 GROUP operation-attributes-tag
 ATTR charset attributes-charset utf-8
 ATTR naturalLanguage attributes-natural-language en
 ATTR uri printer-uri $uri
 ATTR keyword requested-attributes all,media-col-database
 STATUS successful-ok
}
'''
                document += '''{
 NAME "no-jobs-created"
 OPERATION Get-Jobs
 GROUP operation-attributes-tag
 ATTR charset attributes-charset utf-8
 ATTR naturalLanguage attributes-natural-language en
 ATTR uri printer-uri $uri
 ATTR keyword which-jobs all
 STATUS successful-ok
 EXPECT !job-id
}
'''
                tests = out / 'validation.test'
                tests.write_text(document)
                command = ['ipptool', '-I', '-4', '-T', '3', '-tv', '-P', str(out / 'validation.plist'),
                           f'ipp://localhost:{port}/ipp/print', str(tests)]
                run = subprocess.run(command, env=env, capture_output=True, text=True, timeout=20)
                (out / 'validation.out').write_text(run.stdout)
                (out / 'validation.err').write_text(run.stderr)
                results = plistlib.loads((out / 'validation.plist').read_bytes())
                capabilities = next((test for test in results['Tests'] if test['Name'] == 'strict-printer-capabilities'), {})
                formats = [group for group in capabilities.get('ResponseAttributes', []) if 'document-format-default' in group]
                declared_formats = len(formats) == 1 and formats[0]['document-format-default'] == 'application/pdf' and formats[0].get('document-format-supported') == 'application/pdf'
                result.update(status='passed' if results['Successful'] and run.returncode == 0 and len(results['Tests']) == 11 and declared_formats else 'receiver-validation-mismatch',
                              exit_code=run.returncode, planned_case_count=11,
                              test_count=len(results['Tests']), declared_pdf_formats_preserved=declared_formats, results=results)
            finally:
                for process, row in reversed(services):
                    if process.poll() is None:
                        process.terminate()
                        try:
                            process.wait(timeout=3)
                        except subprocess.TimeoutExpired:
                            process.kill(); process.wait()
                    row.update(exit_code=process.returncode, terminal=True)
        result['private_data_removed'] = True
    except BaseException:
        result.update(status='failed', error=traceback.format_exc())
    (out / 'summary.json').write_text(json.dumps(result, indent=2, default=plist_value) + '\n')
    print(json.dumps({k: result.get(k) for k in ['status', 'test_count', 'exit_code', 'error', 'private_data_removed']}))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--library', type=Path)
    args = parser.parse_args()
    raise SystemExit(main(args.out.resolve(), args.binary.resolve(), args.library.resolve() if args.library else None))
