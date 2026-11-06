#!/usr/bin/env python3
"""Private-compositor wallpaper and lock acceptance, with a mount-isolated PAM fixture.

Never installs PAM configuration or contacts the user's compositor. Requires the
existing Weston/Niri harness tools, bubblewrap, a C compiler and Linux-PAM headers.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import time
from PIL import Image

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def verify(binary, session):
    name = session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
                              'hotkey-overlay { skip-at-startup; }\nlayout { gaps 0; }\n')
    wallpaper = binary / 'canopa-wallpaper'
    locker = binary / 'canopa-lock'
    red, blue = session.runtime / 'red.png', session.runtime / 'blue.png'
    Image.new('RGB', (128, 80), (220, 35, 45)).save(red)
    Image.new('RGB', (128, 80), (25, 75, 190)).save(blue)
    config = session.runtime / 'session.kdl'
    config.write_text(f'schema-version 1\nwallpaper {{ image "{red}"; }}\n'
                      'lockscreen { widgets "username" "keyboard"; }\n')
    if os.environ.get('CANOPA_SESSION_TRACE'): session.env['WAYLAND_DEBUG'] = 'client'
    daemon = session.start([str(wallpaper), 'daemon', '--canopa-config', str(config)], 'wallpaper.log')
    session.env.pop('WAYLAND_DEBUG', None)

    def ctl(*args, success=True):
        result = subprocess.run([str(wallpaper), *map(str,args)], env=session.env,
                                capture_output=True, text=True, timeout=15)
        assert (result.returncode == 0) == success, (args, result.stdout, result.stderr)
        return json.loads(result.stdout) if success else result

    def status():
        try:
            return ctl('status')['outputs']
        except (AssertionError, subprocess.SubprocessError):
            return None

    session.wait_for(lambda: status() and status()[0]['width'] > 0, 'wallpaper output')
    def painted(color, filename):
        return session.capture(filename).getpixel((100,100)) == color
    session.wait_for(lambda: painted((220,35,45),'wallpaper-red.png'), 'initial image')
    duplicate = subprocess.run([str(wallpaper),'daemon','--defaults'], env=session.env,
                               capture_output=True,text=True,timeout=5)
    assert duplicate.returncode != 0 and 'already running' in duplicate.stderr
    ctl('set',blue,'--output',name)
    session.wait_for(lambda: painted((25,75,190),'wallpaper-blue.png'), 'live selected-output change')
    ctl('set',red,'--all')
    session.wait_for(lambda: painted((220,35,45),'wallpaper-all.png'), 'all-output change')
    ctl('set',session.runtime/'missing.png','--all',success=False)
    assert painted((220,35,45),'wallpaper-invalid-image.png')
    config.write_text('schema-version 1\nwallpaper { fit "invalid"; }\n')
    ctl('reload',success=False)
    assert painted((220,35,45),'wallpaper-invalid-reload.png')
    config.write_text(f'schema-version 1\nwallpaper {{ image "{blue}"; }}\nlockscreen {{ widgets; }}\n')
    ctl('reload')
    session.wait_for(lambda: painted((25,75,190),'wallpaper-reload.png'), 'valid reload')
    # Scale configure and a fresh buffer, without replacing the daemon PID.
    session.run(['niri','msg','output',name,'scale','1.25'])
    session.wait_for(lambda: painted((25,75,190),'wallpaper-scale.png'), 'scaled wallpaper')
    session.run(['niri','msg','output',name,'scale','1'])
    session.wait_for(lambda: status() and status()[0]['width'] == 1280, 'restored scale')
    ctl('set', blue, '--all')  # acknowledgement follows buffer submission
    ticks = lambda: sum(map(int, Path(f'/proc/{daemon.pid}/stat').read_text().split(') ',1)[1].split()[11:13]))
    time.sleep(0.3)
    start=ticks();time.sleep(2);idle_ticks=ticks()-start
    assert idle_ticks <= 1, idle_ticks
    rss = next(line for line in Path(f'/proc/{daemon.pid}/status').read_text().splitlines() if line.startswith('VmRSS:'))

    # Keep the virtual keyboard alive so the compositor advertises input before locking.
    session.start(['wtype','-s','600000'],'keyboard.log')
    pam = session.runtime / 'pam'
    pam.mkdir()
    module = session.runtime / 'pam-fixture.so'
    deny = session.runtime / 'deny-account'
    subprocess.run(['cc','-shared','-fPIC','-Wall','-Wextra',str(ROOT/'scripts/desktop-fixture/lock-pam.c'),
                    '-lpam','-o',str(module)],check=True,timeout=30)
    (pam/'canopa-lock').write_text(f'auth required {module}\naccount required {module} {deny}\n')
    prefix = ['bwrap','--ro-bind','/','/','--dev','/dev','--ro-bind',str(pam),'/etc/pam.d','--die-with-parent']
    # Fail before launching the locker if this host has disabled user namespaces.
    subprocess.run([*prefix,'/usr/bin/true'],check=True,env=session.env,timeout=10)

    def start_lock(log):
        read_fd,write_fd=os.pipe()
        with (session.output/log).open('w') as stream:
            process=subprocess.Popen([*prefix,str(locker),'--canopa-config',str(config),'--ready-fd',str(write_fd)],
                                     env=session.env,stdout=stream,stderr=stream,pass_fds=(write_fd,),start_new_session=True)
        session.processes.append(process)
        os.close(write_fd)
        try:
            assert select.select([read_fd],[],[],10)[0], 'lock readiness timeout'
            assert os.read(read_fd,1)==b'1', (session.output/log).read_text()
        finally:
            os.close(read_fd)
        # A recovered ext-session-lock may be acknowledged while outputs are
        # still blank. Wait for the first lock surface to receive keyboard focus.
        time.sleep(1.5)
        return process

    def type_password(password):
        session.run(['wtype','-d','15',password,'-k','Return'])

    def retry():
        session.run(['wtype','-k','Return'])
        time.sleep(.5)

    config.write_text(f'schema-version 1\nwallpaper {{ image "{blue}"; }}\n'
                      f'lockscreen {{ image "{session.runtime / "missing-lock.png"}"; widgets; }}\n')
    locked=start_lock('lock.log')
    config.write_text(f'schema-version 1\nwallpaper {{ image "{blue}"; }}\nlockscreen {{ widgets; }}\n')
    # A second lock is refused; the original remains responsible for authentication.
    second=subprocess.run([*prefix,str(locker),'--defaults'],env=session.env,capture_output=True,text=True,timeout=10)
    assert second.returncode != 0 and 'refused' in second.stderr, second.stderr
    type_password('wrong-password');time.sleep(3);assert locked.poll() is None
    session.run(['wtype','-k','Escape']);assert locked.poll() is None
    deny.touch()
    retry()
    type_password('canopa-fixture-password');time.sleep(3);assert locked.poll() is None
    deny.unlink()
    retry()
    type_password('canopa-fixture-password')
    assert locked.wait(timeout=10)==0, (session.output/'lock.log').read_text()
    session.processes.remove(locked)
    session.wait_for(lambda: painted((25,75,190),'unlocked.png'), 'authenticated unlock')

    # Killing the locker must leave the compositor locked. Niri permits a new
    # locker to take responsibility for the dead client's still-locked session.
    killed=start_lock('lock-killed.log')
    killed.kill();killed.wait(timeout=5);session.processes.remove(killed)
    try:
        assert not painted((25,75,190),'locker-crash.png'), 'locker death exposed the desktop'
    except subprocess.CalledProcessError:
        pass  # Compositor may deny ordinary capture while locked.
    recovery=start_lock('lock-recovery.log')
    type_password('wrong-password');time.sleep(3);assert recovery.poll() is None
    retry()
    type_password('canopa-fixture-password');assert recovery.wait(timeout=10)==0
    session.processes.remove(recovery)
    session.wait_for(lambda: painted((25,75,190),'recovered.png'), 'authenticated lock recovery')
    # Tear down only this private desktop; no recovery on the live session is needed.
    summary={'status':'passed','wallpaper_pid':daemon.pid,'wallpaper_idle_ticks_2s':idle_ticks,
             'wallpaper_rss':rss,'checks':['live selected/all image replacement','invalid images/reload retain state',
             'duplicate-daemon exclusion','fractional scale','idle resource use','protocol readiness',
             'duplicate-lock refusal','missing lock wallpaper still locks','wrong password stays locked','Escape stays locked','account rejection stays locked',
             'PAM success unlocks','locker death hides the desktop','replacement locker still requires PAM authentication']}
    (session.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin-dir',type=Path,default=ROOT/'target/debug')
    parser.add_argument('--output',type=Path,default=ROOT/'.scratch/wallpaper-lockscreen/evidence')
    args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='canopa-session-') as temporary:
        runtime=Path(temporary);runtime.chmod(0o700)
        session=desktop.Session(runtime,args.output.resolve())
        try:verify(args.bin_dir.resolve(),session)
        finally:session.close()

if __name__=='__main__':main()
