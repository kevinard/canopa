#!/usr/bin/env python3
"""Private frontend/Iced ScreenCast consent; optional pinned hardware media acceptance."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT=Path(__file__).resolve().parents[1]
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py')
desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)
BACKEND='org.freedesktop.impl.portal.desktop.canopa';FRONTEND='org.freedesktop.portal.Desktop';PATH='/org/freedesktop/portal/desktop';IMPL='org.freedesktop.impl.portal.ScreenCast';PUBLIC='org.freedesktop.portal.ScreenCast'


def verify(args,session):
    from gi.repository import Gio,GLib
    config=session.runtime/'config/canopa/config.kdl';config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.color_scheme}"; }}\n')
    home=session.runtime/'home';home.mkdir();services=session.runtime/'services';services.mkdir();portals=session.runtime/'portals';portals.mkdir()
    shutil.copy(ROOT/'packaging/portal/org.freedesktop.impl.portal.desktop.canopa.service',services)
    descriptor=(args.portal_descriptor or ROOT/'packaging/portal/canopa.portal').read_text()
    if not args.shipped_descriptor and not args.portal_descriptor and IMPL+';' not in descriptor:descriptor=descriptor.replace('Interfaces=','Interfaces='+IMPL+';')
    assert IMPL+';' in descriptor
    (portals/'canopa.portal').write_text(descriptor)
    if args.portal_config:
        shutil.copyfile(args.portal_config, portals/'canopa-portals.conf')
    else:
        (portals/'portals.conf').write_text('[preferred]\ndefault=none\n'+IMPL+'=canopa\norg.freedesktop.impl.portal.Settings=canopa\n')
    applications=Path(session.env['XDG_DATA_HOME'])/'applications';applications.mkdir(parents=True)
    (applications/'org.canopa.CastTest.desktop').write_text('[Desktop Entry]\nType=Application\nName=Screen sharing fixture\nExec=true\n')
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home),XDG_CURRENT_DESKTOP='Canopa',XDG_DESKTOP_PORTAL_DIR=str(portals),XDG_DATA_DIRS=session.env['XDG_DATA_HOME'],GIO_USE_VFS='local',GSETTINGS_BACKEND='memory',GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas',PATH=str(args.binary.parent)+os.pathsep+session.env['PATH'],RUST_LOG='niri=debug')
    output=session.start_niri('debug { dbus-interfaces-in-non-session-instances; }\nprefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 8; border { off; }; focus-ring { off; }; }\n',scale=args.scale,service_directories=[services])
    fixture=ROOT/'scripts/desktop-fixture'
    for mode,suffix in [('client-header','.h'),('private-code','.c')]:
        subprocess.run(['wayland-scanner',mode,str(fixture/'virtual-keyboard-unstable-v1.xml'),str(session.output/('virtual-keyboard'+suffix))],check=True,timeout=15)
    keyboard_binary=session.output/'keyboard'
    subprocess.run(['cc','-Wall','-Wextra','-Werror','-I',str(session.output),str(fixture/'keyboard.c'),str(session.output/'virtual-keyboard.c'),'-lwayland-client','-lxkbcommon','-o',str(keyboard_binary)],check=True,timeout=30)
    keyboard_log=session.output/'keyboard.log'
    with keyboard_log.open('w') as stream:
        keyboard=subprocess.Popen([str(keyboard_binary)],env=session.env,stdin=subprocess.PIPE,stdout=stream,stderr=stream,text=True,start_new_session=True)
    session.processes.append(keyboard)
    session.wait_for(lambda:'ready' in keyboard_log.read_text(),'persistent keyboard')
    key_count=0
    def keys(*codes):
        nonlocal key_count
        for code in codes:
            keyboard.stdin.write(f'{code}\n');keyboard.stdin.flush();key_count+=1
            session.wait_for(lambda:keyboard_log.read_text().splitlines().count('done')==key_count,'key delivery')
    pointer=session.build_pointer()
    flags=Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION
    def connect():return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],flags,None,None)
    connection=connect()
    def call(destination,path,interface,method,signature,values,client=connection,timeout=10000):return client.call_sync(destination,path,interface,method,GLib.Variant(signature,values),None,Gio.DBusCallFlags.NONE,timeout,None).unpack()
    def bus(method,signature,values,timeout=10000):return call('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus',method,signature,values,timeout=timeout)
    def drain():
        while GLib.MainContext.default().pending():GLib.MainContext.default().iteration(False)
    bus('UpdateActivationEnvironment','(a{ss})',({k:v for k,v in session.env.items()},))
    session.start([str(args.frontend),'--verbose'],'frontend.log');bus('StartServiceByName','(su)',(BACKEND,0),timeout=30000)
    backend_pid=bus('GetConnectionUnixProcessID','(s)',(BACKEND,))[0];assert Path(f'/proc/{backend_pid}/exe').resolve()==args.binary
    session.wait_for(lambda:bus('NameHasOwner','(s)',(FRONTEND,))[0],'frontend')
    responses={};closed={};subscriptions=[]
    def registered(client):
        call(FRONTEND,PATH,'org.freedesktop.host.portal.Registry','Register','(sa{sv})',('org.canopa.CastTest',{}),client)
        subscriptions.append((client,client.signal_subscribe(FRONTEND,'org.freedesktop.portal.Request','Response',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:responses.__setitem__(p,v))))
        subscriptions.append((client,client.signal_subscribe(FRONTEND,'org.freedesktop.portal.Session','Closed',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:closed.__setitem__(p,v))))
    registered(connection)
    for name,xml in [('xdg-shell','stable/xdg-shell/xdg-shell.xml'),('xdg-foreign','unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for mode,suffix in [('client-header','.h'),('private-code','.c')]:subprocess.run(['wayland-scanner',mode,str(Path('/usr/share/wayland-protocols')/xml),str(session.output/(name+suffix))],check=True,timeout=15)
    def compile_parent(source,binary):subprocess.run(['cc','-Wall','-Wextra','-Werror','-DPORTAL_CAPTURE_PATTERN',*(['-DPORTAL_CAPTURE_CURSOR'] if args.media else []),'-I',str(session.output),str(source),str(session.output/'xdg-shell.c'),str(session.output/'xdg-foreign.c'),'-lwayland-client','-o',str(binary)],check=True,timeout=30)
    parent_binary=session.output/'parent';compile_parent(ROOT/'scripts/desktop-fixture/portal-parent.c',parent_binary)
    parent=session.start([str(parent_binary)],'parent.log')
    exported=session.wait_for(lambda:next((line for line in (session.output/'parent.log').read_text().splitlines() if line.startswith('wayland:')),None),'parent export')
    def windows():return json.loads(session.run(['niri','msg','--json','windows']))
    session.wait_for(lambda:any(w['pid']==parent.pid for w in windows()),'parent window')
    logical=json.loads(session.run(['niri','msg','--json','outputs']))[output]['logical'];width,height=logical['width'],logical['height']
    if args.media:
        parent_window=next(w for w in windows() if w['pid']==parent.pid)
        if not parent_window['is_floating']:session.run(['niri','msg','action','move-window-to-floating','--id',str(parent_window['id'])])
        session.run(['niri','msg','action','set-window-width','--id',str(parent_window['id']),'400'])
        session.run(['niri','msg','action','set-window-height','--id',str(parent_window['id']),'300'])
        session.wait_for(lambda:next((w for w in windows() if w['id']==parent_window['id'] and tuple(w['layout']['window_size'])==(400,300)),None),'pattern geometry')
        px,py=width-420,height-320
        session.run(['niri','msg','action','move-floating-window','--id',str(parent_window['id']),'--x',str(px),'--y',str(py)])
        pattern_box=tuple(round(v*args.scale) for v in (px,py,px+400,py+300))
    count=0;checks=[]
    def options(**values):
        nonlocal count
        count+=1;return {'handle_token':GLib.Variant('s',f'cast{count}'),**values}
    def result(path,status):
        response=session.wait_for(lambda:(drain(),responses.get(path))[1],'frontend response',timeout=40)
        assert response.get_child_value(0).unpack()==status,response
        return response.get_child_value(1).unpack()
    def create(client=connection):
        request_options=options();request_options['session_handle_token']=GLib.Variant('s',f'session{count}')
        path=call(FRONTEND,PATH,PUBLIC,'CreateSession','(a{sv})',(request_options,),client)[0]
        values=result(path,0);key=values['session_handle'];return key.unpack() if hasattr(key,'unpack') else key
    def select(key,types=3,multiple=False,client=connection,cursor=1):
        path=call(FRONTEND,PATH,PUBLIC,'SelectSources','(oa{sv})',(key,options(types=GLib.Variant('u',types),multiple=GLib.Variant('b',multiple),cursor_mode=GLib.Variant('u',cursor))),client)[0];result(path,0)
    def start(key,client=connection):return call(FRONTEND,PATH,PUBLIC,'Start','(osa{sv})',(key,exported,options()),client)[0]
    def popup():
        def pending():
            drain()
            window=next((w for w in windows() if w['app_id']=='org.canopa.Portal' and w['title']=='Share your screen — Canopa'),None)
            if window is None and path in responses:
                raise RuntimeError(f'Start completed before consent: {responses[path].unpack()}')
            return window
        try:
            window=session.wait_for(pending,'Iced consent',timeout=30)
        except Exception:
            children=set()
            for task in Path(f'/proc/{backend_pid}/task').glob('*'):
                try:children.update(map(int,(task/'children').read_text().split()))
                except (OSError,ValueError):pass
            processes=[]
            for pid in [backend_pid,*sorted(children)[:64]]:
                try:
                    proc=Path(f'/proc/{pid}')
                    processes.append({'pid':pid,'executable':str((proc/'exe').resolve()),
                        'arguments':(proc/'cmdline').read_bytes()[:16384].replace(b'\0',b' ').decode(errors='replace'),
                        'wait':(proc/'wchan').read_text()})
                except OSError:pass
            desktop.write_json(session.output/'consent-startup-failure.json',{'request':path,
                'response':responses[path].unpack() if path in responses else None,
                'windows':windows(),'processes':processes})
            raise
        if not window['is_floating']:session.run(['niri','msg','action','move-window-to-floating','--id',str(window['id'])])
        w,h=min(680,width-40),min(600,height-20)
        session.run(['niri','msg','action','set-window-width','--id',str(window['id']),str(w)]);session.run(['niri','msg','action','set-window-height','--id',str(window['id']),str(h)])
        window=session.wait_for(lambda:next((v for v in windows() if v['id']==window['id'] and abs(v['layout']['window_size'][0]-w)<2 and abs(v['layout']['window_size'][1]-h)<2),None),'consent geometry')
        session.run(['niri','msg','action','move-floating-window','--id',str(window['id']),'--x','20','--y','10']);session.run(['niri','msg','action','focus-window','--id',str(window['id'])]);time.sleep(.2);return window
    def action(window,share):
        w,h=window['layout']['window_size'];session.run([str(pointer),str(round((20+w*(.75 if share else .25))/width*1600)),str(round((10+h-38)/height*1000))])
    def gone(window):session.wait_for(lambda:not any(v['id']==window['id'] for v in windows()),'consent retired')
    control_layouts=[]
    def controls(label):
        window=session.wait_for(lambda:next((w for w in windows() if w['app_id']=='org.canopa.Portal' and w['title']=='Screen sharing — Canopa'),None),'ongoing sharing controls')
        control_layouts.append({'case':label,'initial_floating':window['is_floating'],'initial_layout':window['layout']})
        assert window['is_floating'],'sharing controls should open compact and floating'
        session.run(['niri','msg','action','set-window-width','--id',str(window['id']),'480'])
        session.run(['niri','msg','action','set-window-height','--id',str(window['id']),'280'])
        window=session.wait_for(lambda:next((v for v in windows() if v['id']==window['id'] and tuple(v['layout']['window_size'])==(480,280)),None),'sharing controls geometry')
        session.run(['niri','msg','action','move-floating-window','--id',str(window['id']),'--x','20','--y','10'])
        session.run(['niri','msg','action','focus-window','--id',str(window['id'])]);time.sleep(.2)
        session.capture(label+'-controls.png')
        return window
    def choose_first(window,label):
        from PIL import ImageChops
        w,h=window['layout']['window_size']
        box=tuple(round(value*args.scale) for value in (40,10+h-85,180,10+h-48))
        before=session.capture(label+'-before.png').crop(box)
        keys(108,57) # Linux evdev Down and Space, using the same full US keymap.
        def selected():
            assert any(v['id']==window['id'] for v in windows()),'consent vanished before selection'
            return ImageChops.difference(before,session.capture(label+'.png').crop(box)).getbbox()
        session.wait_for(selected,'visible source selection')

    assert call(BACKEND,PATH,'org.freedesktop.DBus.Properties','Get','(ss)',(IMPL,'version'))[0]==6
    assert call(FRONTEND,PATH,'org.freedesktop.DBus.Properties','Get','(ss)',(PUBLIC,'AvailableSourceTypes'))[0]==3
    key=create();select(key);path=start(key);window=popup();session.capture('01-consent.png')
    keys(28,'ctrl 28');drain();assert path not in responses
    action(window,False);result(path,1);gone(window)
    checks.append('actual frontend opens parented Iced source consent; initial Enter and Cancel expose no streams')

    key=create();select(key);path=start(key);window=popup()
    call(FRONTEND,key,'org.freedesktop.portal.Session','Close','()',());gone(window)
    checks.append('public Session.Close retires pending consent')

    key=create();select(key);path=start(key);window=popup()
    call(FRONTEND,path,'org.freedesktop.portal.Request','Close','()',());gone(window)
    checks.append('public Request.Close retires pending consent without a grant')

    lost=connect();registered(lost);key=create(lost);select(key,client=lost);start(key,client=lost);window=popup();lost.close_sync(None);gone(window)
    checks.append('frontend application disconnect closes its backend session and real Iced helper')

    key=create();select(key,types=1);path=start(key);window=popup()
    keys(15,17,23,49) # Tab into search, then type "win" using the US keymap.
    choose_first(window,'02-selected-display');keys('ctrl 28')
    if args.media:
        values=result(path,0);gone(window);status=controls('04-public-close')
        session.check_stream(connection,key,values,1,logical,pattern_box,
                        lambda:call(FRONTEND,key,'org.freedesktop.portal.Session','Close','()',()))
        gone(status)
        checks.append('keyboard consent delivers actual monitor pixels through the restricted frontend PipeWire FD; public Close retires the producer and sharing controls')
    else:
        assert result(path,2)=={};gone(window)
        assert 'no GBM device available' in (session.output/'niri.log').read_text()
        checks.append('keyboard search, source selection and Ctrl+Enter reach actual installed nested NoGBM startup failure without a stream grant')

    if args.media:
        for mode in ('button','escape','window-close'):
            key=create();select(key,types=1);path=start(key);window=popup();choose_first(window,'05-selected-'+mode);keys('ctrl 28')
            values=result(path,0);gone(window);status=controls('05-'+mode)
            def stop_controls():
                if mode=='button':
                    session.run([str(pointer),str(round(260/width*1600)),str(round(252/height*1000))])
                elif mode=='escape':keys(1)
                else:session.run(['niri','msg','action','close-window','--id',str(status['id'])])
            session.check_stream(connection,key,values,1,logical,pattern_box,stop_controls,label=mode)
            session.wait_for(lambda:(drain(),key in closed)[1],'public control-stop Closed signal');gone(status)
            checks.append(f'ongoing Iced controls {mode} closes the public session and removes its real monitor producer')

        active_caller=connect();registered(active_caller)
        key=create(active_caller);select(key,types=1,client=active_caller);path=start(key,active_caller);window=popup();choose_first(window,'05-selected-caller-loss');keys('ctrl 28')
        values=result(path,0);gone(window);status=controls('05-caller-loss')
        session.check_stream(active_caller,key,values,1,logical,pattern_box,lambda:active_caller.close_sync(None),label='caller-loss');gone(status)
        checks.append('active caller disconnect removes the real producer and its session-owned sharing controls')

        for kind in (1,2):
            for cursor_mode in (1,2,4):
                label=f'06-cursor-{kind}-{cursor_mode}'
                key=create();select(key,types=kind,cursor=cursor_mode);path=start(key);window=popup();choose_first(window,label);keys('ctrl 28')
                values=result(path,0);gone(window);status=controls(label)
                # Keyboard consent can leave the pointer inside the previous source.
                session.run([str(pointer),str(round(40/width*1600)),str(round(40/height*1000))])
                before=(session.output/'parent.log').read_text().count('cursor-enter')
                session.run([str(pointer),str(round((px+60)/width*1600)),str(round((py+50)/height*1000))])
                session.wait_for(lambda:(session.output/'parent.log').read_text().count('cursor-enter')>before,'fixture cursor entered selected source')
                point=tuple(round(value*args.scale) for value in ((px+60,py+50) if kind==1 else (60,50)))
                expected=pattern_box if kind==1 else (0,0,round(400*args.scale),round(300*args.scale))
                session.check_cursor_stream(connection,key,values,kind,cursor_mode,expected,point,
                    lambda:call(FRONTEND,key,'org.freedesktop.portal.Session','Close','()',()))
                gone(status)
                checks.append(f'restricted-FD source type {kind} cursor mode {cursor_mode} delivers correct source pixels and cursor image/metadata')

    key=create();select(key,types=2);path=start(key);window=popup();choose_first(window,'03-selected-window');action(window,True)
    if args.media:
        values=result(path,0);gone(window);status=controls('06-source-loss')
        session.check_stream(connection,key,values,2,logical,None,lambda:session.retire(parent))
        session.wait_for(lambda:(drain(),key in closed)[1],'public source-loss Closed signal');gone(status)
        checks.append('actual window pixels exclude other surfaces; source destruction closes the public session, sharing controls and producer')
    else:
        assert result(path,2)=={};gone(window)
        assert 'record_window' in (session.output/'niri.log').read_text()
        checks.append('window-only source consent uses real RecordWindow and failed startup retires the session')

    if args.media:
        key=create();select(key,types=1);path=start(key);window=popup();choose_first(window,'07-selected-backend-loss');keys('ctrl 28')
        values=result(path,0);gone(window);status=controls('07-backend-loss')
        # The activation PID was verified against this fixture's frozen executable.
        session.check_stream(connection,key,values,1,logical,None,lambda:os.kill(backend_pid,signal.SIGKILL),label='backend-loss',expect_pattern=False)
        gone(status)
        session.wait_for(lambda:(drain(),key in closed)[1],'public backend-loss Closed signal')
        checks.append('SIGKILL of the private backend closes the public session, removes the real producer and retires sharing controls')
    for client,subscription in subscriptions:
        if not client.is_closed():client.signal_unsubscribe(subscription)
    assert hashlib.sha256(args.frontend.read_bytes()).hexdigest()==args.frontend_sha256,'Frontend changed during the run'
    return {'status':'passed','checks':checks,'color_scheme':args.color_scheme,'scale':args.scale,'media_flow_verified':args.media,'shipped_descriptor':args.shipped_descriptor,'descriptor_sha256':hashlib.sha256(descriptor.encode()).hexdigest(),'binary_sha256':hashlib.sha256(args.binary.read_bytes()).hexdigest(),'frontend':str(args.frontend),'frontend_sha256':args.frontend_sha256,'control_layouts':control_layouts,**(session.media_result() if args.media else {})}


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--color-scheme',choices=['dark','light'],default='dark');parser.add_argument('--scale',type=float,default=1);parser.add_argument('--shipped-descriptor',action='store_true')
    parser.add_argument('--media',action='store_true');parser.add_argument('--niri',type=Path);parser.add_argument('--niri-sha256')
    parser.add_argument('--frontend',type=Path);parser.add_argument('--frontend-sha256')
    parser.add_argument('--portal-descriptor',type=Path,help='Use an exact packaged descriptor without adding interfaces')
    parser.add_argument('--portal-config',type=Path,help='Use the packaged Canopa desktop routing')
    args=parser.parse_args();args.binary=args.binary.resolve(strict=True);args.output=args.output.resolve()
    if bool(args.frontend)!=bool(args.frontend_sha256):parser.error('--frontend requires its --frontend-sha256')
    args.frontend=(args.frontend or next(p for p in [Path('/usr/libexec/xdg-desktop-portal'),Path('/usr/lib/xdg-desktop-portal')] if p.is_file())).resolve(strict=True)
    actual=hashlib.sha256(args.frontend.read_bytes()).hexdigest()
    if args.frontend_sha256 and actual!=args.frontend_sha256:parser.error('Frontend checksum mismatch')
    args.frontend_sha256=actual
    if args.media:
        if not args.niri or not args.niri_sha256 or len(args.niri_sha256)!=64 or any(c not in '0123456789abcdef' for c in args.niri_sha256):parser.error('--media requires --niri and its lowercase --niri-sha256')
        args.niri=args.niri.resolve(strict=True)
    elif args.niri or args.niri_sha256:parser.error('--niri/--niri-sha256 require --media')
    args.output.mkdir(parents=True,exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-screencast-portal-') as directory:
        if args.media:
            from screencast_media import MediaSession
            session=MediaSession(Path(directory),args.output,args.niri,args.niri_sha256)
        else:session=desktop.Session(Path(directory),args.output)
        try:result=verify(args,session)
        except Exception as error:desktop.write_json(args.output/'result.json',{'status':'failed','error':str(error)});raise
        finally:session.close()
    desktop.write_json(args.output/'result.json',result);print(json.dumps(result))


if __name__=='__main__':main()
