#!/usr/bin/env python3
"""Read-only preflight for the dedicated Desktop acceptance runner."""

import argparse
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--niribar-root', type=Path,
                        default=Path(__file__).resolve().parents[1] / 'crates/niribar')
    args = parser.parse_args()
    failures = []
    for command in ['cargo', 'rustc', 'git', 'dbus-daemon', 'dbus-run-session',
                    'niri', 'sway', 'grim', 'weston', 'wtype', 'busctl', 'swaymsg',
                    'cc', 'pkg-config', 'wayland-scanner']:
        path = shutil.which(command)
        print(f'{command}: {path or "MISSING"}')
        if not path:
            failures.append(command)
    workspace = args.niribar_root.resolve().parents[1]
    for path in [args.niribar_root / 'Cargo.toml', workspace / 'Cargo.lock',
                 workspace / 'crates/canopa-panel-protocol/Cargo.toml']:
        if not path.is_file():
            failures.append(str(path))
    # Use the same system interpreter that runs the desktop fixture, without
    # importing GI into this process or touching the live accessibility bus.
    checks = [
        [sys.executable, '-c',
         "import gi; gi.require_version('Gio', '2.0'); "
         "gi.require_version('Atspi', '2.0'); from gi.repository import Gio, Atspi"],
        [sys.executable, '-c', 'from PIL import Image, ImageChops'],
        ['pkg-config', '--exists', 'wayland-client', 'alsa', 'libudev', 'xkbcommon', 'libpulse'],
    ]
    for command in checks:
        try:
            result = subprocess.run(command, capture_output=True, text=True, timeout=15)
            if result.returncode:
                failures.append(' '.join(command))
                print(result.stderr.strip(), file=sys.stderr)
        except (OSError, subprocess.TimeoutExpired) as error:
            failures.append(str(error))
    for executable in ['at-spi-bus-launcher', 'at-spi2-registryd']:
        if not any((root / executable).is_file() for root in
                   map(Path, ['/usr/lib', '/usr/libexec', '/usr/lib/at-spi2-core'])):
            failures.append(executable)
    if failures:
        print('Runner prerequisites missing:\n' + '\n'.join(failures), file=sys.stderr)
        return 1
    print('Runner prerequisites available; run the graphical fixture to verify them.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
