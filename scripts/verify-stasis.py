#!/usr/bin/env python3
"""Real Stasis with private Wayland, logind FD inhibitors and isolated PAM policies."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('desktop', ROOT/'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def verify(binaries, stasis, shell_binary, session):
    root = session.runtime
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n')
    niri = next(p for p in session.processes if p.args[0] == 'niri')
    session.env['DBUS_SYSTEM_BUS_ADDRESS'] = session.env['DBUS_SESSION_BUS_ADDRESS']
    session.env.pop('XDG_SESSION_ID', None)
    session.start(['/usr/bin/python3', str(ROOT/'scripts/desktop-fixture/lock-logind.py'), str(root)], 'logind.log')
    session.wait_for(lambda: (root/'logind-ready').exists(), 'private logind')
    session.start(['wtype', '-s', '600000'], 'keyboard.log')
    pam = root/'pam'
    pam.mkdir()
    module = root/'pam-interactive.so'
    subprocess.run(['cc', '-shared', '-fPIC', '-Wall', '-Wextra', str(ROOT/'scripts/desktop-fixture/lock-pam-interactive.c'), '-lpam', '-o', str(module)], check=True)
    (pam/'canopa-lock').write_text(f'auth required {module} password {root}\naccount required {module}\n')
    prefix = ['bwrap', '--ro-bind', '/', '/', '--dev', '/dev', '--bind', str(root), str(root), '--ro-bind', str(pam), '/etc/pam.d', '--die-with-parent']
    config_home = Path(session.env['XDG_CONFIG_HOME'])/'canopa'
    config_home.mkdir(parents=True)
    config = config_home/'config.kdl'
    config.write_text('schema-version 1\nlockscreen { auth-timeout 20; widgets "keyboard"; }\n'
                      'shell { bar "top" { output "all"; edge "top"; collapsed-width 44; '
                      'modules { start "awake"; }; }; '
                      'modules { module "awake" { type "idle-inhibitor"; backend "stasis"; }; }; }\n')
    session.env['PATH'] = str(stasis.parent)+os.pathsep+session.env.get('PATH', os.defpath)
    locker = binaries/'canopa-lock'
    idle = binaries/'canopa-idle'
    jobs = []
    stopped = False
    def events():
        return json.loads((root/'logind-state.json').read_text())
    def auth_count():
        try:
            return sum(line.endswith(' started') for line in (root/'auth-events').read_text().splitlines())
        except FileNotFoundError:
            return 0
    def run(*args):
        return session.run([*prefix, *map(str, args)])
    def start(command, name):
        log = (session.output/name).open('w')
        process = subprocess.Popen([*prefix, *map(str, command)], env=session.env, stdout=log, stderr=log, start_new_session=True)
        log.close()
        jobs.append(process)
        return process
    def notify_start(command, name):
        path = root/(name+'.sock')
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
        sock.bind(str(path))
        sock.settimeout(12)
        session.env['NOTIFY_SOCKET'] = str(path)
        process = session.start([*prefix, *map(str, command)], name+'.log')
        session.env.pop('NOTIFY_SOCKET')
        try:
            assert sock.recv(1024) == b'READY=1'
        finally:
            sock.close()
        return process
    def locked():
        session.wait_for(lambda: events()['locked'] and auth_count(), 'actual compositor lock and PAM prompt')
        time.sleep(.8)
    def unlock():
        session.run(['wtype', '-k', 'Left'])
        session.run(['wtype', '-d', '15', 'canopa-fixture-password', '-k', 'Return'])
        session.wait_for(lambda: not events()['locked'], 'authenticated unlock')
        time.sleep(.3)
    def bus(method, *args):
        return session.run(['busctl', '--address='+session.env['DBUS_SESSION_BUS_ADDRESS'], 'call', 'org.freedesktop.login1', '/org/freedesktop/login1', 'org.freedesktop.login1.Manager', method, *args])
    def status():
        return json.loads(run(locker, 'status'))
    try:
        listener = notify_start([locker, '--canopa-config', config, 'daemon'], 'listener')
        initial = status()
        assert initial['sleep_coordinator'] and not initial['ready'] and initial['session'] == 'canopa-fixture'
        assert any(e.get('inhibitor') == 'acquired' for e in events()['events'])
        rune = config_home/'stasis.rune'
        run(idle, 'init', '--output', rune)
        # Exercise automatic idle locking promptly, without automatic host actions.
        rune.write_text(rune.read_text().replace('timeout 300', 'timeout 2').replace('timeout 120', 'timeout 2').replace('timeout 60\n', 'timeout 10000\n').replace('timeout 30\n', 'timeout 10000\n').replace('timeout 600', 'timeout 10000').replace('debounce_seconds 5', 'debounce_seconds 1'))
        supervisor = notify_start([idle, '--stasis', stasis, 'run', '--config', rune], 'supervisor')
        watch = start([idle, '--stasis', stasis, 'watch'], 'watch.jsonl')
        def snapshots():
            values = []
            for line in (session.output/'watch.jsonl').read_text().splitlines():
                try:
                    value = json.loads(line)
                except ValueError:
                    continue
                if 'state' in value:
                    values.append(value)
            return values
        session.wait_for(snapshots, 'Stasis streaming state')
        locked()
        session.capture('automatic-lock.png')
        assert status()['ready']
        assert any(v['state'] == 'locked' for v in snapshots())
        run(idle, '--stasis', stasis, 'pause')
        session.wait_for(lambda: snapshots()[-1]['manually_paused'], 'manual Stasis pause')
        unlock()
        run(idle, '--stasis', stasis, 'toggle')
        session.wait_for(lambda: not snapshots()[-1]['manually_paused'], 'Stasis toggle')
        run(idle, '--stasis', stasis, 'pause')
        session.wait_for(lambda: snapshots()[-1]['manually_paused'], 'pause before sleep tests')
        # A shell module controls the same Stasis manual hold and reads its
        # streamed state. It must not create a second Wayland inhibitor.
        pointer = session.build_pointer()
        session.env['WAYLAND_DEBUG'] = 'client'
        shell = session.start([str(shell_binary)], 'shell.log')
        session.env.pop('WAYLAND_DEBUG')
        session.wait_for_layers(1)
        time.sleep(.6)
        session.capture('shell-paused.png')
        session.run([str(pointer), '18', '20'])
        session.wait_for(lambda: not snapshots()[-1]['manually_paused'], 'shell click resumes Stasis')
        session.run([str(pointer), '18', '20'])
        session.wait_for(lambda: snapshots()[-1]['manually_paused'], 'shell click pauses Stasis')
        assert 'create_inhibitor(' not in (session.output/'shell.log').read_text()
        session.retire(shell)
        assert snapshots()[-1]['manually_paused'], 'shell lifetime must not own the manual hold'
        # A stalled compositor must not produce a readiness acknowledgement or
        # allow Canopa's own suspend command to request system sleep.
        niri.send_signal(signal.SIGSTOP)
        stopped = True
        before = len(events()['events'])
        request = start([idle, 'suspend'], 'failed-suspend.log')
        assert request.wait(timeout=9) != 0
        assert not any(e.get('sleep') == 'requested' for e in events()['events'][before:])
        assert not status()['ready']
        # An externally initiated sleep must keep the actual FD inhibitor until
        # the compositor replies. This is a real Unix FD passed over private D-Bus.
        bus('Suspend', 'b', 'false')
        pending = start([locker, 'ensure', '--timeout-ms', '300'], 'failed-ensure.log')
        assert pending.wait(timeout=4) != 0
        time.sleep(.3)
        assert not any(e.get('inhibitor') == 'released' or e.get('sleep') == 'performed' for e in events()['events'][before:])
        niri.send_signal(signal.SIGCONT)
        stopped = False
        locked()
        session.wait_for(lambda: any(e.get('sleep') == 'performed' for e in events()['events'][before:]), 'sleep after compositor acknowledgement')
        performed = [e for e in events()['events'][before:] if e.get('sleep') == 'performed']
        assert performed and all(e['locked'] for e in performed)
        assert any(e.get('locked') is True for e in events()['events'][before:])
        session.capture('sleep-lock.png')
        acquired = sum(e.get('inhibitor') == 'acquired' for e in events()['events'])
        bus('Resume')
        session.wait_for(lambda: sum(e.get('inhibitor') == 'acquired' for e in events()['events']) > acquired, 'fresh inhibitor after resume')
        unlock()
        # Explicit suspend also waits for locking before it calls logind.Suspend.
        before = len(events()['events'])
        request = start([idle, 'suspend'], 'explicit-suspend.log')
        locked()
        assert request.wait(timeout=8) == 0
        session.wait_for(lambda: any(e.get('sleep') == 'performed' for e in events()['events'][before:]), 'coordinated explicit sleep')
        assert all(e.get('locked') for e in events()['events'][before:] if e.get('sleep') in ('requested', 'performed'))
        bus('Resume')
        unlock()
        # Concurrent foreground callers share one PAM transaction, remain alive
        # while locked, and both finish only after the authenticated unlock.
        before = auth_count()
        first = start([locker], 'foreground-first.log')
        second = start([locker], 'foreground-second.log')
        locked()
        time.sleep(.3)
        assert auth_count() == before+1 and first.poll() is None and second.poll() is None
        current = status()
        run(locker, 'ensure')
        assert status()['generation'] == current['generation']
        unlock()
        assert first.wait(timeout=5) == 0 and second.wait(timeout=5) == 0
        # Losing the lock coordinator ends the idle backend but preserves an
        # already active locker. This prevents an unattended idle policy backend.
        first = start([locker], 'foreground-preserved.log')
        locked()
        listener.send_signal(signal.SIGTERM)
        listener.wait(timeout=5)
        session.processes.remove(listener)
        assert supervisor.wait(timeout=8) != 0
        session.processes.remove(supervisor)
        assert events()['locked']
        unlock()
        assert first.wait(timeout=5) != 0
        result = {'status': 'passed', 'checks': ['inhibitor before listener readiness',
            'real Stasis startup and watch readiness', 'automatic idle locking',
            'foreground locker lifetime and LockedHint integration', 'streamed pause/toggle state',
            'shell click controls real Stasis manual hold', 'Stasis module creates no Wayland inhibitor',
            'manual hold survives shell exit',
            'own suspend refused before compositor readiness', 'ensure timeout cannot acknowledge stalled compositor',
            'external sleep holds real inhibitor FD', 'sleep follows compositor acknowledgement',
            'resume reacquires inhibitor', 'explicit suspend locks first', 'concurrent requests share one locker',
            'foreground clients wait for authenticated unlock', 'coordinator loss stops Stasis',
            'listener stop preserves active lock'],
            'stasis': {'version': subprocess.check_output([str(stasis), '--version'], text=True).strip(),
                       'sha256': hashlib.sha256(stasis.read_bytes()).hexdigest()}}
        (session.output/'summary.json').write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps(result, indent=2))
    finally:
        if stopped:
            niri.send_signal(signal.SIGCONT)
        (session.output/'logind-events.json').write_text(json.dumps(events(), indent=2)+'\n')
        for process in jobs:
            if process.poll() is None:
                desktop.stop(process)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin-dir', type=Path, default=ROOT/'target/debug')
    parser.add_argument('--stasis', type=Path, default=ROOT/'target/debug/stasis')
    parser.add_argument('--shell', type=Path, default=ROOT/'target/debug/canopa-shell')
    parser.add_argument('--output', type=Path, default=ROOT/'target/native-runs/verify-stasis')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='canopa-stasis-') as temporary:
        root = Path(temporary)
        root.chmod(0o700)
        session = desktop.Session(root, args.output.resolve())
        try:
            verify(args.bin_dir.resolve(), args.stasis.resolve(), args.shell.resolve(), session)
        finally:
            session.close()

if __name__ == '__main__':
    main()
