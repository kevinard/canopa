#!/usr/bin/env python3
"""Private two-output wallpaper persistence, source events and optional slideshow."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import native_accessibility as a11y
from PIL import Image
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py')
desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)

def verify(binary,session):
    root=session.runtime
    session.start_private_bus()
    config=root/'sway.conf'
    config.write_text('output HEADLESS-1 mode 1280x800 position 0 0\noutput HEADLESS-2 mode 800x1200 position 1280 0\nseat seat0 fallback true\nxwayland disable\n')
    session.env.update(WLR_BACKENDS='headless',WLR_HEADLESS_OUTPUTS='2',WLR_RENDERER='pixman')
    session.start(['sway','-c',str(config)],'sway.log')
    session.env['SWAYSOCK']=str(session.wait_for(lambda:list(root.glob('sway-ipc.*.sock')),'Sway IPC')[0])
    session.env['WAYLAND_DISPLAY']=str(session.wait_for(lambda:[p for p in root.glob('wayland-*') if p.is_socket()],'Wayland')[0])
    a11y.start(session)
    protocol=ROOT/'scripts/desktop-fixture/virtual-keyboard-unstable-v1.xml'
    for mode,name in [('client-header','virtual-keyboard.h'),('private-code','virtual-keyboard.c')]:
        subprocess.run(['wayland-scanner',mode,str(protocol),str(session.output/name)],check=True)
    keyboard=session.output/'keyboard'
    subprocess.run(['cc','-Wall','-Wextra','-Wno-misleading-indentation','-I',str(session.output),str(ROOT/'scripts/desktop-fixture/session-keyboard.c'),str(session.output/'virtual-keyboard.c'),'-lwayland-client','-lxkbcommon','-o',str(keyboard)],check=True)
    session.start([str(keyboard),'hold'],'keyboard.log')
    images=root/'images';images.mkdir()
    red,blue,gold,green=[images/(name+'.png') for name in ['red','blue','gold','green']]
    colors=[(220,35,45),(25,75,190),(210,160,30),(35,180,80)]
    for path,color in zip([red,blue,gold,green],colors):Image.new('RGB',(128,80),color).save(path)
    wallpaper_config=root/'wallpaper.kdl'
    wallpaper_config.write_text(f'schema-version 1\nwallpaper {{ image "{red}"; }}\n')
    daemon=None;launches=0
    def ctl(*args,success=True):
        result=subprocess.run([str(binary),*map(str,args)],env=session.env,capture_output=True,text=True,timeout=20)
        assert (result.returncode==0)==success,(args,result.stdout,result.stderr)
        return json.loads(result.stdout) if success else result
    def outputs():
        try:return ctl('status')['outputs']
        except (AssertionError,subprocess.SubprocessError):return []
    def start():
        nonlocal daemon,launches
        launches+=1
        daemon=session.start([str(binary),'daemon','--canopa-config',str(wallpaper_config)],f'wallpaper-{launches}.log')
        session.wait_for(lambda:len(outputs())==2 and all(o['width']>0 for o in outputs()),'two wallpaper outputs')
    def stop():
        ctl('stop');assert daemon.wait(timeout=8)==0;session.processes.remove(daemon)
    def painted(first,second,name):
        def check():
            image=session.capture(name)
            return image.getpixel((100,100))==first and image.getpixel((1380,100))==second
        session.wait_for(check,name)
    start();painted(colors[0],colors[0],'initial.png')
    ctl('set',blue,'--output','HEADLESS-2');painted(colors[0],colors[1],'selected.png')
    stop();start();painted(colors[0],colors[1],'restored.png')
    state=Path(session.env['XDG_STATE_HOME'])/'canopa/wallpaper.json'
    assert state.stat().st_mode&0o777==0o600
    ctl('set',green,'--output','HEADLESS-1','--temporary')
    ctl('set',gold,'--output','HEADLESS-2');painted(colors[3],colors[2],'temporary.png')
    stop();start();painted(colors[0],colors[2],'temporary-not-saved.png')
    # Real file replacement events, with no set/reload request.
    temporary=images/'image.new';Image.new('RGB',(128,80),(230,90,20)).save(temporary,format='PNG');temporary.replace(gold)
    painted(colors[0],(230,90,20),'watched-image.png')
    gold.write_bytes(b'invalid image')
    time.sleep(.4);painted(colors[0],(230,90,20),'invalid-watched-image.png')
    Image.new('RGB',(128,80),colors[2]).save(temporary,format='PNG');temporary.replace(gold)
    painted(colors[0],colors[2],'recovered-image.png')
    wallpaper_config.write_text(f'schema-version 1\nwallpaper {{ image "{blue}"; }}\n')
    painted(colors[1],colors[2],'watched-config.png')
    wallpaper_config.write_text('schema-version 1\nwallpaper { fit "invalid"; }\n')
    time.sleep(.3);painted(colors[1],colors[2],'invalid-config.png')
    wallpaper_config.write_text(f'schema-version 1\nwallpaper {{ image "{blue}"; }}\n')
    ctl('reload');painted(colors[1],colors[1],'explicit-reload.png')
    # Connector assignments survive removal/reappearance, including changes while absent.
    session.run(['swaymsg','output','HEADLESS-2','disable'])
    session.wait_for(lambda:len(outputs())==1,'output retirement')
    ctl('set',gold,'--output','HEADLESS-2')
    session.run(['swaymsg','output','HEADLESS-2','enable'])
    session.wait_for(lambda:len(outputs())==2 and all(o['width']>0 for o in outputs()),'output restoration')
    painted(colors[1],colors[2],'hotplug.png')
    slides=root/'slides';slides.mkdir()
    for name,color in [('01.png',colors[0]),('02.png',colors[1])]:Image.new('RGB',(128,80),color).save(slides/name)
    (slides/'03.png').write_bytes(b'invalid slide')
    ctl('slideshow',slides,'--all','--interval','10');painted(colors[0],colors[0],'slideshow-first.png')
    ctl('next');painted(colors[1],colors[1],'slideshow-next.png')
    ctl('next',success=False);painted(colors[1],colors[1],'slideshow-invalid.png')
    ctl('next');painted(colors[0],colors[0],'slideshow-skips-invalid.png')
    session.wait_for(lambda:all(Path(o['background']['image']).name=='02.png' for o in outputs()),'slideshow timer',timeout=13)
    painted(colors[1],colors[1],'slideshow-timer.png')
    ctl('stop-slideshow');assert ctl('status')['slideshow'] is None
    stop();start();painted(colors[1],colors[1],'stopped-slide-restored.png')
    # The visual picker renders monitor-aspect previews without changing assignments.
    def node(name,*args):return a11y.request(session,'--window','Canopa Wallpaper','--name',name,*args)
    def picker(image,label):
        process=session.start([str(binary),'pick',str(image)],label+'.log')
        node('Apply wallpaper')
        return process
    def close_picker(process):
        node('Close','--click');assert process.wait(timeout=8)==0;session.processes.remove(process)
    def preview(color,name):
        def visible():
            image=session.capture(name)
            pixels=image.get_flattened_data() if hasattr(image,'get_flattened_data') else image.getdata()
            return sum(tuple(pixel[:3])==color for pixel in pixels)>5000
        session.wait_for(visible,'rendered monitor preview')
    selected=picker(red,'picker-cancel')
    preview(colors[0],'picker-all-preview.png')
    assert all(Path(o['background']['image']).name=='02.png' for o in outputs())
    close_picker(selected);painted(colors[1],colors[1],'picker-cancel-preserves.png')
    selected=picker(gold,'picker-apply')
    preview(colors[2],'picker-gold-preview.png')
    node('Monitor: All monitors   · ↑/↓','--click')
    node('Monitor: HEADLESS-1   · ↑/↓','--click')
    node('Monitor: HEADLESS-2   · ↑/↓')
    for fit in ['Fill','Fit','Stretch','Center']:
        node('Fit: '+fit+'   · F','--click')
    preview(colors[2],'picker-selected-preview.png')
    node('Apply wallpaper','--click')
    session.wait_for(lambda:any(o['name']=='HEADLESS-2' and o['background']['image']==str(gold) for o in outputs()),'picker apply to selected display')
    close_picker(selected);painted(colors[1],colors[2],'picker-selected-applied.png')
    selected=picker(slides/'03.png','picker-invalid')
    time.sleep(.3);node('Apply wallpaper','--click');time.sleep(.2)
    close_picker(selected);painted(colors[1],colors[2],'picker-invalid-preserves.png')
    time.sleep(.5)
    def ticks():return sum(map(int,Path(f'/proc/{daemon.pid}/stat').read_text().split(') ',1)[1].split()[11:13]))
    before=ticks();time.sleep(2);idle_ticks=ticks()-before;assert idle_ticks<=1,idle_ticks
    stop()
    result={'status':'passed','idle_ticks_2s':idle_ticks,'scope':'private Sway outputs, no physical connector/DRM claim',
        'checks':['per-output persistence across restart','private atomic state file','temporary choices stay temporary after another save',
        'atomic image replacement is watched','invalid changed image preserves displayed pixels','image recovery is watched',
        'configuration updates preserve manual overlay','invalid configuration preserves pixels','explicit reload restores configured assignments',
        'output retirement and reappearance','assignment while output absent','optional slideshow starts and advances','invalid slide rollback',
        'bad slide is skipped on subsequent advance','timer advances slideshow','stop removes timer and retains image across restart','no idle polling CPU ticks','visual all-monitor previews','preview/cancel leaves assignments intact','monitor selector and fit controls','picker applies only to selected monitor','invalid picker image cannot apply','native accessible picker controls']}
    (session.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/canopa-wallpaper');parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='canopa-wallpaper-followups-') as temporary:
        root=Path(temporary);root.chmod(0o700);session=desktop.Session(root,args.output.resolve())
        try:verify(args.binary.resolve(),session)
        finally:session.close()
if __name__=='__main__':main()
