#!/usr/bin/env python3
"""Test actual worker acknowledgements against an explicitly owned CUPS fixture.

The caller supplies a private Unix scheduler socket and fixture queue. This test
creates three jobs and retires only their recorded IDs. It does not start or
configure an installed scheduler. The public/native matrix separately verifies
Iced consent, actual received bytes, service death and restart.
"""
import argparse
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import traceback

sys.dont_write_bytecode = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--scheduler', required=True, type=Path)
    parser.add_argument('--queue', required=True)
    parser.add_argument('--document', required=True, type=Path)
    parser.add_argument('--receipt', required=True, type=Path)
    args = parser.parse_args()
    assert args.scheduler.is_socket(), 'The fixture requires a private Unix scheduler socket'
    module_path = Path(__file__).parent / 'desktop-fixture/print-cups-proxy.py'
    spec = importlib.util.spec_from_file_location('cups_relay', module_path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    result = dict(status='running', scope='actual isolated worker acknowledgement with real private CUPS replies discarded after receipt; no public UI or physical-printer claim', cases=[])
    env = dict(os.environ)
    payload = args.document.read_bytes()
    result['input_sha256'] = hashlib.sha256(payload).hexdigest()
    result['binary'] = str(args.binary.resolve())
    with args.binary.open('rb') as stream:
        result['binary_sha256'] = hashlib.file_digest(stream, 'sha256').hexdigest()
    document = os.memfd_create('canopa-cups-reply-regression', os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING)
    read_cancel, write_cancel = os.pipe2(os.O_CLOEXEC)
    owned_jobs = set()
    relay = None

    def worker(operation):
        packet = dict(cancel=read_cancel, operation=operation)
        descriptors = (document, read_cancel) if operation['operation'] == 'submit' else (read_cancel,)
        process = subprocess.Popen([str(args.binary.resolve()), '--print-worker'], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, pass_fds=descriptors)
        try:
            output, errors = process.communicate((json.dumps(packet) + '\n').encode(), timeout=25)
        except subprocess.TimeoutExpired:
            process.kill()
            process.communicate()
            raise
        return dict(pid=process.pid, exit_code=process.returncode, frames=[json.loads(line) for line in output.splitlines()], stderr=errors.decode('utf-8', 'replace'))

    try:
        os.write(document, payload)
        fcntl.fcntl(document, fcntl.F_ADD_SEALS, fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_GROW | fcntl.F_SEAL_WRITE | fcntl.F_SEAL_SEAL)
        with tempfile.TemporaryDirectory(prefix='canopa-cups-replies-') as directory:
            relay = module.Relay(Path(directory) / 'proxy.sock', args.scheduler.resolve())
            env['CUPS_SERVER'] = str(Path(directory) / 'proxy.sock')
            try:
                for name, operation in [('forwarded', None), ('lost-create', 5), ('lost-document', 6)]:
                    start = len(relay.records)
                    fault = relay.arm(name, operation) if operation else None
                    response = worker(dict(operation='submit', document=document, destination=dict(queue=args.queue, instance=None), title='Canopa reply regression ' + name, options=dict(copies=1, media=None, sides=None, color=None)))
                    records = relay.records[start:]
                    creations = [r for r in records if r['operation'] == 5]
                    for record in creations:
                        job = record['response'].get('job-id')
                        if job is not None and job > 0:
                            owned_jobs.add(job)
                    result['cases'].append(dict(name=name, worker=response, relay=records))
                    assert len(creations) == 1, 'A submission must not automatically create a replacement'
                    assert creations[0]['response']['job-id'] > 0
                    frames = response['frames']
                    if fault:
                        assert fault['entered'].is_set()
                        assert fault['record']['http_status'] == 200 and fault['record']['ipp_status'] < 0x100
                        assert not fault['record']['forwarded']
                        # The helper reports an operation failure in its frame;
                        # successful protocol delivery itself exits normally.
                        assert response['exit_code'] == 0 and frames[-1]['event'] == 'failed', response
                        assert not any(f['event'] == 'accepted' for f in frames), response
                        if operation == 5:
                            assert not any(f['event'] == 'created' for f in frames)
                            assert not any(r['operation'] == 6 for r in records)
                        else:
                            assert frames[0] == dict(event='created', job=creations[0]['response']['job-id'])
                    else:
                        assert response['exit_code'] == 0
                        assert [f['event'] for f in frames] == ['created', 'accepted']
                        assert frames[0]['job'] == frames[1]['job'] == creations[0]['response']['job-id']
                    assert all(r['request'].get('job-id') == creations[0]['response']['job-id'] for r in records if r['operation'] == 8)
            finally:
                result['cleanup'] = []
                for job in sorted(owned_jobs):
                    cleanup = worker(dict(operation='cancel_job', destination=dict(queue=args.queue, instance=None), job=job))
                    result['cleanup'].append(dict(job=job, worker=cleanup))
                    assert cleanup['exit_code'] == 0 and cleanup['frames'] == [dict(event='retired_job', job=job)], cleanup
                result['relay_errors'] = relay.errors
                relay.close()
                relay = None
            assert not result['relay_errors'], result['relay_errors']
        result['status'] = 'passed'
    except BaseException:
        result.update(status='failed', error=traceback.format_exc())
    finally:
        if relay:
            relay.close()
        os.close(document)
        os.close(read_cancel)
        os.close(write_cancel)
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(status=result['status'], cases=len(result['cases']), receipt=str(args.receipt), error=result.get('error'))))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
