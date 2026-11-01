#!/usr/bin/env python3
"""Verify the enabled companion graph without contacting the running user manager."""
import configparser
import os
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCES = [ROOT / 'packaging/systemd/canopa-session.target',
           ROOT / 'packaging/systemd/canopa-shell.service',
           ROOT / 'packaging/systemd/canopa-notify.service',
           ROOT / 'packaging/systemd/canopa-wallpaper.service',
           ROOT / 'packaging/systemd/canopa-lock.service',
           ROOT / 'packaging/systemd/canopa-polkit.service',
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
    # Niri imports the compositor endpoints before reporting Type=notify readiness.
    # Merely waiting for graphical-session-pre.target lets companions race it.
    (fixture / 'niri.service').write_text(
        '[Unit]\nDescription=Fixture Niri\nBindsTo=graphical-session.target\n'
        'Before=graphical-session.target\nWants=graphical-session-pre.target\n'
        'After=graphical-session-pre.target\n'
        '[Service]\nType=notify\nExecStart=/usr/bin/true\n')
    wants = fixture / 'graphical-session.target.wants'
    wants.mkdir()
    (wants / 'niri.service').symlink_to('../niri.service')
    for source in SOURCES:
        content = source.read_text()
        if source.name in ('canopa-shell.service', 'canopa-notify.service', 'canopa-wallpaper.service', 'canopa-lock.service', 'canopa-polkit.service'):
            unit = configparser.ConfigParser(interpolation=None)
            unit.read_string(content)
            assert 'niri.service' in unit['Unit']['After'].split(), (
                f'{source.name} can start before Niri publishes WAYLAND_DISPLAY')
            if source.name == 'canopa-lock.service':
                assert unit['Service']['Type'] == 'notify'
                assert unit['Unit']['ConditionPathExists'] == '/etc/pam.d/canopa-lock'
                assert unit['Service']['KillMode'] == 'process', 'listener stop must preserve an active locker'
                assert unit['Service']['ExecStart'].endswith('/canopa-lock daemon')
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
    # An ordering edge must also work in sessions without a Niri user service.
    (wants / 'niri.service').unlink()
    (fixture / 'niri.service').unlink()
    without_niri = subprocess.run(command, env=env, capture_output=True, text=True)
    assert without_niri.returncode == 0, without_niri.stderr
    # Prove the verifier checks ordering, by adding an explicit reverse edge.
    target = fixture / 'canopa-session.target'
    target.write_text(target.read_text().replace('Before=graphical-session.target',
                                              'Before=graphical-session.target\nAfter=graphical-session.target'))
    invalid = subprocess.run(command, env=env, capture_output=True, text=True)
    assert invalid.returncode != 0 and 'cycle' in invalid.stderr.lower(), invalid.stderr
print('Companion units: compositor readiness ordering and enabled dependency graph pass; '
      'ordering-cycle regression rejected.')
