"""Private Unix-socket CUPS relay for lost-reply/main-death acceptance.

Forward real HTTP/IPP traffic without changing its payload. A single armed fault
can discard a scheduler reply after it was fully received, or hold it until the
driver has killed its own backend. No installed socket or queue is touched.
"""
import contextlib
import socket
import socketserver
import threading
import time


def attributes(body):
    """Read scalar IPP attributes, stopping before document bytes/collections."""
    result = {}
    offset = 8
    previous = None
    while offset < len(body):
        tag = body[offset]
        offset += 1
        if tag == 3:
            break
        if tag < 16:
            continue
        if offset + 2 > len(body):
            break
        length = int.from_bytes(body[offset:offset + 2], 'big')
        offset += 2
        name = body[offset:offset + length].decode('utf-8', 'replace') if length else previous
        offset += length
        if offset + 2 > len(body):
            break
        length = int.from_bytes(body[offset:offset + 2], 'big')
        offset += 2
        value = body[offset:offset + length]
        offset += length
        if offset > len(body):
            break
        previous = name
        if name and tag in (0x21, 0x23) and length == 4:
            result[name] = int.from_bytes(value, 'big', signed=True)
        elif name and tag in (0x41, 0x42, 0x44, 0x45, 0x47, 0x48, 0x49):
            result[name] = value.decode('utf-8', 'replace')
    return result


def header(stream):
    first = stream.readline(8193)
    if not first:
        return None, {}
    if len(first) > 8192 or not first.endswith(b'\r\n'):
        raise ValueError('Invalid HTTP first line')
    lines = [first]
    fields = {}
    for _ in range(128):
        line = stream.readline(8193)
        if len(line) > 8192 or not line.endswith(b'\r\n'):
            raise ValueError('Invalid HTTP header')
        lines.append(line)
        if line == b'\r\n':
            return b''.join(lines), fields
        name, value = line.split(b':', 1)
        fields[name.strip().lower()] = value.strip().lower()
    raise ValueError('Too many HTTP headers')


def body(stream, fields, sink=None, maximum=16 * 1024 * 1024, before_finish=None):
    captured = bytearray()

    def transfer(length):
        if len(captured) + length > maximum:
            raise ValueError('Fixture HTTP body limit')
        while length:
            chunk = stream.read(min(length, 65536))
            if not chunk:
                raise EOFError('Incomplete HTTP body')
            captured.extend(chunk)
            if sink is not None:
                sink.sendall(chunk)
            length -= len(chunk)

    if fields.get(b'transfer-encoding') == b'chunked':
        while True:
            line = stream.readline(128)
            if not line.endswith(b'\r\n'):
                raise ValueError('Invalid HTTP chunk')
            length = int(line.split(b';', 1)[0], 16)
            if not length:
                if before_finish is not None and before_finish(bytes(captured)):
                    raise EOFError('Injected loss before final HTTP request chunk')
                if sink is not None:
                    sink.sendall(line)
                ending = stream.read(2)
                if ending != b'\r\n':
                    raise ValueError('Unexpected HTTP chunk trailer')
                if sink is not None:
                    sink.sendall(ending)
                break
            if sink is not None:
                sink.sendall(line)
            transfer(length)
            ending = stream.read(2)
            if ending != b'\r\n':
                raise ValueError('Invalid HTTP chunk ending')
            if sink is not None:
                sink.sendall(ending)
    else:
        transfer(int(fields.get(b'content-length', b'0')))
    return bytes(captured)


class Relay:
    def __init__(self, path, upstream, response_hook=None):
        self.upstream = str(upstream)
        self.records = []
        self.errors = []
        self.lock = threading.Lock()
        self.fault = None
        self.response_hook = response_hook
        self.sockets = set()
        relay = self

        class Handler(socketserver.StreamRequestHandler):
            def handle(self):
                peer = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                peer.settimeout(15)
                self.request.settimeout(15)
                with relay.lock:
                    relay.sockets.update((peer, self.request))
                try:
                    peer.connect(relay.upstream)
                    with peer.makefile('rb') as upstream_stream:
                        relay.exchange(self.rfile, self.request, peer, upstream_stream)
                except (BrokenPipeError, ConnectionResetError, TimeoutError, EOFError):
                    # Normal when the owned worker dies or abandons a request.
                    pass
                except Exception as error:
                    with relay.lock:
                        relay.errors.append(repr(error))
                finally:
                    with relay.lock:
                        relay.sockets.discard(peer)
                        relay.sockets.discard(self.request)
                    peer.close()

        class Server(socketserver.ThreadingUnixStreamServer):
            daemon_threads = False
            block_on_close = True

        self.server = Server(str(path), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def arm(self, name, operation, hold=False, phase='response', drop=True):
        assert phase in ('response', 'request-end')
        assert drop or phase == 'response'
        fault = dict(name=name, operation=operation, hold=hold, phase=phase, drop=drop, entered=threading.Event(), release=threading.Event())
        with self.lock:
            assert self.fault is None
            self.fault = fault
        return fault

    def exchange(self, client_stream, client, peer, upstream_stream):
        def hold(fault):
            deadline = time.monotonic() + 20
            while not fault['release'].wait(1):
                if time.monotonic() >= deadline:
                    raise TimeoutError('Fault gate was not released')
                # Keep a deliberately held request alive while the driver
                # observes its worker/journal. This is informational HTTP,
                # never the withheld final scheduler acknowledgement.
                client.sendall(b'HTTP/1.1 100 Continue\r\n\r\n')

        while True:
            request_header, request_fields = header(client_stream)
            if request_header is None:
                return
            peer.sendall(request_header)
            if request_fields.get(b'expect') == b'100-continue':
                interim, fields = header(upstream_stream)
                if interim is None:
                    raise EOFError('No HTTP continuation')
                client.sendall(interim)
                if int(interim.split(b' ', 2)[1]) != 100:
                    body(upstream_stream, fields, client)
                    continue
            def before_finish(request):
                operation = int.from_bytes(request[2:4], 'big') if len(request) >= 8 else None
                with self.lock:
                    fault = self.fault if self.fault and self.fault['phase'] == 'request-end' and self.fault['operation'] == operation else None
                    if fault:
                        self.fault = None
                        record = dict(operation=operation, request_id=int.from_bytes(request[4:8], 'big'), request=attributes(request), request_bytes=len(request), response=None, http_status=None, ipp_status=None, response_bytes=0, forwarded=False, lost_reply=fault['name'], phase='request-end', monotonic=time.monotonic())
                        fault['record'] = record
                        self.records.append(record)
                if not fault:
                    return False
                fault['entered'].set()
                if fault['hold']:
                    hold(fault)
                return True

            request = body(client_stream, request_fields, peer, before_finish=before_finish)
            response_header, response_fields = header(upstream_stream)
            if response_header is None:
                raise EOFError('No scheduler reply')
            while int(response_header.split(b' ', 2)[1]) == 100:
                client.sendall(response_header)
                response_header, response_fields = header(upstream_stream)
                if response_header is None:
                    raise EOFError('No final scheduler reply')
            response = body(upstream_stream, response_fields)
            operation = int.from_bytes(request[2:4], 'big') if len(request) >= 8 else None
            record = dict(operation=operation, request_id=int.from_bytes(request[4:8], 'big'), request=attributes(request), response=attributes(response), http_status=int(response_header.split(b' ', 2)[1]), ipp_status=int.from_bytes(response[2:4], 'big') if len(response) >= 8 else None, response_bytes=len(response), forwarded=False, monotonic=time.monotonic())
            with self.lock:
                fault = self.fault if self.fault and self.fault['phase'] == 'response' and self.fault['operation'] == operation else None
                if fault:
                    self.fault = None
                    record['lost_reply' if fault['drop'] else 'held_reply'] = fault['name']
                    fault['record'] = record
                self.records.append(record)
            if self.response_hook is not None:
                response_header, response_fields = self.response_hook(record, response_header, response_fields)
            if fault:
                fault['entered'].set()
                if fault['hold']:
                    hold(fault)
                if fault['drop']:
                    return
            # Preserve HTTP body semantics when forwarding chunked replies.
            if response_fields.get(b'transfer-encoding') == b'chunked':
                client.sendall(response_header + f'{len(response):x}\r\n'.encode() + response + b'\r\n0\r\n\r\n')
            else:
                client.sendall(response_header + response)
            record['forwarded'] = True
            if response_fields.get(b'connection') == b'close':
                return

    def close(self):
        self.server.shutdown()
        with self.lock:
            sockets = list(self.sockets)
        for connection in sockets:
            with contextlib.suppress(OSError):
                connection.shutdown(socket.SHUT_RDWR)
        self.server.server_close()
        self.thread.join(timeout=5)
        assert not self.thread.is_alive()
