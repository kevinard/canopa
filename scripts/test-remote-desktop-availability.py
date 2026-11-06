#!/usr/bin/env python3
"""Real XDG frontend device-cache invalidation on a private bus; no desktop/input."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]
INPUT = 'org.canopa.Niri.RemoteInput1'
INPUT_PATH = '/org/canopa/Niri/RemoteInput1'
BACKEND = 'org.freedesktop.impl.portal.desktop.canopa'
PUBLIC = 'org.freedesktop.portal.Desktop'
PATH = '/org/freedesktop/portal/desktop'
REMOTE = 'org.freedesktop.portal.RemoteDesktop'
IMPL = 'org.freedesktop.impl.portal.RemoteDesktop'


def digest(path):
    with path.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()


def verify(args,root,out,result):
    from gi.repository import Gio,GLib
    processes=[]; listener=socket.socket(socket.AF_UNIX); stop=threading.Event(); calls=[]
    env={**os.environ,'XDG_RUNTIME_DIR':str(root),'XDG_CONFIG_HOME':str(root/'config'),'XDG_DATA_HOME':str(root/'data'),'XDG_STATE_HOME':str(root/'state'),'XDG_CACHE_HOME':str(root/'cache'),'XDG_CURRENT_DESKTOP':'Canopa','CANOPA_REMOTE_DESKTOP_NATIVE':'1','CANOPA_PORTAL_BINARY':str(args.portal),'NIRI_SOCKET':str(root/'ipc.sock')}
    env.update(DBUS_SYSTEM_BUS_ADDRESS='unix:path='+str(root/'no-system-bus'),GIO_USE_VFS='local',GSETTINGS_BACKEND='memory',GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas')
    for name in ['DISPLAY','WAYLAND_DISPLAY','DBUS_STARTER_ADDRESS','DBUS_STARTER_BUS_TYPE']:env.pop(name,None)
    for name in ['config','data','cache']: (root/name).mkdir()
    portals=root/'portals';portals.mkdir();env['XDG_DESKTOP_PORTAL_DIR']=str(portals);env['XDG_DATA_DIRS']=env['XDG_DATA_HOME']
    (portals/'canopa.portal').write_text('[portal]\nDBusName='+BACKEND+'\nInterfaces='+IMPL+';org.freedesktop.impl.portal.ScreenCast;org.freedesktop.impl.portal.Settings;\nUseIn=Canopa;\n')
    (portals/'portals.conf').write_text('[preferred]\ndefault=none\n'+IMPL+'=canopa\norg.freedesktop.impl.portal.ScreenCast=canopa\norg.freedesktop.impl.portal.Settings=canopa\n')
    bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--print-address=1'],stdout=subprocess.PIPE,stderr=(out/'dbus.log').open('w'),text=True);processes.append(bus);env['DBUS_SESSION_BUS_ADDRESS']=bus.stdout.readline().strip()
    def connection(): return Gio.DBusConnection.new_for_address_sync(env['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
    owner=connection();observer=connection();state={'pointer':1,'registered':True};changes=[]
    def bus_call(c,m,s,v): return c.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus',m,GLib.Variant(s,v),None,Gio.DBusCallFlags.NONE,5000,None).unpack()
    listener.bind(env['NIRI_SOCKET']);listener.listen();listener.settimeout(.2)
    def ipc():
        while not stop.is_set():
            try: peer,_=listener.accept()
            except socket.timeout: continue
            except OSError: break
            with peer:
                peer.settimeout(1)
                try:
                    data=peer.recv(4096)
                    if data:
                        assert data==b'"Version"\n',data
                        peer.sendall(b'{"Ok":{"Version":"availability fixture"}}\n')
                except (OSError,AssertionError): pass
    thread=threading.Thread(target=ipc,daemon=True);thread.start()
    xml='<node><interface name="'+INPUT+'"><property name="Version" type="u" access="read"/><property name="PointerVersion" type="u" access="read"/><property name="MappedPointerVersion" type="u" access="read"/></interface></node>'
    interface=Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0]
    def get_property(c,sender,path,iface,name):
        return GLib.Variant('u',1 if name=='Version' else (2 if name=='MappedPointerVersion' else state['pointer']))
    def method(c,sender,path,iface,name,parameters,invocation):
        calls.append(name);invocation.return_dbus_error('org.freedesktop.DBus.Error.Failed','No input may open in an availability probe')
    registration=owner.register_object(INPUT_PATH,interface,method,get_property,None)
    def incoming(c,message,received):
        if received and message.get_message_type()==Gio.DBusMessageType.METHOD_CALL and message.get_interface()==INPUT:calls.append(message.get_member())
        return message
    owner.add_filter(incoming)
    assert bus_call(owner,'RequestName','(su)',(INPUT,0))[0]==1
    def drain():
        while GLib.MainContext.default().pending(): GLib.MainContext.default().iteration(False)
    def wait(condition,label,timeout=15):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            drain()
            for p in processes:
                assert p.poll() is None,(p.args,p.returncode,label)
            value=condition()
            if value:return value
            time.sleep(.02)
        raise AssertionError('Timed out: '+label)
    def call(c,d,p,i,m,s,v):
        answer={}
        def done(c,r):
            try: answer['value']=c.call_finish(r).unpack()
            except GLib.Error as e: answer['error']=str(e)
        c.call(d,p,i,m,GLib.Variant(s,v),None,Gio.DBusCallFlags.NONE,10000,None,done)
        wait(lambda:answer,'DBus '+m);assert 'error' not in answer,answer
        return answer['value']
    def value(destination,interface): return call(observer,destination,PATH,'org.freedesktop.DBus.Properties','Get','(ss)',(interface,'AvailableDeviceTypes'))[0]
    def passed(name): result['checks'].append(name);(out/'progress.json').write_text(json.dumps(result,indent=2)+'\n');print(name,flush=True)
    def changed(c,s,p,i,n,v):
        iface,props,invalid=v.unpack()
        if iface in [IMPL,REMOTE] and ('AvailableDeviceTypes' in props or 'AvailableDeviceTypes' in invalid): changes.append({'sender':s,'interface':iface,'properties':props,'invalidated':invalid})
    observer.signal_subscribe(None,'org.freedesktop.DBus.Properties','PropertiesChanged',PATH,None,Gio.DBusSignalFlags.NONE,changed)
    def invalidate(): owner.emit_signal(None,INPUT_PATH,'org.freedesktop.DBus.Properties','PropertiesChanged',GLib.Variant('(sa{sv}as)',(INPUT,{},['PointerVersion'])))
    try:
        command = ([str(args.portal), '--defaults'] if args.normal_backend else
                   [str(args.harness),'--exact','screencast::remote::production::tests::native_backend_fixture','--ignored','--nocapture','--test-threads=1'])
        ready = 'component=settings_portal event=ready' if args.normal_backend else 'remote-backend-ready '
        with (out/'backend.log').open('w') as log: backend=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=log,stderr=log,env=env,start_new_session=True)
        processes.append(backend);wait(lambda:ready in (out/'backend.log').read_text(),'backend')
        with (out/'frontend.log').open('w') as log: frontend=subprocess.Popen([str(args.frontend),'--verbose'],stdout=log,stderr=log,env=env,start_new_session=True)
        processes.append(frontend)
        wait(lambda:bus_call(observer,'NameHasOwner','(s)',(PUBLIC,))[0],'frontend')
        assert value(BACKEND,IMPL)==value(PUBLIC,REMOTE)==3;passed('live-owner-initial-mask-reaches-real-frontend')
        for index,expected in [(2,1),(1,3)]:
            state['pointer']=index;invalidate()
            wait(lambda:value(BACKEND,IMPL)==expected,'backend live capability '+str(expected))
            wait(lambda:value(PUBLIC,REMOTE)==expected,'frontend notified capability '+str(expected),timeout=5)
            passed('source-property-change-reaches-frontend-'+str(expected))
        assert bus_call(owner,'ReleaseName','(s)',(INPUT,))[0]==1
        wait(lambda:value(BACKEND,IMPL)==0,'backend owner loss')
        wait(lambda:value(PUBLIC,REMOTE)==0,'frontend owner loss',timeout=5);passed('owner-loss-invalidates-frontend-device-mask')
        replacement=connection();replacement.register_object(INPUT_PATH,interface,method,get_property,None)
        assert bus_call(replacement,'RequestName','(su)',(INPUT,0))[0]==1
        wait(lambda:value(PUBLIC,REMOTE)==3,'replacement owner capability',timeout=5);passed('replacement-owner-is-revalidated-before-frontend-restores-mask')
        assert not calls,calls
        result['changes']=changes;result['input_methods_called']=calls;assert any(x['interface']==REMOTE for x in changes),changes
        replacement.close_sync(None);owner.close_sync(None)
        wait(lambda:value(PUBLIC,REMOTE)==0,'final source loss');passed('probing-and-notifications-never-open-input')
        if args.normal_backend: backend.terminate()
        else: backend.stdin.close()
        assert backend.wait(timeout=20)==0;processes.remove(backend)
    finally:
        stop.set();listener.close();thread.join(timeout=2)
        for p in reversed(processes):
            if p.poll() is None:p.terminate()
            try:p.wait(timeout=5)
            except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
        result['processes']=[{'pid':p.pid,'command':p.args,'exit_code':p.poll()} for p in [*processes,*([backend] if 'backend' in locals() and backend not in processes else [])]]
        if not observer.is_closed():observer.close_sync(None)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--normal-backend', action='store_true', help='Run the normal backend instead of the unit-test fixture')
    for n in ['portal','harness','frontend']:p.add_argument('--'+n,required=n!='harness',type=Path);p.add_argument('--'+n+'-sha256',required=n!='harness')
    p.add_argument('--output',required=True,type=Path);a=p.parse_args()
    if not a.normal_backend and (a.harness is None or a.harness_sha256 is None): p.error('provide --harness and --harness-sha256, or --normal-backend')
    binaries = ['portal', 'frontend'] + ([] if a.normal_backend else ['harness'])
    for n in binaries:
        path=getattr(a,n).resolve();setattr(a,n,path);assert digest(path)==getattr(a,n+'_sha256')
    a.output=a.output.resolve();a.output.mkdir(exist_ok=False);shutil.copy2(__file__,a.output/Path(__file__).name)
    result={'status':'running','normal_backend':a.normal_backend,'scope':'Real standard frontend cached AvailableDeviceTypes with pinned production backend and fake same-process input/IPC provider; no input grant or desktop','checks':[],'fixture_sha256':digest(Path(__file__)),'binaries':{n:{'path':str(getattr(a,n)),'sha256':getattr(a,n+'_sha256')} for n in binaries}};started=time.monotonic();code=1
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-availability-') as directory:verify(a,Path(directory),a.output,result)
        result['status']='passed';code=0
    except Exception as error:result.update(status='failed',error=repr(error),traceback=traceback.format_exc())
    result.update(seconds=round(time.monotonic()-started,2),binaries_unchanged=all(digest(getattr(a,n))==getattr(a,n+'_sha256') for n in binaries),fixture_unchanged=digest(Path(__file__))==result['fixture_sha256'])
    (a.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result.get(k) for k in ['status','seconds','error']}))
    if code:print(result['traceback'])
    raise SystemExit(code)


if __name__=='__main__':main()
