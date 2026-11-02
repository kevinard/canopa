"""Private AT-SPI launcher and independent peer for native session scenarios."""
from pathlib import Path
import subprocess
import sys
ROOT = Path(__file__).resolve().parents[1]


def start(session):
    candidates = [Path('/usr/lib'), Path('/usr/libexec'), Path('/usr/lib/at-spi2-core')]
    launcher = next((p/'at-spi-bus-launcher' for p in candidates if (p/'at-spi-bus-launcher').is_file()), None)
    registry = next((p/'at-spi2-registryd' for p in candidates if (p/'at-spi2-registryd').is_file()), None)
    if not launcher or not registry:
        raise RuntimeError('AT-SPI launcher/registry required')
    # at-spi2-core 2.62 removed --a11y despite retaining it in its usage string.
    # Use the common CLI and the supported property, with memory-only GSettings.
    session.env['GSETTINGS_BACKEND'] = 'memory'
    session.start([str(launcher), '--launch-immediately'], 'a11y-launcher.log')
    def ready():
        try:
            return 'true' in session.run(['busctl', '--address='+session.env['DBUS_SESSION_BUS_ADDRESS'], 'call', 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', 's', 'org.a11y.Bus'])
        except subprocess.SubprocessError:
            return False
    session.wait_for(ready, 'private AT-SPI bus')
    session.run(['busctl', '--address='+session.env['DBUS_SESSION_BUS_ADDRESS'],
                 'set-property', 'org.a11y.Bus', '/org/a11y/bus', 'org.a11y.Status', 'IsEnabled', 'b', 'true'])
    session.start([str(registry)], 'a11y-registry.log')


def request(session, *arguments):
    try:
        return session.run([sys.executable, '-B', str(ROOT/'scripts/desktop-fixture/session-a11y.py'), *map(str, arguments)])
    except subprocess.CalledProcessError as error:
        raise RuntimeError(error.stderr) from error
