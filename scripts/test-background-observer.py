#!/usr/bin/env python3
"""Verify the Rust observer against real private Niri and bubblewrap namespaces.

This checks application observation only. It does not run Background permission
policy, start real applications or alter portal routing.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
CASE = 'background_observer::native_observer_fixture'
APP_A, APP_B = 'org.canopa.SandboxA', 'org.canopa.SandboxB'


def verify(harness, session, clients):
    assert CASE + ': test' in subprocess.check_output([str(harness), '--list'], text=True)
    session.start_niri('animations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n')
    niri = Path(shutil.which('niri', path=session.env['PATH'])).resolve()
    compositor = dict(path=str(niri), version=session.run(['niri', '--version']).strip(), sha256=hashlib.sha256(niri.read_bytes()).hexdigest())
    protocols = Path('/usr/share/wayland-protocols')
    for name, xml in [('xdg-shell', protocols / 'stable/xdg-shell/xdg-shell.xml'), ('xdg-foreign', protocols / 'unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for kind, suffix in [('client-header', '.h'), ('private-code', '.c')]:
            subprocess.run(['wayland-scanner', kind, str(xml), str(session.output / (name + suffix))], check=True)
    executable = session.output / 'parent'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', '-I', str(session.output), str(ROOT / 'scripts/desktop-fixture/portal-parent.c'), str(session.output / 'xdg-shell.c'), str(session.output / 'xdg-foreign.c'), '-lwayland-client', '-o', str(executable)], check=True)
    env = {**session.env, 'CANOPA_BACKGROUND_NATIVE_FIXTURE': '1'}
    with (session.output / 'observer.log').open('w') as log:
        helper = subprocess.Popen([str(harness), '--ignored', '--exact', CASE, '--nocapture'], env=env, stdin=subprocess.PIPE, stdout=log, stderr=log, text=True, start_new_session=True)
    clients.append(helper)

    def observation():
        values = []
        for line in (session.output / 'observer.log').read_text().splitlines():
            if 'OBSERVATION ' in line:
                values.append(json.loads(line.split('OBSERVATION ', 1)[1]))
        assert helper.poll() is None, 'observer fixture exited prematurely'
        return values[-1] if values else None
    def apps(expected):
        session.wait_for(lambda: observation() == {'apps': expected}, f'observed applications {expected}')
    def windows():
        return json.loads(session.run(['niri', 'msg', '--json', 'windows']))
    def focus(window):
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(window['id'])])
        session.wait_for(lambda: any(w['id'] == window['id'] and w['is_focused'] for w in windows()), 'fixture window focus')
    def start(name, identity=None, sandbox=True):
        info = session.output / (name + '.flatpak-info')
        if identity:
            info.write_text(f'[Application]\nname={identity}\n[Instance]\ninstance-id={name}\n')
        command = [str(executable)]
        if sandbox:
            command = ['bwrap', '--unshare-user', '--uid', '0', '--gid', '0', '--die-with-parent', '--tmpfs', '/', '--ro-bind', '/usr', '/usr']
            for library in ['/lib', '/lib64']:
                if Path(library).exists():
                    command += ['--ro-bind', library, library]
            command += ['--proc', '/proc', '--dev', '/dev', '--ro-bind', str(executable), '/fixture', '--bind', str(session.runtime), str(session.runtime), '--chdir', '/']
            if identity:
                command += ['--ro-bind', str(info), '/.flatpak-info']
            command += ['--setenv', 'WAYLAND_DISPLAY', session.env['WAYLAND_DISPLAY'], '--setenv', 'XDG_RUNTIME_DIR', str(session.runtime), '--', '/usr/bin/sh', '-c', 'printf "CLIENT_PID:%s\\n" "$$"; exec /fixture']
        process = session.start(command, name + '.log')
        session.processes.remove(process)  # These clients close normally below.
        clients.append(process)
        if sandbox:
            def pid():
                return next((int(line.split(':', 1)[1]) for line in (session.output / (name + '.log')).read_text().splitlines() if line.startswith('CLIENT_PID:')), None)
            client_pid = session.wait_for(pid, 'sandbox client PID')
        else:
            client_pid = process.pid
        window = session.wait_for(lambda: next((w for w in windows() if w['pid'] == client_pid), None), 'sandbox Wayland window')
        assert window['app_id'] == 'org.canopa.PortalTest'  # Deliberately differs from metadata.
        return process, window
    def close(process, window):
        session.run(['niri', 'msg', 'action', 'close-window', '--id', str(window['id'])])
        session.wait_for(lambda: all(w['id'] != window['id'] for w in windows()), 'fixture window closes')
        assert process.wait(timeout=5) == 0

    checks = []
    apps({})
    checks.append('Rust observer publishes an empty map only after the complete native snapshot')
    first, first_window = start('first', APP_A)
    focus(first_window)
    apps({APP_A: 2})
    checks.append('actual process-root metadata supplies identity despite a different declared Wayland app ID')
    second, second_window = start('second', APP_A)
    focus(second_window)
    apps({APP_A: 2})
    third, third_window = start('third', APP_B)
    focus(third_window)
    apps({APP_A: 1, APP_B: 2})
    focus(first_window)
    apps({APP_A: 2, APP_B: 1})
    checks.append('multiple namespaces/windows aggregate by authoritative application and exact current focus')
    close(first, first_window)
    focus(second_window)
    apps({APP_A: 2, APP_B: 1})
    close(second, second_window)
    focus(third_window)
    apps({APP_B: 2})
    checks.append('one window can close without losing its sibling; last-window close removes the application')
    host, host_window = start('host', sandbox=False)
    focus(host_window)
    apps({APP_B: 1})
    close(host, host_window)
    close(third, third_window)
    apps({})
    checks.append('confirmed host windows are excluded instead of trusting their declared app IDs')
    unknown, unknown_window = start('unknown')
    session.wait_for(lambda: observation() == {'unavailable': 'Identity'}, 'unidentified namespace fails observation')
    close(unknown, unknown_window)
    apps({})
    checks.append('an unidentified namespace reports unavailable rather than a fabricated empty application map')
    helper.stdin.write('stop\n')
    helper.stdin.flush()
    assert helper.wait(timeout=5) == 0
    checks.append('explicit fixture shutdown retires the observer and its worker')
    return checks, compositor


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--harness', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-background-native-') as directory:
        session = desktop.Session(Path(directory), args.output.resolve())
        clients = []
        try:
            checks, compositor = verify(args.harness.resolve(), session, clients)
            desktop.write_json(args.output / 'result.json', dict(passed=True, checks=checks, compositor=compositor, harness=dict(path=str(args.harness.resolve()), sha256=hashlib.sha256(args.harness.read_bytes()).hexdigest()), scope='native observer and synthetic sandbox namespaces; full Background policy and real Flatpak instance acceptance pending'))
        finally:
            for process in reversed(clients + session.processes):
                desktop.stop(process)
    print(f'Background observer: {len(checks)} native checks passed')


if __name__ == '__main__':
    main()
