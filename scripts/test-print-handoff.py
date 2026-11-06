#!/usr/bin/env python3
"""Check actual Print setup completion with a retained CLOEXEC fork copy."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import resource
import select
import signal
import subprocess
import tempfile
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]
WRAPPER = ROOT / 'scripts/desktop-fixture/print-handoff-inherit.py'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--harness', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--copy', choices=['held', 'closed'], default='held')
    parser.add_argument('--expect-would-block', action='store_true', help='Record an old-worker reproducer')
    args = parser.parse_args()
    args.binary, args.harness = args.binary.resolve(strict=True), args.harness.resolve(strict=True)
    args.artifacts = args.artifacts.resolve()
    args.artifacts.mkdir(parents=True, exist_ok=False)
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    # PR_SET_CHILD_SUBREAPER affects only this controller and its descendants.
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(36, 1, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), 'Cannot adopt owned fixture descendants')
    result = dict(status='running', copy=args.copy, expect_would_block=args.expect_would_block,
                  pins={str(path): digest(path) for path in [Path(__file__), WRAPPER, args.binary, args.harness]},
                  scope='Existing native file-client harness and actual normal file workers, with only a controlled CLOEXEC fork copy of the setup stream. No new executable, scheduler, Iced, printer or user-file acceptance.',
                  processes=[])
    owned, parent, private = {}, None, None

    def remember(pid, started, role):
        key = (pid, started)
        if key in owned:
            return
        try:
            state = Path(f'/proc/{pid}/stat').read_text()
            descriptor = os.pidfd_open(pid) if state[state.rfind(')') + 2:].split()[19] == started else None
        except OSError:
            descriptor = None
        row = dict(pid=pid, start_time=started, role=role)
        result['processes'].append(row)
        owned[key] = descriptor, row

    def observe():
        for path in folder.glob('*.json'):
            try:
                row = json.loads(path.read_text())
            except (OSError, ValueError):
                continue
            if 'pid' in row:
                remember(row['pid'], row['start_time'], row['role'])

    try:
        private = tempfile.TemporaryDirectory(prefix='canopa-print-handoff-')
        folder = Path(private.name)
        if args.copy == 'closed':
            (folder / 'release').write_text('release\n')
        env = dict(os.environ, CANOPA_HANDOFF_ROOT=str(folder), CANOPA_HANDOFF_COPY=args.copy,
                   CANOPA_HANDOFF_REAL=str(args.binary), CANOPA_PRINT_FILE_BINARY=str(WRAPPER),
                   CANOPA_PRINT_FILE_OUTPUT=str(args.artifacts / 'client-checks.json'),
                   HOME=str(folder), TMPDIR=str(folder), CUPS_SERVER=str(folder / 'unused-cups.sock'),
                   CARGO_MANIFEST_DIR=str(ROOT / 'crates/canopa-portal'), PYTHONDONTWRITEBYTECODE='1')
        for name in ['LD_LIBRARY_PATH', 'CANOPA_PRINT_CUPS_LIBRARY']:
            env.pop(name, None)
        with (args.artifacts / 'parent.out').open('w') as out, (args.artifacts / 'parent.err').open('w') as err:
            parent = subprocess.Popen([str(args.harness), '--ignored', '--exact',
                'print::worker::client::publication_tests::native_file_client_fixture', '--nocapture'],
                cwd=ROOT / 'crates/canopa-portal', env=env, stdout=out, stderr=err, start_new_session=True)
            state = Path(f'/proc/{parent.pid}/stat').read_text()
            remember(parent.pid, state[state.rfind(')') + 2:].split()[19], 'existing native harness')
            started = time.monotonic()
            while parent.poll() is None and time.monotonic() - started < 60:
                observe()
                time.sleep(.02)
            assert parent.poll() is not None, 'native client fixture timed out'
            result.update(exit_code=parent.wait(), seconds=time.monotonic() - started)
        observe()
        holders = [json.loads(path.read_text()) for path in folder.glob('*-holder.json')]
        assert holders and all(row['cloexec'] for row in holders)
        result['holders'] = holders
        live = [row['pid'] for row in holders
                if (descriptor := owned[(row['pid'], row['start_time'])][0]) is not None
                and not select.select([descriptor], [], [], 0)[0]]
        result['live_holders_before_release'] = live
        if args.copy == 'held':
            assert live, 'the copy must remain live until the native client finishes'
        errors = (args.artifacts / 'parent.err').read_text()
        if args.expect_would_block:
            assert result['exit_code'] == 101 and 'Print staging handoff failed' in errors and 'Resource temporarily unavailable' in errors, errors
            result['status'] = 'old-worker-WouldBlock-reproduced'
        else:
            assert result['exit_code'] == 0, errors
            checks = json.loads((args.artifacts / 'client-checks.json').read_text())
            assert checks['status'] == 'passed'
            result.update(status='passed', client_checks=checks['checks'])
    except BaseException:
        result.update(status='failed', error=traceback.format_exc())
    finally:
        if private:
            (folder / 'release').write_text('release\n')
        if parent and parent.poll() is None:
            os.killpg(parent.pid, signal.SIGKILL)
            parent.wait(timeout=10)
        if private:
            observe()
        for descriptor, row in owned.values():
            if descriptor is not None:
                row['terminal'] = bool(select.select([descriptor], [], [], 5)[0])
                os.close(descriptor)
            else:
                try:
                    state = Path(f"/proc/{row['pid']}/stat").read_text()
                    row['terminal'] = state[state.rfind(')') + 2:].split()[19] != row['start_time']
                except OSError:
                    row['terminal'] = True
            if not row['terminal']:
                result['status'] = 'failed'
            try:
                os.waitpid(row['pid'], os.WNOHANG)
            except ChildProcessError:
                pass
        if private:
            private.cleanup()
            result['private_data_removed'] = True
        result['changed_inputs'] = [name for name, expected in result['pins'].items()
                                    if digest(Path(name)) != expected]
        if result['changed_inputs']:
            result['status'] = 'failed'
        (args.artifacts / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(status=result['status'], receipt=str(args.artifacts / 'summary.json'))))
    return 0 if result['status'] in ['passed', 'old-worker-WouldBlock-reproduced'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
