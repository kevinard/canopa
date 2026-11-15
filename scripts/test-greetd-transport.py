#!/usr/bin/env python3
"""Exercise Canopa's transport against real greetd with private PAM and namespaces.

No host accounts, PAM files, VTs, login manager or logind sessions are used.
Namespace root maps to the caller. A fixture shim skips initgroups because a
single-ID unprivileged namespace cannot set supplementary groups. This verifies
IPC/PAM retry and command handoff, not privileged seat/session setup.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--greetd', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=ROOT/'target/native-runs/test-greetd-transport')
    args = parser.parse_args()
    if os.geteuid() == 0:
        parser.error('run as an unprivileged user; namespace root is sufficient')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    build = subprocess.run([
        'bash', str(ROOT / 'scripts/cargo.sh'), 'test', '--locked', '-p', 'canopa-greeter', '--test', 'integration',
        '--no-run', '--message-format=json',
    ], cwd=ROOT, capture_output=True, text=True)
    (output / 'build.log').write_text(build.stderr)
    if build.returncode:
        raise RuntimeError(f'transport test build failed; see {output / "build.log"}')
    artifacts = [json.loads(line) for line in build.stdout.splitlines()]
    binary = next(a['executable'] for a in artifacts
                  if a.get('reason') == 'compiler-artifact' and a.get('executable')
                  and a['target']['name'] == 'integration')
    with tempfile.TemporaryDirectory(prefix='canopa-real-greetd-') as directory:
        root = Path(directory)
        pam = root / 'pam'
        pam.mkdir()
        module = root / 'pam-fixture.so'
        subprocess.run(['cc', '-shared', '-fPIC', '-Wall', '-Wextra',
                        str(ROOT / 'scripts/desktop-fixture/lock-pam.c'), '-lpam',
                        '-o', str(module)], check=True)
        (root / 'nogroups.c').write_text(
            '#include <sys/types.h>\nint initgroups(const char *user, gid_t group) '
            '{ (void)user; (void)group; return 0; }\n')
        shim = root / 'nogroups.so'
        subprocess.run(['cc', '-shared', '-fPIC', '-Wall', '-Wextra',
                        str(root / 'nogroups.c'), '-o', str(shim)], check=True)
        for service in ['greetd', 'greetd-greeter']:
            (pam / service).write_text(
                f'auth required {module}\naccount required pam_permit.so\n'
                'session required pam_permit.so\n')
        (root / 'passwd').write_text(f'root:x:0:0:Fixture:{root}:/bin/sh\n')
        (root / 'group').write_text('root:x:0:\n')
        (root / 'session.py').write_text('''import json, os, sys, time
from pathlib import Path
root = Path(__file__).parent
assert (root / 'greeter-finished').read_text() == 'ok'
assert sys.argv[1:] == ['arg with spaces', "'quote\\\"", f'$(touch {root}/unexpected)', '']
assert os.environ['CANOPA_HANDOFF'] == 'literal value'
assert not (root / 'unexpected').exists()
(root / 'result.json').write_text(json.dumps(dict(status='passed', checks=[
    'real greetd framing and PAM conversation', 'failed password retry',
    'session launches after greeter exit', 'literal argv and environment handoff'])))
time.sleep(30)
''')
        command = shlex.join([
            '/usr/bin/env', 'CANOPA_GREETD_ACCEPTANCE=1', f'CANOPA_GREETD_ROOT={root}',
            binary, '--ignored', '--exact',
            'greetd_transport::failed_password_retries_then_hands_off_to_real_greetd', '--nocapture',
        ])
        config = root / 'config.toml'
        config.write_text('[terminal]\nvt = "none"\nswitch = false\n[general]\n'
                          f'source_profile = false\nrunfile = "{root}/runfile"\n'
                          '[default_session]\nuser = "root"\n'
                          f'command = {json.dumps(command)}\n')
        sandbox = [
            'bwrap', '--unshare-user', '--unshare-pid', '--unshare-net', '--unshare-ipc',
            '--unshare-uts', '--uid', '0', '--gid', '0', '--ro-bind', '/', '/',
            '--dev', '/dev', '--proc', '/proc', '--bind', str(root), str(root),
            '--tmpfs', '/run', '--ro-bind', str(pam), '/etc/pam.d',
            '--ro-bind', str(root / 'passwd'), '/etc/passwd',
            '--ro-bind', str(root / 'group'), '/etc/group',
            '--setenv', 'LD_PRELOAD', str(shim), '--die-with-parent',
            str(args.greetd.resolve()), '-c', str(config), '-s', str(root / 'greetd.sock'),
        ]
        with (output / 'greetd.log').open('w') as log:
            process = subprocess.Popen(sandbox, stdout=log, stderr=log, start_new_session=True)
            try:
                deadline = time.monotonic() + 40
                while not (root / 'result.json').exists():
                    if process.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError(f'greetd acceptance failed; see {output / "greetd.log"}')
                    time.sleep(.05)
                result = json.loads((root / 'result.json').read_text())
                result['greetd'] = str(args.greetd.resolve())
                result['greetd_sha256'] = hashlib.sha256(args.greetd.read_bytes()).hexdigest()
                result['test_sha256'] = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
                (output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
                print(json.dumps(result, indent=2))
            finally:
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()


if __name__ == '__main__':
    main()
