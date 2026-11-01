#!/usr/bin/env python3
"""Native Wayland pinentry and real GnuPG acceptance, using only private keys/config."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import time
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('desktop', ROOT / 'scripts/validate-desktop.py')
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


class Peer:
    def __init__(self, binary, session):
        self.buffer = b''
        self.log = (session.output / 'peer-stderr.log').open('ab')
        self.process = subprocess.Popen([str(binary), '--defaults'], env=session.env,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=self.log, bufsize=0, start_new_session=True)
        assert self.read()[0].startswith(b'OK ')

    def send(self, command):
        self.process.stdin.write(command.encode() + b'\n')
        self.process.stdin.flush()

    def read(self, timeout=12):
        result = []
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if b'\n' not in self.buffer:
                if not select.select([self.process.stdout], [], [], max(0, deadline-time.monotonic()))[0]:
                    break
                chunk = os.read(self.process.stdout.fileno(), 4096)
                if not chunk:
                    raise RuntimeError('Pinentry disconnected before completing the transaction')
                self.buffer += chunk
            while b'\n' in self.buffer:
                line, self.buffer = self.buffer.split(b'\n', 1)
                result.append(line)
                if line == b'OK' or line.startswith((b'OK ', b'ERR ')):
                    return result
        raise RuntimeError('Pinentry protocol response timed out')

    def command(self, command):
        self.send(command)
        result = self.read()
        assert result[-1].startswith(b'OK')
        return result

    def close(self):
        try:
            if self.process.poll() is None:
                desktop.stop(self.process)
        finally:
            self.process.stdin.close()
            self.process.stdout.close()
            self.log.close()


def verify(binary, session):
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nwindow-rule { match app-id="org.canopa.Pinentry"; open-floating true; }\n')
    protocol = ROOT / 'scripts/desktop-fixture/virtual-keyboard-unstable-v1.xml'
    for mode, name in [('client-header', 'virtual-keyboard.h'), ('private-code', 'virtual-keyboard.c')]:
        subprocess.run(['wayland-scanner', mode, str(protocol), str(session.output/name)], check=True)
    keyboard = session.output/'keyboard'
    subprocess.run(['cc', '-Wall', '-Wextra', '-Wno-misleading-indentation', '-I', str(session.output),
                    str(ROOT/'scripts/desktop-fixture/keyboard.c'), str(session.output/'virtual-keyboard.c'),
                    '-lwayland-client', '-lxkbcommon', '-o', str(keyboard)], check=True)
    session.start([str(keyboard), 'hold'], 'keyboard.log')
    def windows():
        return [w for w in json.loads(session.run(['niri', 'msg', '--json', 'windows']))
                if w.get('app_id') == 'org.canopa.Pinentry']
    def dialog():
        session.wait_for(windows, 'pinentry dialog')
        session.run(['niri', 'msg', 'action', 'focus-window', '--id', str(windows()[-1]['id'])])
        time.sleep(.25)
    def key(value):
        session.run([str(keyboard), 'key', value])
    def answer(value):
        session.run([str(keyboard), 'text', value])
    def retired():
        session.wait_for(lambda: not windows(), 'pinentry window retirement')
    def code(result):
        assert result[-1].startswith(b'ERR ')
        return int(result[-1].split()[1]) & 65535
    peers = []
    home = session.runtime/'gnupg'
    home.mkdir(mode=0o700)
    session.env['GNUPGHOME'] = str(home)
    try:
        peer = Peer(binary, session)
        peers.append(peer)
        assert not windows()
        peer.command('OPTION ttyname=/must-not-open')
        peer.command('OPTION default-ok=_Unlock')
        peer.command('SETTITLE Test key')
        peer.command('SETDESC OpenPGP key%0AFingerprint: 100%25')
        peer.send('GETPIN')
        dialog()
        session.capture('passphrase.png')
        answer('canopa%fixture')
        assert peer.read() == [b'D canopa%25fixture', b'OK']
        retired()
        peer.command('SETREPEAT Repeat passphrase:')
        peer.send('GETPIN')
        dialog()
        answer('first-attempt')
        time.sleep(.2)
        answer('mismatched-attempt')
        time.sleep(.2)
        session.capture('repeat-error.png')
        answer('matching-pair')
        time.sleep(.2)
        answer('matching-pair')
        assert peer.read() == [b'S PIN_REPEATED', b'D matching-pair', b'OK']
        retired()
        # Repeat applies to the next GETPIN only; fresh requests never retain input.
        peer.send('GETPIN')
        dialog()
        key('Escape')
        assert code(peer.read()) == 99
        retired()
        peer.command('RESET')
        peer.command('SETDESC ' + 'Long request context. '*40)
        peer.send('GETPIN')
        dialog()
        session.run([str(keyboard), 'type', 'discarded'])
        key('Right')
        session.capture('details.png')
        key('Right')
        key('Left')
        key('Return')
        answer('fresh-entry')
        assert peer.read() == [b'D fresh-entry', b'OK']
        retired()
        peer.command('RESET')
        peer.command('SETDESC Allow the requested operation?')
        peer.command('SETNOTOK _No')
        peer.send('CONFIRM')
        dialog()
        session.capture('confirmation.png')
        key('Tab')
        key('Return')
        assert code(peer.read()) == 114
        retired()
        peer.send('CONFIRM --focus=cancel')
        dialog()
        key('Return')
        assert code(peer.read()) == 99
        retired()
        peer.send('CONFIRM')
        dialog()
        key('Return')
        assert peer.read() == [b'OK']
        retired()
        for command in ['MESSAGE', 'CONFIRM --one-button']:
            peer.send(command)
            dialog()
            key('Return')
            assert peer.read() == [b'OK']
            retired()
        peer.command('SETTIMEOUT 1')
        peer.send('GETPIN')
        assert code(peer.read()) == 62
        retired()
        peer.command('SETTIMEOUT 0')
        peer.send('GETPIN')
        dialog()
        session.run([str(keyboard), 'type', 'discarded'])
        peer.send('NOP')  # Queued input must neither spin nor conceal pipe EOF.
        peer.process.stdin.close()
        assert code(peer.read()) == 99
        assert peer.process.wait(timeout=5) == 0
        retired()
        # Actual agent/key derivation/crypto, isolated from the user's agent/keyring.
        (home/'gpg-agent.conf').write_text(f'pinentry-program {binary}\nno-allow-external-cache\n')
        plain = session.runtime/'plain'
        cipher = session.runtime/'cipher.gpg'
        decrypted = session.runtime/'decrypted'
        plain.write_bytes(b'Canopa private pinentry integration fixture\n')
        base = ['gpg', '--homedir', str(home), '--no-options', '--no-symkey-cache']
        with (session.output/'gpg-stderr.log').open('wb') as log:
            process = subprocess.Popen(base+['--symmetric', '--cipher-algo', 'AES256', '--output', str(cipher), str(plain)],
                                       env=session.env, stdout=subprocess.DEVNULL, stderr=log, start_new_session=True)
            try:
                dialog()
                session.capture('gpg-new-passphrase.png')
                answer('canopa-fixture-passphrase')
                time.sleep(.4)
                answer('canopa-fixture-passphrase')
                assert process.wait(timeout=15) == 0
                retired()
            finally:
                if process.poll() is None:
                    desktop.stop(process)
            process = subprocess.Popen(base+['--output', str(decrypted), '--decrypt', str(cipher)],
                                       env=session.env, stdout=subprocess.DEVNULL, stderr=log, start_new_session=True)
            try:
                dialog()
                answer('canopa-fixture-passphrase')
                assert process.wait(timeout=15) == 0
                assert decrypted.read_bytes() == plain.read_bytes()
                retired()
            finally:
                if process.poll() is None:
                    desktop.stop(process)
        logs = (session.output/'peer-stderr.log').read_bytes() + (session.output/'gpg-stderr.log').read_bytes()
        assert all(secret not in logs for secret in [b'canopa%fixture', b'matching-pair', b'fresh-entry', b'canopa-fixture-passphrase'])
        summary = {'status': 'passed', 'checks': ['idle has no window', 'percent escaping',
            'repeat mismatch/retry', 'repeat status', 'fresh request cancellation', 'paged context clears input',
            'negative confirmation', 'cancel default focus', 'positive confirmation', 'message',
            'one-button confirmation', 'timeout', 'peer disconnect', 'real GnuPG symmetric encrypt/decrypt', 'no secrets in diagnostics']}
        (session.output/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
        print(json.dumps(summary, indent=2))
    finally:
        try:
            subprocess.run(['gpgconf', '--homedir', str(home), '--kill', 'gpg-agent'], env=session.env,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10, check=False)
        finally:
            for peer in peers:
                peer.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT/'target/debug/canopa-pinentry')
    parser.add_argument('--output', type=Path, default=ROOT/'.scratch/pinentry/desktop')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='canopa-pinentry-') as temporary:
        root = Path(temporary)
        root.chmod(0o700)
        session = desktop.Session(root, args.output.resolve())
        try:
            verify(args.binary.resolve(), session)
        finally:
            session.close()

if __name__ == '__main__':
    main()
