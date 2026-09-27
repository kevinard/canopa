#!/usr/bin/env python3
"""Verify the enabled companion graph without contacting the running user manager."""
import os
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCES = [ROOT / 'packaging/systemd/canopa-session.target',
           ROOT / 'packaging/systemd/niribar.service',
           ROOT / 'packaging/systemd/canopa-notify.service',
           ROOT / 'packaging/systemd/xdg-desktop-portal-canopa.service']

with tempfile.TemporaryDirectory(prefix='canopa-units-') as directory:
    fixture = Path(directory)
    runtime = fixture / 'runtime'
    runtime.mkdir(mode=0o700)
    # A small, complete user-manager graph, isolated from installed user units.
    for name in ['basic', 'shutdown', 'sockets', 'graphical-session-pre', 'graphical-session']:
        (fixture / f'{name}.target').write_text(f'[Unit]\nDescription=Fixture {name}\n')
    (fixture / 'dbus.socket').write_text('[Socket]\nListenStream=%t/bus\n')
    (fixture / 'dbus.service').write_text('[Service]\nExecStart=/usr/bin/true\n')
    wants = fixture / 'graphical-session.target.wants'
    wants.mkdir()
    for source in SOURCES:
        content = source.read_text()
        # Check dependency ordering independent of where release binaries are
        # installed; verify never executes this fixture command.
        content = re.sub(r'^ExecStart=.*$', 'ExecStart=/usr/bin/true', content, flags=re.M)
        (fixture / source.name).write_text(content)
        (wants / source.name).symlink_to(f'../{source.name}')
    env = {**os.environ, 'SYSTEMD_UNIT_PATH': str(fixture), 'XDG_RUNTIME_DIR': str(runtime)}
    command = ['systemd-analyze', '--user', '--man=no', '--recursive-errors=yes', 'verify',
               str(fixture / 'graphical-session.target'),
               *(str(fixture / source.name) for source in SOURCES)]
    result = subprocess.run(command, env=env, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    # Prove the verifier checks ordering, by adding an explicit reverse edge.
    target = fixture / 'canopa-session.target'
    target.write_text(target.read_text().replace('Before=graphical-session.target',
                                              'Before=graphical-session.target\nAfter=graphical-session.target'))
    invalid = subprocess.run(command, env=env, capture_output=True, text=True)
    assert invalid.returncode != 0 and 'cycle' in invalid.stderr.lower(), invalid.stderr
print('Companion units: enabled dependency graph passes; ordering-cycle regression rejected.')
