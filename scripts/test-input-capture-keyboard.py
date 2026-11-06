#!/usr/bin/env python3
"""Private Niri keyboard interception; no public portal, EI or Iced consent claim."""
import argparse
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import tempfile
import time
import traceback

sys.dont_write_bytecode = True
from native_compositor import freeze_niri
from screencast_media import MediaSession, digest

ROOT = Path(__file__).resolve().parents[1]
SERVICE = 'org.canopa.Niri.InputCapture1'
PATH = '/org/canopa/Niri/InputCapture1'


def verify(session, result):
    from gi.repository import Gio, GLib
    session.start_niri('debug { dbus-interfaces-in-non-session-instances; }\n'
                       'prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
                       'hotkey-overlay { skip-at-startup; }\n')
    connection = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
    def call(method, parameters=None, interface=SERVICE):
        return connection.call_sync(SERVICE, PATH, interface, method, parameters, None,
                                    Gio.DBusCallFlags.NO_AUTO_START, 5000, None).unpack()
    def passed(name):
        result['checks'].append(name)
        (session.output / 'progress.json').write_text(json.dumps(result, indent=2)+'\n')
        print(name, flush=True)
    properties = call('GetAll', GLib.Variant('(s)', (SERVICE,)), 'org.freedesktop.DBus.Properties')[0]
    assert properties['Version'] == 2 and properties['SupportedCapabilities'] == 3, properties
    fixture = ROOT / 'scripts/desktop-fixture'
    protocols = [('virtual-pointer', fixture/'wlr-virtual-pointer-unstable-v1.xml'),
                 ('virtual-keyboard', fixture/'virtual-keyboard-unstable-v1.xml'),
                 ('xdg-shell', Path('/usr/share/wayland-protocols/stable/xdg-shell/xdg-shell.xml'))]
    for name, source in protocols:
        for mode, suffix in [('client-header','.h'),('private-code','.c')]:
            subprocess.run(['wayland-scanner',mode,str(source),str(session.output/(name+suffix))],check=True,timeout=15)
    for name, source, protocol, libs in [
        ('pointer','input-capture-pointer.c','virtual-pointer',['-lm']),
        ('keyboard','input-capture-keyboard.c','virtual-keyboard',['-lxkbcommon']),
        ('receiver','remote-input-pointer-receiver.c','xdg-shell',['-lxkbcommon'])]:
        subprocess.run(['cc','-Wall','-Wextra','-Werror','-I',str(session.output),str(fixture/source),
                        str(session.output/(protocol+'.c')),'-lwayland-client',*libs,'-o',str(session.output/name)],check=True,timeout=30)
    receiver=session.start([str(session.output/'receiver')],'receiver.log')
    receiver_log=session.output/'receiver.log'
    session.wait_for(lambda:'focused' in receiver_log.read_text(),'local receiver focus')
    windows=json.loads(session.run(['niri','msg','--json','windows']))
    window=next(w for w in windows if w['pid']==receiver.pid)
    session.run(['niri','msg','action','fullscreen-window','--id',str(window['id'])])
    def source(name):
        log=session.output/(name+'.log')
        with log.open('w') as output:
            process=subprocess.Popen([str(session.output/name)],stdin=subprocess.PIPE,stdout=output,stderr=output,
                                     env=session.env,start_new_session=True)
        session.processes.append(process)
        session.wait_for(lambda:'ready' in log.read_text(),name+' ready')
        sequence=0
        def send(command):
            nonlocal sequence
            sequence+=1;process.stdin.write((command+'\n').encode());process.stdin.flush()
            session.wait_for(lambda:f'ack {sequence}\n' in log.read_text(),name+' ordered command')
        return process,send
    pointer,mouse=source('pointer'); keyboard,keys=source('keyboard')
    sockets=[]
    class Lease:
        def __init__(self, grants):
            reply,fds=connection.call_with_unix_fd_list_sync(SERVICE,PATH,SERVICE,'OpenCapture',
                GLib.Variant('(u)',(grants,)),GLib.VariantType.new('(hua(uuii)us)'),
                Gio.DBusCallFlags.NO_AUTO_START,5000,None,None)
            handle,self.generation,self.zones,self.keymap,self.text=reply.unpack()
            owned=fds.steal_fds();assert handle==0 and len(owned)==1
            self.socket=socket.socket(fileno=owned[0]);self.socket.settimeout(3);sockets.append(self.socket)
            self.sequence=0;self.events=[]
        def exact(self,length):
            body=b''
            while len(body)<length:
                more=self.socket.recv(length-len(body))
                if not more:raise EOFError('keyboard capture lease closed')
                body+=more
            return body
        def receive(self):
            length,=struct.unpack('<I',self.exact(4));assert 0<length<=16384
            event=json.loads(self.exact(length));self.events.append(event);return event
        def event(self,name):
            for _ in range(64):
                event=self.receive()
                if event['event']==name:return event
            raise AssertionError('missing '+name)
        def command(self,op,**options):
            self.sequence+=1
            body=json.dumps({'sequence':self.sequence,'command':{'op':op,**options}}).encode()
            self.socket.sendall(struct.pack('<I',len(body))+body)
            for _ in range(64):
                event=self.receive()
                if event['event']=='reply' and event['sequence']==self.sequence:return event
            raise AssertionError('missing control reply')
        def arm(self):
            w,h,x,y=self.zones[0]
            assert self.command('set_barriers',zone_set=self.generation,barriers=[[7,[x+w,y,x+w,y+h-1]]])['error'] is None
            assert self.command('enable',keymap=self.keymap)['error'] is None
        def activate(self):
            mouse('absolute 40000 40000 100000 100000');mouse('relative 5000 0')
            activated=self.event('activated');self.initial_modifiers=self.event('modifiers');return activated['activation']
    try:
        # Hold Control before capture, then prove its release stays with the
        # same ordinary client while newly pressed A reaches only the lease.
        keys('key 29 1')
        session.wait_for(lambda:'key 29 1 ' in receiver_log.read_text(),'preexisting local Control')
        first=Lease(1);assert first.text.startswith('xkb_keymap') and first.keymap>0
        assert first.command('enable')['error']=='stale_keyboard_snapshot'
        first.arm();activation=first.activate()
        keys('key 30 1');assert first.event('key')['code']==30
        keys('key 30 0');assert first.event('key')['pressed'] is False
        keys('key 29 0')
        session.wait_for(lambda:'key 29 0 ' in receiver_log.read_text(),'original local Control release')
        assert 'key 30 ' not in receiver_log.read_text()
        first.command('disable')
        passed('preheld-local-control-release-is-preserved-and-new-key-events-are-diverted')

        offset=len(receiver_log.read_text())
        reply,fds=connection.call_with_unix_fd_list_sync('org.canopa.Niri.RemoteInput1',
            '/org/canopa/Niri/RemoteInput1','org.canopa.Niri.RemoteInput1','OpenKeyboard',None,None,
            Gio.DBusCallFlags.NO_AUTO_START,5000,None,None)
        handle,_,_=reply.unpack();owned=fds.steal_fds();assert handle==0 and len(owned)==1
        injected=socket.socket(fileno=owned[0]);injected.settimeout(3);sockets.append(injected)
        injected.sendall(struct.pack('<II',29,1))
        session.wait_for(lambda:'key 29 1 ' in receiver_log.read_text()[offset:],'ordinary injected Control press')
        first.keymap,first.text=call('KeyboardSnapshot')
        first.arm();first.activate();assert first.initial_modifiers['depressed'] & 4 == 0
        keys('key 30 1');assert first.event('key')['code']==30
        keys('key 30 0');first.event('key')
        injected.sendall(struct.pack('<II',29,0))
        session.wait_for(lambda:'key 29 0 ' in receiver_log.read_text()[offset:],'ordinary injected Control release')
        first.command('disable');injected.close()
        passed('injected-control-keeps-its-ordinary-route-without-seeding-capture-modifiers')

        keys('mods 4 0 0 0')
        session.wait_for(lambda:receiver_log.read_text().splitlines()[-1]=='mods 4 0 0 0','preexisting explicit local modifier')
        first.arm();first.activate();assert first.initial_modifiers['depressed']==4
        modifier_offset=len(receiver_log.read_text())
        keys('key 42 1');first.event('key');first.event('modifiers')
        keys('mods 5 0 0 0')
        keys('key 30 1');first.event('key');keys('key 30 0');first.event('key')
        keys('mods 1 0 0 0')
        session.wait_for(lambda:receiver_log.read_text().splitlines()[-1]=='mods 0 0 0 0','original local explicit modifier clear')
        keys('key 42 0');first.event('key');keys('mods 0 0 0 0')
        first.command('disable')
        keys('key 194 1');keys('key 194 0')
        session.wait_for(lambda:'key 194 0 F24' in receiver_log.read_text()[modifier_offset:],'marker after retained-local and new-captured modifier bits')
        local=receiver_log.read_text()[modifier_offset:]
        assert 'key 42 ' not in local and 'key 30 ' not in local,local
        assert all(int(line.split()[1]) & 1 == 0 for line in local.splitlines() if line.startswith('mods ')),local
        passed('preexisting-explicit-local-modifier-mask-keeps-its-local-clear')

        first.arm();activation=first.activate();before=len(receiver_log.read_text())
        keys('key 42 1');assert first.event('key')['code']==42
        assert first.event('modifiers')['depressed']!=0
        assert first.command('release',activation=activation)['error'] is None
        keys('key 42 0');keys('key 194 1');keys('key 194 0')
        session.wait_for(lambda:'key 194 0 F24' in receiver_log.read_text()[before:],'ordered local marker after captured Shift tail')
        local=receiver_log.read_text()[before:]
        assert 'key 42 ' not in local,local
        assert all(line.split()[1]=='0' for line in local.splitlines() if line.startswith('mods ')),local
        passed('captured-modifiers-and-post-release-key-tails-never-reach-the-local-client')

        first.arm();first.activate();before=len(receiver_log.read_text())
        keys('key 29 1');first.event('key');first.event('modifiers');keys('sync')
        keys('key 30 1');first.event('key');keys('key 30 0');first.event('key')
        keys('key 29 0');first.event('key');keys('sync');first.command('disable')
        keys('key 194 1');keys('key 194 0')
        session.wait_for(lambda:'key 194 0 F24' in receiver_log.read_text()[before:],'local marker after explicit modifier protocol')
        local=receiver_log.read_text()[before:]
        assert all(line.split()[1]=='0' for line in local.splitlines() if line.startswith('mods ')),local
        assert 'key 29 ' not in local and 'key 30 ' not in local,local
        passed('explicit-virtual-keyboard-modifier-updates-follow-the-capture-recipient')

        first.arm();first.activate();before=len(receiver_log.read_text())
        keys('key 29 1');first.event('key');first.event('modifiers');keys('sync')
        first.command('disable')
        keys('sync');keys('key 29 0');keys('sync')
        keys('key 194 1');keys('key 194 0')
        session.wait_for(lambda:'key 194 0 F24' in receiver_log.read_text()[before:],'marker after captured modifier-channel tail')
        local=receiver_log.read_text()[before:]
        assert 'key 29 ' not in local,local
        assert all(line.split()[1]=='0' for line in local.splitlines() if line.startswith('mods ')),local
        passed('explicit-modifier-tails-remain-suppressed-after-capture-stops')

        first.arm();first.activate();keys('map fr')
        first.event('deactivated');first.event('disabled');changed=first.event('keymap_changed')['keymap']
        assert changed!=first.keymap
        assert first.command('enable',keymap=first.keymap)['error']=='stale_keyboard_snapshot'
        revision,text=call('KeyboardSnapshot');assert revision==changed and text!=first.text
        first.keymap,first.text=revision,text
        first.arm();first.activate();keys('key 30 1');assert first.event('key')['code']==30
        keys('key 30 0');first.event('key');first.command('disable')
        passed('keymap-change-pauses-before-revision-notice-and-only-current-map-reenables')

        combined=Lease(3);combined.arm();combined.activate()
        mouse('relative 2 1');assert combined.event('motion')['delta']==[2.0,1.0]
        keys('key 30 1');assert combined.event('key')['code']==30
        keys('key 30 0');assert combined.event('key')['pressed'] is False
        combined.command('disable');combined.socket.close()
        passed('combined-pointer-and-keyboard-grant-delivers-both-on-the-same-activation')

        first.arm();first.activate()
        keys('key 29 1');first.event('key');first.event('modifiers')
        keys('key 56 1');first.event('key');first.event('modifiers')
        keys('key 1 1');first.event('deactivated');first.event('disabled')
        assert keyboard.poll() is None
        for code in [1,56,29]:keys(f'key {code} 0')
        passed('control-alt-escape-stops-capture-without-destroying-the-input-source')
    finally:
        for descriptor in sockets:descriptor.close()
        connection.close_sync(None)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--niri',type=Path,required=True);parser.add_argument('--niri-sha256',required=True)
    parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    args.niri=args.niri.resolve(strict=True);assert digest(args.niri)==args.niri_sha256
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    frozen=freeze_niri(args.niri,args.niri_sha256)
    paths=[Path(__file__),ROOT/'scripts/desktop-fixture/input-capture-keyboard.c',
           ROOT/'scripts/desktop-fixture/input-capture-pointer.c',ROOT/'scripts/desktop-fixture/remote-input-pointer-receiver.c',frozen]
    pins={str(p):digest(p) for p in paths}
    result={'status':'running','scope':__doc__,'checks':[],'pins':pins};started=time.monotonic()
    with tempfile.TemporaryDirectory(prefix='canopa-keyboard-capture-') as directory:
        session=MediaSession(Path(directory),args.output,frozen,args.niri_sha256,reference_frozen=True)
        try:verify(session,result);result['status']='passed'
        except Exception as error:result.update(status='failed',error=repr(error),traceback=traceback.format_exc())
        finally:
            processes=list(session.processes);session.close()
            result['processes']=[{'pid':p.pid,'command':p.args,'exit_code':p.poll()} for p in processes]
    result.update(seconds=round(time.monotonic()-started,2),changed_inputs=[p for p,h in pins.items() if digest(Path(p))!=h])
    if result['changed_inputs'] or any(p['exit_code'] is None for p in result['processes']):result['status']='failed'
    (args.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:result.get(k) for k in ['status','error','seconds']}))
    return 0 if result['status']=='passed' else 1


if __name__=='__main__':raise SystemExit(main())
