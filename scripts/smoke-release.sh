#!/usr/bin/env bash
set -euo pipefail
archive=${1:?usage: smoke-release.sh ARCHIVE}
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
tar -xzf "$archive" -C "$stage"
bundle="$stage/niribar-x86_64-unknown-linux-gnu"
test -x "$bundle/bin/niribar"
test -x "$bundle/bin/niribar-wifi-provider"
test -s "$bundle/share/niribar/LICENSE"
test -s "$bundle/share/niribar/default.kdl"
test -s "$bundle/share/systemd/user/niribar.service"
PATH="$bundle/bin" niribar --version
python3 - "$bundle/bin/niribar-wifi-provider" <<'PY'
import json
import os
import struct
import subprocess
import sys

magic = b'NIRIBAR_EXTERNAL_PANEL_JSON_V1\n'
def frame(message):
    body = json.dumps(message).encode()
    return magic + struct.pack('>Q', len(body)) + body

handshake = {'type': 'handshake', 'payload': {'protocol_version': 1,
             'session_id': 'release-smoke', 'provider_name': 'wifi',
             'origin': {'bar_name': 'main', 'output_name': 'test', 'origin_kind': 'bar'}}}
close = {'type': 'event', 'payload': {'type': 'close'}}
result = subprocess.run([sys.argv[1]], input=frame(handshake) + frame(close),
                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5,
                        env={'PATH': os.path.dirname(sys.argv[1]),
                             'NIRIBAR_WIFI_PROVIDER_BACKEND': 'none'}, check=True)
assert result.stdout.startswith(magic)
size = struct.unpack('>Q', result.stdout[len(magic):len(magic)+8])[0]
message = json.loads(result.stdout[len(magic)+8:len(magic)+8+size])
assert message['type'] == 'handshake' and message['protocol_version'] == 1
assert message['content']['elements'][0]['title'] == 'Wi-Fi unavailable'
print('Release bundle: both binaries execute; provider handshake and close pass.')
PY
