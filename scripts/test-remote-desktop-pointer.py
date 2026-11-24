#!/usr/bin/env python3
"""Private public-portal pointer consent, Notify delivery and EI handoff/media."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / 'scripts'))
from screencast_media import MediaSession, digest

FRONTEND = 'org.freedesktop.portal.Desktop'
BACKEND = 'org.freedesktop.impl.portal.desktop.canopa'
PATH = '/org/freedesktop/portal/desktop'
REMOTE = 'org.freedesktop.portal.RemoteDesktop'
CAST = 'org.freedesktop.portal.ScreenCast'


def verify(args, session, result):
    from gi.repository import Gio, GLib
    out = session.output
    config = session.runtime / 'canopa.kdl'
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.scheme}"; }}\n')
    portals = session.runtime / 'portals'; portals.mkdir()
    names = ['org.freedesktop.impl.portal.'+n for n in ['RemoteDesktop','ScreenCast','Settings']]
    descriptor = (args.portal_descriptor.read_text() if args.portal_descriptor else
                  '[portal]\nDBusName='+BACKEND+'\nInterfaces='+';'.join(names)+';\nUseIn=Canopa;\n')
    assert all(name+';' in descriptor for name in names)
    (portals/'canopa.portal').write_text(descriptor)
    (portals/'portals.conf').write_text('[preferred]\ndefault=none\n'+''.join(n+'=canopa\n' for n in names))
    applications = Path(session.env['XDG_DATA_HOME'])/'applications'; applications.mkdir(parents=True)
    (applications/'org.canopa.RemoteDesktopFixture.desktop').write_text('[Desktop Entry]\nType=Application\nName=Remote desktop fixture\nExec=true\n')
    home = session.runtime/'home'; home.mkdir()
    session.env.update(CANOPA_PORTAL_CONFIG=str(config), CANOPA_REMOTE_DESKTOP_NATIVE='1',CANOPA_PORTAL_BINARY=str(args.portal),XDG_CURRENT_DESKTOP='Canopa',XDG_DESKTOP_PORTAL_DIR=str(portals),XDG_DATA_DIRS=session.env['XDG_DATA_HOME'],HOME=str(home),GIO_USE_VFS='local',GSETTINGS_BACKEND='memory',GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas')
    for p in [config,portals/'canopa.portal',portals/'portals.conf']:
        shutil.copy2(p,out/p.name)
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "fr"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
layout { border { off; }; focus-ring { off; }; }
''',scale=args.scale)
    for mode,suffix in [('client-header','.h'),('private-code','.c')]:
        subprocess.run(['wayland-scanner',mode,'/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml',str(out/('xdg-shell'+suffix))],check=True)
    subprocess.run(['cc','-Wall','-Wextra','-Werror','-I',str(out),str(ROOT/'scripts/desktop-fixture/remote-input-pointer-receiver.c'),str(out/'xdg-shell.c'),'-lwayland-client','-lxkbcommon','-o',str(out/'receiver')],check=True)
    flags = subprocess.check_output(['pkg-config','--cflags','--libs','libei-1.0'],text=True).split()
    subprocess.run(['cc','-Wall','-Wextra','-Werror',str(ROOT/'scripts/desktop-fixture/remote-input-ei-pointer.c'),'-o',str(out/'client'),*flags],check=True)
    receiver = session.start([str(out/'receiver')],'receiver.log'); log = out/'receiver.log'
    session.wait_for(lambda:'focused' in log.read_text(),'receiver focus')
    def action(*a): return session.run(['niri','msg','action',*map(str,a)])
    def windows(): return json.loads(session.run(['niri','msg','--json','windows']))
    wid = session.wait_for(lambda:next((w['id'] for w in windows() if w['pid']==receiver.pid),None),'receiver window')
    action('move-window-to-floating','--id',wid)
    for dimension,value in [('width',400),('height',300)]: action('set-window-'+dimension,'--id',wid,value)
    action('move-floating-window','--id',wid,'--x',100,'--y',100)
    pointer = session.build_pointer()
    logical = json.loads(session.run(['niri','msg','--json','outputs']))['winit']['logical']
    def click(x,y): session.run([str(pointer),str(round(x/logical['width']*1600)),str(round(y/logical['height']*1000))])
    def keys(*a): session.run(['wtype',*a])
    def drain():
        while GLib.MainContext.default().pending(): GLib.MainContext.default().iteration(False)
    def connection(): return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
    def call(c,d,p,i,m,s,v): return c.call_sync(d,p,i,m,GLib.Variant(s,v),None,Gio.DBusCallFlags.NONE,10000,None).unpack()
    observer = connection()
    call(observer,'org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','UpdateActivationEnvironment','(a{ss})',({**session.env},))
    command = ([str(args.portal), '--canopa-config', str(config)] if args.normal_backend else
               [str(args.harness),'--exact','screencast::remote::production::tests::native_backend_fixture','--ignored','--nocapture','--test-threads=1'])
    ready = 'component=settings_portal event=ready' if args.normal_backend else 'remote-backend-ready '
    with (out/'backend.log').open('w') as stream:
        backend = subprocess.Popen(command,stdin=subprocess.PIPE,stdout=stream,stderr=stream,env=session.env,start_new_session=True)
    session.processes.append(backend)
    session.wait_for(lambda:ready in (out/'backend.log').read_text(),'production backend')
    session.start([str(args.frontend),'--verbose'],'frontend.log')
    session.wait_for(lambda:call(observer,'org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','NameHasOwner','(s)',(FRONTEND,))[0],'frontend')
    properties = call(observer,FRONTEND,PATH,'org.freedesktop.DBus.Properties','GetAll','(s)',(REMOTE,))[0]
    assert properties['AvailableDeviceTypes']==3 and properties['version']==2,properties
    def passed(name,**details):
        result['checks'].append(dict(name=name,**details)); (out/'progress.json').write_text(json.dumps(result,indent=2)+'\n'); print(name,flush=True)
    responses,closed = {},set(); token = 0
    def options(**kw):
        nonlocal token
        token+=1; return {'handle_token':GLib.Variant('s',f'pointer{token}'),**kw}
    def response(path):
        value = session.wait_for(lambda:(drain(),responses.get(path))[1],'portal response',timeout=40)
        assert value[0]==0,value; return value[1]
    def request(c,i,m,s,v): return call(c,FRONTEND,PATH,i,m,s,v)[0]
    def notify(c,key,m,s,v): return call(c,FRONTEND,PATH,REMOTE,m,s,(key,{},*v))
    def line(text,before): session.wait_for(lambda:any(text in v for v in log.read_text().splitlines()[before:]),text)
    def motion(c,key,x,y):
        before = len(log.read_text().splitlines()); notify(c,key,'NotifyPointerMotionAbsolute','(oa{sv}udd)',(node,x,y))
        session.wait_for(lambda:any(re.fullmatch(fr'(?:enter|motion) {x:.2f} {y:.2f}',v) for v in log.read_text().splitlines()[before:]),'actual pointer point')
    def denied(c,key,m,s,v):
        try: notify(c,key,m,s,v)
        except GLib.Error as error:
            name = Gio.DBusError.get_remote_error(error)
            assert name=='org.freedesktop.DBus.Error.AccessDenied' or (name=='org.freedesktop.DBus.Error.Failed' and 'Session is not allowed to send input notifications' in str(error)),str(error)
        else: raise AssertionError('unapproved input accepted')
    def finish(p,expected=0):
        status = p.wait(timeout=20); assert status==expected,(p.args,status,expected)
        if p in session.processes: session.processes.remove(p)
        result.setdefault('retired',[]).append({'pid':p.pid,'command':p.args,'exit_code':status})
    def stop_backend():
        if args.normal_backend: backend.terminate()
        else: backend.stdin.close()
        finish(backend)
    def client_events(path): return [json.loads(v) for v in path.read_text().splitlines() if v.startswith('{')]
    def sample(c,key,values,label,size=(400,300),scale=None,monitor=False):
        import gi
        gi.require_version('Gst','1.0'); gi.require_version('GstVideo','1.0')
        from gi.repository import Gst,GstVideo
        from PIL import Image
        node,props = next(v for v in values['streams'] if v[1]['source_type']==(1 if monitor else 2)); serial = props['pipewire-serial']
        scale = args.scale if scale is None else scale
        assert props['mapping_id'],props
        if not monitor: assert 'position' not in props,props
        assert any(v['id']==node and int(v.get('info',{}).get('props',{}).get('object.serial',0))==serial for v in session.registry())
        fd = session.remote(c,key)
        for name in ['XDG_RUNTIME_DIR','PIPEWIRE_RUNTIME_DIR','PIPEWIRE_REMOTE']:
            os.environ[name]=session.env[name]
        Gst.init(None); pipeline = None
        try:
            pipeline = Gst.parse_launch(f'pipewiresrc fd={fd} target-object={serial} ! videoconvert ! video/x-raw,format=RGBA ! appsink name=frames sync=false max-buffers=2 drop=true')
            sink = pipeline.get_by_name('frames'); pipeline.set_state(Gst.State.PLAYING)
            image = None; deadline = time.monotonic()+10
            expected = session.pixel_size if monitor else tuple(round(v*scale) for v in size)
            while time.monotonic()<deadline:
                frame = sink.emit('try-pull-sample',Gst.SECOND)
                if frame is None: continue
                video = GstVideo.VideoInfo.new_from_caps(frame.get_caps()); buffer = frame.get_buffer(); ok,mapping = buffer.map(Gst.MapFlags.READ); assert ok
                try: image=Image.frombytes('RGBA',(video.width,video.height),bytes(mapping.data)[video.offset[0]:],'raw','RGBA',video.stride[0])
                finally: buffer.unmap(mapping)
                if image.size==expected: break
            assert image is not None and image.size==expected,(image.size if image else None,expected)
            for (x,y),color in zip([(1,1),(3,1),(1,3),(3,3)],[(208,32,32),(32,176,80),(48,79,208),(224,176,32)]):
                if monitor:
                    current = next(v for v in windows() if v['id']==wid)['layout']
                    position = current['window_offset_in_tile'] if not current.get('tile_pos_in_workspace_view') else [current['tile_pos_in_workspace_view'][i]+current['window_offset_in_tile'][i] for i in range(2)]
                    point = (round((position[0]+size[0]*x/4)*scale),round((position[1]+size[1]*y/4)*scale))
                else: point=(image.width*x//4,image.height*y//4)
                pixel=image.getpixel(point)[:3]; assert max(abs(a-b) for a,b in zip(pixel,color))<=8,(pixel,color,point)
            image.save(out/(label+'.png')); result.setdefault('frames',[]).append({'node':node,'serial':serial,'mapping_id':props['mapping_id'],'size':image.size,'file':label+'.png'})
        finally:
            if pipeline is not None: pipeline.set_state(Gst.State.NULL); pipeline.get_state(5*Gst.SECOND)
            os.close(fd)
    def source_ended(node,serial):
        return not any(v['id']==node and int(v.get('info',{}).get('props',{}).get('object.serial',0))==serial for v in session.registry())
    def input_workers():
        found = set()
        for task in Path(f'/proc/{backend.pid}/task').glob('*'):
            try: found.update(map(int,(task/'children').read_text().split()))
            except (OSError,ValueError): pass
        live = set()
        for pid in found:
            try:
                command = Path(f'/proc/{pid}/cmdline').read_bytes().split(b'\0')
                if b'--remote-input-worker' in command and str(args.portal).encode() in command: live.add(pid)
            except OSError: pass
        return live
    cases = [('matrix',3)] if args.matrix else [('view-only',0),('legacy-stop',2),('legacy-worker-loss',3),('pointer',2),('combined',3)]
    for case,approved in cases:
        caller = connection()
        call(caller,FRONTEND,PATH,'org.freedesktop.host.portal.Registry','Register','(sa{sv})',('org.canopa.RemoteDesktopFixture',{}))
        caller.signal_subscribe(FRONTEND,'org.freedesktop.portal.Request','Response',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:responses.__setitem__(p,v.unpack()))
        caller.signal_subscribe(FRONTEND,'org.freedesktop.portal.Session','Closed',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:closed.add(p))
        values = response(request(caller,REMOTE,'CreateSession','(a{sv})',(options(session_handle_token=GLib.Variant('s',f's{token}')),))); key = values['session_handle']
        response(request(caller,REMOTE,'SelectDevices','(oa{sv})',(key,options(types=GLib.Variant('u',3)))))
        response(request(caller,CAST,'SelectSources','(oa{sv})',(key,options(types=GLib.Variant('u',3 if args.matrix else 2),multiple=GLib.Variant('b',args.matrix),cursor_mode=GLib.Variant('u',1)))))
        start = request(caller,REMOTE,'Start','(osa{sv})',(key,'',options()))
        dialog = session.wait_for(lambda:next((w for w in windows() if w['app_id']=='org.canopa.Portal' and w['title']=='Remote desktop — Canopa'),None),'Iced combined consent')
        if not dialog['is_floating']: action('move-window-to-floating','--id',dialog['id'])
        for dimension,value in [('width',680),('height',min(740,logical['height']-20))]: action('set-window-'+dimension,'--id',dialog['id'],value)
        action('move-floating-window','--id',dialog['id'],'--x',20,'--y',10); action('focus-window','--id',dialog['id'])
        keys('-M','ctrl','-k','Return','-m','ctrl'); drain(); assert start not in responses
        session.capture(case+'-fresh.png')
        if approved & 1: click(280,130)
        if approved & 2: click(280,180)
        keys('-k','Down','-k','space')
        if args.matrix: keys('-k','Down','-k','space')
        time.sleep(.2); session.capture(case+'-selected.png')
        keys('-M','ctrl','-k','Return','-m','ctrl'); values = response(start)
        assert values['devices']==approved and not values['clipboard_enabled'],values
        wi = next(i for i,v in enumerate(values['streams']) if v[1]['source_type']==2)
        node,props = values['streams'][wi]; assert props['source_type']==2 and list(props['size'])==[400,300] and props['mapping_id'],props
        assert len(values['streams'])==(2 if args.matrix else 1),values
        result.setdefault('starts',[]).append({'case':case,'results':values})
        controls = session.wait_for(lambda:next((w for w in windows() if w['app_id']=='org.canopa.Portal' and w['title']==('Screen sharing — Canopa' if approved==0 else 'Remote control — Canopa')),None),'ongoing controls')
        action('move-floating-window','--id',controls['id'],'--x',520,'--y',20); action('focus-window','--id',wid)
        session.capture(case+'-controls.png')
        sample(caller,key,values,case+'-before')
        if approved==0:
            denied(caller,key,'NotifyPointerButton','(oa{sv}iu)',(272,1))
            call(caller,FRONTEND,key,'org.freedesktop.portal.Session','Close','()',())
            session.wait_for(lambda:source_ended(node,props['pipewire-serial']),'view-only producer retired')
            caller.close_sync(None); passed('fresh-view-only-selection-does-not-grant-pointer-or-keyboard'); continue
        motion(caller,key,30,40)
        pointer_workers = input_workers(); assert len(pointer_workers)==1,pointer_workers
        before = len(log.read_text().splitlines()); notify(caller,key,'NotifyPointerMotion','(oa{sv}dd)',(5.5,-2.25)); line('motion 35.50 37.75',before)
        passed(case+'-fresh-explicit-pointer-consent-delivers-absolute-and-relative-input')
        before = len(log.read_text().splitlines())
        notify(caller,key,'NotifyPointerAxis','(oa{sv}dd)',(1.5,-2.25)); line('axis 0 -2.25',before); line('axis 1 1.50',before)
        call(caller,FRONTEND,PATH,REMOTE,'NotifyPointerAxis','(oa{sv}dd)',(key,{'finish':GLib.Variant('b',True)},0.,3.5)); line('axis 0 3.50',before); line('axis-stop 0',before); line('axis-stop 1',before)
        events = log.read_text().splitlines()[before:]; assert events.index('axis 0 3.50') < events.index('axis-stop 0')
        before = len(log.read_text().splitlines()); notify(caller,key,'NotifyPointerAxisDiscrete','(oa{sv}ui)',(0,0)); motion(caller,key,31,41)
        assert not any(v.startswith('axis') for v in log.read_text().splitlines()[before:])
        notify(caller,key,'NotifyPointerAxisDiscrete','(oa{sv}ui)',(1,-2)); line('axis-discrete 1 -2',before)
        passed(case+'-public-smooth-final-delta-finish-and-discrete-zero-step-semantics')
        if approved & 1:
            before = len(log.read_text().splitlines()); notify(caller,key,'NotifyKeyboardKeycode','(oa{sv}iu)',(42,1)); line('key 42 1 Shift_L',before)
        else: denied(caller,key,'NotifyKeyboardKeycode','(oa{sv}iu)',(42,1))
        before = len(log.read_text().splitlines()); notify(caller,key,'NotifyPointerButton','(oa{sv}iu)',(272,1)); line('button 272 1',before)
        if case.startswith('legacy-'):
            before = len(log.read_text().splitlines())
            if case=='legacy-stop': call(caller,FRONTEND,key,'org.freedesktop.portal.Session','Close','()',())
            else:
                pid = next(iter(pointer_workers)); assert pid in input_workers()
                os.kill(pid,9)
            if case=='legacy-worker-loss': session.wait_for(lambda:(drain(),key in closed)[1],'legacy session closed')
            line('button 272 0',before)
            if approved & 1: line('key 42 0 Shift_L',before)
            session.wait_for(lambda:source_ended(node,props['pipewire-serial']),'legacy capture producer retired')
            session.wait_for(lambda:not any(w['id']==controls['id'] for w in windows()),'legacy controls retired')
            session.wait_for(lambda:not input_workers(),'legacy workers reaped')
            caller.close_sync(None); passed(case+'-retires-legacy-held-input-capture-and-controls'); continue
        # The reply is the handoff barrier; EI gets no inherited held state.
        before = len(log.read_text().splitlines())
        reply,fds = caller.call_with_unix_fd_list_sync(FRONTEND,PATH,REMOTE,'ConnectToEIS',GLib.Variant('(oa{sv})',(key,{})),None,Gio.DBusCallFlags.NONE,40000,None,None)
        owned = fds.steal_fds(); assert reply.unpack()==(0,) and len(owned)==1
        line('button 272 0',before)
        if approved & 1: line('key 42 0 Shift_L',before)
        denied(caller,key,'NotifyPointerButton','(oa{sv}iu)',(272,1))
        control_read,control_write = os.pipe(); client_log = out/(case+'-client.log')
        with client_log.open('w') as stream:
            client = subprocess.Popen([str(out/'client'),str(control_read),str(approved),'1'],stdin=owned[0],stdout=stream,stderr=stream,env=session.env,pass_fds=(control_read,),start_new_session=True)
        os.close(owned[0]); os.close(control_read); session.processes.append(client)
        session.wait_for(lambda:any(e.get('resumed')=='pointer' for e in client_events(client_log)),'real EI pointer')
        regions = [e for e in client_events(client_log) if e.get('added')=='pointer'][0]['regions']
        expected=[];offset=0
        for _,meta in values['streams']:
            expected.append({'id':meta['mapping_id'],'offset':[offset,0],'size':list(meta['size']),'scale':args.scale});offset+=meta['size'][0]
        assert regions==expected,regions
        def command(value):
            before = len(client_events(client_log)); os.write(control_write,(value+'\n').encode()); session.wait_for(lambda:any(e.get('sent')==value for e in client_events(client_log)[before:]),'EI command '+value)
        before = len(log.read_text().splitlines()); command(f'absolute {wi} 33 43'); line('motion 33.00 43.00',before)
        sample(caller,key,values,case+'-after-handoff')
        command('button 272 1'); command('button 272 0'); line('button 272 0',before)
        passed(case+'-legacy-held-input-cleanup-precedes-EI-handoff-on-the-same-paired-stream',regions=regions)
        if args.matrix:
            mi = next(i for i,v in enumerate(values['streams']) if v[1]['source_type']==1)
            before=len(log.read_text().splitlines());command(f'absolute {mi} 134 144');line('motion 34.00 44.00',before)
            sample(caller,key,values,'matrix-monitor-before',monitor=True)
            passed('monitor-and-window-public-streams-pair-to-distinct-EI-regions-and-client-coordinates')
            before=len(log.read_text().splitlines());command('key 42 1');line('key 42 1 Shift_L',before)
            released_keys=log.read_text().splitlines().count('key 42 0 Shift_L')
            size=(400,300);scale=args.scale
            def generation():return [e for e in client_events(client_log) if e.get('added')=='pointer'][-1]
            for index,(kind,target) in enumerate([('size',(420,320)),('scale',1.5),('scale',1.25),('size',(400,300))]):
                previous=generation()['generation'];before=len(log.read_text().splitlines());command('button 272 1');line('button 272 1',before)
                before=len(log.read_text().splitlines())
                if kind=='size':
                    size=target
                    for dimension,value in zip(['width','height'],size):action('set-window-'+dimension,'--id',wid,value)
                    session.wait_for(lambda:next(w for w in windows() if w['id']==wid)['layout']['window_size']==list(size),'committed window size')
                else:
                    scale=target;session.run(['niri','msg','output','winit','scale',str(scale)])
                session.wait_for(lambda:generation()['generation']>previous
                    and generation()['regions'][wi]['size']==list(size)
                    and generation()['regions'][wi]['scale']==scale,'public EI region replacement')
                current=generation();g=current['generation']
                session.wait_for(lambda:any(e.get('resumed')=='pointer' and e.get('generation')==g for e in client_events(client_log)),'new pointer resumed')
                line('button 272 0',before)
                assert current['regions'][wi]['size']==list(size) and current['regions'][wi]['scale']==scale,current
                assert [v['id'] for v in current['regions']]==[v[1]['mapping_id'] for v in values['streams']]
                assert not any(e.get('removed')=='keyboard' for e in client_events(client_log))
                assert log.read_text().splitlines().count('key 42 0 Shift_L')==released_keys
                before=len(log.read_text().splitlines());command(f'absolute {wi} {35+index} {45+index}');line(f'motion {35+index:.2f} {45+index:.2f}',before)
                command('button 272 1');command('button 272 0')
                sample(caller,key,values,'matrix-window-change-'+str(index),size=size,scale=scale)
                sample(caller,key,values,'matrix-monitor-change-'+str(index),size=size,scale=scale,monitor=True)
                passed('public-multiple-source-'+kind+'-replacement-preserves-keyboard-and-video-'+str(index),regions=current['regions'])
            before=len(log.read_text().splitlines());command('key 42 0');line('key 42 0 Shift_L',before)
        before = len(log.read_text().splitlines()); command('button 272 1'); line('button 272 1',before)
        if approved & 1: command('key 42 1'); line('key 42 1 Shift_L',before)
        before = len(log.read_text().splitlines())
        ending=args.ending if args.matrix else 'close'
        if ending=='client-kill':session.processes.remove(client);client.kill()
        elif ending=='backend-kill':session.processes.remove(backend);backend.kill();finish(backend,expected=-9)
        elif ending=='ui-stop':action('close-window','--id',controls['id'])
        else:call(caller,FRONTEND,key,'org.freedesktop.portal.Session','Close','()',())
        finish(client,expected=-9 if ending=='client-kill' else 0); os.close(control_write); line('button 272 0',before)
        if approved & 1: line('key 42 0 Shift_L',before)
        if ending!='close':session.wait_for(lambda:(drain(),key in closed)[1],'unexpected input/control loss closes public session')
        caller.close_sync(None)
        for old_node,meta in values['streams']:session.wait_for(lambda:source_ended(old_node,meta['pipewire-serial']),'EI capture producer retired')
        session.wait_for(lambda:not input_workers(),'EI worker reaped')
        session.wait_for(lambda:not any(w['id']==controls['id'] for w in windows()),'control panel retired')
        passed(case+'-public-'+ending+'-cleans-EI-held-input-and-controls')
    if args.matrix:
        if backend.poll() is None:stop_backend()
        observer.close_sync(None);return
    caller = connection()
    call(caller,FRONTEND,PATH,'org.freedesktop.host.portal.Registry','Register','(sa{sv})',('org.canopa.RemoteDesktopFixture',{}))
    caller.signal_subscribe(FRONTEND,'org.freedesktop.portal.Request','Response',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:responses.__setitem__(p,v.unpack()))
    values = response(request(caller,REMOTE,'CreateSession','(a{sv})',(options(session_handle_token=GLib.Variant('s',f's{token}')),))); key = values['session_handle']
    response(request(caller,REMOTE,'SelectDevices','(oa{sv})',(key,options(types=GLib.Variant('u',2)))))
    start = request(caller,REMOTE,'Start','(osa{sv})',(key,'',options()))
    dialog = session.wait_for(lambda:next((w for w in windows() if w['title']=='Allow pointer control — Canopa'),None),'input-only pointer consent')
    if not dialog['is_floating']: action('move-window-to-floating','--id',dialog['id'])
    for dimension,value in [('width',520),('height',420)]: action('set-window-'+dimension,'--id',dialog['id'],value)
    action('move-floating-window','--id',dialog['id'],'--x',20,'--y',10); action('focus-window','--id',dialog['id'])
    keys('-M','ctrl','-k','Return','-m','ctrl'); drain(); assert start not in responses
    session.capture('input-only-fresh.png'); click(280,130); time.sleep(.2); session.capture('input-only-selected.png'); keys('-M','ctrl','-k','Return','-m','ctrl')
    values = response(start); assert values['devices']==2 and 'streams' not in values,values
    controls = session.wait_for(lambda:next((w for w in windows() if w['title']=='Remote control — Canopa'),None),'input-only controls')
    action('move-floating-window','--id',controls['id'],'--x',520,'--y',20); action('focus-window','--id',wid)
    session.capture('input-only-controls.png')
    points = lambda:[tuple(map(float,m.groups())) for v in log.read_text().splitlines() if (m:=re.fullmatch(r'(?:enter|motion) ([-\d.]+) ([-\d.]+)',v))]
    session.wait_for(lambda:points() and abs(points()[-1][0]-180.)<1 and abs(points()[-1][1]-30.)<1,'pointer entered receiver after consent')
    start_point = points()[-1]
    def relative_point(expected,before):
        session.wait_for(lambda:any((m:=re.fullmatch(r'motion ([-\d.]+) ([-\d.]+)',v)) and all(abs(float(a)-b)<.04 for a,b in zip(m.groups(),expected)) for v in log.read_text().splitlines()[before:]),'input-only client coordinates')
    before = len(log.read_text().splitlines()); notify(caller,key,'NotifyPointerMotion','(oa{sv}dd)',(1.25,-2.5)); relative_point((start_point[0]+1.25,start_point[1]-2.5),before)
    passed('input-only-pointer-consent-allows-relative-input-without-capture-or-keyboard')
    reply,fds = caller.call_with_unix_fd_list_sync(FRONTEND,PATH,REMOTE,'ConnectToEIS',GLib.Variant('(oa{sv})',(key,{})),None,Gio.DBusCallFlags.NONE,40000,None,None)
    owned = fds.steal_fds(); assert reply.unpack()==(0,) and len(owned)==1
    control_read,control_write = os.pipe(); client_log = out/'input-only-client.log'
    with client_log.open('w') as stream: client=subprocess.Popen([str(out/'client'),str(control_read),'2','0'],stdin=owned[0],stdout=stream,stderr=stream,env=session.env,pass_fds=(control_read,),start_new_session=True)
    os.close(owned[0]); os.close(control_read); session.processes.append(client)
    session.wait_for(lambda:any(e.get('resumed')=='pointer' for e in client_events(client_log)),'relative-only EI pointer')
    assert [e for e in client_events(client_log) if e.get('added')=='pointer'][0]['regions']==[]
    before = len(log.read_text().splitlines()); os.write(control_write,b'relative 2 -1\n'); relative_point((start_point[0]+3.25,start_point[1]-3.5),before)
    call(caller,FRONTEND,key,'org.freedesktop.portal.Session','Close','()',()); finish(client); os.close(control_write)
    session.wait_for(lambda:not input_workers(),'input-only worker reaped'); caller.close_sync(None)
    passed('input-only-EI-handoff-announces-relative-pointer-without-regions-or-keyboard')
    stop_backend(); observer.close_sync(None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--normal-backend',action='store_true',help='Run the normal executable instead of the unit-test fixture')
    parser.add_argument('--portal-descriptor',type=Path,help='Use the exact optional package descriptor')
    for name in ['portal','harness','niri','frontend']:
        parser.add_argument('--'+name,required=name!='harness',type=Path); parser.add_argument('--'+name+'-sha256',required=name!='harness')
    parser.add_argument('--scheme',choices=['dark','light'],default='dark'); parser.add_argument('--scale',type=float,default=1.25); parser.add_argument('--matrix',action='store_true',help='exercise monitor+window mapping and four mutable projection transitions'); parser.add_argument('--ending',choices=['close','ui-stop','client-kill','backend-kill'],default='close'); parser.add_argument('--output',required=True,type=Path)
    args = parser.parse_args()
    if not args.normal_backend and (args.harness is None or args.harness_sha256 is None):parser.error('provide --harness and --harness-sha256, or --normal-backend')
    binaries = ['portal','niri','frontend'] + ([] if args.normal_backend else ['harness'])
    for name in binaries:
        p = getattr(args,name).resolve(); setattr(args,name,p); assert digest(p)==getattr(args,name+'_sha256')
    args.output = args.output.resolve(); args.output.mkdir(exist_ok=False)
    sources = [Path(__file__),ROOT/'scripts/screencast_media.py',ROOT/'scripts/validate-desktop.py',ROOT/'scripts/desktop-fixture/remote-input-ei-pointer.c',ROOT/'scripts/desktop-fixture/remote-input-pointer-receiver.c']
    if args.portal_descriptor:
        args.portal_descriptor=args.portal_descriptor.resolve(strict=True);sources.append(args.portal_descriptor)
    archive = args.output/'sources'; archive.mkdir(); pins = {str(p):digest(p) for p in sources}
    for p in sources: shutil.copy2(p,archive/p.name)
    result = {'status':'running','normal_backend':args.normal_backend,'scope':'Private bus, pinned frontend, real Iced consent and pointer/keyboard delivery; no Flatpak or full distribution acceptance','checks':[],'pins':pins,'binaries':{n:{'path':str(getattr(args,n)),'sha256':getattr(args,n+'_sha256')} for n in binaries}}
    started = time.monotonic(); code = 1
    with tempfile.TemporaryDirectory(prefix='canopa-public-pointer-') as folder:
        session = MediaSession(Path(folder),args.output,args.niri,args.niri_sha256,reference_frozen=True)
        try: verify(args,session,result); result['status']='passed'; code=0
        except Exception as error: result.update(status='failed',error=repr(error),traceback=traceback.format_exc())
        finally:
            processes = list(session.processes); session.close(); result['processes']=[{'pid':p.pid,'command':p.args,'exit_code':p.poll()} for p in processes]
    result.update(seconds=round(time.monotonic()-started,2),changed_sources=[p for p,h in pins.items() if digest(Path(p))!=h],exit_code=code,binaries_unchanged=all(digest(getattr(args,n))==getattr(args,n+'_sha256') for n in binaries))
    (args.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n'); print(json.dumps({k:result.get(k) for k in ['status','seconds','error']}))
    if code: print(result['traceback'])
    raise SystemExit(code)


if __name__=='__main__': main()
