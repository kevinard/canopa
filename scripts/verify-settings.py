#!/usr/bin/env python3
"""Accept Settings against private Niri, native input and optional real Stasis."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import struct
import zlib

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py')
desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)


def verify(args):
    output=args.output.resolve();output.mkdir(parents=True,exist_ok=False)
    result={'status':'failed','checks':[],'binary_sha256':hashlib.sha256(args.binary.read_bytes()).hexdigest()}
    with tempfile.TemporaryDirectory(prefix='c-settings-') as temporary:
        runtime=Path(temporary);runtime.chmod(0o700)
        session=desktop.Session(runtime,output)
        for variable in list(session.env):
            if variable.startswith(('CANOPA_', 'NIRIBAR_', 'STASIS_')):
                session.env.pop(variable)
        home=runtime/'home';home.mkdir()
        private_bin=runtime/'bin';private_bin.mkdir()
        niri=Path(args.niri or shutil.which('niri')).resolve()
        shutil.copy2(niri,private_bin/'niri')
        result['niri_sha256']=hashlib.sha256(niri.read_bytes()).hexdigest()
        if args.niri_sha256 and result['niri_sha256'] != args.niri_sha256.lower():
            raise ValueError('selected compositor SHA-256 does not match')
        snapshot=output/'executables';snapshot.mkdir()
        shutil.copy2(niri,snapshot/'niri')
        shutil.copy2(args.binary,snapshot/'canopa-settings')
        args.binary=snapshot/'canopa-settings'
        if hashlib.sha256(args.binary.read_bytes()).hexdigest()!=result['binary_sha256']:
            raise ValueError('Settings executable changed while taking its snapshot')
        session.env['CANOPA_NIRI_BIN']=str(private_bin/'niri')
        session.env.update(HOME=str(home),PATH=str(private_bin)+os.pathsep+str(args.tools_dir)+os.pathsep+session.env['PATH'],ICED_BACKEND='tiny-skia')
        if args.stasis:
            shutil.copy2(args.stasis,private_bin/'stasis')
            result['stasis_sha256']=hashlib.sha256(args.stasis.read_bytes()).hexdigest()
        profile=runtime/'config/canopa/niri.kdl'
        config=runtime/'config/canopa/config.kdl'
        displays=runtime/'config/canopa/niri-displays.kdl'
        input_file=runtime/'config/canopa/niri-input.kdl'
        app=None
        def key(*arguments):
            session.run(['wtype',*arguments])
        def tabs(count):
            return [item for _ in range(count) for item in ('-k','Tab')]
        def windows():
            return json.loads(session.run(['niri','msg','--json','windows']))
        def launch():
            process=session.start([str(args.binary)],f'settings-{len(session.processes)}.log')
            session.wait_for(lambda:next((w for w in windows() if w.get('app_id')=='org.canopa.Settings'),None),'settings window')
            time.sleep(.7)
            return process
        def close(process):
            process.terminate();process.wait(timeout=10)
            session.processes.remove(process)
            session.wait_for(lambda:not any(w.get('app_id')=='org.canopa.Settings' for w in windows()),'closed settings window')
        def scale():
            return json.loads(session.run(['niri','msg','--json','outputs']))[name]['logical']['scale']
        def check(label):
            result['checks'].append(label)
        try:
            session.run([str(args.binary),'init-session','--defaults-file',str(ROOT/'packaging/niri/defaults.kdl')])
            session.env['NIRI_CONFIG']=str(profile)
            name=session.start_niri(f'include "{profile}"\nanimations {{ off; }}\nxwayland-satellite {{ off; }}\nwindow-rule {{ match app-id="org.canopa.Settings"; open-maximized true; }}\n')
            app=launch();session.screenshot('displays-dark.png')
            check('ordinary shared-theme settings window')
            # Every key is sent in one native gesture, exercising synchronous Tab
            # traversal before queued text reaches the destination field.
            key(*tabs(2),'-k','Return',*tabs(4),'-M','ctrl','-k','a','-m','ctrl','us,fr','-M','ctrl','-k','s','-m','ctrl')
            session.wait_for(lambda:'us,fr' in input_file.read_text(),'saved input layouts')
            session.wait_for(lambda:len(json.loads(session.run(['niri','msg','--json','keyboard-layouts']))['names'])==2,'live keyboard layouts')
            time.sleep(.2);session.screenshot('input-dark.png');check('batched keyboard navigation and validated live input save')
            close(app);app=launch()
            original=displays.read_bytes()
            key(*tabs(10),'-k','Right','-M','ctrl','-k','s','-m','ctrl')
            session.wait_for(lambda:abs(scale()-1.25)<.001,'temporary display scale')
            time.sleep(.3);session.screenshot('display-confirmation.png')
            key('-k','Escape')
            session.wait_for(lambda:abs(scale()-1.)<.001,'Escape display rollback')
            assert displays.read_bytes()==original
            check('real compositor trial and Escape rollback without persistence')
            close(app);app=launch()
            key(*tabs(10),'-k','Right','-M','ctrl','-k','s','-m','ctrl')
            session.wait_for(lambda:abs(scale()-1.25)<.001,'second temporary display scale')
            time.sleep(.5);key('-M','ctrl','-k','Return','-m','ctrl')
            session.wait_for(lambda:'scale 1.25' in displays.read_text(),'confirmed display save')
            close(app)
            session.run(['niri','msg','action','load-config-file'])
            session.wait_for(lambda:abs(scale()-1.25)<.001,'persisted output reload')
            check('explicit confirmation survives compositor configuration reload')
            app=launch()
            key(*tabs(10),'-k','Right','-M','ctrl','-k','s','-m','ctrl')
            session.wait_for(lambda:abs(scale()-1.5)<.001,'crash trial scale')
            saved=displays.read_bytes();app.kill();app.wait(timeout=10)
            session.processes.remove(app)
            session.wait_for(lambda:abs(scale()-1.25)<.001,'independent helper after GUI crash')
            assert displays.read_bytes()==saved
            check('GUI SIGKILL leaves an independent rollback helper')
            # Restore physical scale for easily inspected personalization frames.
            session.run(['niri','msg','output',name,'scale','1'])
            session.wait_for(lambda:abs(scale()-1.)<.001,'native fixture scale')
            app=launch()
            key(*tabs(3),'-k','Return',*tabs(3),'-k','space','-k','Tab','-M','ctrl','-k','a','-m','ctrl','#f38ba8','-M','ctrl','-k','s','-m','ctrl')
            session.wait_for(lambda:config.exists() and '#f38ba8' in config.read_text(),'shared appearance save',timeout=30)
            time.sleep(.5);session.screenshot('personalization-light.png')
            assert 'light' in config.read_text();check('native light/accent preference save through owning validators')
            key('-M','ctrl','3','-m','ctrl');time.sleep(.2)
            key(*tabs(6),'-M','ctrl','-k','a','-m','ctrl','%H:%M:%S','-M','ctrl','-k','s','-m','ctrl')
            session.wait_for(lambda:'%H:%M:%S' in config.read_text(),'native lockscreen preference save')
            wallpaper=args.tools_dir.resolve()/'canopa-wallpaper'
            session.start([str(wallpaper),'daemon'],'wallpaper.log')
            def wallpaper_ready():
                reply=subprocess.run([str(wallpaper),'status'],env=session.env,capture_output=True,text=True,timeout=5)
                return reply.returncode==0 and bool(json.loads(reply.stdout).get('outputs'))
            session.wait_for(wallpaper_ready,'private wallpaper owner')
            library=home/'Pictures/Wallpapers';library.mkdir(parents=True)
            def png_chunk(kind, payload):
                return struct.pack('>I',len(payload))+kind+payload+struct.pack('>I',zlib.crc32(kind+payload))
            image=(b'\x89PNG\r\n\x1a\n'+png_chunk(b'IHDR',struct.pack('>IIBBBBB',64,64,8,2,0,0,0))
                   +png_chunk(b'IDAT',zlib.compress((b'\0'+bytes((40,80,110))*64)*64))+png_chunk(b'IEND',b''))
            (library/'fixture.png').write_bytes(image)
            key('-M','ctrl','3','-m','ctrl');time.sleep(.2)
            key(*tabs(5),'-k','Return')
            session.wait_for(lambda:any(w.get('app_id')=='org.canopa.WallpaperPicker' for w in windows()),'existing wallpaper picker')
            time.sleep(.3);session.screenshot('wallpaper-picker.png')
            key('-k','Escape')
            session.wait_for(lambda:not any(w.get('app_id')=='org.canopa.WallpaperPicker' for w in windows()),'closed wallpaper picker')
            check('lockscreen preference save and existing wallpaper picker launch')
            close(app);app=launch()
            key(*tabs(4),'-k','Return');time.sleep(2.2)
            session.screenshot('power.png')
            check('power page and explicit unavailable backend presentation')
            if args.stasis:
                close(app)
                session.env.update(CANOPA_LOCK='/usr/bin/true',CANOPA_IDLE='/usr/bin/true',CANOPA_NIRI=str(private_bin/'niri'))
                idle=runtime/'config/canopa/stasis.rune'
                text=idle.read_text().replace('enable_loginctl_integration true','enable_loginctl_integration false').replace('enable_dbus_inhibit true','enable_dbus_inhibit false')
                text+='\npresentation:\n  mode "overlay"\n  debounce_seconds 10\nend\n'
                idle.write_text(text)
                policy=session.start([str(private_bin/'stasis'),'--config',str(idle)],'stasis.log')
                def info():
                    process=subprocess.run([str(private_bin/'stasis'),'info','--json'],env=session.env,capture_output=True,text=True,timeout=5)
                    return json.loads(process.stdout) if process.returncode==0 and json.loads(process.stdout).get('alt')!='not_running' else None
                session.wait_for(info,'real Stasis policy')
                app=launch();key('-M','ctrl','4','-m','ctrl');time.sleep(1)
                session.screenshot('power-stasis-before-pause.png')
                key(*tabs(2),'-k','Return')
                session.wait_for(lambda:info() and info().get('alt')=='manually_inhibited','native idle pause')
                time.sleep(.2);key(*tabs(2),'-k','Return')
                session.wait_for(lambda:info() and info().get('alt')!='manually_inhibited','native idle resume')
                close(app);app=launch()
                key('-M','ctrl','4','-m','ctrl');time.sleep(1)
                key(*tabs(4),'-M','ctrl','-k','a','-m','ctrl','presentation','-k','Tab','-k','Return')
                session.wait_for(lambda:info() and info().get('profile')=='presentation','native idle profile selection')
                time.sleep(.3)
                key('-M','shift','-k','Tab','-m','shift','-M','ctrl','-k','a','-m','ctrl','none','-k','Tab','-k','Return')
                session.wait_for(lambda:info() and info().get('profile')=='default','native idle default profile restoration')
                check('native Stasis profile selection and default restoration')
                close(app);app=launch()
                key('-M','ctrl','4','-m','ctrl');time.sleep(1);key(*tabs(6),'-M','ctrl','-k','a','-m','ctrl','720','-M','ctrl','-k','s','-m','ctrl')
                session.wait_for(lambda:'timeout 720' in idle.read_text(),'native idle timeout save')
                assert '  lock_screen:\n    timeout 720' in idle.read_text()
                assert '    lock_screen:\n      timeout 120' in idle.read_text()
                assert policy.poll() is None
                assert 'CANOPA_LOCK' in idle.read_text()
                assert 'presentation:\n  mode "overlay"\n  debounce_seconds 10\nend\n' in idle.read_text()
                session.screenshot('power-stasis.png')
                check('real Stasis pause/resume and native timeout reload preserve commands')
            result['status']='passed'
            for file in (config,displays,input_file):
                if file.exists():shutil.copyfile(file,output/file.name)
        except Exception as error:
            result['error']=str(error)
            if isinstance(error,subprocess.CalledProcessError): result['diagnostics']=error.stderr
            try: session.screenshot('failure.png')
            except Exception: pass
            raise
        finally:
            session.close()
            (output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/canopa-settings')
    parser.add_argument('--tools-dir',type=Path,default=ROOT/'target/release')
    parser.add_argument('--niri',type=Path)
    parser.add_argument('--niri-sha256',help='require the exact selected compositor')
    parser.add_argument('--stasis',type=Path)
    parser.add_argument('--output',type=Path,required=True)
    verify(parser.parse_args())


if __name__=='__main__':main()
