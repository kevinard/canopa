#!/usr/bin/env python3
"""Window screenshot acceptance through private Niri/PipeWire, XDG and real Iced."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import traceback
from urllib.parse import unquote,urlsplit
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from screencast_media import MediaSession,digest
from window_screenshot_fixture import receiver,pointer
BACKEND='org.freedesktop.impl.portal.desktop.canopa'
FRONTEND='org.freedesktop.portal.Desktop'
PATH='/org/freedesktop/portal/desktop'
IMPL='org.freedesktop.impl.portal.Screenshot'
PUBLIC='org.freedesktop.portal.Screenshot'


def verify(args,s,result):
    from gi.repository import Gio,GLib
    from PIL import Image,ImageChops
    config=s.runtime/'canopa.kdl';config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.scheme}"; }}\n')
    portals=s.runtime/'portals';portals.mkdir()
    shutil.copy2(ROOT/'packaging/portal/canopa.portal',portals/'canopa.portal')
    (portals/'portals.conf').write_text('[preferred]\ndefault=none\n'+''.join(n+'=canopa\n' for n in [IMPL,'org.freedesktop.impl.portal.Access','org.freedesktop.impl.portal.Settings']))
    data=Path(s.env['XDG_DATA_HOME']);(data/'applications').mkdir(parents=True)
    (data/'applications/org.canopa.WindowScreenshotFixture.desktop').write_text('[Desktop Entry]\nType=Application\nName=Window screenshot fixture\nExec=true\n')
    home=s.runtime/'home';home.mkdir()
    s.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    s.env.update(HOME=str(home),CANOPA_PORTAL_CONFIG=str(config),CANOPA_PORTAL_BINARY=str(args.binary),
        XDG_CURRENT_DESKTOP='Canopa',XDG_DESKTOP_PORTAL_DIR=str(portals),XDG_DATA_DIRS=str(data),GIO_USE_VFS='local',
        GSETTINGS_BACKEND='memory',GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas')
    output=s.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
layout { border { off; }; focus-ring { off; }; }
''',width=round(1280*args.scale),height=round(800*args.scale),scale=args.scale)
    logical=json.loads(s.run(['niri','msg','--json','outputs']))[output]['logical']
    # The nested Winit output can keep its own size despite a larger parent.
    s.pixel_size=tuple(round(logical[n]*logical['scale']) for n in ['width','height'])
    result['output']=dict(logical=logical,physical_size=s.pixel_size)
    def action(*args):return s.run(['niri','msg','action',*map(str,args)])
    def windows():return json.loads(s.run(['niri','msg','--json','windows']))
    def keys(*args):return s.run(['wtype',*map(str,args)])
    source=s.start([str(args.receiver)],'source.log')
    window=s.wait_for(lambda:next((w for w in windows() if w['app_id']=='org.canopa.TouchReceiver'),None),'source window')
    wid=window['id'];action('move-window-to-floating','--id',wid)
    action('set-window-width','--id',wid,400);action('set-window-height','--id',wid,300)
    action('move-floating-window','--id',wid,'--x',100,'--y',100)
    s.wait_for(lambda:'configured 400 300' in (s.output/'source.log').read_text(),'source geometry')
    action('move-window-to-workspace','--window-id',wid,'--focus','false','2')
    workspaces=json.loads(s.run(['niri','msg','--json','workspaces']))
    focused=next(v['id'] for v in workspaces if v['is_focused'])
    assert next(w for w in windows() if w['id']==wid)['workspace_id']!=focused
    result['source_on_another_workspace']=True
    clipboard_log=(s.output/'clipboard.log').open('w')
    clipboard=subprocess.Popen(['wl-copy','--foreground','--type','text/plain'],stdin=subprocess.PIPE,stdout=clipboard_log,stderr=clipboard_log,env=s.env,start_new_session=True)
    clipboard_log.close();s.processes.append(clipboard);clipboard.stdin.write(b'window-screenshot-sentinel');clipboard.stdin.close()
    def clip():
        try:return s.run(['wl-paste','--no-newline','--type','text/plain'])
        except subprocess.CalledProcessError:return None
    s.wait_for(lambda:clip()=='window-screenshot-sentinel','clipboard sentinel')
    c=Gio.DBusConnection.new_for_address_sync(s.env['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
    def call(destination,path,interface,method,signature='()',values=()):
        return c.call_sync(destination,path,interface,method,GLib.Variant(signature,values),None,Gio.DBusCallFlags.NONE,15000,None).unpack()
    call('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','UpdateActivationEnvironment','(a{ss})',({**s.env},))
    backend=s.start([str(args.binary),'--canopa-config',str(config)],'backend.log')
    s.wait_for(lambda:'component=settings_portal event=ready' in (s.output/'backend.log').read_text(),'normal backend')
    s.start([str(args.frontend),'--verbose'],'frontend.log')
    s.wait_for(lambda:call('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','NameHasOwner','(s)',(FRONTEND,))[0],'frontend')
    def targets(destination,interface):return call(destination,PATH,'org.freedesktop.DBus.Properties','Get','(ss)',(interface,'AvailableTargets'))[0]
    result['initial_targets']=dict(backend=targets(BACKEND,IMPL),frontend=targets(FRONTEND,PUBLIC))
    assert result['initial_targets']==dict(backend=15,frontend=15),result['initial_targets']
    call(FRONTEND,PATH,'org.freedesktop.host.portal.Registry','Register','(sa{sv})',('org.canopa.WindowScreenshotFixture',{}))
    responses={};token=0
    c.signal_subscribe(FRONTEND,'org.freedesktop.portal.Request','Response',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:responses.__setitem__(p,v.unpack()))
    def drain():
        while GLib.MainContext.default().pending():GLib.MainContext.default().iteration(False)
    def begin(target=None):
        nonlocal token
        token+=1;opts={'handle_token':GLib.Variant('s',f'window{token}'),'interactive':GLib.Variant('b',True)}
        if target is not None:opts['target']=GLib.Variant('u',target)
        return call(FRONTEND,PATH,PUBLIC,'Screenshot','(sa{sv})',('',opts))[0]
    def response(path,status):
        value=s.wait_for(lambda:(drain(),responses.get(path))[1],'screenshot response',timeout=40)
        assert value[0]==status,value;return value[1]
    def popup(title,width=900,height=680):
        window=s.wait_for(lambda:(drain(),next((w for w in windows() if w['app_id']=='org.canopa.Portal' and w['title']==title),None))[1],title,timeout=40)
        width=min(width,logical['width']-40);height=min(height,logical['height']-20)
        action('move-window-to-floating','--id',window['id']);action('set-window-width','--id',window['id'],width);action('set-window-height','--id',window['id'],height)
        window=s.wait_for(lambda:next((w for w in windows() if w['id']==window['id'] and abs(w['layout']['window_size'][0]-width)<2 and abs(w['layout']['window_size'][1]-height)<2),None),'dialog geometry')
        action('move-floating-window','--id',window['id'],'--x',20,'--y',10);action('focus-window','--id',window['id']);time.sleep(.2)
        assert 'libgtk' not in Path(f"/proc/{window['pid']}/maps").read_text()
        return window
    def picker():return popup('Choose a screenshot window — Canopa',680,600)
    def review():return popup('Screenshot — Canopa')
    def gone(window):s.wait_for(lambda:not any(w['id']==window['id'] for w in windows()),'helper closes')
    def accept():
        # Standard Iced buttons currently activate through pointer/touch.
        window=next(w for w in windows() if w['app_id']=='org.canopa.Portal' and w['title']=='Screenshot — Canopa')
        width,height=window['layout']['window_size']
        x=(20+width*.75)/(s.pixel_size[0]/args.scale)*1600
        y=(10+height-38)/(s.pixel_size[1]/args.scale)*1000
        s.run([str(args.pointer),str(round(x)),str(round(y))])
    cache=Path(s.env['XDG_CACHE_HOME'])/'canopa/portal/screenshots'
    def files():return sorted(cache.glob('*.png'))
    def image(path):
        uri=response(path,0)['uri'];p=Path(unquote(urlsplit(uri).path));assert p.is_relative_to(cache),p
        assert p.stat().st_mode&0o777==0o400
        return Image.open(p).convert('RGBA')
    def blue(frame):
        expected=(round(400*args.scale),round(300*args.scale));assert frame.size==expected,(frame.size,expected)
        interior=frame.crop((4,4,frame.width-4,frame.height-4)).convert('RGB')
        diff=ImageChops.difference(interior,Image.new('RGB',interior.size,(32,144,192)))
        assert all(high<=2 for low,high in diff.getextrema()),diff.getextrema()
    def passed(name,**extra):
        assert clip()=='window-screenshot-sentinel','screenshot changed clipboard'
        result['checks'].append(dict(name=name,**extra));(s.output/'progress.json').write_text(json.dumps(result,indent=2)+'\n');print(name,flush=True)
    path=begin(2);choose=picker();s.capture('window-picker.png')
    keys('-k','Down','-k','space','-M','ctrl','-k','Return','-m','ctrl');gone(choose)
    shot=review();s.capture('window-review.png');accept();blue(image(path));gone(shot)
    passed('selected-window-on-another-workspace-produces-its-own-full-physical-pixels')
    path=begin();shot=review();keys('-M','alt','w','-m','alt');gone(shot)
    choose=picker();keys('-k','Down','-k','space','-M','ctrl','-k','Return','-m','ctrl');gone(choose)
    shot=review();accept();blue(image(path));gone(shot)
    passed('interactive-Iced-window-choice-captures-a-window-without-publishing-the-screen-snapshot')
    before=files();path=begin(2);choose=picker();keys('-k','Escape');response(path,1);gone(choose);assert files()==before
    passed('cancelling-window-selection-publishes-nothing')
    action('move-window-to-workspace','--window-id',wid,'--focus','true','1');action('focus-window','--id',wid)
    path=begin(8);shot=review();s.capture('active-window-review.png')
    result.setdefault('intentionally_retired',[]).append(dict(pid=source.pid,command=source.args));s.retire(source)
    accept();blue(image(path));gone(shot)
    passed('active-window-is-resolved-before-Iced-and-frozen-review-survives-source-closure')
    before=files();path=begin(8);response(path,2);assert files()==before
    passed('missing-active-window-fails-without-capturing-a-different-target')
    path=begin(1);shot=review();keys('-M','alt','w','-m','alt');drain()
    assert not any(w['title']=='Choose a screenshot window — Canopa' for w in windows())
    accept();frame=image(path);gone(shot);assert frame.size==s.pixel_size
    passed('explicit-screen-capture-retains-its-target-and-physical-dimensions')
    niri=next(p for p in s.processes if str(s.compositor) in p.args)
    for process in [clipboard,niri]:
        result.setdefault('intentionally_retired',[]).append(dict(pid=process.pid,command=process.args));s.retire(process)
    s.wait_for(lambda:targets(BACKEND,IMPL)==5 and targets(FRONTEND,PUBLIC)==5,'window targets retire from frontend cache')
    result['checks'].append(dict(name='compositor-loss-withdraws-window-targets-from-backend-and-frontend'))
    c.close_sync(None)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['binary','niri','frontend']:
        p.add_argument('--'+name,type=Path,required=True);p.add_argument('--'+name+'-sha256',required=True)
    p.add_argument('--receiver',type=Path,default=None)
    p.add_argument('--pointer',type=Path,default=None)
    p.add_argument('--scheme',choices=['dark','light'],default='dark');p.add_argument('--scale',type=float,default=1.)
    p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    for name in ['binary','niri','frontend']:
        value=getattr(args,name).resolve(strict=True);setattr(args,name,value);assert digest(value)==getattr(args,name+'_sha256')
    args.receiver=(args.receiver if args.receiver is not None else receiver()).resolve(strict=True)
    args.pointer=(args.pointer if args.pointer is not None else pointer()).resolve(strict=True)
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    sources=[Path(__file__).resolve(),ROOT/'scripts/window_screenshot_fixture.py',ROOT/'scripts/test-remote-touch-lease.py',ROOT/'scripts/screencast_media.py',ROOT/'scripts/native_compositor.py',ROOT/'scripts/validate-desktop.py',ROOT/'scripts/desktop-fixture/remote-input-touch-receiver.c',ROOT/'scripts/desktop-fixture/click.c',ROOT/'scripts/desktop-fixture/wlr-virtual-pointer-unstable-v1.xml']
    (args.output/'sources').mkdir()
    for source in sources:shutil.copy2(source,args.output/'sources'/source.name)
    result=dict(status='running',checks=[],scheme=args.scheme,scale=args.scale,binaries={n:dict(path=str(getattr(args,n)),sha256=getattr(args,n+'_sha256')) for n in ['binary','niri','frontend']},sources={str(n.relative_to(ROOT)):digest(n) for n in sources})
    result['tools']={n:dict(path=str(getattr(args,n)),sha256=digest(getattr(args,n))) for n in ['receiver','pointer']}
    started=time.monotonic();code=1
    with tempfile.TemporaryDirectory(prefix='canopa-window-screenshot-') as folder:
        s=MediaSession(Path(folder),args.output,args.niri,args.niri_sha256)
        try:verify(args,s,result);result['status']='passed';code=0
        except Exception as error:result.update(status='failed',error=repr(error),traceback=traceback.format_exc())
        finally:
            processes=list(s.processes);s.close();result['processes']=[dict(pid=v.pid,command=v.args,exit_code=v.poll()) for v in processes]
    result.update(exit_code=code,seconds=round(time.monotonic()-started,2));(args.output/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));return code


if __name__=='__main__':raise SystemExit(main())
