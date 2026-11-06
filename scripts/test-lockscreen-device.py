#!/usr/bin/env python3
"""Opt-in physical FIDO touch acceptance; private PAM/compositor, no host policy edits.

Creates a non-resident credential for a test-only origin; deletes its temporary
mapping afterward. Requires two deliberate touches. Never reads SSH credentials.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import pwd
import shlex
import subprocess
import tempfile
import time
import native_accessibility as a11y

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py')
desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)
ORIGIN='pam://canopa-device-test'
CUE='Canopa device test: touch your security key'


def verify(args,session):
    root=session.runtime
    config=root/'lock.kdl'
    config.write_text(f'schema-version 1\nlockscreen {{\n use-wallpaper #false\n auth-timeout {min(600,args.touch_timeout+10)}\n widgets "keyboard"\n}}\n')
    session.run([str(args.binary),'--canopa-config',str(config),'check-config'])
    mapping=root/'test-credential'
    user=pwd.getpwuid(os.getuid()).pw_name
    if args.u2f_enrollment:
        helper=root/'u2f-enroll'
        flags=shlex.split(subprocess.check_output(['pkg-config','--cflags','--libs','libfido2'],text=True))
        subprocess.run(['cc','-Wall','-Wextra','-Werror',str(ROOT/'scripts/desktop-fixture/u2f-enroll.c'),'-o',str(helper),*flags],check=True)
        enrollment=[str(helper),str(args.device),ORIGIN,user]
    else:
        enrollment=[str(args.enroll),'-u',user,'-o',ORIGIN,'-i',ORIGIN]
    print('Touch the FIDO key once for temporary non-resident enrollment.',flush=True)
    descriptor=os.open(mapping,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
    with os.fdopen(descriptor,'wb') as output, (session.output/'enrollment.log').open('wb') as log:
        process=subprocess.Popen(enrollment,stdin=subprocess.DEVNULL,
                                 stdout=output,stderr=log,start_new_session=True)
        try:
            assert process.wait(timeout=args.touch_timeout)==0,'temporary device enrollment failed'
        finally:
            if process.poll() is None:desktop.stop(process)
    assert 0<mapping.stat().st_size<16384,'invalid temporary mapping'
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\n')
    a11y.start(session)
    session.env['DBUS_SYSTEM_BUS_ADDRESS']=session.env['DBUS_SESSION_BUS_ADDRESS']
    session.env.pop('XDG_SESSION_ID',None)
    session.start(['/usr/bin/python3',str(ROOT/'scripts/desktop-fixture/lock-logind.py'),str(root)],'logind.log')
    session.wait_for(lambda:(root/'logind-ready').exists(),'private logind')
    pam=root/'pam';pam.mkdir()
    security=root/'security';security.mkdir()
    (pam/'canopa-lock').write_text(f'auth required {args.module} authfile={mapping} origin={ORIGIN} cue [cue_prompt={CUE}] authpending_file={root}/touch-pending\naccount required {args.permit}\n')
    prefix=['bwrap','--ro-bind','/','/','--dev','/dev','--dev-bind',str(args.device),str(args.device),
            '--bind',str(root),str(root),'--ro-bind',str(pam),'/etc/pam.d',
            '--ro-bind',str(security),'/etc/security','--die-with-parent']
    locker=session.start([*prefix,str(args.binary),'--session','canopa-fixture','--canopa-config',str(config)],'locker.log')
    state=lambda:json.loads((root/'logind-state.json').read_text())
    session.wait_for(lambda:state()['locked'],'real compositor lock acknowledgement')
    a11y.request(session,'--window','Canopa Lock','--name',CUE)
    time.sleep(.5)  # AT-SPI publication precedes the compositor's presented frame.
    session.capture('device-touch-prompt.png')
    assert locker.poll() is None and state()['locked'],'device accepted without the second touch'
    print('Canopa is locked and showing the real PAM cue. Touch the key again to authenticate.',flush=True)
    assert locker.wait(timeout=args.touch_timeout)==0,'device unlock failed'
    session.processes.remove(locker)
    session.wait_for(lambda:not state()['locked'],'authenticated LockedHint clear')
    mapping.unlink()
    summary=dict(status='passed',device=str(args.device),origin=ORIGIN,enrollment='U2F touch-only' if args.u2f_enrollment else 'pamu2fcfg',
                 checks=['non-resident test-only enrollment','real pam-u2f cue through Canopa/AT-SPI',
                         'compositor-confirmed lock remains pending before touch','physical FIDO authentication unlocks',
                         'temporary credential mapping removed'],
                 limitations=['private graphical seat and PAM stack; no host PAM/VT/DRM or display-manager policy validation'])
    (session.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2),flush=True)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/canopa-lock')
    method=parser.add_mutually_exclusive_group(required=True)
    method.add_argument('--enroll',type=Path)
    method.add_argument('--u2f-enrollment',action='store_true',help='test-only non-resident U2F credential; no PIN prompt or PIN policy changes')
    parser.add_argument('--module',type=Path,required=True)
    parser.add_argument('--device',type=Path,required=True)
    parser.add_argument('--permit',type=Path,default=Path('/usr/lib/security/pam_permit.so'))
    parser.add_argument('--touch-timeout',type=int,default=90)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    if not 10<=args.touch_timeout<=590:parser.error('touch-timeout must be 10..590')
    if args.device.parent!=Path('/dev') or not args.device.name.startswith('hidraw') or not args.device.name[6:].isdigit():parser.error('select a /dev/hidrawN device')
    for path in (args.binary,args.module,args.permit,*([args.enroll] if args.enroll else [])):
        if not path.is_absolute() or not path.is_file() or any(c.isspace() for c in str(path)):parser.error('tool/module paths must be existing absolute paths without whitespace')
    args.output.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='canopa-device-test-') as temporary:
        root=Path(temporary);root.chmod(0o700)
        session=desktop.Session(root,args.output.resolve())
        try:verify(args,session)
        except Exception:
            if 'WAYLAND_DISPLAY' in session.env:
                try:session.capture('failure.png')
                except Exception:pass
            if (root/'logind-state.json').exists():(args.output/'logind-state.json').write_bytes((root/'logind-state.json').read_bytes())
            raise
        finally:session.close()


if __name__=='__main__':main()
