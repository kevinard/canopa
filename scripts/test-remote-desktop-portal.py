#!/usr/bin/env python3
"""Unadvertised keyboard EIS sessions with real Iced consent on a private desktop.
Legacy Notify, pointer/touch and distribution acceptance remain separate work.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time

ROOT=Path(__file__).resolve().parents[1]
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py')
desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)
BUS='org.freedesktop.impl.portal.desktop.canopa';PATH='/org/freedesktop/portal/desktop'
REMOTE='org.freedesktop.impl.portal.RemoteDesktop';SESSION='org.freedesktop.impl.portal.Session'
APP='org.canopa.RemoteDesktopFixture'

def digest(path):
    with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()

def identity(pid):
    try:
        p=Path('/proc')/str(pid);fields=(p/'stat').read_text().rsplit(') ',1)[1].split()
        return dict(state=fields[0],ppid=int(fields[1]),start_ticks=fields[19],command=(p/'cmdline').read_bytes().replace(b'\0',b' ').decode())
    except FileNotFoundError:return None

def verify(args,session,result):
    from gi.repository import Gio,GLib
    config=session.runtime/'canopa.kdl'
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.scheme}"; }}\n')
    session.env.update(CANOPA_PORTAL_CONFIG=str(config),PATH=str(args.niri.parent)+os.pathsep+session.env['PATH'],
        CANOPA_REMOTE_DESKTOP_NATIVE='1',CANOPA_PORTAL_BINARY=str(args.portal),
        GSETTINGS_BACKEND='memory')
    session.env.pop('REIS_DEBUG',None)
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "fr"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
layout { border { off; }; focus-ring { off; }; }
''',scale=args.scale)
    for mode,suffix in [('client-header','.h'),('private-code','.c')]:
        subprocess.run(['wayland-scanner',mode,'/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml',str(session.output/('xdg-shell'+suffix))],check=True,timeout=15)
    receiver=session.output/'receiver';client=session.output/'ei-client'
    subprocess.run(['cc','-Wall','-Wextra','-Werror','-I',str(session.output),str(ROOT/'scripts/desktop-fixture/remote-input-receiver.c'),str(session.output/'xdg-shell.c'),'-lwayland-client','-lxkbcommon','-o',str(receiver)],check=True,timeout=30)
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','libei-1.0','xkbcommon'],text=True).split()
    subprocess.run(['cc','-Wall','-Wextra','-Werror',str(ROOT/'scripts/desktop-fixture/remote-input-ei-client.c'),'-o',str(client),*flags],check=True,timeout=30)
    receiver_process=session.start([str(receiver)],'receiver.log');receiver_log=session.output/'receiver.log'
    session.wait_for(lambda:'focused' in receiver_log.read_text(),'harmless receiver')
    pointer=session.build_pointer()
    def windows():return json.loads(session.run(['niri','msg','--json','windows']))
    receiver_window=session.wait_for(lambda:next((w for w in windows() if w['pid']==receiver_process.pid),None),'receiver window')
    def connect():return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
    def drain():
        while GLib.MainContext.default().pending():GLib.MainContext.default().iteration(False)
    def call(connection,path,interface,method,signature,values):
        return connection.call_sync(BUS,path,interface,method,GLib.Variant(signature,values),None,Gio.DBusCallFlags.NONE,10000,None).unpack()
    def finish(process,status=0):
        if process in session.processes:session.processes.remove(process)
        actual=process.wait(timeout=10);assert actual==status,(process.args,actual,status)
    count=0
    for case in ['cancel','ui-stop','session-close','client-loss','caller-loss','backend-kill']:
        caller=connect();log=session.output/(case+'-backend.log')
        with log.open('w') as output:
            backend=subprocess.Popen([str(args.harness),'--exact','screencast::remote::production::tests::native_backend_fixture','--ignored','--nocapture','--test-threads=1'],stdin=subprocess.PIPE,stdout=output,stderr=output,env=session.env,start_new_session=True)
        session.processes.append(backend)
        session.wait_for(lambda:'remote-backend-ready ' in log.read_text(),'private real production backend')
        info=json.loads(next(line.split('remote-backend-ready ',1)[1] for line in log.read_text().splitlines() if 'remote-backend-ready ' in line))
        assert info['pid']==backend.pid
        count+=1;key=f'/org/freedesktop/portal/desktop/session/remote_native/s{count}'
        handle=f'/org/freedesktop/portal/desktop/request/remote_native/create{count}'
        assert call(caller,PATH,REMOTE,'CreateSession','(oosa{sv})',(handle,key,APP,{}))[0]==0
        handle=f'/org/freedesktop/portal/desktop/request/remote_native/devices{count}'
        assert call(caller,PATH,REMOTE,'SelectDevices','(oosa{sv})',(handle,key,APP,{'types':GLib.Variant('u',1),'persist_mode':GLib.Variant('u',2)}))[0]==0
        response={};closed=[]
        subscription=caller.signal_subscribe(BUS,SESSION,'Closed',key,None,Gio.DBusSignalFlags.NONE,lambda *a:closed.append(True))
        handle=f'/org/freedesktop/portal/desktop/request/remote_native/start{count}'
        def ready(connection,value,*_):
            try:response['value']=connection.call_finish(value).unpack()
            except GLib.Error as error:response['error']=str(error)
        caller.call(BUS,PATH,REMOTE,'Start',GLib.Variant('(oossa{sv})',(handle,key,APP,'',{})),None,Gio.DBusCallFlags.NONE,40000,None,ready,None)
        def popup():
            drain();assert 'error' not in response,response
            assert 'value' not in response,('Start returned without fresh consent',response)
            return next((w for w in windows() if w['app_id']=='org.canopa.Portal' and w['title']=='Allow keyboard control — Canopa'),None)
        dialog=session.wait_for(popup,'fresh Iced keyboard approval',timeout=30)
        if not dialog['is_floating']:session.run(['niri','msg','action','move-window-to-floating','--id',str(dialog['id'])])
        for dimension,value in [('width',520),('height',340)]:session.run(['niri','msg','action','set-window-'+dimension,'--id',str(dialog['id']),str(value)])
        session.run(['niri','msg','action','move-floating-window','--id',str(dialog['id']),'--x','20','--y','10'])
        session.run(['niri','msg','action','focus-window','--id',str(dialog['id'])]);time.sleep(.2)
        session.run(['wtype','-M','ctrl','-k','Return','-m','ctrl']);drain();assert not response
        session.capture(case+'-fresh-consent.png')
        if case=='cancel':
            call(caller,handle,'org.freedesktop.impl.portal.Request','Close','()',())
            session.wait_for(lambda:(drain(),response.get('value'))[1],'canceled Start')
            assert response['value'][0]==1,response
            session.wait_for(lambda:not any(w['id']==dialog['id'] for w in windows()),'canceled Iced dialog gone')
            result['checks'].append('fresh-unchecked-consent-and-request-cancel')
        else:
            outputs=json.loads(session.run(['niri','msg','--json','outputs']));logical=next(v['logical'] for v in outputs.values() if v.get('logical'))
            session.run([str(pointer),str(round(280/logical['width']*1600)),str(round(130/logical['height']*1000))])
            session.capture(case+'-approved-consent.png')
            session.run(['wtype','-M','ctrl','-k','Return','-m','ctrl'])
            session.wait_for(lambda:(drain(),response.get('value') or response.get('error'))[1],'approved Start',timeout=40)
            assert 'error' not in response,response
            status,values=response['value'];assert status==0 and values['devices']==1 and not values['clipboard_enabled'],response
            assert values['persist_mode']==0 and 'streams' not in values,response
            controls=session.wait_for(lambda:next((w for w in windows() if w['app_id']=='org.canopa.Portal' and w['title']=='Remote control — Canopa'),None),'ongoing keyboard control surface')
            session.capture(case+'-controls.png')
            session.run(['niri','msg','action','focus-window','--id',str(receiver_window['id'])])
            before=len(receiver_log.read_text().splitlines())
            value,fdlist=caller.call_with_unix_fd_list_sync(BUS,PATH,REMOTE,'ConnectToEIS',GLib.Variant('(osa{sv})',(key,APP,{})),None,Gio.DBusCallFlags.NONE,40000,None,None)
            fds=fdlist.steal_fds();assert value.unpack()==(0,) and len(fds)==1
            ei=socket.socket(fileno=fds[0]);worker_pid,uid,_=struct.unpack('3i',ei.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12));owned=identity(worker_pid)
            assert uid==os.getuid() and owned and owned['ppid']==backend.pid and str(args.portal) in owned['command'] and '--remote-input-worker' in owned['command']
            client_log=session.output/(case+'-client.log')
            with client_log.open('w') as output:remote=subprocess.Popen([str(client),'held'],stdin=ei,stdout=output,stderr=output,env=session.env,start_new_session=True)
            ei.close();fdlist=None;value=None;session.processes.append(remote)
            def received(fragment):return any(fragment in line for line in receiver_log.read_text().splitlines()[before:])
            session.wait_for(lambda:received('key 16 1 A'),'actual French shifted A')
            session.wait_for(lambda:'modifier update' in client_log.read_text(),'authoritative EI modifier feedback')
            session.processes.remove(remote) # Every following action intentionally ends this client.
            if case=='ui-stop':session.run(['niri','msg','action','close-window','--id',str(controls['id'])])
            elif case=='session-close':call(caller,key,SESSION,'Close','()',())
            elif case=='client-loss':remote.kill()
            elif case=='caller-loss':caller.close_sync(None)
            else:
                session.processes.remove(backend);backend.kill();finish(backend,-signal.SIGKILL)
            finish(remote,-signal.SIGKILL if case=='client-loss' else 0)
            session.wait_for(lambda:received('key 42 0 Shift_L') and received('mods 0 '),'held key released and modifiers cleared')
            session.wait_for(lambda:not any(w['id']==controls['id'] for w in windows()),'control surface retired')
            session.wait_for(lambda:(now:=identity(worker_pid)) is None or now['start_ticks']!=owned['start_ticks'] or now['state']=='Z','owned input worker terminal')
            if case in ['ui-stop','client-loss']:session.wait_for(lambda:(drain(),bool(closed))[1],'session Closed after control/input loss')
            result['checks'].append(case+'-real-iced-eis-session-lifetime')
            result.setdefault('workers',[]).append({'pid':worker_pid,'start_ticks':owned['start_ticks'],'terminal':True})
        if backend.poll() is None:
            session.processes.remove(backend);backend.stdin.close();finish(backend)
        elif case!='backend-kill':
            raise AssertionError(('backend exited unexpectedly',backend.returncode))
        if not caller.is_closed():caller.signal_unsubscribe(subscription);caller.close_sync(None)
        (session.output/'progress.json').write_text(json.dumps(result,indent=2)+'\n');print(case+': passed',flush=True)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['niri','portal','harness']:
        parser.add_argument('--'+name,required=True,type=Path);parser.add_argument('--'+name+'-sha256',required=True)
    parser.add_argument('--scheme',choices=('dark','light'),default='dark')
    parser.add_argument('--scale',type=float,default=1.0)
    parser.add_argument('--output',required=True,type=Path);args=parser.parse_args()
    for name in ['niri','portal','harness']:
        path=getattr(args,name).resolve();setattr(args,name,path);assert digest(path)==getattr(args,name+'_sha256')
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    sources=[Path(__file__),ROOT/'scripts/validate-desktop.py',ROOT/'scripts/desktop-fixture/remote-input-ei-client.c',ROOT/'scripts/desktop-fixture/remote-input-receiver.c']
    result={'status':'running','scheme':args.scheme,'scale':args.scale,'scope':'Private unadvertised keyboard EIS production session, real Iced consent/controls; not legacy, pointer/touch, shared-media or frontend acceptance','checks':[],
        'binaries':{name:{'path':str(getattr(args,name)),'sha256':getattr(args,name+'_sha256')} for name in ['niri','portal','harness']},'fixture_sources':[{'path':str(p.relative_to(ROOT)),'sha256':digest(p)} for p in sources]}
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-remote-session-') as directory:
            session=desktop.Session(Path(directory),args.output)
            try:verify(args,session,result)
            finally:session.close()
        for entry in result['binaries'].values():assert digest(Path(entry['path']))==entry['sha256']
        result['changed_fixture_sources']=[entry['path'] for entry in result['fixture_sources'] if digest(ROOT/entry['path'])!=entry['sha256']]
        assert not result['changed_fixture_sources']
        result.update(status='passed',binaries_unchanged=True)
    except Exception as error:result.update(status='failed',error=str(error));raise
    finally:(args.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))
if __name__=='__main__':main()
