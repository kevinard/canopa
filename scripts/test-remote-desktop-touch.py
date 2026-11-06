#!/usr/bin/env python3
"""Private public-portal/Iced/Notify/libei touch acceptance, without live routing changes."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import traceback

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from screencast_media import MediaSession,digest
_spec=importlib.util.spec_from_file_location('touch_lease',ROOT/'scripts/test-remote-touch-lease.py')
lease_fixture=importlib.util.module_from_spec(_spec);_spec.loader.exec_module(lease_fixture)
FRONTEND='org.freedesktop.portal.Desktop'
BACKEND='org.freedesktop.impl.portal.desktop.canopa'
PATH='/org/freedesktop/portal/desktop'
REMOTE='org.freedesktop.portal.RemoteDesktop'
CAST='org.freedesktop.portal.ScreenCast'
TOOLS=ROOT/'target/native-test-tools/remote-touch'


def clients():
    receiver=lease_fixture.receiver()
    source=ROOT/'scripts/desktop-fixture/remote-input-ei-touch.c';binary=TOOLS/'ei-touch'
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','libei-1.0'],text=True).split()
    inputs=dict(source=digest(source),flags=flags,compiler=subprocess.check_output(['cc','--version'],text=True).splitlines()[0])
    receipt=TOOLS/'ei-touch-build.json'
    old=json.loads(receipt.read_text()) if receipt.exists() else {}
    if not binary.exists() or old.get('inputs')!=inputs or old.get('sha256')!=digest(binary):
        command=['cc','-Wall','-Wextra','-Werror',str(source),'-o',str(binary),*flags]
        subprocess.run(command,check=True)
        receipt.write_text(json.dumps(dict(inputs=inputs,command=command,sha256=digest(binary),bytes=binary.stat().st_size),indent=2)+'\n')
    return receiver,binary


def verify(args,session,result):
    from gi.repository import Gio,GLib
    out=session.output;config=session.runtime/'canopa.kdl'
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.scheme}"; }}\n')
    portals=session.runtime/'portals';portals.mkdir()
    names=['org.freedesktop.impl.portal.'+n for n in ['RemoteDesktop','ScreenCast','Settings']]
    (portals/'canopa.portal').write_text('[portal]\nDBusName='+BACKEND+'\nInterfaces='+';'.join(names)+';\nUseIn=Canopa;\n')
    (portals/'portals.conf').write_text('[preferred]\ndefault=none\n'+''.join(n+'=canopa\n' for n in names))
    applications=Path(session.env['XDG_DATA_HOME'])/'applications';applications.mkdir(parents=True)
    (applications/'org.canopa.RemoteDesktopFixture.desktop').write_text('[Desktop Entry]\nType=Application\nName=Remote desktop fixture\nExec=true\n')
    home=session.runtime/'home';home.mkdir()
    session.env.update(CANOPA_PORTAL_CONFIG=str(config),CANOPA_PORTAL_BINARY=str(args.portal),
        XDG_CURRENT_DESKTOP='Canopa',XDG_DESKTOP_PORTAL_DIR=str(portals),XDG_DATA_DIRS=session.env['XDG_DATA_HOME'],
        HOME=str(home),GIO_USE_VFS='local',GSETTINGS_BACKEND='memory',GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas')
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
layout { border { off; }; focus-ring { off; }; }
''',scale=args.scale)
    receiver=session.start([str(args.receiver)],'receiver.log');log=out/'receiver.log'
    def lines():
        assert receiver.poll() is None
        return log.read_text().splitlines() if log.exists() else []
    session.wait_for(lambda:any(v.startswith('configured ') for v in lines()),'touch receiver mapped')
    def windows():return json.loads(session.run(['niri','msg','--json','windows']))
    def action(*values):return session.run(['niri','msg','action',*map(str,values)])
    def keys(*values):return session.run(['wtype',*values])
    wid=session.wait_for(lambda:next((w['id'] for w in windows() if w['pid']==receiver.pid),None),'receiver window')
    action('move-window-to-floating','--id',wid)
    action('set-window-width','--id',wid,400);action('set-window-height','--id',wid,300)
    action('move-floating-window','--id',wid,'--x',100,'--y',100)
    session.wait_for(lambda:'configured 400 300' in lines(),'fixed receiver geometry')
    def connection():return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
    def call(c,d,p,i,m,s='()',v=()):return c.call_sync(d,p,i,m,GLib.Variant(s,v),None,Gio.DBusCallFlags.NONE,10000,None).unpack()
    observer=connection();responses={};closed=set();token=0
    call(observer,'org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','UpdateActivationEnvironment','(a{ss})',({**session.env},))
    backend=session.start([str(args.portal),'--canopa-config',str(config)],'backend.log')
    session.wait_for(lambda:'component=settings_portal event=ready' in (out/'backend.log').read_text(),'normal portal backend')
    session.start([str(args.frontend),'--verbose'],'frontend.log')
    session.wait_for(lambda:call(observer,'org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','NameHasOwner','(s)',(FRONTEND,))[0],'public frontend')
    props=call(observer,FRONTEND,PATH,'org.freedesktop.DBus.Properties','GetAll','(s)',(REMOTE,))[0]
    assert props['AvailableDeviceTypes']==7 and props['version']==2,props
    result['properties']=props
    def drain():
        while GLib.MainContext.default().pending():GLib.MainContext.default().iteration(False)
    def options(**extra):
        nonlocal token
        token+=1;return {'handle_token':GLib.Variant('s',f'touch{token}'),**extra}
    def response(path):
        value=session.wait_for(lambda:(drain(),responses.get(path))[1],'portal response',timeout=40)
        assert value[0]==0,value;return value[1]
    def request(c,i,m,s,v):return call(c,FRONTEND,PATH,i,m,s,v)[0]
    def notify(c,key,method,signature,values):return call(c,FRONTEND,PATH,REMOTE,method,signature,(key,{},*values))
    def denied(c,key,method,signature,values):
        try:notify(c,key,method,signature,values)
        except GLib.Error as e:
            name=Gio.DBusError.get_remote_error(e)
            assert name=='org.freedesktop.DBus.Error.AccessDenied' or (name=='org.freedesktop.DBus.Error.Failed' and 'Session is not allowed to send input notifications' in str(e)),e
        else:raise AssertionError('Unapproved touch accepted')
    def wait_line(text,before):return session.wait_for(lambda:next((v for v in lines()[before:] if v==text),None),text)
    def passed(name,**details):
        result['checks'].append(dict(name=name,**details));print(name,flush=True)
        (out/'progress.json').write_text(json.dumps(result,indent=2)+'\n')
    def start(approve):
        c=connection();call(c,FRONTEND,PATH,'org.freedesktop.host.portal.Registry','Register','(sa{sv})',('org.canopa.RemoteDesktopFixture',{}))
        c.signal_subscribe(FRONTEND,'org.freedesktop.portal.Request','Response',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:responses.__setitem__(p,v.unpack()))
        c.signal_subscribe(FRONTEND,'org.freedesktop.portal.Session','Closed',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:closed.add(p))
        value=response(request(c,REMOTE,'CreateSession','(a{sv})',(options(session_handle_token=GLib.Variant('s',f's{token}')),)));key=value['session_handle']
        response(request(c,REMOTE,'SelectDevices','(oa{sv})',(key,options(types=GLib.Variant('u',4)))))
        response(request(c,CAST,'SelectSources','(oa{sv})',(key,options(types=GLib.Variant('u',2),cursor_mode=GLib.Variant('u',1)))))
        pending=request(c,REMOTE,'Start','(osa{sv})',(key,'',options()))
        dialog=session.wait_for(lambda:next((w for w in windows() if w['app_id']=='org.canopa.Portal' and w['title']=='Share and allow control — Canopa'),None),'real Iced touch consent')
        width=min(680,int(session.pixel_size[0]/args.scale)-40)
        height=min(740,int(session.pixel_size[1]/args.scale)-20)
        action('move-window-to-floating','--id',dialog['id']);action('set-window-width','--id',dialog['id'],width)
        action('set-window-height','--id',dialog['id'],height);action('move-floating-window','--id',dialog['id'],'--x',20,'--y',10)
        action('focus-window','--id',dialog['id']);keys('-M','ctrl','-k','Return','-m','ctrl');drain();assert pending not in responses
        maps=Path(f"/proc/{dialog['pid']}/maps").read_text();assert 'libgtk' not in maps
        session.capture(('approved' if approve else 'default')+'-fresh.png')
        if approve:keys('-M','alt','t','-m','alt')
        keys('-k','Down','-k','space');time.sleep(.2)
        session.capture(('approved' if approve else 'default')+'-selected.png')
        keys('-M','ctrl','-k','Return','-m','ctrl');values=response(pending)
        assert values['devices']==(4 if approve else 0),values
        node,meta=values['streams'][0];assert len(values['streams'])==1 and meta['mapping_id'] and list(meta['size'])==[400,300]
        controls=session.wait_for(lambda:next((w for w in windows() if w['app_id']=='org.canopa.Portal' and w['title']==('Remote control — Canopa' if approve else 'Screen sharing — Canopa')),None),'Iced ongoing controls')
        action('move-floating-window','--id',controls['id'],'--x',800,'--y',20);action('focus-window','--id',wid)
        result.setdefault('starts',[]).append(values);return c,key,node,meta
    first,key,node,meta=start(False)
    denied(first,key,'NotifyTouchDown','(oa{sv}uudd)',(node,0,20.,30.))
    call(first,FRONTEND,key,'org.freedesktop.portal.Session','Close');first.close_sync(None)
    passed('fresh-unchecked-touch-choice-does-not-grant-input')
    caller,key,node,meta=start(True)
    denied(caller,key,'NotifyKeyboardKeycode','(oa{sv}iu)',(30,1))
    denied(caller,key,'NotifyPointerButton','(oa{sv}iu)',(272,1))
    before=len(lines());notify(caller,key,'NotifyTouchDown','(oa{sv}uudd)',(node,7,20.25,30.5))
    event=session.wait_for(lambda:next((v for v in lines()[before:] if v.startswith('down ')),None),'actual Notify touch down')
    parts=event.split();a=int(parts[1]);assert list(map(float,parts[2:]))==[20.25,30.5]
    before=len(lines());notify(caller,key,'NotifyTouchMotion','(oa{sv}uudd)',(node,7,50.5,60.25));wait_line(f'motion {a} 50.50 60.25',before)
    before=len(lines());notify(caller,key,'NotifyTouchDown','(oa{sv}uudd)',(node,0,80.,90.))
    event=session.wait_for(lambda:next((v for v in lines()[before:] if v.startswith('down ')),None),'second Notify contact');b=int(event.split()[1]);assert a!=b
    passed('explicit-Iced-approval-delivers-logical-touch-coordinates-and-two-contacts',slots=[a,b])
    before=len(lines());value,fds=caller.call_with_unix_fd_list_sync(FRONTEND,PATH,REMOTE,'ConnectToEIS',GLib.Variant('(oa{sv})',(key,{})),None,Gio.DBusCallFlags.NONE,15000,None,None)
    fd=fds.get(value.unpack()[0]);del fds
    wait_line(f'up {a}',before);wait_line(f'up {b}',before);assert 'cancel' not in lines()[before:]
    denied(caller,key,'NotifyTouchDown','(oa{sv}uudd)',(node,9,30.,40.))
    passed('EI-handoff-releases-legacy-contacts-and-rejects-further-Notify')
    read,write=os.pipe();client_log=out/'ei.log'
    with client_log.open('w') as stream:
        client=subprocess.Popen([str(args.ei),str(read)],stdin=fd,stdout=stream,stderr=stream,pass_fds=(read,),env=session.env,start_new_session=True)
    os.close(fd);os.close(read);session.processes.append(client)
    def events():
        assert client.poll() is None,client_log.read_text()
        return [json.loads(v) for v in client_log.read_text().splitlines() if v.startswith('{')]
    session.wait_for(lambda:any('resumed' in e for e in events()),'real libei touchscreen resumed')
    added=next(e for e in events() if e.get('added'));assert added['mapping']==meta['mapping_id'] and added['width']==400 and added['height']==300 and abs(added['scale']-args.scale)<.001,added
    def command(text):os.write(write,(text+'\n').encode())
    before=len(lines());command('down 0 35.25 45.5')
    event=session.wait_for(lambda:next((v for v in lines()[before:] if v.startswith('down ')),None),'libei touch down');c=int(event.split()[1]);assert list(map(float,event.split()[2:]))==[35.25,45.5]
    before=len(lines());command('motion 0 55.5 65.25');wait_line(f'motion {c} 55.50 65.25',before)
    before=len(lines());command('up 0');wait_line(f'up {c}',before)
    passed('real-libei-touch-regions-and-events-reach-the-approved-Wayland-client',region=added)
    before=len(lines());command('down 2 45 55')
    event=session.wait_for(lambda:next((v for v in lines()[before:] if v.startswith('down ')),None),'contact before EI stop');old=int(event.split()[1])
    before=len(lines());command('stop');wait_line(f'up {old}',before)
    session.wait_for(lambda:any(e.get('resumed',0)>1 for e in events()),'fresh touchscreen after stop-emulating')
    passed('stop-emulating-releases-contacts-and-creates-a-fresh-touch-device')
    before=len(lines());command('down 1 25 35')
    event=session.wait_for(lambda:next((v for v in lines()[before:] if v.startswith('down ')),None),'held libei contact');held=int(event.split()[1])
    before=len(lines());client.terminate();assert client.wait(timeout=10) in [-15,0];os.close(write)
    session.processes.remove(client)
    result['intentionally_stopped']=[dict(pid=client.pid,command=client.args,exit_code=client.returncode)]
    wait_line(f'up {held}',before)
    session.wait_for(lambda:(drain(),key in closed)[1],'public session closes after EI client loss')
    assert 'cancel' not in lines()[before:]
    passed('EI-client-loss-releases-held-contact-and-closes-public-session')
    caller.close_sync(None);observer.close_sync(None)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['portal','niri','frontend']:
        parser.add_argument('--'+name,type=Path,required=True);parser.add_argument('--'+name+'-sha256',required=True)
    parser.add_argument('--scheme',choices=['dark','light'],default='dark');parser.add_argument('--scale',type=float,default=1.)
    parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    for name in ['portal','niri','frontend']:
        path=getattr(args,name).resolve(strict=True);setattr(args,name,path);assert digest(path)==getattr(args,name+'_sha256')
    args.receiver,args.ei=clients();args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    sources=[Path(__file__),ROOT/'scripts/test-remote-touch-lease.py',ROOT/'scripts/desktop-fixture/remote-input-touch-receiver.c',ROOT/'scripts/desktop-fixture/remote-input-ei-touch.c',ROOT/'scripts/screencast_media.py',ROOT/'scripts/native_compositor.py',ROOT/'scripts/validate-desktop.py']
    (args.output/'sources').mkdir()
    for source in sources:shutil.copy2(source,args.output/'sources'/source.name)
    result=dict(status='running',scope=__doc__,checks=[],scheme=args.scheme,scale=args.scale,sources={str(p.relative_to(ROOT)):digest(p) for p in sources},
                binaries={n:dict(path=str(getattr(args,n)),sha256=getattr(args,n+'_sha256')) for n in ['portal','niri','frontend']})
    started=time.monotonic();code=1
    with tempfile.TemporaryDirectory(prefix='canopa-public-touch-') as folder:
        session=MediaSession(Path(folder),args.output,args.niri,args.niri_sha256)
        try:verify(args,session,result);result['status']='passed';code=0
        except Exception as e:result.update(status='failed',error=repr(e),traceback=traceback.format_exc())
        finally:
            processes=list(session.processes);session.close();result['processes']=[dict(pid=p.pid,command=p.args,exit_code=p.poll()) for p in processes]
    result.update(seconds=round(time.monotonic()-started,2),exit_code=code)
    (args.output/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));return code


if __name__=='__main__':raise SystemExit(main())
