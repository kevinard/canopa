#!/usr/bin/env python3
"""Read-only checks against the exact development bundle's built programs."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    identities = json.loads((ROOT / 'target/portal-window-screenshot-95/delivery-identities-01.json').read_text())['identities']
    shared = Path('/home/kevinard/.config/canopa/config.kdl')
    niri_config = Path('/home/kevinard/.config/niri/config.kdl')
    watched = [shared, *sorted(niri_config.parent.rglob('*.kdl'))]
    before = {str(p): digest(p) for p in watched if p.is_file()}
    checks = []
    commands = [('niri', ROOT / 'target/niri-screencast-native/debug/niri',
                 ['validate', '--config', str(niri_config)],
                 'libexec/canopa/desktop-sharing/niri')]
    for name, flag in [('canopa-shell', '--check-config'), ('canopa-notify', 'check-config'), ('canopa-files', '--check-config')]:
        commands.append((name, ROOT / 'target/debug' / name,
                         [flag, '--canopa-config', str(shared)], 'bin/' + name))
    validation = OUT / 'validation'
    validation.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='canopa-config-review-') as folder:
        env = {**os.environ, 'XDG_STATE_HOME': folder + '/state', 'XDG_CACHE_HOME': folder + '/cache'}
        for label, binary, args, member in commands:
            checksum = digest(binary)
            expected = identities['canopa-x86_64-unknown-linux-gnu/' + member]['sha256']
            assert checksum == expected, f'{label}: built program differs from bundle'
            with (validation / (label + '.log')).open('w') as log:
                p = subprocess.run([str(binary), *args], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=20)
            checks.append(dict(name=label, command=[str(binary), *args], sha256=checksum,
                               matches_bundle=True, exit_code=p.returncode))
    after = {str(p): digest(p) for p in watched if p.is_file()}
    assert before == after, 'Configuration changed during review'
    result = dict(status='passed' if all(v['exit_code'] == 0 for v in checks) else 'failed',
                  checks=checks, configuration_sha256=before, configurations_unchanged=True,
                  scope='Configuration parsing/validation; no live service or compositor changes.')
    (validation / 'configuration-results.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
