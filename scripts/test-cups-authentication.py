#!/usr/bin/env python3
"""Check a worker-only CUPS image against a strict private authentication oracle.

Only printer-discovery IPP operations are accepted. No scheduler, printer, Iced
session, system configuration or application credential is used or changed.
"""
import argparse
import errno
import fcntl
import hashlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import resource
import select
import shutil
import socket
import ssl
import stat
import subprocess
import tempfile
import threading
import time
import traceback

ROOT = Path(__file__).resolve().parent.parent
ORACLE = ROOT / 'scripts/desktop-fixture/print-digest-oracle.py'
spec = importlib.util.spec_from_file_location('digest_oracle', ORACLE)
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def repeated(text, size):
    pattern = text.encode()
    return pattern * (size // len(pattern)) + b'x' * (size % len(pattern))


def scenarios():
    return {
        'plain': {},
        'username-quote': dict(user=b'probe"user'),
        'username-backslash': dict(user=b'DOMAIN\\person'),
        'username-combined': dict(user='é, \\"person'.encode()),
        'password-punctuation': dict(password='private, \\"ß:word'.encode()),
        'realm-quote': dict(realm=b'Office "A"'),
        'realm-backslash': dict(realm=b'Office\\Branch'),
        'realm-combined': dict(realm=b'Office \\"A",branch'),
        'nonce-quote': dict(nonce=b'nonce"quoted'),
        'nonce-backslash': dict(nonce=b'nonce\\branch'),
        'opaque-quote': dict(opaque=b'opaque"quoted'),
        'opaque-backslash': dict(opaque=b'opaque\\branch'),
        'maximum-ascii': dict(user=repeated('u\\"', 127), password=repeated('p,:\\"', 1023),
                              realm=repeated('r\\"', 255), nonce=repeated('n\\"', 255), opaque=repeated('o\\"', 255)),
        'maximum-utf8': dict(user=repeated('é\\"', 127), password=repeated('ß,\\"', 1023),
                             realm=repeated('é, \\"', 255), nonce=repeated('n\\"', 255), opaque=repeated('o\\"', 255)),
        'wrong-password': dict(reply='wrong-password'),
        'decline': dict(reply='decline'),
        'wrong-private-id': dict(reply='wrong-id'),
        'unsupported-algorithm': dict(offer='unsupported-algorithm'),
        'unsupported-qop': dict(offer='unsupported-qop'),
        'quoted-fake-basic': dict(offer='unsupported-qop', opaque=b'before\\", Basic realm=\\"foreign'),
    }


def receive(stream, size):
    data = bytearray()
    while len(data) < size:
        part = stream.recv(size - len(data))
        if not part:
            raise EOFError
        data.extend(part)
    return bytes(data)


def credentials(identifier, values):
    frame = bytearray([1])
    frame.extend(identifier.to_bytes(4, 'big'))
    frame.append(0 if values is None else 1)
    if values is not None:
        frame.append(len(values))
        for value in values:
            frame.extend(len(value).to_bytes(2, 'big'))
            frame.extend(value)
    return len(frame).to_bytes(4, 'big') + frame


def ipp_attribute(tag, name, value):
    name, value = name.encode(), value.encode()
    return bytes([tag]) + len(name).to_bytes(2, 'big') + name + len(value).to_bytes(2, 'big') + value


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *_args):
        pass

    def do_POST(self):
        fixture = self.server.fixture
        fixture.observe_mapping()
        assert self.headers.get('Transfer-Encoding') is None
        size = int(self.headers.get('Content-Length', '0'))
        assert 8 <= size <= 1024 * 1024
        body = self.rfile.read(size)
        assert len(body) == size
        operation = int.from_bytes(body[2:4], 'big')
        assert operation in (0x4001, 0x4002), 'Only discovery operations are allowed'
        accepted, row = fixture.verifier.authorize(self.headers.get('Authorization'), self.command, self.path)
        row.update(operation=operation, body_bytes=size,
                   scheme=self.headers.get('Authorization', '').split(' ', 1)[0])
        fixture.case['audit'].append(row)
        if not accepted:
            challenge = fixture.verifier.challenge()
            offer = fixture.scenario.get('offer')
            if offer == 'unsupported-algorithm':
                challenge = challenge.replace('algorithm=' + fixture.algorithm, 'algorithm=SHA-512-256')
                if fixture.algorithm == 'legacy':
                    challenge += ', algorithm=SHA-512-256'
            elif offer == 'unsupported-qop':
                challenge = challenge.replace('qop="auth"', 'qop="auth-int"')
                if fixture.algorithm == 'legacy':
                    challenge += ', qop="auth-int"'
            self.send_response(401)
            self.send_header('WWW-Authenticate', challenge)
            self.send_header('Content-Length', '0')
            self.end_headers()
            return
        reply = (body[:2] + b'\0\0' + body[4:8] + b'\1'
                 + ipp_attribute(0x47, 'attributes-charset', 'utf-8')
                 + ipp_attribute(0x48, 'attributes-natural-language', 'en') + b'\3')
        self.send_response(200)
        self.send_header('Content-Type', 'application/ipp')
        self.send_header('Content-Length', str(len(reply)))
        self.end_headers()
        self.wfile.write(reply)


class Fixture:
    def __init__(self, args, output, temporary):
        self.args, self.output, self.temporary = args, output, temporary
        # One sealed image is reused throughout; only the Rust child loads it.
        fd = os.open(args.library, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
        try:
            info = os.fstat(fd)
            assert stat.S_ISREG(info.st_mode) and 4 <= info.st_size <= 16 * 1024 * 1024
            with os.fdopen(os.dup(fd), 'rb') as stream:
                image = stream.read(16 * 1024 * 1024 + 1)
            assert len(image) == info.st_size and image.startswith(b'\x7fELF')
        finally:
            os.close(fd)
        flags = os.MFD_CLOEXEC | os.MFD_ALLOW_SEALING
        try:
            self.image = os.memfd_create('canopa-print-client', flags | getattr(os, 'MFD_EXEC', 0x10))
        except OSError as error:
            if error.errno != errno.EINVAL:
                raise
            self.image = os.memfd_create('canopa-print-client', flags)
        os.write(self.image, image)
        fcntl.fcntl(self.image, fcntl.F_ADD_SEALS,
                    fcntl.F_SEAL_WRITE | fcntl.F_SEAL_GROW | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_SEAL)
        self.inode = os.fstat(self.image).st_ino
        self.library_sha256 = hashlib.sha256(image).hexdigest()
        if args.expected_library_sha256:
            assert args.expected_library_sha256 == self.library_sha256
        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = True
        self.server.fixture = self
        if args.tls:
            key, cert = temporary / 'key.pem', temporary / 'cert.pem'
            subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1',
                            '-subj', '/CN=localhost', '-addext', 'subjectAltName=DNS:localhost,IP:127.0.0.1',
                            '-keyout', str(key), '-out', str(cert)], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15)
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(cert, key)
            self.server.socket = context.wrap_socket(self.server.socket, server_side=True)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def observe_mapping(self):
        lines = Path(f'/proc/{self.process.pid}/maps').read_text().splitlines()
        selected = [line for line in lines if 'memfd:canopa-print-client' in line or '/libcups.so' in line]
        assert selected and all(int(line.split()[4]) == self.inode for line in selected), 'Unexpected CUPS mapping'
        self.case['selected_image_mapped'] = True

    def run(self, algorithm, name, scenario, number):
        self.algorithm, self.scenario = algorithm, scenario
        password = scenario.get('password', ('private-' + os.urandom(12).hex()).encode())
        user = scenario.get('user', b'probe-user')
        self.verifier = oracle.Verifier(user, password, algorithm, scenario.get('realm', b'Canopa private probe'),
                                       scenario.get('nonce', b'probe-nonce'), scenario.get('opaque', b'probe-opaque'))
        self.case = dict(name=name, algorithm=algorithm, audit=[], challenges=[], selected_image_mapped=False)
        parent, child = socket.socketpair()
        parent.settimeout(5)
        cancel, writer = os.pipe2(os.O_CLOEXEC)
        process, pidfd = None, None
        started = time.monotonic()
        out, err = self.output / f'worker-{number:03}.jsonl', self.output / f'worker-{number:03}.err'
        try:
            environment = dict(os.environ, HOME=str(self.temporary), CUPS_USER='fixture',
                               CUPS_SERVER=f'localhost:{self.server.server_port}')
            environment.pop('LD_LIBRARY_PATH', None)
            environment.pop('CANOPA_PRINT_CUPS_LIBRARY', None)
            if self.args.tls:
                environment['CUPS_ENCRYPTION'] = 'Always'
            else:
                environment.pop('CUPS_ENCRYPTION', None)
            with out.open('w') as output, err.open('w') as errors:
                process = subprocess.Popen([self.args.binary, '--print-worker'], env=environment,
                                           stdin=subprocess.PIPE, stdout=output, stderr=errors,
                                           pass_fds=(self.image, cancel, child.fileno()), start_new_session=True)
                self.process = process
                pidfd = os.pidfd_open(process.pid)
                packet = dict(cancel=cancel, authentication=child.fileno(),
                              cups_runtime=dict(kind='image', descriptor=self.image), operation=dict(operation='printers'))
                process.stdin.write((json.dumps(packet) + '\n').encode())
                process.stdin.close()
                child.close()
                while process.poll() is None:
                    assert time.monotonic() - started < 20, 'Worker exceeded fixture budget'
                    if not select.select([parent], [], [], .05)[0]:
                        continue
                    try:
                        size = int.from_bytes(receive(parent, 4), 'big')
                    except EOFError:
                        break
                    assert 0 < size <= 8192
                    challenge = json.loads(receive(parent, size))
                    assert challenge['kind']['kind'] == 'server' and challenge['kind']['limits'] == 'canopa_v1'
                    assert challenge['endpoint'] == dict(server='localhost', port=self.server.server_port,
                                                          transport='tls' if self.args.tls else 'loopback')
                    assert len(self.case['challenges']) < 3
                    self.case['challenges'].append(dict(id=challenge['id'], attempt=challenge['attempt'],
                        realm_matches=challenge['kind']['realm'].encode() == self.verifier.realm))
                    mode = scenario.get('reply')
                    values = None if mode in ('decline', 'wrong-id') else [user, b'wrong-private-value' if mode == 'wrong-password' else password]
                    parent.sendall(credentials(challenge['id'] + (mode == 'wrong-id'), values))
                assert process.wait(timeout=5) == 0
            frames = [json.loads(line) for line in out.read_text().splitlines()]
            audit, questions = self.case['audit'], self.case['challenges']
            accepted, sent = any(row['accepted'] for row in audit), any(row['present'] for row in audit)
            mode, offer = scenario.get('reply'), scenario.get('offer')
            if mode == 'decline':
                correct = frames[-1]['event'] == 'authentication_cancelled' and len(questions) == 1 and not sent
            elif mode == 'wrong-id':
                correct = frames[-1]['event'] == 'failed' and len(questions) == 1 and not sent
            elif mode == 'wrong-password':
                correct = frames[-1]['event'] == 'failed' and 1 <= len(questions) <= 3 and not accepted
            elif offer:
                correct = frames[-1]['event'] == 'failed' and not sent
            else:
                correct = frames[-1]['event'] == 'printers' and accepted and bool(questions)
            correct &= self.case['selected_image_mapped'] and all(row['realm_matches'] for row in questions)
            self.case.update(status='passed' if correct else 'failed', frames=frames,
                             authorized=accepted, authorization_sent=sent, seconds=time.monotonic() - started)
            assert password not in out.read_bytes() and password not in err.read_bytes()
            return self.case
        finally:
            if process and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            if pidfd is not None:
                self.case['process'] = dict(pid=process.pid, terminal=bool(select.select([pidfd], [], [], 0)[0]))
                os.close(pidfd)
            parent.close()
            child.close()
            os.close(cancel)
            os.close(writer)

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        os.close(self.image)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-test', action='store_true', help='Check published vectors and quoted syntax without native dependencies')
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--library', type=Path)
    parser.add_argument('--expected-library-sha256')
    parser.add_argument('--artifacts', type=Path)
    parser.add_argument('--algorithm', action='append', choices=['Basic', 'MD5', 'SHA-256', 'legacy'])
    parser.add_argument('--case', action='append', choices=list(scenarios()))
    parser.add_argument('--tls', action='store_true', help='Use a disposable loopback TLS endpoint (requires openssl)')
    args = parser.parse_args()
    references = oracle.check_reference()
    if args.self_test:
        print(json.dumps(dict(status='passed', oracle=references)))
        return
    if not all((args.binary, args.library, args.artifacts)):
        parser.error('native checks require --binary, --library and a new --artifacts directory')
    selected = [(algorithm, name, scenario)
                for algorithm in args.algorithm or ['Basic', 'MD5', 'SHA-256', 'legacy']
                for name, scenario in scenarios().items()
                if (not args.case or name in args.case) and not (algorithm == 'Basic' and scenario.get('offer'))]
    if not selected:
        parser.error('the selected authentication cases do not apply to the selected algorithms')
    args.binary, args.library = args.binary.resolve(), args.library.resolve()
    output = args.artifacts.resolve()
    output.mkdir(parents=True, exist_ok=False)
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    result = dict(status='running', oracle=references, binary_sha256=sha(args.binary), cases=[], tls=args.tls,
                  scope='Actual sealed-runtime Rust worker and private strict HTTP/empty-IPP oracle; no scheduler, Iced, PAM, document submission or device acceptance.')
    def save():
        (output / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    for path in [Path(__file__), ORACLE]:
        shutil.copyfile(path, output / path.name)
    fixture = None
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-cups-authentication-') as temporary:
            try:
                fixture = Fixture(args, output, Path(temporary))
                result['library_sha256'] = fixture.library_sha256
                for algorithm, name, scenario in selected:
                    result['cases'].append(fixture.run(algorithm, name, scenario, len(result['cases']) + 1))
                    save()
                result['status'] = 'passed' if all(case['status'] == 'passed' for case in result['cases']) else 'failed'
            finally:
                if fixture:
                    fixture.close()
        result['private_data_removed'] = True
    except BaseException:
        result.update(status='fixture-failed', error=traceback.format_exc())
    finally:
        save()
        print(json.dumps(dict(status=result['status'], cases=len(result['cases']), receipt=str(output / 'summary.json'))))
    if result['status'] != 'passed':
        raise SystemExit(1)


if __name__ == '__main__':
    main()
