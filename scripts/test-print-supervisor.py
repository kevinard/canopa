#!/usr/bin/env python3
"""Measure real Client/Broker deadlines and parent FD retirement with controlled children."""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import tempfile
import threading
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / 'scripts/desktop-fixture/print-supervisor-helper.py'
SOURCE = ROOT / 'scripts/desktop-fixture/print-supervisor-client.rs'
STOP = threading.Event()


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def write(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


class Case:
    def __init__(self, args, name, authenticated, cooperative=True):
        self.args, self.name = args, name
        self.authenticated, self.cooperative = authenticated, cooperative
        self.output = args.artifacts / name
        self.output.mkdir()
        self.result = dict(status='running', name=name, rounds=[], processes=[])
        self.observed = {}
        self.parent = None
        self.private = None

    def save(self):
        write(self.output / 'summary.json', self.result)

    def observe(self):
        for path in self.folder.glob('*.audit.jsonl'):
            for line in path.read_text().splitlines():
                try:
                    row = json.loads(line)
                except ValueError:
                    continue
                key = (row['pid'], row['start_time'])
                if key not in self.observed:
                    self.remember(row['pid'], row['start_time'], row['role'])

    def remember(self, pid, started, role):
        key = (pid, started)
        try:
            state = Path(f'/proc/{pid}/stat').read_text()
            same = state[state.rfind(')') + 2:].split()[19] == started
            descriptor = os.pidfd_open(pid) if same else None
        except (OSError, IndexError):
            descriptor = None
        row = dict(pid=pid, start_time=started, role=role)
        self.result['processes'].append(row)
        self.observed[key] = (descriptor, row)

    def read(self, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.observe()
            if STOP.is_set():
                raise RuntimeError('Validation stopped by storage reserve')
            if select.select([self.parent.stdout], [], [], .05)[0]:
                line = self.parent.stdout.readline()
                if not line:
                    raise RuntimeError('Native parent exited before its result')
                return json.loads(line)
        raise TimeoutError('Native parent result observation expired')

    def command(self, identifier, behavior, authenticated, interruption='none', timeout=20):
        control = dict(id=identifier, behavior=behavior, authenticated=authenticated,
                       library_sha256=self.args.expected_library_sha256)
        write(self.folder / 'control.json', control)
        self.parent.stdin.write(json.dumps(dict(id=identifier, action='run',
                                     authenticated=authenticated, interruption=interruption)) + '\n')
        self.parent.stdin.flush()
        reply = self.read(timeout)
        assert reply['event'] == 'result' and reply['id'] == identifier, reply
        assert reply['children'] == [] and reply['fds'] == reply['baseline_fds'], reply
        assert reply['image_descriptors'] == 1 and not reply['parent_cups_mapped'], reply
        self.observe()
        audit = [json.loads(line) for line in (self.folder / f'{identifier}.audit.jsonl').read_text().splitlines()]
        reply['audit'] = audit
        self.result['rounds'].append(reply)
        self.save()
        return reply

    def run(self):
        try:
            self.private = tempfile.TemporaryDirectory(prefix='canopa-print-supervisor-')
            self.folder = Path(self.private.name)
            env = dict(os.environ, CANOPA_PRINT_CUPS_LIBRARY=str(self.args.library),
                       CUPS_SERVER=str(self.folder / 'unused-cups.sock'), HOME=str(self.folder),
                       PYTHONDONTWRITEBYTECODE='1')
            env.pop('LD_LIBRARY_PATH', None)
            with (self.output / 'parent.err').open('w') as errors:
                self.parent = subprocess.Popen([str(self.args.parent), str(HELPER), str(self.folder)],
                                              cwd=ROOT, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                              stderr=errors, text=True, start_new_session=True)
            state = Path(f'/proc/{self.parent.pid}/stat').read_text()
            self.remember(self.parent.pid, state[state.rfind(')') + 2:].split()[19], 'real Rust Client/Broker parent')
            ready = self.read(10)
            assert ready['event'] == 'ready' and ready['image_descriptors'] == 1
            warm = self.command('warmup', 'success', True)
            assert warm['kind'] == 'success', warm
            if self.args.mode == 'deadlines':
                duration = 300 if self.authenticated else 60
                behavior = 'deadline-cooperative' if self.cooperative else 'deadline-uncooperative'
                reply = self.command('deadline', behavior, self.authenticated, timeout=duration + 40)
                assert reply['kind'] == 'error' and 'Print worker deadline exceeded' in reply['error'], reply
                cancellation = [row for row in reply['audit'] if row['role'] == 'cancel-eof']
                assert len(cancellation) == 1, cancellation
                assert duration - 2 <= cancellation[0]['seconds'] <= duration + 5, cancellation
                expected = duration if self.cooperative else duration + 10
                assert expected - 2 <= reply['seconds'] <= expected + 8, reply
                assert not any(row['role'] == 'credential-helper' for row in reply['audit'])
                self.result['deadline'] = dict(seconds=duration, cooperative=self.cooperative,
                                               cancel_eof_seconds=cancellation[0]['seconds'],
                                               reply_seconds=reply['seconds'])
            else:
                cycles = 1 if self.args.mode == 'controls' else 8
                for index in range(cycles):
                    for behavior, interruption, expected in [
                        ('success', 'none', 'success'),
                        ('prompt-stall', 'cancel', 'cancelled'),
                        ('prompt-stall', 'drop', 'dropped'),
                        ('invalid-reply', 'none', 'error'),
                        ('helper-crash', 'none', 'error'),
                    ]:
                        reply = self.command(f'round-{index}-{behavior}-{interruption}', behavior, True, interruption)
                        assert reply['kind'] == expected, reply
                        if interruption != 'none':
                            assert reply['interrupted'], reply
                        if behavior == 'success':
                            assert sum(row['role'] == 'credential-helper' for row in reply['audit']) == 1
                            assert sum(row['role'].startswith('credential-exchange-') for row in reply['audit']) == 2
                ordinary = self.command('ordinary-after-authentication', 'success', False)
                assert ordinary['kind'] == 'success'
            self.parent.stdin.write('{"action":"stop"}\n')
            self.parent.stdin.flush()
            stopped = self.read(20)
            assert stopped['event'] == 'stopped' and not stopped['children'], stopped
            assert self.parent.wait(timeout=10) == 0
            self.result['status'] = 'passed'
        except BaseException:
            self.result.update(status='failed', error=traceback.format_exc())
        finally:
            if self.parent and self.parent.poll() is None:
                os.killpg(self.parent.pid, signal.SIGKILL)
                self.parent.wait(timeout=10)
                self.result['forced_parent_cleanup'] = True
            if self.private:
                self.observe()
            for descriptor, row in self.observed.values():
                if descriptor is not None:
                    row['terminal'] = bool(select.select([descriptor], [], [], 2)[0])
                    row['terminal_proof'] = 'pidfd'
                    os.close(descriptor)
                else:
                    try:
                        state = Path(f"/proc/{row['pid']}/stat").read_text()
                        row['terminal'] = state[state.rfind(')') + 2:].split()[19] != row['start_time']
                    except OSError:
                        row['terminal'] = True
                    row['terminal_proof'] = 'recorded identity no longer present'
                if not row['terminal']:
                    self.result['status'] = 'failed'
            if self.private:
                self.private.cleanup()
                self.result['private_data_removed'] = True
            self.save()
        return dict(name=self.name, status=self.result['status'], receipt=str(self.output / 'summary.json'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--parent-build', type=Path, required=True)
    parser.add_argument('--library', type=Path, required=True)
    parser.add_argument('--expected-library-sha256', required=True)
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--mode', choices=['controls', 'resources', 'deadlines'], required=True)
    args = parser.parse_args()
    build = json.loads(args.parent_build.read_text())
    assert build['status'] == 'passed' and build['exit_code'] == 0 and not build['changed_inputs']
    args.parent = Path(build['parent']['path'])
    args.library = args.library.resolve(strict=True)
    assert sha(args.parent) == build['parent']['sha256']
    assert sha(SOURCE) == build['source_sha256']
    assert all(sha(Path(record['path'])) == record['sha256'] for record in build['externs'].values())
    assert sha(args.library) == args.expected_library_sha256
    args.artifacts = args.artifacts.resolve()
    args.artifacts.mkdir(parents=True, exist_ok=False)
    pins = {str(path): sha(path) for path in [HELPER, SOURCE, Path(__file__), args.parent, args.library]}
    result = dict(status='running', mode=args.mode, pins=pins, cases=[],
                  scope='Actual retained Rust Client/Broker/Dialog and supervisor with controlled child/credential protocols. Image bytes/seals and parent FD retirement are checked; no CUPS library loading, scheduler, Iced, job mutation or physical acceptance.')
    path = args.artifacts / 'summary.json'
    write(path, result)
    cases = [Case(args, f'{kind}-{style}', kind == 'authenticated', style == 'cooperative')
             for kind in ['ordinary', 'authenticated'] for style in ['cooperative', 'uncooperative']]
    if args.mode != 'deadlines':
        cases = [Case(args, args.mode, True)]
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(cases)) as pool:
        pending = {pool.submit(case.run) for case in cases}
        while pending:
            done, pending = concurrent.futures.wait(pending, timeout=10,
                                                    return_when=concurrent.futures.FIRST_COMPLETED)
            for future in done:
                result['cases'].append(future.result())
            if shutil.disk_usage(ROOT).free < 5_000_000_000:
                STOP.set()
            write(path, result)
    result['changed_inputs'] = [name for name, expected in pins.items() if sha(Path(name)) != expected]
    result['status'] = 'passed' if all(row['status'] == 'passed' for row in result['cases']) and not result['changed_inputs'] else 'failed'
    write(path, result)
    print(json.dumps(dict(status=result['status'], mode=args.mode, cases=result['cases'], receipt=str(path))))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
