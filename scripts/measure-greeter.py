#!/usr/bin/env python3
"""Alternate Cage/Weston greeter sessions in an isolated, fixed-size software desktop.

Measures launcher spawn to first greeter frame callback, then whole process-tree
CPU and RSS. A frame callback is compositor readiness, not physical scanout. No
host login configuration or authentication is used. Build before running; never
compile or run other acceptance/measurement fixtures concurrently with this test.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import select
import socket
import statistics
import subprocess
import sys
import tempfile
import time
from collections import Counter

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def process_stat(pid):
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(') ', 1)[1].split()
    if fields[0] in ('Z', 'X'):
        raise RuntimeError(f'process {pid} exited during measurement')
    return dict(pid=pid, start_ticks=int(fields[19]),
                cpu_ticks=int(fields[11]) + int(fields[12]),
                rss_kib=int(fields[21]) * os.sysconf('SC_PAGE_SIZE') / 1024)


def process_tree(pid):
    result = [pid]
    for child in Path(f'/proc/{pid}/task/{pid}/children').read_text().split():
        result.extend(process_tree(int(child)))
    return result


def sample(pid):
    processes = [process_stat(p) for p in process_tree(pid)]
    return dict(monotonic=time.monotonic(), processes=processes,
                cpu_ticks=sum(p['cpu_ticks'] for p in processes),
                rss_sum_kib=sum(p['rss_kib'] for p in processes))


def summarize(runs):
    result = {}
    for compositor in ('cage', 'weston'):
        group = [r for r in runs if r['compositor'] == compositor]
        startup = [r['startup_ms'] for r in group]
        memory = [r['rss_median_kib'] / 1024 for r in group]
        ticks = sum(r['idle_cpu_ticks'] for r in group)
        elapsed = sum(r['idle_elapsed_seconds'] for r in group)
        result[compositor] = dict(
            runs=len(group), startup_median_ms=statistics.median(startup),
            startup_range_ms=[min(startup), max(startup)],
            idle_cpu_percent_one_core=100 * ticks / os.sysconf('SC_CLK_TCK') / elapsed,
            idle_cpu_ticks=ticks, idle_elapsed_seconds=elapsed,
            cpu_resolution_percentage_points=100 / os.sysconf('SC_CLK_TCK') / elapsed,
            rss_sum_median_mib=statistics.median(memory),
            rss_sum_range_mib=[min(memory), max(memory)],
        )
    return result


def trial(args, session, compositor, number, measured):
    endpoint = session.runtime / 'first-frame.sock'
    endpoint.unlink(missing_ok=True)
    ready = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
    ready.bind(str(endpoint))
    binary = args.cage if compositor == 'cage' else args.weston
    command = [str(args.binary), '--canopa-config', str(session.runtime / 'greeter.kdl'),
               '--ready-socket', str(endpoint), 'session', '--compositor', compositor,
               '--compositor-bin', str(binary), '--renderer', 'software']
    if compositor == 'weston':
        command.extend('--compositor-arg=' + v for v in
                       ['--backend=wayland', '--fullscreen', '--width=1280', '--height=800'])
    name = f'{compositor}-{number}'
    started = time.monotonic_ns()
    process = session.start(command, name + '.log')
    try:
        deadline = time.monotonic() + 20
        while True:
            if select.select([ready], [], [], .1)[0]:
                first_frame = time.monotonic_ns()
                info = json.loads(ready.recv(4096))
                break
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError(f'{name}: no first frame; see compositor log')
        assert (info['width'], info['height']) == (1280, 800), info
        assert info['pid'] in process_tree(process.pid), 'greeter is outside measured tree'
        # A callback alone does not prove that the nested parent displays pixels.
        # Reject blank/black kiosks before accepting any performance sample.
        def visible():
            image = session.capture(name + '-visible.png')
            pixels = image.get_flattened_data() if hasattr(image, 'get_flattened_data') else image.getdata()
            colors = Counter(pixels).most_common(2)
            return colors[0][0] != (0, 0, 0) and len(colors) == 2 and colors[1][1] > 1000
        session.wait_for(visible, name + ' populated greeter pixels', timeout=5)
        result = dict(compositor=compositor, number=number, measured=measured,
                      command=command, greeter=info,
                      startup_ms=(first_frame - started) / 1e6,
                      load_average=os.getloadavg())
        if measured:
            print(f'{name}: first frame {result["startup_ms"]:.1f} ms; settling then sampling', flush=True)
            time.sleep(args.settle_seconds)
            samples = [sample(process.pid)]
            epoch = time.monotonic()
            for index in range(1, args.sample_seconds + 1):
                time.sleep(max(0, epoch + index - time.monotonic()))
                current = sample(process.pid)
                # Missing/exited helpers must never silently lower measured usage.
                identity = lambda s: {(p['pid'], p['start_ticks']) for p in s['processes']}
                assert identity(current) == identity(samples[0]), 'measured process tree changed'
                samples.append(current)
            first, last = samples[0], samples[-1]
            result.update(samples=samples,
                          idle_elapsed_seconds=last['monotonic'] - first['monotonic'],
                          idle_cpu_ticks=last['cpu_ticks'] - first['cpu_ticks'],
                          rss_median_kib=statistics.median(s['rss_sum_kib'] for s in samples))
            if number == 1:
                session.capture(compositor + '.png')
        save(session.output / (name + '.json'), result)
        return result
    finally:
        ready.close()
        endpoint.unlink(missing_ok=True)
        session.retire(process)
        time.sleep(.3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/canopa-greeter')
    parser.add_argument('--cage', type=Path, default=Path('/usr/bin/cage'))
    parser.add_argument('--weston', type=Path, default=Path('/usr/bin/weston'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--runs', type=int, default=5, help='measured runs per compositor, after one warmup each')
    parser.add_argument('--settle-seconds', type=int, default=5)
    parser.add_argument('--sample-seconds', type=int, default=60)
    args = parser.parse_args()
    if args.runs < 1 or args.settle_seconds < 0 or args.sample_seconds < 1:
        parser.error('runs/sample seconds must be positive; settle seconds must be nonnegative')
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    for key in ('binary', 'cage', 'weston'):
        setattr(args, key, getattr(args, key).resolve(strict=True))
    (args.output / 'sampler.py').write_bytes(Path(__file__).read_bytes())
    metadata = dict(started_utc=time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
                    platform=platform.platform(), cpuinfo=Path('/proc/cpuinfo').read_text(),
                    meminfo=Path('/proc/meminfo').read_text(),
                    executables={key: dict(path=str(getattr(args, key)), sha256=digest(getattr(args, key)))
                                 for key in ('binary', 'cage', 'weston')},
                    sampler_sha256=digest(__file__),
                    versions={key: subprocess.check_output([str(getattr(args, key)), flag], text=True).strip()
                              for key, flag in [('cage', '-v'), ('weston', '--version')]},
                    runs_per_compositor=args.runs, settle_seconds=args.settle_seconds,
                    sample_seconds=args.sample_seconds, clock_ticks_per_second=os.sysconf('SC_CLK_TCK'),
                    resolution=[1280, 800], renderer='pixman in both candidate compositors',
                    scope='nested Wayland on the same private Weston/Niri; parent/backend/greetd peer excluded',
                    startup='launcher Popen to receipt of first greeter surface frame callback; warm page cache; not scanout',
                    memory='sum of process RSS, including greeter; shared pages may be counted twice; no PSS access to hardened greeter')
    save(args.output / 'metadata.json', metadata)
    runs = []
    with tempfile.TemporaryDirectory(prefix='canopa-greeter-bench-') as directory:
        session = desktop.Session(Path(directory), args.output)
        try:
            session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nwindow-rule { open-fullscreen true; }\n')
            config = ROOT / 'packaging/greetd/greeter.kdl'
            (session.runtime / 'greeter.kdl').write_bytes(config.read_bytes())
            session.start([sys.executable, str(ROOT / 'scripts/desktop-fixture/greetd.py'), str(session.runtime)], 'greetd.log')
            session.wait_for(lambda: (session.runtime / 'greetd.sock').is_socket(), 'fixture greetd')
            session.env['GREETD_SOCK'] = str(session.runtime / 'greetd.sock')
            session.env.pop('WAYLAND_DEBUG', None)
            metadata.update(config_sha256=digest(config),
                            outputs=json.loads(session.run(['niri', 'msg', '--json', 'outputs'])),
                            niri=session.run(['niri', '--version']).strip(),
                            niri_sha256=digest(subprocess.check_output(['which', 'niri'], text=True).strip()),
                            relevant_environment={k:session.env.get(k) for k in
                                ['LIBGL_ALWAYS_SOFTWARE', 'WLR_RENDERER', 'WLR_BACKENDS', 'LD_LIBRARY_PATH']})
            save(args.output / 'metadata.json', metadata)
            for compositor in ('cage', 'weston'):
                trial(args, session, compositor, 0, False)
            for index in range(1, args.runs + 1):
                for compositor in (('cage', 'weston') if index % 2 else ('weston', 'cage')):
                    runs.append(trial(args, session, compositor, index, True))
                    save(args.output / 'runs.json', runs)
            result = dict(status='passed', results=summarize(runs), limitations=metadata['scope'])
            save(args.output / 'summary.json', result)
            print(json.dumps(result, indent=2))
        finally:
            session.close()


if __name__ == '__main__':
    main()
