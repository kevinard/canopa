#!/usr/bin/env python3
"""Private Wayland/logind/polkit authority and native-session acceptance; no host grants."""
import argparse,importlib.util,json,os,shlex,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
from PIL import Image
import native_accessibility as a11y
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py');desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)
def verify(binary,session):
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nwindow-rule { match app-id="org.canopa.Polkit"; open-floating true; }\n')
    a11y.start(session)
    root=session.runtime;config=root/'config.kdl'
    config.write_text('schema-version 1\npolkit { auth-timeout 5; }\n')
    session.env['DBUS_SYSTEM_BUS_ADDRESS']=session.env['DBUS_SESSION_BUS_ADDRESS']
    session.env.pop('XDG_SESSION_ID',None)
    session.start(['/usr/bin/python3',str(ROOT/'scripts/desktop-fixture/lock-logind.py'),str(root)],'logind.log')
    session.wait_for(lambda:(root/'logind-ready').exists(),'private logind')
    authority=session.start(['/usr/bin/python3',str(ROOT/'scripts/desktop-fixture/polkit.py'),str(root)],'authority.log')
    session.wait_for(lambda:(root/'polkit-ready').exists(),'private authority')
    # Public identity metadata is resolved only for authority-permitted UIDs.
    Image.new('RGBA',(32,48),(220,40,60,255)).save(root/'avatar-alice',format='PNG')
    session.start(['/usr/bin/python3',str(ROOT/'scripts/desktop-fixture/accounts.py'),str(root)],'accounts.log')
    session.wait_for(lambda:(root/'accounts-ready').exists(),'private account metadata')
    library=session.output/'session.so';flags=shlex.split(subprocess.check_output(['pkg-config','--cflags','--libs','polkit-agent-1'],text=True))
    subprocess.run(['cc','-shared','-fPIC','-Wall','-Wextra','-Werror','-Wno-misleading-indentation',str(ROOT/'scripts/desktop-fixture/polkit-session.c'),'-o',str(library),*flags],check=True)
    protocol=ROOT/'scripts/desktop-fixture/virtual-keyboard-unstable-v1.xml'
    for mode,name in [('client-header','virtual-keyboard.h'),('private-code','virtual-keyboard.c')]:subprocess.run(['wayland-scanner',mode,str(protocol),str(session.output/name)],check=True)
    keyboard=session.output/'keyboard';subprocess.run(['cc','-Wall','-Wextra','-Wno-misleading-indentation','-I',str(session.output),str(ROOT/'scripts/desktop-fixture/session-keyboard.c'),str(session.output/'virtual-keyboard.c'),'-lwayland-client','-lxkbcommon','-o',str(keyboard)],check=True)
    session.start([str(keyboard),'hold'],'keyboard.log')
    def events():
        try:return json.loads((root/'polkit-events.json').read_text())
        except FileNotFoundError:return []
    def native_events():
        try:return (root/'polkit-session-events').read_text().splitlines()
        except FileNotFoundError:return []
    def wait(fn,label,timeout=12):return session.wait_for(fn,label,timeout=timeout)
    def windows():return [w for w in json.loads(session.run(['niri','msg','--json','windows'])) if w.get('app_id')=='org.canopa.Polkit']
    def key(value):session.run([str(keyboard),'key',value])
    def answer(value):session.run([str(keyboard),'text',value])
    def start(name,selected=False):
        before=sum(e['event']=='registered' for e in events())
        notify_path=root/(name+'-notify.sock');notify=socket.socket(socket.AF_UNIX,socket.SOCK_DGRAM);notify.bind(str(notify_path));notify.settimeout(5)
        old=session.env.copy();session.env['NOTIFY_SOCKET']=str(notify_path);session.env.update(LD_PRELOAD=str(library),CANOPA_POLKIT_FIXTURE_ROOT=str(root),CANOPA_POLKIT_ASSERT_ENV='1',POLKIT_DEBUG='1',GREETD_SOCK='/must-not-inherit')
        command=[str(binary),'--canopa-config',str(config)]
        if selected:command+=['--session','canopa-fixture']
        process=session.start(command,name+'.log');session.env=old
        wait(lambda:sum(e['event']=='registered' for e in events())>before,'agent registration');assert notify.recv(128)==b'READY=1';notify.close();time.sleep(.2)
        return process
    def stop(process,expected=0):
        process.send_signal(signal.SIGTERM);assert process.wait(timeout=8)==expected;session.processes.remove(process)
    # Preview and schema checks have no registration or custom command execution.
    preview=session.output/'preview.png';session.run([str(binary),'--canopa-config',str(config),'preview',str(preview)])
    with Image.open(preview) as image:image.verify()
    assert not any(e['event']=='registered' for e in events())
    foreign=session.start([str(binary),'--canopa-config',str(config),'--session','other-user'],'foreign-session.log');assert foreign.wait(timeout=6)!=0;session.processes.remove(foreign)
    (root/'logind-control.json').write_text(json.dumps({'no-pid-session':True,'ambiguous':True}))
    ambiguous=session.start([str(binary),'--canopa-config',str(config)],'ambiguous-session.log');assert ambiguous.wait(timeout=6)!=0;session.processes.remove(ambiguous)
    (root/'logind-control.json').write_text(json.dumps({'no-pid-session':True}))
    app=start('agent');assert not windows()
    duplicate=session.start([str(binary),'--canopa-config',str(config),'--session','canopa-fixture'],'duplicate.log');assert duplicate.wait(timeout=8)!=0;session.processes.remove(duplicate);assert app.poll() is None
    nonce=0
    def request(mode='password',uids=None,message=None,details=None,publisher=None):
        nonlocal nonce
        nonce+=1;(root/'polkit-touch').unlink(missing_ok=True)
        data=dict(nonce=nonce,mode=mode,uids=[os.getuid()] if uids is None else uids)
        if message is not None:data['message']=message
        if details is not None:data['details']=details
        if publisher is not None:data['publisher']=publisher
        before=len(events());native_before=len(native_events());(root/'polkit-control.json').write_text(json.dumps(data))
        wait(lambda:len(events())>before and any(e['event']=='begin' for e in events()[before:]),'authority request')
        wait(windows,'authentication dialog');window=windows()[-1];session.run(['niri','msg','action','focus-window','--id',str(window['id'])]);time.sleep(.4)
        wait(lambda:len(native_events())>native_before,'native session start')
        return before
    def done(before,authorized=True):
        wait(lambda:any(e['event']=='done' and e['authorized']==authorized for e in events()[before:]),'authorization result')
        wait(lambda:not windows(),'dialog retired');assert app.poll() is None
    before=request(uids=[1000],message='Change DNS servers',details={'polkit.subject-pid':'2481'},
                   publisher='Fixture Network Settings')
    a11y.request(session,'--window','Canopa Authorization','--name','Account: Alice Example')
    a11y.request(session,'--window','Canopa Authorization','--name','Process 2481')
    a11y.request(session,'--window','Canopa Authorization','--name','Fixture Network Settings')
    shot=session.capture('identity-avatar.png').convert('RGB')
    assert sum(r>180 and g<80 and b<100 for r,g,b in shot.getdata())>100, 'identity avatar absent'
    lookups=[e['uid'] for e in json.loads((root/'accounts-events.json').read_text()) if e.get('event')=='find-user']
    assert lookups == [1000], lookups
    a11y.request(session,'--window','Canopa Authorization','--name','Policy details','--click')
    a11y.request(session,'--window','Canopa Authorization','--name','Back to authentication','--click')
    a11y.request(session,'--window','Canopa Authorization','--name','Password:')
    answer('canopa-fixture-password');done(before)
    before=request('otp');session.capture('password.png')
    a11y.request(session,'--window','Canopa Authorization','--name','Password:','--value','canopa-fixture-password')
    time.sleep(.2)
    protected=json.loads(a11y.request(session,'--window','Canopa Authorization','--name','Password:'))
    assert protected['role']=='password text' and 'canopa-fixture-password' not in protected['text'],protected
    a11y.request(session,'--window','Canopa Authorization','--name','Submit response','--click')
    wait(lambda:sum(line.startswith('accepted') for line in native_events())>=1,'first factor');time.sleep(.4);session.capture('otp.png');answer('123456');done(before)
    before=request(uids=[0,os.getuid()]);session.run([str(keyboard),'type','discarded']);key('Tab');time.sleep(.4);answer('canopa-fixture-password');done(before);assert any(e['event']=='receipt' and e['uid']==0 for e in events()[before:])
    before=request();rejected=sum(line.startswith('rejected ') for line in native_events());answer('wrong')
    wait(lambda:sum(line.startswith('rejected ') for line in native_events())>rejected,'first password rejected')
    time.sleep(2.3);started=sum(line.startswith('initiate ') for line in native_events());key('Return')
    wait(lambda:sum(line.startswith('initiate ') for line in native_events())>started,'retry session started')
    a11y.request(session,'--window','Canopa Authorization','--name','Password:')
    answer('canopa-fixture-password');done(before)
    before=request(message='A long authorization description. '*12);key('Next');session.capture('details.png');key('Next');key('Prior');key('Return');answer('canopa-fixture-password');done(before)
    before=request('device');session.capture('device.png');assert not any(line.startswith('accepted') for line in native_events()[-1:]);(root/'polkit-touch').touch();done(before)
    before=request('late');key('Escape');done(before,False)
    before=request();answer('canopa-fixture-password');done(before)
    before=request('device');(root/'polkit-cancel').touch();done(before,False)
    before=request('device');done(before,False)
    before=request('device');stop(app);wait(lambda:any(e['event']=='done' and not e['authorized'] for e in events()[before:]),'logout cancellation')
    app=start('agent-restart',True);before=request();answer('canopa-fixture-password');done(before)
    # A blocked native cancellation cannot retain an agent/dialog indefinitely.
    before=request('blocked-cancel');key('Escape');assert app.wait(timeout=8)!=0;session.processes.remove(app);wait(lambda:not windows(),'failed backend retired')
    wait(lambda:any(e['event']=='agent-disconnected' for e in events()[before:]),'lost registration')
    app=start('agent-after-failure',True);before=request();answer('canopa-fixture-password');done(before)
    # Authority loss ends the process so the session service can register afresh.
    before=request('device');session.retire(authority);assert app.wait(timeout=8)!=0;session.processes.remove(app)
    assert not any(e['event']=='receipt' for e in events()[before:])
    (session.output/'authority-before-restart.json').write_text(json.dumps(events(),indent=2))
    (root/'polkit-ready').unlink();(root/'polkit-control.json').write_text('{}')
    authority=session.start(['/usr/bin/python3',str(ROOT/'scripts/desktop-fixture/polkit.py'),str(root)],'authority-restart.log')
    wait(lambda:(root/'polkit-ready').exists(),'restarted authority')
    app=start('agent-after-authority',True);before=request();answer('canopa-fixture-password');done(before);stop(app)
    (session.output/'authority-events.json').write_text(json.dumps(events(),indent=2));(session.output/'session-events').write_text('\n'.join(native_events())+'\n')
    result=dict(status='passed',checks=['command-free preview','foreign session refused','ambiguous user-manager session refused','owned user-manager session discovery','idle agent has no window','duplicate agent registration refused','password plus fresh visible OTP','authority-supplied identity selection','permitted identity name and photo without enumeration','wrong password retry','paged full request details','device-only factor','user cancellation and late completion ignored','fresh request after cancellation','authority cancellation','bounded request timeout','logout cancellation and registration teardown','restart and fresh authentication','blocked cancellation has bounded process lifetime','recovery after failed backend','authority loss cancels pending authentication','fresh registration after authority restart','systemd readiness after registration'])
    (session.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/canopa-polkit');parser.add_argument('--output',type=Path,default=ROOT/'.scratch/polkit-agent/desktop');args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='canopa-polkit-') as temporary:
        root=Path(temporary);root.chmod(0o700);session=desktop.Session(root,args.output.resolve())
        try:verify(args.binary.resolve(),session)
        finally:
            for name in ['polkit-events.json','polkit-session-events']:
                if (root/name).exists():(session.output/name).write_bytes((root/name).read_bytes())
            session.close()
if __name__=='__main__':main()
