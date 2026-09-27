#!/usr/bin/env python3
"""Measure the real headless executable on a new private bus and temporary XDG paths."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import socket
import statistics
import subprocess
import sys
import tempfile
import time


def control(path, request):
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(5)
        client.connect(str(path))
        client.sendall((request + '\n').encode())
        reply = bytearray()
        while part := client.recv(65536):
            reply.extend(part)
            if len(reply) > 1_048_576:
                raise RuntimeError('measurement control reply exceeded limit')
        return json.loads(reply)


def memory(pid):
    fields = {}
    for line in Path(f'/proc/{pid}/status').read_text().splitlines():
        if line.startswith(('VmRSS:', 'VmHWM:')):
            key, value, _ = line.split()
            fields[key.rstrip(':')] = int(value)
    return fields


def measure(binary, count):
    import gi
    gi.require_version('Gio', '2.0')
    from gi.repository import Gio, GLib
    with tempfile.TemporaryDirectory(prefix='canopa-measure-') as directory:
        root = Path(directory)
        env = os.environ.copy()
        for name in ('RUNTIME', 'STATE', 'CONFIG', 'DATA', 'CACHE'):
            path = root / name.lower()
            path.mkdir(mode=0o700)
            env[f'XDG_{name}_HOME' if name != 'RUNTIME' else 'XDG_RUNTIME_DIR'] = str(path)
        path = root / 'runtime' / 'control.sock'
        log = (root / 'daemon.log').open('w+')
        daemon = subprocess.Popen([str(binary), 'daemon', '--headless', '--socket', str(path),
                                   '--dev-bus-name', 'org.canopa.NotifyMeasurement'], env=env,
                                  stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 10
            while True:
                if daemon.poll() is not None:
                    raise RuntimeError('measurement daemon failed startup')
                try:
                    control(path, 'control {"command":"status"}')
                    break
                except (OSError, ValueError):
                    if time.monotonic() > deadline:
                        raise RuntimeError('measurement daemon was not ready')
                    time.sleep(.02)
            bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
            def call(method, args, reply):
                return bus.call_sync('org.canopa.NotifyMeasurement',
                    '/org/freedesktop/Notifications', 'org.freedesktop.Notifications',
                    method, args, GLib.VariantType.new(reply), Gio.DBusCallFlags.NONE, 5000, None)
            def receipt(transient):
                hints = {'transient': GLib.Variant('b', transient)}
                start = time.perf_counter_ns()
                result = call('Notify', GLib.Variant('(susssasa{sv}i)',
                    ('Measurement', 0, '', 'Fixed workload', 'x' * 4096, [], hints, 0)), '(u)')
                latency = (time.perf_counter_ns() - start) / 1_000_000
                call('CloseNotification', GLib.Variant('(u)', result.unpack()), '()')
                return latency
            for _ in range(32):
                receipt(True)
            workloads = []
            for transient in (True, False):
                before = memory(daemon.pid)
                start = time.monotonic()
                latencies = sorted(receipt(transient) for _ in range(count))
                elapsed = time.monotonic() - start
                workloads.append({'transient': transient, 'notifications': count,
                    'body_bytes': 4096, 'pattern': 'serial Notify then CloseNotification',
                    'elapsed_seconds': round(elapsed, 3),
                    'notify_ms': {'median': statistics.median(latencies),
                        'p95': latencies[int((count - 1) * .95)],
                        'p99': latencies[int((count - 1) * .99)], 'max': max(latencies)},
                    'before_kib': before, 'after_kib': memory(daemon.pid)})
            status = control(path, 'control {"command":"status"}')['value']
            assert status['timeout_records'] == 0, status
            assert status['sender_publication_error'] is None, status
            start = time.monotonic()
            daemon.terminate()
            assert daemon.wait(timeout=10) == 0
            shutdown_ms = (time.monotonic() - start) * 1000
            assert not path.exists(), 'shutdown retained socket'
            return {'binary_sha256': hashlib.file_digest(binary.open('rb'), 'sha256').hexdigest(),
                    'platform': platform.platform(), 'cpu_count': os.cpu_count(),
                    'cpu_model': next((line.split(':', 1)[1].strip() for line in
                        Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), 'unknown'),
                    'workloads': workloads, 'shutdown_ms': shutdown_ms}
        finally:
            if daemon.poll() is None:
                daemon.kill()
                daemon.wait()
            log.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('--count', type=int, default=1000)
    parser.add_argument('--inside-private-bus', action='store_true', help=argparse.SUPPRESS)
    args = parser.parse_args()
    if not 100 <= args.count <= 10000:
        parser.error('count must be between 100 and 10000')
    binary = args.binary.resolve(strict=True)
    if args.inside_private_bus:
        print(json.dumps(measure(binary, args.count), indent=2))
    else:
        sys.exit(subprocess.call(['dbus-run-session', '--', sys.executable, str(Path(__file__).resolve()),
                                  str(binary), '--count', str(args.count), '--inside-private-bus']))
