#!/usr/bin/python3
"""Retain one CLOEXEC setup-stream copy around the actual normal file worker."""
from pathlib import Path
import fcntl
import json
import os
import sys
import time

root = Path(os.environ['CANOPA_HANDOFF_ROOT'])
binary = os.environ['CANOPA_HANDOFF_REAL']
raw = sys.stdin.buffer.readline()
request = json.loads(raw)
operation = request['operation']


def identity(pid):
    state = Path(f'/proc/{pid}/stat').read_text()
    return dict(pid=pid, start_time=state[state.rfind(')') + 2:].split()[19])


def mark(name, value):
    (root / name).write_text(json.dumps(value) + '\n')


mark(f'{os.getpid()}-worker.json', dict(**identity(os.getpid()), role='actual worker via exec',
                                      operation=operation['operation']))
if operation['operation'] == 'prepare_file':
    channel = operation['channel']
    fcntl.fcntl(channel, fcntl.F_SETFD, fcntl.FD_CLOEXEC)
    child = os.fork()
    if child == 0:
        mark(f'{os.getpid()}-holder.json', dict(**identity(os.getpid()), role='controlled fork copy',
             cloexec=bool(fcntl.fcntl(channel, fcntl.F_GETFD) & fcntl.FD_CLOEXEC),
             socket_inode=os.fstat(channel).st_ino))
        for path in Path('/proc/self/fd').iterdir():
            descriptor = int(path.name)
            if descriptor != channel:
                try:
                    os.close(descriptor)
                except OSError:
                    pass
        deadline = time.monotonic() + 90
        while not (root / 'release').exists() and time.monotonic() < deadline:
            time.sleep(.01)
        os.close(channel)
        os._exit(0)
    mark(f'{child}-owned.json', dict(**identity(child), role='controlled fork copy', cloexec=True))
    if os.environ['CANOPA_HANDOFF_COPY'] == 'closed':
        assert (root / 'release').exists()
        os.waitpid(child, 0)
    # Restore only this process's descriptor flag for the intentional exec.
    # The forked copy keeps CLOEXEC and remains open until the controller releases it.
    os.set_inheritable(channel, True)

input_fd = os.memfd_create('canopa-handoff-input', os.MFD_CLOEXEC)
os.write(input_fd, raw)
os.lseek(input_fd, 0, os.SEEK_SET)
os.dup2(input_fd, 0)
os.close(input_fd)
os.execv(binary, [binary, '--print-worker'])
