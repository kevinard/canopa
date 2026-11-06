#!/usr/bin/env python3
"""Private greeter success-only state, live catalogs/avatars and logind power policy."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from PIL import Image
import native_accessibility as a11y
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py')
desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)

def verify(binary,session):
    root=session.runtime
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n')
    a11y.start(session)
    session.env['DBUS_SYSTEM_BUS_ADDRESS']=session.env['DBUS_SESSION_BUS_ADDRESS']
    for script in ['accounts','lock-logind','greetd']:
        session.start(['/usr/bin/python3',str(ROOT/f'scripts/desktop-fixture/{script}.py'),str(root)],script+'.log')
    session.wait_for(lambda:(root/'accounts-ready').exists() and (root/'logind-ready').exists() and (root/'greetd.sock').exists(),'private greeter peers')
    session.env['GREETD_SOCK']=str(root/'greetd.sock')
    for name,color in [('alice',(200,50,60)),('bob',(40,120,220))]:
        Image.new('RGB',(96,96),color).save(root/('avatar-'+name),format='PNG')
    sessions=root/'sessions';sessions.mkdir()
    for name,label in [('canopa','Canopa'),('other','Other')]:
        (sessions/(name+'.desktop')).write_text(f'[Desktop Entry]\nType=Application\nName={label}\nExec=/usr/bin/true\n')
    config=root/'greeter.kdl'
    config.write_text(f'''schema-version 1
greeter {{
 users "alice" "bob"
 user-source "accountsservice"
 session-dirs "{sessions}"
 view {{ auth-timeout 30; widgets "avatar"; }}
}}
''')
    protocol=ROOT/'scripts/desktop-fixture/virtual-keyboard-unstable-v1.xml'
    for mode,name in [('client-header','virtual-keyboard.h'),('private-code','virtual-keyboard.c')]:
        subprocess.run(['wayland-scanner',mode,str(protocol),str(session.output/name)],check=True)
    keyboard=session.output/'keyboard'
    subprocess.run(['cc','-Wall','-Wextra','-Wno-misleading-indentation','-I',str(session.output),str(ROOT/'scripts/desktop-fixture/session-keyboard.c'),str(session.output/'virtual-keyboard.c'),'-lwayland-client','-lxkbcommon','-o',str(keyboard)],check=True)
    session.start([str(keyboard),'hold'],'keyboard.log')
    state=Path(session.env['XDG_STATE_HOME'])/'canopa/greeter.json'
    def events():
        return json.loads((root/'greetd-events.json').read_text())
    def power_events():
        return [e for e in json.loads((root/'logind-state.json').read_text())['events'] if 'power' in e]
    def key(value):session.run([str(keyboard),'key',value])
    def node(name,*args):return a11y.request(session,'--window','Canopa Login','--name',name,*args)
    def start(name,refused=False):
        (root/'greetd-control.json').write_text(json.dumps({'mode':'password','start-refused':refused}))
        before=len(events())
        process=session.start([str(binary),'--canopa-config',str(config)],name+'.log')
        session.wait_for(lambda:any(e['event']=='cancel' for e in events()[before:]),'initial greetd reset')
        session.wait_for(lambda:json.loads(session.run(['niri','msg','--json','windows'])),'greeter window')
        window=max(json.loads(session.run(['niri','msg','--json','windows'])),key=lambda w:w['id'])
        session.run(['niri','msg','action','focus-window','--id',str(window['id'])]);time.sleep(.5)
        node('Username:')
        return process,before
    def prompt(before):
        session.wait_for(lambda:any(e['event']=='prompt' and e['text']=='Password:' for e in events()[before:]),'fresh password prompt')
    def authenticate():
        node('Password:','--value','canopa-fixture-password');time.sleep(.1)
        node('Submit response','--click')
    def completed(process):
        assert process.wait(timeout=10)==0;session.processes.remove(process)
    app,before=start('first')
    assert not state.exists()
    key('Down');key('Tab');key('Return');prompt(before)
    node('Password:','--value','wrong');time.sleep(.1);node('Submit response','--click');time.sleep(2.2)
    assert not state.exists(),'failed authentication persisted a choice'
    key('Return');prompt(len(events())-1);authenticate();completed(app)
    saved=json.loads(state.read_text());assert saved=={'last_user':'bob','sessions':{'bob':'other'}},saved
    assert state.stat().st_mode&0o777==0o600
    app,before=start('remembered')
    assert json.loads(node('Username:'))['text']=='bob'
    node('Session: Other   ‹ Tab ›')
    key('Up');node('Session: Canopa   ‹ Tab ›')
    key('Return');prompt(before);authenticate();completed(app)
    saved=json.loads(state.read_text());assert saved['sessions']=={'alice':'canopa','bob':'other'}
    app,before=start('live')
    # AccountsService changes refresh labels without replacing typed canonical input.
    (root/'accounts-control.json').write_text(json.dumps({'revision':1,'real_names':{'alice':'Alice Updated'},'extra_user':True}))
    node('User: Alice Updated (alice)')
    node('Username:','--value','carol');time.sleep(.2);node('User: Carol Example (carol)')
    node('Username:','--value','bob');time.sleep(.2)
    def avatar(color,name):
        def visible():
            image=session.capture(name)
            pixels=image.get_flattened_data() if hasattr(image,'get_flattened_data') else image.getdata()
            return sum(tuple(p[:3])==color for p in pixels)>2000
        session.wait_for(visible,'updated avatar '+name)
    avatar((40,120,220),'avatar-before.png')
    temporary=root/'avatar.new';Image.new('RGB',(96,96),(30,190,80)).save(temporary,format='PNG');temporary.replace(root/'avatar-bob')
    avatar((30,190,80),'avatar-after.png')
    # A newly installed desktop appears without restarting the frontend.
    third=sessions/'third.desktop';third.write_text('[Desktop Entry]\nType=Application\nName=Third\nExec=/usr/bin/true\n')
    time.sleep(.3);key('Tab');key('Tab');node('Session: Third   ‹ Tab ›')
    at=len(events());key('Return');prompt(at)
    third.unlink()
    session.wait_for(lambda:any(e['event']=='cancel' for e in events()[at:]),'catalog removal cancels active authentication')
    node('Username:')
    assert not any(e['event']=='handoff' for e in events()[before:])
    assert json.loads(state.read_text())==saved,'catalog changes or cancellation changed saved preferences'
    # Frontend confirmation and a fresh backend CanPowerOff check both precede a call.
    control=root/'logind-control.json'
    control.write_text(json.dumps({'power':'yes','power-revision':1}))
    node('Power   · F10','--click');node('› Shut down','--click');node('Confirm shut down')
    assert not power_events()
    control.write_text(json.dumps({'power':'no','power-revision':1}))
    node('Confirm shut down','--click');time.sleep(.3)
    node('Username:');assert not power_events(),'power request bypassed the fresh policy check'
    control.write_text(json.dumps({'power':'yes','power-revision':2}))
    node('Power   · F10','--click');node('Restart','--click');node('Confirm restart','--click')
    session.wait_for(power_events,'confirmed logind request')
    assert power_events()==[{'power':'Reboot','interactive':False,'permitted':True}],power_events()
    session.retire(app)
    assert json.loads(state.read_text())==saved
    # A refused session launch cannot overwrite the previous successful selection.
    app,before=start('refused',True)
    key('Down');key('Tab');key('Return');prompt(before);authenticate()
    session.wait_for(lambda:any(e['event']=='start-refused' for e in events()[before:]),'refused handoff')
    assert json.loads(state.read_text())==saved;session.retire(app)
    result={'status':'passed','checks':['failed login does not persist','success-only last user','private atomic preference file',
        'per-user session recall','AccountsService live display names and added user','typed username preserved',
        'atomic avatar replacement invalidates cache','new desktop detected','removed desktop cancels active authentication',
        'catalog updates cannot redirect authentication','power confirmation','fresh logind policy check','permitted confirmed restart',
        'power actions do not change saved choices','failed session launch does not persist']}
    (session.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/canopa-greeter');parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='canopa-greeter-followups-') as temporary:
        root=Path(temporary);root.chmod(0o700);session=desktop.Session(root,args.output.resolve())
        try:verify(args.binary.resolve(),session)
        finally:
            for name in ['greetd-events.json','accounts-events.json','logind-state.json']:
                if (root/name).exists():(session.output/name).write_bytes((root/name).read_bytes())
            session.close()
if __name__=='__main__':main()
