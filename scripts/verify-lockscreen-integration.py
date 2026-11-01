#!/usr/bin/env python3
"""Private logind/loginctl, interactive PAM, widget and layout acceptance.

The system-bus address, compositor and PAM mount are test-owned. Host logind/PAM
and the live session are never used. Hardware operations are modeled by PAM fixtures.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time
from PIL import Image
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py')
desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)

def verify(binaries,session):
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n')
    session.env['DBUS_SYSTEM_BUS_ADDRESS']=session.env['DBUS_SESSION_BUS_ADDRESS']
    session.env.pop('XDG_SESSION_ID',None)
    root=session.runtime;locker=binaries/'canopa-lock'
    logind=session.start(['/usr/bin/python3',str(ROOT/'scripts/desktop-fixture/lock-logind.py'),str(root)],'logind.log')
    session.wait_for(lambda:(root/'logind-ready').exists(),'private logind service')
    session.start(['wtype','-s','600000'],'keyboard.log')
    pam=root/'pam';pam.mkdir()
    module=root/'pam-interactive.so'
    subprocess.run(['cc','-shared','-fPIC','-Wall','-Wextra',str(ROOT/'scripts/desktop-fixture/lock-pam-interactive.c'),'-lpam','-o',str(module)],check=True)
    prefix=['bwrap','--ro-bind','/','/','--dev','/dev','--bind',str(root),str(root),'--ro-bind',str(pam),'/etc/pam.d','--die-with-parent']
    subprocess.run([*prefix,'/usr/bin/true'],env=session.env,check=True,timeout=10)
    config=root/'lock.kdl'
    widget=root/'widget.py'
    widget.write_text('''import json,os
from pathlib import Path
p=Path(__file__).with_name("widget-runs")
with p.open("a") as f:f.write(str(os.getpid())+"\\n")
print("custom widget live")
''')
    slow=root/'slow-widget.py'
    slow.write_text('''import os,time
from pathlib import Path
Path(__file__).with_name("slow-pid").write_text(str(os.getpid()))
time.sleep(60)
''')
    config.write_text(f'''schema-version 1
lockscreen {{
    use-wallpaper #false
    auth-timeout 8
    widgets "username" "status" "slow"
    layout {{
        margin 24
        gap 12
        top-left "clock" "date"
        center "username" "authentication"
        bottom-right "status" "slow"
    }}
    widget "status" {{ command {{ argv "python3" "{widget}"; }}; interval 1; color "#ff0000"; size 24; preview "PREVIEW STATUS"; }}
    widget "slow" {{ command {{ argv "python3" "{slow}"; }}; interval 60; timeout 60; }}
}}
''')
    # Preview uses the same layout, with no command execution and no PAM session.
    preview=session.output/'layout-preview.png'
    session.run([str(locker),'--canopa-config',str(config),'preview',str(preview)])
    assert not (root/'widget-runs').exists() and not (root/'slow-pid').exists()
    pixels=Image.open(preview).convert('RGB');w,h=pixels.size
    red=[(x,y) for y in range(h) for x in range(w) if (lambda p:p[0]>150 and p[1]<80 and p[2]<80)(pixels.getpixel((x,y)))]
    assert red and min(x for x,y in red)>w/2 and min(y for x,y in red)>h/2, 'custom preview placement ignored'

    def policy(mode):
        (pam/'canopa-lock').write_text(f'auth required {module} {mode} {root}\naccount required {module}\n')
        (root/'touch').unlink(missing_ok=True)
        (root/'slow-pid').unlink(missing_ok=True)
    def state():return json.loads((root/'logind-state.json').read_text())
    def events():
        try:return (root/'auth-events').read_text().splitlines()
        except FileNotFoundError:return []
    def mark():return len(events())
    def wait_event(text,after):
        result=session.wait_for(lambda:next((line for line in events()[after:] if line.endswith(text)),None),text)
        time.sleep(.4)  # allow the received PAM event to reach the software-rendered surface
        return result
    def key(name):session.run(['wtype','-k',name])
    def answer(value):
        # Niri may give a newly bound wl_keyboard its default keymap while wtype
        # reuses the preceding virtual device's map. Force a harmless distinct map
        # first so the synthetic character map is broadcast to the new client.
        key('Left')
        session.run(['wtype','-d','15',value,'-k','Return'])
    def request(mode,all_sessions=False):
        policy(mode);before=mark()
        session.run(['loginctl','lock-sessions'] if all_sessions else ['loginctl','lock-session','canopa-fixture'])
        session.wait_for(lambda:state()['locked'],'compositor-confirmed logind LockedHint')
        line=wait_event('started',before)
        # Locked acknowledgement precedes the first presented lock surface. Give
        # Niri's virtual seat time to enter that surface before injecting keys.
        time.sleep(1.5)
        return before,int(line.split()[1])
    def retry(expect='Password:'):
        before=mark();key('Return');wait_event(expect,before);return before
    def unlocked():session.wait_for(lambda:not state()['locked'],'authenticated LockedHint clear')
    def alive(pid):
        try:return Path(f'/proc/{pid}/stat').read_text().split(') ',1)[1][0]!='Z'
        except FileNotFoundError:return False

    policy('password')
    # User services often have no PID-attached session: test enumeration fallback.
    (root/'logind-control.json').write_text(json.dumps({'no-pid-session':True,'ambiguous':True}))
    ambiguous=subprocess.run([*prefix,str(locker),'daemon'],env=session.env,capture_output=True,text=True,timeout=10)
    assert ambiguous.returncode!=0 and 'unique' in ambiguous.stderr,ambiguous.stderr
    (root/'logind-control.json').write_text(json.dumps({'no-pid-session':True}))
    wrong_user=subprocess.run([*prefix,str(locker),'--session','other-user','daemon'],env=session.env,capture_output=True,text=True,timeout=10)
    assert wrong_user.returncode!=0 and 'another user' in wrong_user.stderr,wrong_user.stderr
    notify=socket.socket(socket.AF_UNIX,socket.SOCK_DGRAM)
    notify.bind(str(root/'notify'));notify.settimeout(10)
    session.env['NOTIFY_SOCKET']=str(root/'notify')
    if os.environ.get('CANOPA_SESSION_TRACE'):session.env['WAYLAND_DEBUG']='client'
    listener=session.start([*prefix,str(locker),'--canopa-config',str(config),'daemon'],'listener.log')
    session.env.pop('WAYLAND_DEBUG',None)
    assert notify.recv(1024)==b'READY=1','listener became ready before subscribing'
    session.env.pop('NOTIFY_SOCKET');notify.close()
    duplicate=subprocess.run([*prefix,str(locker),'--session','canopa-fixture','daemon'],env=session.env,capture_output=True,text=True,timeout=10)
    assert duplicate.returncode!=0 and 'already running' in duplicate.stderr,duplicate.stderr
    session.run(['loginctl','lock-session','other-user']);time.sleep(.4)
    assert not state()['locked'] and not events(), 'foreign session signal started a locker'

    # Success needs BOTH fresh password and OTP, including ECHO_ON prompts.
    before,pid=request('otp');wait_event('Password:',before)
    session.run(['loginctl','lock-session','canopa-fixture'])
    answer('canopa-fixture-password');wait_event('One-time code:',before)
    assert state()['locked'] and alive(pid)
    answer('000000');time.sleep(2.3);assert state()['locked'] and alive(pid)
    retry();before=mark();answer('canopa-fixture-password');wait_event('One-time code:',before)
    answer('123456');unlocked();session.wait_for(lambda:not alive(pid),'locker exit')
    session.wait_for(lambda:(root/'widget-runs').exists(),'custom widget execution')
    if (root/'slow-pid').exists():assert not alive(int((root/'slow-pid').read_text())), 'custom command survived normal locker exit'
    run_count=len((root/'widget-runs').read_text().splitlines());time.sleep(1.5)
    assert not state()['locked'],'duplicate Lock queued a new lock after authenticated unlock'
    assert len((root/'widget-runs').read_text().splitlines())==run_count,'custom widget kept polling after exit'

    # Hardware-touch flow remains locked after password until PAM accepts the device.
    before,pid=request('token',True);wait_event('Password:',before)
    answer('canopa-fixture-password');cue=wait_event('Touch your security key',before)
    worker_pid=int(cue.split()[0]);assert state()['locked']
    key('Escape');session.wait_for(lambda:not alive(worker_pid),'cancelled PAM worker reap')
    (root/'touch').touch();time.sleep(.5);assert state()['locked'],'late hardware result unlocked after cancellation'
    (root/'touch').unlink();before=retry();answer('canopa-fixture-password');wait_event('Touch your security key',before)
    (root/'touch').touch();unlocked()

    # Fingerprint-style PAM can authenticate without any password prompt or typing.
    before,pid=request('fingerprint');wait_event('Scan your fingerprint',before)
    assert not any(line.endswith('Password:') for line in events()[before:]) and state()['locked']
    (root/'touch').touch();unlocked()

    # A blocked module has a bounded lifetime and cannot unlock after timing out.
    before,pid=request('token');wait_event('Password:',before);answer('canopa-fixture-password')
    cue=wait_event('Touch your security key',before);worker_pid=int(cue.split()[0])
    session.wait_for(lambda:not alive(worker_pid),'timed-out worker reap')
    (root/'touch').touch();time.sleep(.5);assert state()['locked'] and alive(pid)
    (root/'touch').unlink();before=retry();answer('canopa-fixture-password');wait_event('Touch your security key',before)
    (root/'touch').touch();unlocked()

    # A failed worker never authorizes unlock; another loginctl request can recover.
    before,pid=request('token');wait_event('Password:',before);answer('canopa-fixture-password')
    cue=wait_event('Touch your security key',before)
    # logind owner loss/reconnection must restore true while the same lock lives.
    session.retire(logind);(root/'logind-ready').unlink()
    logind=session.start(['/usr/bin/python3',str(ROOT/'scripts/desktop-fixture/lock-logind.py'),str(root)],'logind-restarted.log')
    session.wait_for(lambda:(root/'logind-ready').exists(),'restarted logind')
    session.wait_for(lambda:state()['locked'],'hint restoration after logind restart')
    session.run(['loginctl','unlock-session','canopa-fixture']);time.sleep(.3)
    assert state()['locked'] and alive(pid),'logind Unlock bypassed PAM'
    os.kill(int(cue.split()[0]),signal.SIGKILL)
    time.sleep(2.3);assert state()['locked'] and alive(pid)
    before=retry();answer('canopa-fixture-password');wait_event('Touch your security key',before)
    (root/'touch').touch();unlocked()

    # Killing the locker keeps the hint true and kills its PAM/command leaders.
    before,pid=request('token');wait_event('Password:',before)
    worker_pid=int(next(line.split()[0] for line in events()[before:] if line.endswith('started')))
    session.wait_for(lambda:(root/'slow-pid').exists(),'slow command start')
    command_pid=int((root/'slow-pid').read_text())
    os.kill(pid,signal.SIGKILL);time.sleep(.7)
    assert state()['locked'] and not alive(worker_pid) and not alive(command_pid)
    policy('password');before=mark()
    recovery=session.start([*prefix,str(locker),'--session','canopa-fixture','--canopa-config',str(config)],'direct-recovery.log')
    wait_event('Password:',before);time.sleep(1.5)
    answer('canopa-fixture-password');unlocked();assert recovery.wait(timeout=10)==0
    session.processes.remove(recovery)
    session.retire(logind);(root/'logind-ready').unlink()
    logind=session.start(['/usr/bin/python3',str(ROOT/'scripts/desktop-fixture/lock-logind.py'),str(root)],'logind-after-recovery.log')
    session.wait_for(lambda:(root/'logind-ready').exists(),'logind after direct crash recovery')
    time.sleep(1.5);assert not state()['locked'],'listener resurrected a stale crashed-locker hint'

    # Updating the listener must leave an active locker in control of its session.
    before,pid=request('token');wait_event('Password:',before)
    session.retire(listener);assert state()['locked'] and alive(pid)
    answer('canopa-fixture-password');wait_event('Touch your security key',before)
    (root/'touch').touch();unlocked()

    # Direct invocation also reports state, independently of listener-owned children.
    policy('password');before=mark()
    direct=session.start([*prefix,str(locker),'--session','canopa-fixture','--canopa-config',str(config)],'direct.log')
    session.wait_for(lambda:state()['locked'],'direct invocation hint');wait_event('Password:',before)
    answer('canopa-fixture-password');unlocked();assert direct.wait(timeout=10)==0
    session.processes.remove(direct)

    result={'status':'passed','checks':['actual loginctl lock-session and lock-sessions transport',
        'session ownership and ambiguous-session refusal','listener exclusion','foreign signal isolation','duplicate Lock coalesced',
        'compositor-confirmed true / authenticated false hints','password plus fresh OTP',
        'visible PAM prompts','hardware-touch cue and pending device','cancelled worker reaped / stale result refused',
        'fingerprint-style passwordless PAM success','worker death remains locked','authentication timeout and fresh retry',
        'logind restart restores hint','Unlock signal cannot bypass PAM','locker crash preserves hint and cancels worker/command',
        'direct crash recovery does not resurrect stale hints',
        'listener stop preserves active locker','systemd readiness after subscription','direct locker state synchronization',
        'custom command execution and cleanup','preview has no command execution','custom widget placement']}
    (session.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin-dir',type=Path,default=ROOT/'target/debug')
    parser.add_argument('--output',type=Path,default=ROOT/'.scratch/lockscreen-integration/evidence')
    args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='canopa-lock-integration-') as temporary:
        root=Path(temporary);root.chmod(0o700);session=desktop.Session(root,args.output.resolve())
        try:verify(args.bin_dir.resolve(),session)
        finally:
            for name in ['auth-events','logind-state.json']:
                if (root/name).exists():(args.output/name).write_bytes((root/name).read_bytes())
            session.close()
if __name__=='__main__':main()
