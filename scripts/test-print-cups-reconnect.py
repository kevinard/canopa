#!/usr/bin/env python3
"""Exercise recovery across reconnects between two explicitly owned schedulers.

Supply two private Unix sockets with the same disposable queue and matching job-ID
sequences. Only jobs created here are cleaned up. No document is uploaded, and no
installed scheduler, physical printer or public consent path is involved.
"""
import argparse
import hashlib
import http.client
import importlib.util
import json
import os
from pathlib import Path
import pwd
import select
import socket
import subprocess
import sys
import tempfile
import traceback

sys.dont_write_bytecode = True


class UnixHTTP(http.client.HTTPConnection):
    def __init__(self, path):
        super().__init__('localhost', timeout=5)
        self.path = path

    def connect(self):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(5)
        self.sock.connect(str(self.path))


def attribute(tag, name, value):
    name = name.encode()
    value = value if isinstance(value, bytes) else value.encode()
    return bytes([tag]) + len(name).to_bytes(2, 'big') + name + len(value).to_bytes(2, 'big') + value


def identity(pid):
    stat = Path(f'/proc/{pid}/stat').read_text()
    return dict(pid=pid, start=int(stat[stat.rfind(')') + 2:].split()[19]), boot=Path('/proc/sys/kernel/random/boot_id').read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--scheduler', required=True, type=Path)
    parser.add_argument('--replacement', required=True, type=Path)
    parser.add_argument('--queue', required=True)
    parser.add_argument('--receipt', required=True, type=Path)
    args = parser.parse_args()
    assert args.scheduler.is_socket() and args.replacement.is_socket()
    assert args.scheduler.resolve() != args.replacement.resolve()
    module_path = Path(__file__).parent / 'desktop-fixture/print-cups-proxy.py'
    spec = importlib.util.spec_from_file_location('cups_relay', module_path)
    proxy = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(proxy)
    user = pwd.getpwuid(os.getuid()).pw_name
    result = dict(status='running', cases=[], cleanup=[], processes=[], scope=__doc__)
    with args.binary.open('rb') as stream:
        result['binary_sha256'] = hashlib.file_digest(stream, 'sha256').hexdigest()
    owned = []

    def ipp(server, operation, values):
        body = b'\x01\x01' + operation.to_bytes(2, 'big') + b'\x00\x00\x00\x01\x01'
        body += attribute(0x47, 'attributes-charset', 'utf-8')
        body += attribute(0x48, 'attributes-natural-language', 'en')
        body += attribute(0x42, 'requesting-user-name', user)
        for tag, name, value in values:
            body += attribute(tag, name, value)
        connection = UnixHTTP(server)
        try:
            connection.request('POST', '/', body + b'\x03', {'Content-Type': 'application/ipp'})
            response = connection.getresponse()
            data = response.read(128 * 1024 + 1)
            assert response.status == 200 and 8 <= len(data) <= 128 * 1024
            assert int.from_bytes(data[2:4], 'big') < 0x100, data[:200]
            return proxy.attributes(data)
        finally:
            connection.close()

    def state(server, job):
        return ipp(server, 9, [(0x45, 'job-uri', f'ipp://localhost/jobs/{job}')])['job-state']

    def create(server, title):
        job = ipp(server, 5, [(0x45, 'printer-uri', f'ipp://localhost/printers/{args.queue}'), (0x42, 'job-name', title)])['job-id']
        owned.append((server, job))
        assert state(server, job) == 4
        return job

    try:
        with tempfile.TemporaryDirectory(prefix='canopa-cups-reconnect-') as directory:
            base = Path(directory)
            for mode in ['stable', 'benign-reconnect', 'replacement', 'repeated-reconnect', 'multiple-records']:
                folder = base / mode
                folder.mkdir()
                store = folder / 'recovery'
                store.mkdir(mode=0o700)
                switches = []
                queries = 0
                threshold = 2

                def hook(record, header, fields):
                    nonlocal queries, threshold
                    if record['operation'] == 10:
                        queries += 1
                    reconnect = mode in ['benign-reconnect', 'replacement', 'repeated-reconnect']
                    if reconnect and queries >= threshold and record['operation'] == 11 and (not switches or mode == 'repeated-reconnect'):
                        target = args.replacement if mode == 'replacement' else args.scheduler
                        relay.upstream = str(target)
                        lines = [line for line in header.split(b'\r\n') if line and not line.lower().startswith(b'connection:')]
                        header = b'\r\n'.join(lines) + b'\r\nConnection: close\r\n\r\n'
                        fields = dict(fields)
                        fields[b'connection'] = b'close'
                        change = dict(after_operation=11, after_owned_queries=queries, target=str(target))
                        switches.append(change)
                        record['namespace_switch'] = change
                        threshold += 2
                    return header, fields

                relay = proxy.Relay(folder / 'proxy.sock', args.scheduler, response_hook=hook)
                try:
                    entries = []
                    jobs = []
                    for _ in range(2 if mode == 'multiple-records' else 1):
                        key = os.urandom(16).hex()
                        title = '[Canopa:' + key + '] owned incomplete job'
                        first = create(args.scheduler, title)
                        second = create(args.replacement, 'Unrelated scheduler job ' + mode)
                        assert first == second, 'Use fresh owned schedulers with matching job-ID sequences'
                        jobs.append(first)
                        owner = identity(os.getpid())
                        dead = dict(owner, boot=owner['boot'] + 'old')
                        intent = dict(key=key, server=dict(name=list(os.fsencode(folder / 'proxy.sock')), port=631, encryption=0, user=list(user.encode())), destination=dict(queue=args.queue, instance=None), name=title)
                        entries.append(dict(receipt=dict(intent=intent, job=first, accepted=False), owner=owner, worker=dead))
                    catalog = store / 'printer-jobs.json'
                    catalog.write_text(json.dumps(dict(entries=entries)))
                    catalog.chmod(0o600)
                    read_cancel, write_cancel = os.pipe2(os.O_CLOEXEC)
                    try:
                        packet = dict(cancel=read_cancel, operation=dict(operation='recover_printer_jobs', store=dict(path=list(os.fsencode(store)))))
                        env = dict(os.environ, HOME=str(base), CUPS_USER=user, CUPS_SERVER=str(folder / 'proxy.sock'))
                        process = subprocess.Popen([str(args.binary.resolve()), '--print-worker'], env=env, pass_fds=(read_cancel,), stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                        pidfd = os.pidfd_open(process.pid)
                        try:
                            try:
                                output, errors = process.communicate((json.dumps(packet) + '\n').encode(), timeout=30)
                            except subprocess.TimeoutExpired:
                                process.kill()
                                process.communicate()
                                result['cleanup_kill'] = True
                                raise
                            result['processes'].append(dict(pid=process.pid, exit_code=process.returncode, terminal=bool(select.select([pidfd], [], [], 0)[0])))
                        finally:
                            os.close(pidfd)
                    finally:
                        os.close(read_cancel)
                        os.close(write_cancel)
                    frames = [json.loads(line) for line in output.splitlines()]
                    states = [dict(job=job, original=state(args.scheduler, job), unrelated=state(args.replacement, job)) for job in jobs]
                    journal = json.loads(catalog.read_text())
                    record = dict(name=mode, frames=frames, stderr=errors.decode('utf-8', 'replace'), states=states, switches=switches, relay=relay.records, relay_errors=relay.errors, journal=journal)
                    result['cases'].append(record)
                    uncertain = mode in ['replacement', 'repeated-reconnect']
                    report = dict(retired=0 if uncertain else len(jobs), unresolved=len(jobs) if uncertain else 0, busy=0)
                    assert process.returncode == 0 and frames == [dict(event='retired_printer_jobs', report=report)], record
                    assert all(s['unrelated'] == 4 and s['original'] == (4 if uncertain else 7) for s in states), record
                    assert len(journal['entries']) == (len(jobs) if uncertain else 0), record
                    cancels = [r for r in relay.records if r['operation'] == 8]
                    assert len(cancels) == (0 if uncertain else len(jobs)), record
                    expected_switches = 3 if mode == 'repeated-reconnect' else int(mode in ['benign-reconnect', 'replacement'])
                    assert len(switches) == expected_switches and not relay.errors, record
                    record['passed'] = True
                    print(json.dumps(dict(case=mode, passed=True, report=report)), flush=True)
                finally:
                    relay.close()
        result['status'] = 'passed'
    except BaseException:
        result.update(status='failed', error=traceback.format_exc())
    finally:
        for server, job in owned:
            try:
                before = state(server, job)
                if before not in [7, 8, 9]:
                    ipp(server, 8, [(0x45, 'job-uri', f'ipp://localhost/jobs/{job}')])
                result['cleanup'].append(dict(server=str(server), job=job, before=before, after=state(server, job)))
            except BaseException:
                result.setdefault('cleanup_errors', []).append(traceback.format_exc())
                result['status'] = 'failed'
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(status=result['status'], receipt=str(args.receipt), error=result.get('error'))))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
