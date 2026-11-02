#!/usr/bin/env python3
"""Real PolkitAgentSession/helper/PAM against a private authority in user/PID/mount namespaces."""
import argparse,json,os,subprocess,tempfile,textwrap
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,default=ROOT/'.scratch/polkit-agent/helper');args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
    build=subprocess.run(['bash',str(ROOT/'scripts/cargo.sh'),'test','--locked','-p','canopa-polkit','--test','native','--no-run','--message-format=json'],cwd=ROOT,capture_output=True,text=True,check=True)
    executable=next(json.loads(line)['executable'] for line in build.stdout.splitlines() if line.startswith('{') and json.loads(line).get('reason')=='compiler-artifact' and json.loads(line).get('target',{}).get('name')=='native' and json.loads(line).get('executable'))
    with tempfile.TemporaryDirectory(prefix='canopa-polkit-helper-') as directory:
        root=Path(directory);pam=root/'pam';pam.mkdir()
        module=root/'pam.so';subprocess.run(['cc','-shared','-fPIC','-Wall','-Wextra','-Werror',str(ROOT/'scripts/desktop-fixture/lock-pam-interactive.c'),'-lpam','-o',str(module)],check=True)
        (root/'passwd').write_text('root:x:0:0:Fixture Administrator:/nonexistent:/bin/sh\n')
        (root/'group').write_text('root:x:0:\n');(root/'nsswitch.conf').write_text('passwd: files\ngroup: files\n')
        inner=root/'inner.py';inner.write_text(textwrap.dedent('''
            import json,os,subprocess,sys,time
            from pathlib import Path
            root=Path(sys.argv[1]);repo=Path(sys.argv[2]);binary=sys.argv[3]
            runtime=root/'runtime';runtime.mkdir(mode=0o700);os.environ['XDG_RUNTIME_DIR']=str(runtime)
            Path('/run/dbus').mkdir(parents=True,exist_ok=True)
            for scenario,pam_mode in [('retry','password'),('otp','otp'),('device','fingerprint')]:
                case=root/scenario;case.mkdir()
                (root/'pam/polkit-1').write_text(f'auth required {root}/pam.so {pam_mode} {case}\\naccount required {root}/pam.so\\n')
                (case/'polkit-control.json').write_text(json.dumps(dict(auto=True,mode='password',uids=[0])))
                bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--print-address=1','--address=unix:path=/run/dbus/system_bus_socket'],stdout=subprocess.PIPE,stderr=(root/(scenario+'-bus.log')).open('w'),text=True)
                address=bus.stdout.readline().strip();env={**os.environ,'DBUS_SYSTEM_BUS_ADDRESS':address,'DBUS_SESSION_BUS_ADDRESS':address,'CANOPA_POLKIT_FIXTURE_ROOT':str(case),'CANOPA_POLKIT_CASE':scenario}
                env.pop('LD_PRELOAD',None);env.pop('POLKIT_DEBUG',None)
                peer=subprocess.Popen(['/usr/bin/python3',str(repo/'scripts/desktop-fixture/polkit.py'),str(case)],env=env,stdout=(root/(scenario+'-peer.log')).open('w'),stderr=subprocess.STDOUT)
                try:
                    deadline=time.monotonic()+5
                    while not (case/'polkit-ready').exists():
                        assert peer.poll() is None,'private authority exited';assert time.monotonic()<deadline;time.sleep(.01)
                    with (root/(scenario+'.log')).open('w') as log:
                        test=subprocess.Popen([binary,'--ignored','--exact','native_child','--nocapture'],env=env,stdout=log,stderr=log)
                        deadline=time.monotonic()+20
                        try:
                            while test.poll() is None:
                                assert time.monotonic()<deadline,'real polkit helper timed out'
                                if (case/'polkit-touch').exists():(case/'touch').touch()
                                time.sleep(.01)
                            assert test.returncode==0,f'real helper {scenario} failed'
                        finally:
                            if test.poll() is None:test.kill();test.wait()
                    events=json.loads((case/'polkit-events.json').read_text())
                    assert sum(e['event']=='receipt' for e in events)==1
                    assert any(e['event']=='done' and e['authorized'] for e in events)
                    (root/(scenario+'-events.json')).write_text(json.dumps(events,indent=2))
                finally:
                    peer.terminate();peer.wait();bus.terminate();bus.wait()
            (root/'result.json').write_text(json.dumps(dict(status='passed',checks=['real native polkit session and helper','actual private polkit-1 PAM conversation','wrong password retry without an authorization receipt','fresh secret password and visible OTP','device-only PAM factor','helper-issued receipt to private authority']),indent=2))
        '''))
        command=['bwrap','--unshare-user','--unshare-pid','--unshare-net','--unshare-ipc','--unshare-uts','--uid','0','--gid','0','--ro-bind','/','/','--dev','/dev','--proc','/proc','--bind',str(root),str(root),'--tmpfs','/run','--ro-bind',str(pam),'/etc/pam.d','--ro-bind',str(root/'passwd'),'/etc/passwd','--ro-bind',str(root/'group'),'/etc/group','--ro-bind',str(root/'nsswitch.conf'),'/etc/nsswitch.conf','--unsetenv','LD_PRELOAD','--unsetenv','POLKIT_DEBUG','--die-with-parent','/usr/bin/python3',str(inner),str(root),str(ROOT),executable]
        result=subprocess.run(command,capture_output=True,text=True,timeout=75)
        (args.output/'namespace.log').write_text(result.stdout+result.stderr)
        for file in root.glob('*.log'):(args.output/file.name).write_bytes(file.read_bytes())
        for file in root.glob('*-events.json'):(args.output/file.name).write_bytes(file.read_bytes())
        if result.returncode:raise RuntimeError(f'Private polkit helper test failed; see {args.output}')
        data=json.loads((root/'result.json').read_text());(args.output/'summary.json').write_text(json.dumps(data,indent=2)+'\n');print(json.dumps(data,indent=2))
if __name__=='__main__':main()
