#!/usr/bin/env python3
"""Verify the real parent adapter, worker and libei client on a private desktop."""
import argparse
import array
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import sys
import tempfile

ROOT=Path(__file__).resolve().parents[1]
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py')
desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)

def digest(path):
    with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()

def verify(args,session,result):
    from gi.repository import Gio
    session.env.update(PATH=str(args.niri.parent)+os.pathsep+session.env['PATH'],
        CANOPA_INPUT_PARENT_NATIVE='1',CANOPA_INPUT_WORKER_BINARY=str(args.worker))
    session.env.pop('REIS_DEBUG',None)
    session.start_niri('''debug { dbus-interfaces-in-non-session-instances; }
input { keyboard { xkb { layout "fr"; }; }; }
prefer-no-csd
animations { off; }
xwayland-satellite { off; }
hotkey-overlay { skip-at-startup; }
''')
    def connect():return Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
    for mode,suffix in [('client-header','.h'),('private-code','.c')]:
        subprocess.run(['wayland-scanner',mode,'/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml',
                        str(session.output/('xdg-shell'+suffix))],check=True,timeout=15)
    receiver=session.output/'receiver';client=session.output/'ei-client'
    subprocess.run(['cc','-Wall','-Wextra','-Werror','-I',str(session.output),str(ROOT/'scripts/desktop-fixture/remote-input-receiver.c'),
        str(session.output/'xdg-shell.c'),'-lwayland-client','-lxkbcommon','-o',str(receiver)],check=True,timeout=30)
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','libei-1.0','xkbcommon'],text=True).split()
    subprocess.run(['cc','-Wall','-Wextra','-Werror',str(ROOT/'scripts/desktop-fixture/remote-input-ei-client.c'),'-o',str(client),*flags],check=True,timeout=30)
    receiver_process=session.start([str(receiver)],'receiver.log');log=session.output/'receiver.log'
    session.wait_for(lambda:log.exists() and 'focused' in log.read_text(),'harmless receiver focus')
    def lines():
        assert receiver_process.poll() is None
        return log.read_text().splitlines()
    def wait_line(value,before):session.wait_for(lambda:any(value in line for line in lines()[before:]),value)
    def finish(process,status):
        actual=process.wait(timeout=10);session.processes.remove(process)
        assert actual==status,(process.args,actual,status)
    def worker_identity(pid):
        p=Path('/proc')/str(pid)
        try:
            fields=(p/'stat').read_text().rsplit(') ',1)[1].split()
            return dict(state=fields[0],ppid=int(fields[1]),start_ticks=fields[19],command=(p/'cmdline').read_text().replace('\0',' '))
        except FileNotFoundError:return None
    def terminal(pid,identity):
        current=worker_identity(pid)
        return current is None or current['start_ticks']!=identity['start_ticks'] or current['state']=='Z'
    for name in ['parent-eof','client-sigkill','worker-sigkill','parent-sigkill','caller-loss','keymap-loss']:
        caller=connect();before=len(lines());fd_receiver,fd_sender=socket.socketpair();fd_receiver.settimeout(40)
        parent_log=session.output/(name+'-parent.log')
        with parent_log.open('w') as output:
            parent=subprocess.Popen([str(args.harness),'--exact','remote_input::parent::tests::native_parent_fixture',
                '--ignored','--nocapture','--test-threads=1'],stdin=subprocess.PIPE,stdout=output,stderr=fd_sender,
                env=session.env,start_new_session=True)
        fd_sender.close();session.processes.append(parent)
        parent.stdin.write(json.dumps({'caller':caller.get_unique_name(),'worker_success':name!='worker-sigkill'}).encode()+b'\n');parent.stdin.flush()
        payload,control,flags,_=fd_receiver.recvmsg(1,socket.CMSG_SPACE(4),socket.MSG_CMSG_CLOEXEC)
        rights=array.array('i')
        for level,kind,data in control:
            assert level==socket.SOL_SOCKET and kind==socket.SCM_RIGHTS
            rights.frombytes(data)
        assert payload==b'\x01' and len(rights)==1 and not flags & ~socket.MSG_CMSG_CLOEXEC,(payload,rights,flags)
        ei=socket.socket(fileno=rights[0]);worker_pid,uid,_=struct.unpack('3i',ei.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12))
        identity=worker_identity(worker_pid)
        assert identity and identity['ppid']==parent.pid and str(args.worker) in identity['command'] and '--remote-input-worker' in identity['command']
        session.wait_for(lambda:'input-parent-ready ' in parent_log.read_text(),'parent ready metadata')
        record=json.loads(next(line.split('input-parent-ready ',1)[1] for line in parent_log.read_text().splitlines() if 'input-parent-ready ' in line))
        assert record['worker']==worker_pid and uid==os.getuid()
        client_log=session.output/(name+'-client.log')
        with client_log.open('w') as output:
            remote=subprocess.Popen([str(client),'abrupt' if name=='worker-sigkill' else 'held'],stdin=ei,
                stdout=output,stderr=output,env=session.env,start_new_session=True)
        ei.close();session.processes.append(remote)
        wait_line('key 16 1 A',before)
        session.wait_for(lambda:'modifier update' in client_log.read_text(),'real libei modifier feedback')
        if name=='parent-eof':parent.stdin.close()
        elif name=='client-sigkill':remote.kill();finish(remote,-signal.SIGKILL)
        elif name=='worker-sigkill':
            current=worker_identity(worker_pid)
            assert current and current['start_ticks']==identity['start_ticks'] and current['ppid']==parent.pid
            os.kill(worker_pid,signal.SIGKILL)
        elif name=='parent-sigkill':parent.kill();finish(parent,-signal.SIGKILL)
        elif name=='caller-loss':caller.close_sync(None)
        else:
            config=session.runtime/'niri.kdl';config.write_text(config.read_text().replace('layout "fr"','layout "us"'))
        if name!='parent-sigkill':finish(parent,0)
        if name!='client-sigkill':finish(remote,0)
        wait_line('key 42 0 Shift_L',before);wait_line('mods 0 ',before)
        session.wait_for(lambda:terminal(worker_pid,identity),'owned worker terminal')
        if name!='parent-sigkill':
            assert 'input-parent-retired '+str(name!='worker-sigkill').lower() in parent_log.read_text()
        fd_receiver.close()
        if not caller.is_closed():caller.close_sync(None)
        result['checks'].append(name+'-real-parent-fd-lifetime')
        result.setdefault('workers',[]).append({'pid':worker_pid,'start_ticks':identity['start_ticks'],'terminal':True})
        (session.output/'progress.json').write_text(json.dumps(result,indent=2)+'\n')
        print(name+': passed',flush=True)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['niri','worker','harness']:parser.add_argument('--'+name,required=True,type=Path);parser.add_argument('--'+name+'-sha256',required=True)
    parser.add_argument('--output',required=True,type=Path);args=parser.parse_args()
    sources=[Path(__file__),ROOT/'scripts/desktop-fixture/remote-input-ei-client.c',ROOT/'scripts/desktop-fixture/remote-input-receiver.c',ROOT/'scripts/validate-desktop.py']
    for name in ['niri','worker','harness']:
        path=getattr(args,name).resolve();setattr(args,name,path);assert digest(path)==getattr(args,name+'_sha256')
    assert args.niri.name=='niri'
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    result={'status':'running','checks':[],'binaries':{name:{'path':str(getattr(args,name)),'sha256':getattr(args,name+'_sha256')} for name in ['niri','worker','harness']},
        'fixture_sources':[{'path':str(p.relative_to(ROOT)),'sha256':digest(p)} for p in sources]}
    try:
        with tempfile.TemporaryDirectory(prefix='canopa-input-parent-') as directory:
            session=desktop.Session(Path(directory),args.output)
            try:verify(args,session,result)
            finally:session.close()
        for entry in result['binaries'].values():assert digest(Path(entry['path']))==entry['sha256']
        result.update(status='passed',binaries_unchanged=True)
    except Exception as error:result.update(status='failed',error=str(error));raise
    finally:(args.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))
if __name__=='__main__':main()
