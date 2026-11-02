#!/usr/bin/env python3
"""Test-only greetd-ipc peer. Never authenticates host accounts or executes sessions."""
import asyncio,json,os,struct,sys,time,shlex
from pathlib import Path
root=Path(sys.argv[1]);events=[];current=None;sequence=0;active_clients=0

def record(kind,**data):
    events.append(dict(event=kind,**data));p=root/'greetd-events.new';p.write_text(json.dumps(events));p.replace(root/'greetd-events.json')
def control():
    try:return json.loads((root/'greetd-control.json').read_text())
    except (OSError,ValueError):return {}
def response(kind='success',**data):return dict(type=kind,**data)
def prompt(text,style='secret'):
    record('prompt',text=text,style=style);return response('auth_message',auth_message_type=style,auth_message=text)
def error(text='Authentication failed',kind='auth_error'):return response('error',error_type=kind,description=text)
def cancel():
    global current,sequence
    sequence+=1
    if current:current['cancel'].set()
    current=None;record('cancel')
async def device(session):
    record('device-wait')
    while not (root/'device-touch').exists() and not session['cancel'].is_set():await asyncio.sleep(.025)
    if session['cancel'].is_set():return response() if session['late'] else error('cancelled')
    session['authenticated']=True;record('device-accepted');return response()
async def handle(request):
    global current,sequence
    kind=request['type']
    if kind=='cancel_session':
        if control().get('cancel-blocked'):await asyncio.sleep(10)
        if control().get('cancel-refused'):return error('cancellation refused','error')
        if current and current.get('worker-exited'):
            cancel();record('failed-worker-cleanup');return error('worker already exited','error')
        cancel();return response()
    if kind=='create_session':
        if current:return error('Existing attempt','error')
        cfg=control();sequence+=1
        current=dict(user=request['username'],mode=cfg.get('mode','otp'),step='password',cancel=asyncio.Event(),late=cfg.get('late-success',False),authenticated=False)
        record('begin',user=current['user'],mode=current['mode'])
        if current['mode']=='oversized':return 'oversized'
        if current['mode']=='disconnect':return 'disconnect'
        if current['mode']=='fingerprint':current['step']='device';return prompt('Scan your fingerprint','info')
        return prompt('Password:')
    if kind=='post_auth_message_response':
        if not current:return error('No attempt','error')
        s=current;value=request.get('response')
        if s['step']=='device':
            assert value is None,'informational message was not acknowledged with null'
            return await device(s)
        if s['step']=='password':
            accepted=value=='canopa-fixture-password';record('answer',accepted=accepted)
            if not accepted:s['worker-exited']=True;return error()
            if s['mode']=='password':s['authenticated']=True;return response()
            if s['mode']=='token':s['step']='device';return prompt('Touch your security key','info')
            s['step']='otp-info';return prompt('Verify your one-time code','error')
        if s['step']=='otp-info':
            assert value is None
            s['step']='otp';return prompt('One-time code:','visible')
        if s['step']=='otp':
            accepted=value=='123456';record('answer',accepted=accepted)
            if not accepted:s['worker-exited']=True;return error()
            s['authenticated']=True;return response()
        return error('Unknown state','error')
    if kind=='start_session':
        assert current and current['authenticated'],'session handoff before authentication'
        if control().get('start-refused'):record('start-refused');return error('Session launch refused','error')
        record('handoff',cmd=shlex.split(' '.join(request['cmd'])),wire_cmd=request['cmd'],env=request['env'],user=current['user']);return response()
    return error('Unknown request','error')
async def client(reader,writer):
    global active_clients
    active_clients+=1;record('client-open',active=active_clients)
    try:
        while True:
            try:header=await reader.readexactly(4)
            except asyncio.IncompleteReadError:return
            length=struct.unpack('=I',header)[0];assert 0<length<=65536
            request=json.loads(await reader.readexactly(length));reply=await handle(request)
            if reply=='disconnect':return
            if reply=='oversized':writer.write(struct.pack('=I',1000000));await writer.drain();return
            payload=json.dumps(reply).encode();writer.write(struct.pack('=I',len(payload))+payload);await writer.drain()
    except (BrokenPipeError,ConnectionResetError):cancel();record('broken-client-cancel')
    finally:
        writer.close()
        try:await writer.wait_closed()
        except (BrokenPipeError,ConnectionResetError):pass
        active_clients-=1;record('client-close',active=active_clients)
async def main():
    socket=root/'greetd.sock';socket.unlink(missing_ok=True)
    server=await asyncio.start_unix_server(client,str(socket));socket.chmod(0o600)
    record('ready')
    async with server:await server.serve_forever()
asyncio.run(main())
