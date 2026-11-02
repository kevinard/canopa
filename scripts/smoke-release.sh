#!/usr/bin/env bash
set -euo pipefail
archive=${1:?usage: smoke-release.sh ARCHIVE}
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
tar -xzf "$archive" -C "$stage"
bundle="$stage/canopa-x86_64-unknown-linux-gnu"
python3 "$(dirname -- "${BASH_SOURCE[0]}")/package-documentation.py" --check "$bundle"
test -x "$bundle/bin/canopa-files"
"$bundle/bin/canopa-files" --help > /dev/null
"$bundle/bin/canopa-files" --internal-check-copy
"$bundle/bin/canopa-files" --check-config --config "$bundle/share/canopa/canopa.kdl" > /dev/null
test -s "$bundle/share/canopa-files/README.md"
test -s "$bundle/share/canopa-files/LICENSE-MIT"
test -s "$bundle/share/canopa-files/LICENSE-APACHE"
test -s "$bundle/share/canopa/files.md"
desktop-file-validate "$bundle/share/applications/org.canopa.Files.desktop"
test -x "$bundle/bin/canopa-wallpaper"
test -x "$bundle/bin/canopa-lock"
test -x "$bundle/bin/canopa-greeter"
test -x "$bundle/bin/canopa-pinentry"
"$bundle/bin/canopa-pinentry" --defaults check-config > /dev/null
test -x "$bundle/bin/canopa-idle"
"$bundle/bin/canopa-idle" --help > /dev/null
test -s "$bundle/share/canopa/stasis/stasis.rune"
test -s "$bundle/share/systemd/user/canopa-idle.service"
test -x "$bundle/bin/canopa-polkit"
"$bundle/bin/canopa-polkit" --defaults check-config > /dev/null
test -s "$bundle/share/systemd/user/canopa-polkit.service"
"$bundle/bin/canopa-greeter" --defaults check-config > /dev/null
test -s "$bundle/share/canopa/greetd/config.toml"
test -s "$bundle/share/canopa/greetd/config-weston.toml"
"$bundle/bin/canopa-greeter" --defaults session --compositor cage --print-command > /dev/null
"$bundle/bin/canopa-greeter" --defaults session --compositor weston --print-command > /dev/null
test -s "$bundle/share/canopa/greetd/greeter.kdl"
test -s "$bundle/share/wayland-sessions/canopa.desktop"
"$bundle/bin/canopa-wallpaper" check-config --defaults > /dev/null
"$bundle/bin/canopa-lock" --defaults check-config > /dev/null
test -s "$bundle/share/systemd/user/canopa-wallpaper.service"
test -s "$bundle/share/systemd/user/canopa-lock.service"
test -s "$bundle/share/canopa/pam/canopa-lock.arch"
test -s "$bundle/share/canopa/pam/canopa-lock.debian"
test -s "$bundle/share/canopa/session-tools.md"
test -x "$bundle/bin/canopa"
"$bundle/bin/canopa" config check --file "$bundle/share/canopa/canopa.kdl"
"$bundle/bin/canopa" config check --file "$bundle/share/canopa/examples/canopa-desktop.kdl"
"$bundle/bin/canopa" config check --file "$bundle/share/canopa/examples/custom-theme.kdl"
test -x "$bundle/bin/canopa-shell"
test -x "$bundle/bin/canopa-wifi-provider"
"$bundle/bin/canopa-audio-provider" --help > /dev/null
"$bundle/bin/canopa-media-provider" --help > /dev/null
test -x "$bundle/bin/canopa-notify"
"$bundle/bin/xdg-desktop-portal-canopa" --help > /dev/null
test -s "$bundle/share/xdg-desktop-portal/portals/canopa.portal"
test -s "$bundle/share/dbus-1/services/org.freedesktop.impl.portal.desktop.canopa.service"
test -s "$bundle/share/dbus-1/services/org.freedesktop.FileManager1.service"
rg -Fx 'Name=org.freedesktop.FileManager1' "$bundle/share/dbus-1/services/org.freedesktop.FileManager1.service"
rg -Fx 'Exec=/usr/bin/env canopa-files --daemon' "$bundle/share/dbus-1/services/org.freedesktop.FileManager1.service"
test -s "$bundle/share/systemd/user/xdg-desktop-portal-canopa.service"
test -s "$bundle/share/canopa/dependency-attributions.md"
test -s "$bundle/share/canopa-notify/LICENSE-MIT"
test -s "$bundle/share/systemd/user/canopa-notify.service"
"$bundle/bin/canopa-notify" --help > /dev/null
test -s "$bundle/share/canopa-shell/LICENSE"
test -s "$bundle/share/canopa-shell/default.kdl"
test -s "$bundle/share/systemd/user/canopa-shell.service"
test -s "$bundle/share/systemd/user/canopa-session.target"
test -s "$bundle/share/canopa/notifications.md"
test -s "$bundle/share/canopa/audio.md"
test -s "$bundle/share/canopa/shell-rename.md"
PATH="$bundle/bin" canopa-shell --version
python3 - "$bundle/bin/canopa-wifi-provider" "$bundle/bin/canopa-audio-provider" "$bundle/bin/canopa-media-provider" "$bundle/bin/canopa" <<'PY'
import json
import os
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

with tempfile.TemporaryDirectory(prefix='canopa-release-files-config-') as directory:
    config = Path(directory) / 'config.kdl'
    config.write_text('schema-version 1\nfiles { view mode="list"; }\n')
    checked = subprocess.run([sys.argv[4], 'config', 'check', '--file', str(config)],
                             capture_output=True, text=True, timeout=15, check=True)
    reports = [json.loads(line) for line in checked.stdout.splitlines()]
    assert {report['component'] for report in reports} == {'shell', 'notifications', 'files', 'wallpaper', 'lockscreen', 'greeter', 'polkit', 'pinentry'}
    assert all('source' in report for report in reports), checked.stdout

magic = b'NIRIBAR_EXTERNAL_PANEL_JSON_V1\n'
def frame(message):
    body = json.dumps(message).encode()
    return magic + struct.pack('>Q', len(body)) + body

def first_v5(result):
    messages = [json.loads(line) for line in result.stdout.splitlines()]
    assert messages, 'provider returned no initial content'
    assert all(message['protocol_version'] == 5 for message in messages)
    revisions = [message['revision'] for message in messages]
    assert revisions[0] == 1 and all(a < b for a, b in zip(revisions, revisions[1:]))
    return messages[0]

handshake = {'type': 'handshake', 'payload': {'protocol_version': 1,
             'session_id': 'release-smoke', 'provider_name': 'wifi',
             'origin': {'bar_name': 'main', 'output_name': 'test', 'origin_kind': 'bar'}}}
close = {'type': 'event', 'payload': {'type': 'close'}}
result = subprocess.run([sys.argv[1]], input=frame(handshake) + frame(close),
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5,
                        env={'PATH': os.path.dirname(sys.argv[1]),
                             'CANOPA_WIFI_PROVIDER_BACKEND': 'none'}, check=True)
assert result.stdout.startswith(magic)
size = struct.unpack('>Q', result.stdout[len(magic):len(magic)+8])[0]
message = json.loads(result.stdout[len(magic)+8:len(magic)+8+size])
assert message['type'] == 'handshake' and message['protocol_version'] == 1
assert message['content']['elements'][0]['title'] == 'Wi-Fi unavailable'
result = subprocess.run([sys.argv[1], '--protocol', 'canopa-v5'],
                        input=b'{"type":"close","protocol_version":5}\n',
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5,
                        env={'PATH': os.path.dirname(sys.argv[1]),
                             'CANOPA_WIFI_PROVIDER_BACKEND': 'none'}, check=True)
message = first_v5(result)
assert message['protocol_version'] == 5 and message['revision'] == 1
assert message['content']['kind'] == 'form'
assert message['content']['body']['elements'][0]['title'] == 'Wi-Fi unavailable'
result = subprocess.run([sys.argv[2], '--protocol', 'canopa-v5'],
                        input=b'{"type":"close","protocol_version":5}\n',
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5,
                        env={'PATH': os.path.dirname(sys.argv[2]),
                             'PULSE_SERVER': 'unix:/nonexistent-canopa-release-audio'}, check=True)
message = first_v5(result)
assert message['protocol_version'] == 5 and message['revision'] == 1
assert message['content']['kind'] == 'rich'
assert message['content']['body']['title'] == 'Sound'
assert 'unavailable' in message['content']['body']['error']
result = subprocess.run([sys.argv[3], '--protocol', 'canopa-v5'],
                        input=b'{"type":"close","protocol_version":5}\n',
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5,
                        env={'PATH': os.path.dirname(sys.argv[3]),
                             'DBUS_SESSION_BUS_ADDRESS': 'unix:path=/nonexistent-canopa-release-media'}, check=True)
message = first_v5(result)
assert message['protocol_version'] == 5 and message['revision'] == 1
assert message['content']['kind'] == 'rich'
assert message['content']['body']['title'] == 'Media'
print('Release bundle: all bundled binaries execute; Files assets and v1/v5 provider close pass.')
PY
