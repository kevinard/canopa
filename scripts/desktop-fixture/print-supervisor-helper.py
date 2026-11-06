#!/usr/bin/python3
"""Controlled child messages around a real Rust Client/Broker supervisor."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import select
import socket
import sys
import time

folder = Path(sys.argv[1])
control = json.loads((folder / 'control.json').read_text())
request = json.loads(sys.stdin.buffer.read())
identifier = control['id']
started = time.monotonic()


def mark(role, **values):
    state = Path('/proc/self/stat').read_text()
    payload = dict(pid=os.getpid(), start_time=state[state.rfind(')') + 2:].split()[19],
                   role=role, monotonic=time.monotonic(), **values)
    (folder / f'{identifier}.{role}.json').write_text(json.dumps(payload) + '\n')
    fd = os.open(folder / f'{identifier}.audit.jsonl', os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    try:
        os.write(fd, (json.dumps(payload) + '\n').encode())
    finally:
        os.close(fd)


def receive(channel, size):
    data = bytearray()
    while len(data) < size:
        chunk = channel.recv(size - len(data))
        if not chunk:
            raise EOFError
        data.extend(chunk)
    return bytes(data)


def frame(channel):
    length = int.from_bytes(receive(channel, 4), 'big')
    assert 6 <= length <= 8192
    return receive(channel, length)


def cancelled():
    assert select.select([request['cancel']], [], [], 340)[0]
    assert os.read(request['cancel'], 1) == b''
    mark('cancel-eof', seconds=time.monotonic() - started)


if sys.argv[-1] == '--print-authentication-dialog':
    assert not any('memfd:canopa-print-client' in str(path.readlink())
                   for path in Path('/proc/self/fd').iterdir() if path.exists())
    mark('credential-helper')
    (folder / f'{identifier}.prompt-ready').write_text('ready\n')
    channel = socket.socket(fileno=request['channel'])
    if control['behavior'] == 'prompt-stall':
        time.sleep(95)
        raise AssertionError('the parent must withdraw this helper first')
    if control['behavior'] == 'helper-crash':
        sys.exit(3)
    challenge = request['challenge']
    values = [b'probe-user', b'private-fixture-value']
    body = bytearray([1])
    body.extend(challenge['id'].to_bytes(4, 'big'))
    body.extend([1, len(values)])
    for value in values:
        body.extend(len(value).to_bytes(2, 'big'))
        body.extend(value)
    if control['behavior'] == 'invalid-reply':
        body[4] ^= 1
    channel.sendall(len(body).to_bytes(4, 'big') + body)
    channel.close()
    print('true', flush=True)
elif sys.argv[-1] == '--print-worker':
    assert request['operation']['operation'] == ('recover_printer_jobs' if control['authenticated'] else 'printers')
    assert (request.get('authentication') is not None) == control['authenticated']
    runtime = request['cups_runtime']
    assert runtime['kind'] == 'image'
    image = runtime['descriptor']
    seals = fcntl.fcntl(image, fcntl.F_GET_SEALS)
    required = fcntl.F_SEAL_SEAL | fcntl.F_SEAL_GROW | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_WRITE
    assert seals & required == required
    with open(f'/proc/self/fd/{image}', 'rb') as stream:
        assert hashlib.file_digest(stream, 'sha256').hexdigest() == control['library_sha256']
    mark('worker', authenticated=control['authenticated'], image_sha256=control['library_sha256'])
    behavior = control['behavior']
    if behavior in ['deadline-cooperative', 'deadline-uncooperative']:
        cancelled()
        if behavior == 'deadline-uncooperative':
            time.sleep(45)
            raise AssertionError('the real supervisor must kill this child first')
        print(json.dumps(dict(event='failed', message='controlled deadline stop')), flush=True)
    elif not control['authenticated']:
        assert behavior == 'success'
        print(json.dumps(dict(event='printers', printers=[])), flush=True)
    else:
        channel = socket.socket(fileno=request['authentication'])
        for number in range(1, 3):
            challenge = dict(version=1, interactive=True, id=number, attempt=1,
                             endpoint=dict(server='localhost', port=631, transport='loopback'),
                             kind=dict(kind='server', realm='Controlled scope', resource='/', limits='canopa_v1'),
                             suggested_user='probe-user')
            data = json.dumps(challenge).encode()
            channel.sendall(len(data).to_bytes(4, 'big') + data)
            ready = select.select([channel, request['cancel']], [], [], 15)[0]
            assert ready
            if request['cancel'] in ready:
                assert os.read(request['cancel'], 1) == b''
                mark('cancel-eof', seconds=time.monotonic() - started)
                print(json.dumps(dict(event='authentication_cancelled')), flush=True)
                sys.exit(0)
            try:
                reply = frame(channel)
            except EOFError:
                sys.exit(2)
            expected = bytearray([1])
            expected.extend(number.to_bytes(4, 'big'))
            expected.extend([1, 2])
            for value in [b'probe-user', b'private-fixture-value']:
                expected.extend(len(value).to_bytes(2, 'big'))
                expected.extend(value)
            assert reply == bytes(expected)
            mark(f'credential-exchange-{number}', cache_reply=number == 2)
        print(json.dumps(dict(event='retired_printer_jobs', report=dict(retired=0, unresolved=0, busy=0))), flush=True)
else:
    raise AssertionError(sys.argv[-1])
