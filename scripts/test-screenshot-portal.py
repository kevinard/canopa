#!/usr/bin/env python3
"""Exercise Screenshot/PickColor through a private frontend and real Iced UI."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT=Path(__file__).resolve().parents[1]
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py')
desktop=importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)
BACKEND='org.freedesktop.impl.portal.desktop.canopa'
FRONTEND='org.freedesktop.portal.Desktop'
PATH='/org/freedesktop/portal/desktop'
IMPL='org.freedesktop.impl.portal.Screenshot'
PUBLIC='org.freedesktop.portal.Screenshot'


def verify(args,session):
    from gi.repository import Gio,GLib
    from PIL import Image,ImageChops
    config=session.runtime/'config/canopa/config.kdl'
    config.parent.mkdir(parents=True)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.color_scheme}"; }}\n')
    home=session.runtime/'home';home.mkdir()
    services=session.runtime/'services';services.mkdir()
    portals=session.runtime/'portals';portals.mkdir()
    shutil.copy(ROOT/'packaging/portal/org.freedesktop.impl.portal.desktop.canopa.service',services)
    shutil.copy(Path('/usr/share/dbus-1/services/org.freedesktop.impl.portal.PermissionStore.service'),services)
    descriptor=(ROOT/'packaging/portal/canopa.portal').read_text()
    if not args.shipped_descriptor and IMPL+';' not in descriptor:
        descriptor=descriptor.replace('Interfaces=','Interfaces='+IMPL+';')
    assert IMPL+';' in descriptor
    (portals/'canopa.portal').write_text(descriptor)
    (portals/'portals.conf').write_text('[preferred]\ndefault=none\n'+IMPL+'=canopa\norg.freedesktop.impl.portal.Access=canopa\norg.freedesktop.impl.portal.Settings=canopa\n')
    applications=Path(session.env['XDG_DATA_HOME'])/'applications';applications.mkdir(parents=True)
    (applications/'org.canopa.ScreenshotTest.desktop').write_text('[Desktop Entry]\nType=Application\nName=Screenshot Fixture\nExec=true\n')
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home),WAYLAND_DEBUG='1',XDG_CURRENT_DESKTOP='Canopa',XDG_DESKTOP_PORTAL_DIR=str(portals),XDG_DATA_DIRS=session.env['XDG_DATA_HOME'],GIO_USE_VFS='local',GSETTINGS_BACKEND='memory',GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas',PATH=str(args.binary.parent)+os.pathsep+session.env['PATH'])
    output=session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { gaps 8; border { off; }; focus-ring { off; }; }\n',width=round(1280*args.scale),height=round(800*args.scale),scale=args.scale,service_directories=[services])
    session.start(['wtype','-s','600000'],'keyboard.log')
    flags=Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION
    def connect():return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],flags,None,None)
    connection=connect()
    def call(destination,path,interface,method,signature,values,client=connection):
        return client.call_sync(destination,path,interface,method,GLib.Variant(signature,values),None,Gio.DBusCallFlags.NONE,15000,None).unpack()
    def bus(method,signature,values):return call('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus',method,signature,values)
    def drain():
        while GLib.MainContext.default().pending():GLib.MainContext.default().iteration(False)
    bus('UpdateActivationEnvironment','(a{ss})',({k:v for k,v in session.env.items()},))
    frontend=next(p for p in [Path('/usr/libexec/xdg-desktop-portal'),Path('/usr/lib/xdg-desktop-portal')] if p.is_file())
    session.start([str(frontend),'--verbose'],'frontend.log')
    bus('StartServiceByName','(su)',(BACKEND,0))
    backend_pid=bus('GetConnectionUnixProcessID','(s)',(BACKEND,))[0]
    assert Path(f'/proc/{backend_pid}/exe').resolve()==args.binary
    session.wait_for(lambda:bus('NameHasOwner','(s)',(FRONTEND,))[0],'frontend')
    call(FRONTEND,PATH,'org.freedesktop.host.portal.Registry','Register','(sa{sv})',('org.canopa.ScreenshotTest',{}))
    assert call(BACKEND,PATH,'org.freedesktop.DBus.Properties','Get','(ss)',(IMPL,'version'))[0]==3
    assert call(FRONTEND,PATH,'org.freedesktop.DBus.Properties','Get','(ss)',(PUBLIC,'AvailableTargets'))[0]==5
    for name,xml in [('xdg-shell','stable/xdg-shell/xdg-shell.xml'),('xdg-foreign','unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
        for mode,suffix in [('client-header','.h'),('private-code','.c')]:
            subprocess.run(['wayland-scanner',mode,str(Path('/usr/share/wayland-protocols')/xml),str(session.output/(name+suffix))],check=True,timeout=15)
    parent_binary=session.output/'parent'
    subprocess.run(['cc','-DPORTAL_CAPTURE_PATTERN','-I',str(session.output),str(ROOT/'scripts/desktop-fixture/portal-parent.c'),str(session.output/'xdg-shell.c'),str(session.output/'xdg-foreign.c'),'-lwayland-client','-o',str(parent_binary)],check=True,timeout=30)
    parent=session.start([str(parent_binary)],'parent.log')
    exported=session.wait_for(lambda:next((line for line in (session.output/'parent.log').read_text().splitlines() if line.startswith('wayland:')),None),'parent export')
    def windows():return json.loads(session.run(['niri','msg','--json','windows']))
    parent_window=session.wait_for(lambda:next((w for w in windows() if w['pid']==parent.pid),None),'parent window')
    session.run(['niri','msg','action','move-window-to-floating','--id',str(parent_window['id'])])
    session.run(['niri','msg','action','move-floating-window','--id',str(parent_window['id']),'--x','60','--y','80'])
    pointer=session.build_pointer()
    logical=json.loads(session.run(['niri','msg','--json','outputs']))[output]['logical']
    width,height=logical['width'],logical['height']
    session.start(['wl-copy','--foreground','--type','text/plain','Canopa screenshot fixture'],'clipboard.log')
    def clipboard_ready():
        try:return session.run(['wl-paste','--no-newline'])=='Canopa screenshot fixture'
        except subprocess.CalledProcessError:return False
    session.wait_for(clipboard_ready,'clipboard marker')
    responses={}
    subscription=connection.signal_subscribe(FRONTEND,'org.freedesktop.portal.Request','Response',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:responses.__setitem__(p,v))
    count=0;checks=[]
    cache=Path(session.env['XDG_CACHE_HOME'])/'canopa/portal/screenshots'
    def files():return sorted(cache.glob('*.png')) if cache.exists() else []
    def begin(method='Screenshot',interactive=True,target=None,parent_window=exported,client=connection):
        nonlocal count
        count+=1
        options={'handle_token':GLib.Variant('s',f'shot{count}')}
        if method=='Screenshot':options['interactive']=GLib.Variant('b',interactive)
        if target is not None:options['target']=GLib.Variant('u',target)
        return call(FRONTEND,PATH,PUBLIC,method,'(sa{sv})',(parent_window,options),client)[0]
    def popup():
        window=session.wait_for(lambda:(drain(),next((w for w in windows() if w['app_id']=='org.canopa.Portal'),None))[1],'Iced dialog',timeout=30)
        if not window['is_floating']:session.run(['niri','msg','action','move-window-to-floating','--id',str(window['id'])])
        desired_width,desired_height=min(900,width-40),min(680,height-20)
        session.run(['niri','msg','action','set-window-width','--id',str(window['id']),str(desired_width)])
        session.run(['niri','msg','action','set-window-height','--id',str(window['id']),str(desired_height)])
        window=session.wait_for(lambda:next((w for w in windows() if w['id']==window['id'] and abs(w['layout']['window_size'][0]-desired_width)<2 and abs(w['layout']['window_size'][1]-desired_height)<2),None),'dialog geometry')
        session.run(['niri','msg','action','move-floating-window','--id',str(window['id']),'--x','20','--y','10'])
        session.run(['niri','msg','action','focus-window','--id',str(window['id'])])
        time.sleep(.2)
        return window
    def action(window,accept):
        w,h=window['layout']['window_size']
        session.run([str(pointer),str(round((20+w-(66 if accept else 160))/width*1600)),str(round((10+h-38)/height*1000))])
    def result(path,status):
        response=session.wait_for(lambda:(drain(),responses.get(path))[1],'portal response',timeout=40)
        assert response.get_child_value(0).unpack()==status,response
        return response.get_child_value(1).unpack()
    def closed(window):session.wait_for(lambda:not any(w['id']==window['id'] for w in windows()),'dialog closes')
    def image_result(path):
        values=result(path,0);uri=values['uri']
        if hasattr(uri,'unpack'):uri=uri.unpack()
        from urllib.parse import urlsplit,unquote
        file=Path(unquote(urlsplit(uri).path));assert file.is_relative_to(cache),file
        return file,Image.open(file).convert('RGBA')
    def color_box(frame,color,box=None):
        region=frame.crop(box) if box else frame
        difference=ImageChops.difference(region.convert('RGB'),Image.new('RGB',region.size,color))
        channels=difference.split()
        distance=ImageChops.lighter(ImageChops.lighter(channels[0],channels[1]),channels[2])
        bounds=distance.point(lambda value:255 if value<=2 else 0).getbbox()
        assert bounds,'source color absent from review'
        return tuple(bounds[i]+(box[i%2] if box else 0) for i in range(4))

    path=begin();window=popup()
    assert 'Screenshot' in window['title'] and window['is_floating']
    session.capture('01-review.png')
    session.run(['wtype','-k','Return']);drain();assert path not in responses
    action(window,False);result(path,1);closed(window);assert files()==[]
    checks.append('interactive public request opens parented Iced review; initial Enter and Cancel publish nothing')

    reference=session.capture('02-source.png')
    path=begin(target=1);window=popup()
    extra=session.start([str(parent_binary)],'extra.log')
    session.wait_for(lambda:any(w['pid']==extra.pid for w in windows()),'changed desktop')
    action(window,True);file,image=image_result(path);closed(window)
    assert image.size==reference.size
    assert ImageChops.difference(image.convert('RGB'),reference).getbbox() is None,'accepted screenshot reread changed desktop'
    assert file.stat().st_mode&0o777==0o400
    session.retire(extra)
    checks.append('explicit Share publishes the frozen complete physical image after the live desktop changes')

    reference=session.capture('03-color-source.png')
    path=begin('PickColor');window=popup()
    session.run(['wtype','-k','Right'])
    action(window,True)
    values=result(path,0);color=values['color'];color=color.unpack() if hasattr(color,'unpack') else color
    expected=reference.getpixel((reference.width//2+1,reference.height//2))
    assert all(abs(color[i]-expected[i]/255)<1e-8 for i in range(3)),(color,expected)
    closed(window)
    checks.append('public PickColor confirms exact source RGB with keyboard pixel adjustment')

    before=files();path=begin('PickColor');window=popup()
    frame=session.capture('04-color-review.png')
    w,h=window['layout']['window_size']
    box=color_box(frame,(20,150,40),(round(20*args.scale),round(140*args.scale),round((20+w)*args.scale),round((10+h-100)*args.scale)))
    px,py=(box[0]+box[2])/2,(box[1]+box[3])/2
    session.run([str(pointer),str(round(px/frame.width*1600)),str(round(py/frame.height*1000))])
    session.capture('05-color-selected.png')
    action(window,True);values=result(path,0);color=values['color'];color=color.unpack() if hasattr(color,'unpack') else color
    assert all(abs(color[i]-(20,150,40)[i]/255)<1e-8 for i in range(3)),color
    closed(window);assert files()==before
    checks.append('pointer selection maps the scaled preview to exact original RGB and PickColor publishes no image')

    reference=session.capture('04-area-source.png')
    path=begin(target=4);window=popup()
    green=color_box(reference,(20,150,40))
    origin=(green[0]+8,green[3]-5)
    moves=[]
    for delta,negative,positive in [(origin[0]-reference.width//2,'Left','Right'),(origin[1]-reference.height//2,'Up','Down')]:
        key=positive if delta>=0 else negative
        large,small=divmod(abs(delta),10)
        if large:moves+=['-M','shift']+['-k',key]*large+['-m','shift']
        moves+=['-k',key]*small
    subprocess.run(['wtype',*moves],env=session.env,check=True,timeout=30)
    session.run(['wtype','-k','space','-M','shift','-k','Right','-k','Down','-m','shift','-k','space'])
    action(window,True);_,image=image_result(path);closed(window)
    assert image.size==(11,11),image.size
    expected=reference.crop((origin[0],origin[1],origin[0]+11,origin[1]+11)).convert('RGB')
    assert len(expected.getcolors(121))>1,'area fixture must cross source color boundaries'
    assert ImageChops.difference(image.convert('RGB'),expected).getbbox() is None
    assert image.getextrema()[3]==(255,255)
    checks.append('explicit Area target uses Iced keyboard selection and publishes only selected source pixels')

    path=begin(target=4);window=popup()
    frame=session.capture('06-area-review.png');w,h=window['layout']['window_size']
    box=color_box(frame,(20,150,40),(round(20*args.scale),round(140*args.scale),round((20+w)*args.scale),round((10+h-100)*args.scale)))
    points=[(box[0]+(box[2]-box[0])*part,box[1]+(box[3]-box[1])*part) for part in (.3,.6)]
    coordinates=[str(round(value/size*extent)) for point in points for value,size,extent in zip(point,frame.size,(1600,1000))]
    drag=session.build_pointer('drag');ready=session.runtime/'drag-ready';release=session.runtime/'drag-release'
    dragging=session.start([str(drag),*coordinates,str(ready),str(release)],'drag.log')
    session.wait_for(ready.exists,'area pointer held');release.touch()
    assert dragging.wait(timeout=5)==0;session.processes.remove(dragging)
    action(window,True);_,image=image_result(path);closed(window)
    assert image.width>1 and image.height>1
    assert image.convert('RGB').getcolors(image.width*image.height)==[(image.width*image.height,(20,150,40))]
    checks.append('real pointer drag selects and publishes only original pixels inside the preview area')

    before=files();path=begin();window=popup()
    call(FRONTEND,path,'org.freedesktop.portal.Request','Close','()',())
    closed(window);assert files()==before
    checks.append('public Close retires the review and leaves existing cache images intact')

    before=files();lost=connect()
    call(FRONTEND,PATH,'org.freedesktop.host.portal.Registry','Register','(sa{sv})',('org.canopa.ScreenshotTest',{}),lost)
    begin(client=lost);window=popup();lost.close_sync(None);closed(window)
    assert files()==before
    checks.append('frontend caller loss retires its real Iced helper without publication')

    path=begin(interactive=False,target=1);window=popup()
    assert window['title'] not in ('Screenshot — Canopa','Choose a color — Canopa'),'frontend permission check unexpectedly skipped'
    action(window,True);image_result(path);closed(window)
    path=begin(interactive=False,target=1);image_result(path)
    assert not any(w['app_id']=='org.canopa.Portal' for w in windows())
    checks.append('frontend Access consent controls non-interactive capture; subsequent permitted requests need no second review')

    old={str(file):hashlib.sha256(file.read_bytes()).hexdigest() for file in files()}
    path=begin();window=popup();os.kill(backend_pid,signal.SIGTERM)
    closed(window);result(path,2)
    session.wait_for(lambda:not bus('NameHasOwner','(s)',(BACKEND,))[0],'backend exits')
    assert {str(file):hashlib.sha256(file.read_bytes()).hexdigest() for file in files()}==old
    bus('StartServiceByName','(su)',(BACKEND,0))
    session.wait_for(lambda:call(FRONTEND,PATH,'org.freedesktop.DBus.Properties','Get','(ss)',(PUBLIC,'AvailableTargets'))[0]==5,'frontend observes restored target capabilities')
    image_result(begin(interactive=False,target=1))
    assert all(Path(file).exists() and hashlib.sha256(Path(file).read_bytes()).hexdigest()==digest for file,digest in old.items())
    checks.append('backend loss cancels an unaccepted review; accepted image files remain readable across restart')

    before=files();cache.chmod(0)
    try:result(begin(interactive=False,target=1),2)
    finally:cache.chmod(0o700)
    assert files()==before
    assert session.run(['wl-paste','--no-newline'])=='Canopa screenshot fixture'
    checks.append('publication failure is explicit and unchanged clipboard/cache images are preserved')
    connection.signal_unsubscribe(subscription)
    return {'status':'passed','checks':checks,'color_scheme':args.color_scheme,'scale':args.scale,'shipped_descriptor':args.shipped_descriptor,'binary_sha256':hashlib.sha256(args.binary.read_bytes()).hexdigest()}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--color-scheme',choices=['dark','light'],default='dark')
    parser.add_argument('--scale',type=float,default=1)
    parser.add_argument('--shipped-descriptor',action='store_true')
    args=parser.parse_args();args.binary=args.binary.resolve(strict=True);args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='canopa-screenshot-portal-') as folder:
        session=desktop.Session(Path(folder),args.output)
        try:result=verify(args,session)
        except Exception as error:
            desktop.write_json(args.output/'result.json',{'status':'failed','error':str(error)})
            raise
        finally:session.close()
    desktop.write_json(args.output/'result.json',result);print(json.dumps(result))

if __name__=='__main__':main()
