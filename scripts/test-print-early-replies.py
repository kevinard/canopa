#!/usr/bin/env python3
"""Check public CUPS write results with controlled pending TLS rejections.

This loads the explicitly selected client in disposable Python children, sends
synthetic bytes to loopback TLS peers and records only public statuses. It uses
no document, scheduler, password, Iced or system configuration. A complete reply
sent by a peer must not be confused with a rejection observed by the client.
"""
import argparse
import ctypes as C
import hashlib
import json
import os
import resource
from pathlib import Path
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import traceback

sys.dont_write_bytecode = True
resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def client(args):
    cups = C.CDLL(str(args.library.resolve()))
    def api(name, result, *parameters):
        fn = getattr(cups, name)
        fn.restype, fn.argtypes = result, parameters
        return fn
    pointer, integer, string = C.c_void_p, C.c_int, C.c_char_p
    connect = api('httpConnect2', pointer, string, integer, pointer, integer, integer, integer, integer, pointer)
    close = api('httpClose', None, pointer)
    expect = api('httpSetExpect', None, pointer, integer)
    length = api('httpSetLength', None, pointer, C.c_size_t)
    field = api('httpSetField', None, pointer, integer, string)
    post = api('httpPost', integer, pointer, string)
    wait = api('httpWait', integer, pointer, integer)
    write = api('cupsWriteRequestData', integer, pointer, string, C.c_size_t)
    status = api('httpGetStatus', integer, pointer)
    get = api('httpGetField', string, pointer, integer)
    http = connect(b'localhost', args.port, None, socket.AF_INET, 3, 1, 2000, None)
    assert http, 'TLS connection failed'
    try:
        if args.operation == 'start':
            # The same public request API called by cupsStartDestDocument.
            callbacks = []
            callback_type = C.CFUNCTYPE(pointer, string, pointer, string, string, pointer)
            @callback_type
            def refuse_password(_prompt, _http, _method, _resource, _data):
                callbacks.append(True)
                return None
            api('cupsSetUser', None, string)(b'canopa_transport_probe')
            api('cupsSetPasswordCB2', None, callback_type, pointer)(refuse_password, None)
            request = api('ippNewRequest', pointer, integer)(6)
            assert request
            try:
                returned = api('cupsSendRequest', integer, pointer, pointer, string, C.c_size_t)(
                    http, request, b'/printers/transport-probe', 0)
                print(json.dumps(dict(request_status=returned, password_callbacks=len(callbacks),
                                      final_status=status(http), challenge_present=bool(get(http, 26)))))
            finally:
                api('ippDelete', None, pointer)(request)
            return
        expect(http, 100)
        length(http, 0)
        field(http, 10, b'application/ipp')
        assert post(http, b'/printers/transport-probe') == 0
        assert os.read(args.gate, 1) == b'G'
        readable = wait(http, 1000)
        # Synthetic bytes only; no document, scheduler or credential is involved.
        data = b'x' * (8 * 1024 * 1024)
        written = write(http, data, len(data))
        print(json.dumps(dict(readable_before_write=readable, write_status=written,
                              final_status=status(http), challenge_present=bool(get(http, 26)))))
    finally:
        close(http)


def main(args):
    assert args.receipt is not None, '--receipt is required for the controller'
    assert args.library.is_file()
    result = dict(status='running', scope=__doc__, library=str(args.library.resolve()),
                  library_sha256=sha(args.library), source_sha256=sha(Path(__file__)), cases=[])
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-early-tls-') as directory:
            base = Path(directory)
            key, cert = base / 'key.pem', base / 'cert.pem'
            subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1',
                            '-subj', '/CN=localhost', '-addext', 'subjectAltName=DNS:localhost',
                            '-keyout', str(key), '-out', str(cert)], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15)
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(cert, key)
            cases = [('write', mode) for mode in ['full-drained', 'full-closed', 'prefix-closed', 'missing-closed']]
            cases += [('start', mode) for mode in ['full-drained', 'status-closed', 'prefix-closed', 'missing-closed']]
            for operation, mode in cases:
                row = dict(operation=operation, mode=mode)
                result['cases'].append(row)
                ready = threading.Event()
                read, write = os.pipe2(os.O_CLOEXEC)
                with socket.socket() as listener:
                    listener.bind(('127.0.0.1', 0)); listener.listen(1); listener.settimeout(5)
                    def serve():
                        try:
                            raw, _address = listener.accept()
                            with raw, context.wrap_socket(raw, server_side=True) as connection:
                                connection.settimeout(5)
                                header = bytearray()
                                while b'\r\n\r\n' not in header:
                                    data = connection.recv(1)
                                    assert data and len(header) < 16384
                                    header.extend(data)
                                assert b'Transfer-Encoding: chunked' in header
                                if operation == 'start':
                                    # Drain the initial IPP chunk before publishing the
                                    # rejection: no failed request write can hide it.
                                    with connection.makefile('rb') as request:
                                        line = request.readline(128)
                                        assert line.endswith(b'\r\n') and len(line) < 128
                                        size = int(line.strip(), 16)
                                        assert 0 < size < 8192
                                        assert len(request.read(size)) == size
                                        assert request.read(2) == b'\r\n'
                                        row['initial_request_chunk_bytes'] = size
                                response = b'HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm="Transport fixture"\r\n'
                                if mode == 'status-closed':
                                    response = b'HTTP/1.1 401 Unauthorized\r\n'
                                if mode.startswith('full'):
                                    response += b'Content-Length: 0\r\nConnection: close\r\n\r\n'
                                if mode != 'missing-closed':
                                    connection.sendall(response)
                                row['response_bytes_sent'] = len(response) if mode != 'missing-closed' else 0
                                row['complete_response_sent'] = mode.startswith('full')
                                if mode == 'full-drained':
                                    ready.set()
                                    count = 0
                                    while data := connection.recv(65536):
                                        count += len(data)
                                        assert count < 16 * 1024 * 1024
                                    row['drained_request_bytes'] = count
                            ready.set()
                        except BaseException:
                            row['server_error'] = traceback.format_exc()
                            ready.set()
                    thread = threading.Thread(target=serve, daemon=True)
                    thread.start()
                    process = subprocess.Popen([sys.executable, __file__, '--library', str(args.library.resolve()),
                                                '--port', str(listener.getsockname()[1]), '--gate', str(read),
                                                '--operation', operation],
                                               pass_fds=(read,), stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                               env=dict(os.environ, HOME=str(base), PYTHONDONTWRITEBYTECODE='1'))
                    row['pid'] = process.pid
                    row['start_ticks'] = int(Path(f'/proc/{process.pid}/stat').read_text().rsplit(')', 1)[1].split()[19])
                    try:
                        assert ready.wait(5), 'Server did not publish the controlled reply'
                        if operation == 'write':
                            os.write(write, b'G')
                        output, errors = process.communicate(timeout=3)
                        row.update(exit_code=process.returncode, stderr=errors.decode())
                        assert process.returncode == 0, row
                        row['client'] = json.loads(output)
                        assert not errors
                    finally:
                        if process.poll() is None:
                            process.kill(); process.communicate()
                        row['terminal'] = process.poll() is not None
                        os.close(read); os.close(write)
                        thread.join(timeout=6)
                        row['server_terminal'] = not thread.is_alive()
                    assert row['server_terminal'] and 'server_error' not in row, row
                    if operation == 'start':
                        expected = 1000 if mode == 'full-drained' else -1
                        assert row['client']['request_status'] == expected, row
                        assert row['client']['password_callbacks'] == (1 if mode == 'full-drained' else 0), row
                    else:
                        expected = 401 if mode == 'full-drained' else -1
                        assert row['client']['write_status'] == expected, row
                        assert row['client']['readable_before_write'] == 1, row
                        assert row['client']['challenge_present'] == (mode == 'full-drained'), row
                        assert row['client']['final_status'] == (401 if mode == 'full-drained' else 100), row
                        if mode == 'full-drained':
                            assert row['drained_request_bytes'] >= 8 * 1024 * 1024, row
            result['status'] = 'passed'
        result['private_tls_data_removed'] = True
        assert sha(args.library) == result['library_sha256'], 'Library changed during the check'
    except BaseException:
        result.update(status='failed', error=traceback.format_exc())
        result['private_tls_data_removed'] = 'base' not in locals() or not base.exists()
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(status=result['status'], cases=len(result['cases']), receipt=str(args.receipt), error=result.get('error'))))
    return 0 if result['status'] == 'passed' else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library', required=True, type=Path)
    parser.add_argument('--receipt', type=Path)
    parser.add_argument('--port', type=int)
    parser.add_argument('--gate', type=int)
    parser.add_argument('--operation', choices=['write', 'start'], default='write')
    arguments = parser.parse_args()
    if arguments.port:
        client(arguments)
    else:
        raise SystemExit(main(arguments))
