#!/usr/bin/env python3
"""Repeat the compiled configuration suite with bounded, optional CPU contention.

Build first: bash scripts/test-notify.sh --test configuration_intake --no-run
Pass its executable path from Cargo's output. Each run uses a fresh private bus.
"""

import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


def pressure():
    return {kind: Path(f'/proc/pressure/{kind}').read_text()
            for kind in ['cpu', 'memory', 'io']}


def terminate(_signal, _frame):
    # Supervisor cancellation must release workers and the detached test group.
    raise KeyboardInterrupt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--test-binary', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=5)
    parser.add_argument('--cpu-workers', type=int, default=0)
    args = parser.parse_args()
    signal.signal(signal.SIGTERM, terminate)
    if not 1 <= args.runs <= 100 or not 0 <= args.cpu_workers <= 16:
        parser.error('runs must be 1..100 and cpu-workers 0..16')
    binary = args.test_binary.resolve(strict=True)
    args.artifacts.mkdir(parents=True, mode=0o700, exist_ok=False)
    workers = []
    results = []
    try:
        for _ in range(args.cpu_workers):
            workers.append(subprocess.Popen([sys.executable, '-c', 'while True: pass']))
        for index in range(args.runs):
            before = pressure()
            start = time.monotonic()
            with (args.artifacts / f'run-{index + 1}.log').open('w') as log:
                child = subprocess.Popen(['dbus-run-session', '--', str(binary), '--nocapture'],
                                         stdout=log, stderr=subprocess.STDOUT,
                                         start_new_session=True)
                code = 'interrupted'
                try:
                    code = child.wait(timeout=120)
                except subprocess.TimeoutExpired:
                    code = 'timeout'
                finally:
                    # Includes daemon children if libtest panicked or timed out.
                    try:
                        os.killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    child.wait()
                    result = dict(run=index + 1, result=code, seconds=time.monotonic() - start,
                                  pressure_before=before, pressure_after=pressure())
                    results.append(result)
                    print(f'run {index + 1}: {code} ({result["seconds"]:.2f}s)', flush=True)
    finally:
        for worker in workers:
            worker.terminate()
        for worker in workers:
            worker.wait()
        (args.artifacts / 'results.json').write_text(json.dumps(
            dict(binary=str(binary), cpu_workers=args.cpu_workers,
                 cpus=sorted(os.sched_getaffinity(0)), results=results), indent=2) + '\n')
    return int(any(result['result'] != 0 for result in results))


if __name__ == '__main__':
    sys.exit(main())
