#!/usr/bin/env python3
"""A live worker thread must keep its printer journal busy after leader exit."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import subprocess
import tempfile
import time
import traceback


def identity(pid):
    stat = Path(f'/proc/{pid}/stat').read_text()
    return dict(pid=pid, start=int(stat[stat.rfind(')') + 2:].split()[19]), boot=Path('/proc/sys/kernel/random/boot_id').read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--receipt', required=True, type=Path)
    args = parser.parse_args()
    result = dict(status='running', scope='actual printer recovery worker must not claim a journal whose thread group is still live; no installed CUPS or queues contacted')
    with args.binary.open('rb') as stream:
        result['binary_sha256'] = hashlib.file_digest(stream, 'sha256').hexdigest()
    child = None
    pidfd = None
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-printer-lifetime-') as directory:
            base = Path(directory)
            fixture = Path(__file__).parent / 'desktop-fixture/thread-group-lifetime.py'
            child = subprocess.Popen(['python3', str(fixture)], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
            pidfd = os.pidfd_open(child.pid)
            assert child.stdout.readline() == b'ready\n'
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                stat = Path(f'/proc/{child.pid}/stat').read_text()
                state = stat[stat.rfind(')') + 2:].split()[0]
                if state == 'Z':
                    break
                time.sleep(.01)
            assert state == 'Z' and not select.select([pidfd], [], [], 0)[0]
            key = 'a' * 32
            entry = dict(receipt=dict(intent=dict(key=key, server=dict(name=list(b'/intent/unused-cups.sock'), port=631, encryption=0, user=list(b'fixture')), destination=dict(queue='Fixture', instance=None), name='[Canopa:' + key + '] live worker'), job=None, accepted=False), owner=identity(os.getpid()), worker=identity(child.pid))
            state_path = base / 'state'
            state_path.mkdir(mode=0o700)
            catalog = state_path / 'printer-jobs.json'
            catalog.write_text(json.dumps(dict(entries=[entry])))
            catalog.chmod(0o600)
            reader, writer = os.pipe2(os.O_CLOEXEC)
            try:
                packet = dict(cancel=reader, operation=dict(operation='recover_printer_jobs', store=dict(path=list(os.fsencode(state_path)))))
                # The mismatched, nonexistent server is a guard: even a broken
                # busy check cannot connect to the user's installed scheduler.
                env = dict(os.environ, CUPS_SERVER=str(base / 'different-unused.sock'), HOME=str(base))
                helper = subprocess.Popen([str(args.binary.resolve()), '--print-worker'], env=env, pass_fds=(reader,), stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                try:
                    output, errors = helper.communicate((json.dumps(packet) + '\n').encode(), timeout=20)
                except subprocess.TimeoutExpired:
                    helper.kill();helper.communicate();raise
                frames = [json.loads(line) for line in output.splitlines()]
                result.update(leader_pid=child.pid, leader_state=state, pidfd_terminal_while_thread_alive=bool(select.select([pidfd], [], [], 0)[0]), helper_exit=helper.returncode, frames=frames, stderr=errors.decode('utf-8', 'replace'))
                assert helper.returncode == 0 and frames == [dict(event='retired_printer_jobs', report=dict(retired=0, unresolved=0, busy=1))], result
                assert json.loads(catalog.read_text())['entries'] == [entry], 'A live worker record was rewritten'
            finally:
                os.close(reader);os.close(writer)
            result['status'] = 'passed'
    except BaseException:
        result.update(status='failed', error=traceback.format_exc())
    finally:
        if child:
            if child.stdin and not child.stdin.closed:
                child.stdin.close()
            try:
                result['owned_worker_exit'] = child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill();result['owned_worker_exit'] = child.wait();result['cleanup_kill'] = True;result['status'] = 'failed'
        if pidfd is not None:
            result['pidfd_terminal_after_cleanup'] = bool(select.select([pidfd], [], [], 0)[0]);os.close(pidfd)
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
