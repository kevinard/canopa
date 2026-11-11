#!/usr/bin/env python3
"""Real Rust/Iced USB consent through a private frontend and synthetic devices only."""
import argparse
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import tempfile
import time
import traceback
import xml.etree.ElementTree as ET

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
FRONT = 'org.freedesktop.portal.Desktop'
BACK = 'org.freedesktop.impl.portal.desktop.canopa'
PATH = '/org/freedesktop/portal/desktop'
PUBLIC = 'org.freedesktop.portal.Usb'
IMPL = 'org.freedesktop.impl.portal.Usb'
APP = 'org.canopa.UsbFixture'


def digest(path):
    import hashlib
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify(args, session, result, testbed):
    from gi.repository import Gio, GLib
    config = session.runtime / 'canopa.kdl'
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{args.scheme}"; }}\n')
    portals = session.runtime / 'portals'; portals.mkdir()
    descriptor = (args.portal_descriptor or ROOT / 'packaging/portal/canopa.portal').read_text()
    if not args.shipped_descriptor and not args.portal_descriptor:
        descriptor = descriptor.replace('Interfaces=', 'Interfaces=' + IMPL + ';')
    assert IMPL + ';' in descriptor
    (portals / 'canopa.portal').write_text(descriptor)
    (portals / 'portals.conf').write_text('[preferred]\ndefault=none\n' + IMPL + '=canopa\n')
    data = Path(session.env['XDG_DATA_HOME']); (data / 'applications').mkdir(parents=True)
    (data / 'applications' / (APP + '.desktop')).write_text('[Desktop Entry]\nType=Application\nName=USB test application\nExec=true\n')
    home = session.runtime / 'home'; home.mkdir()
    session.env['CANOPA_LANGUAGE'] = 'en'  # dialogs are checked by their English text
    session.env.update(HOME=str(home), XDG_CURRENT_DESKTOP='Canopa', XDG_DESKTOP_PORTAL_DIR=str(portals),
                       XDG_DATA_DIRS=str(data), GIO_USE_VFS='local', GSETTINGS_BACKEND='memory',
                       GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas')
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
        'hotkey-overlay { skip-at-startup; }\nlayout { border { off; }; focus-ring { off; }; }\n',
        width=round(1280 * args.scale), height=round(800 * args.scale), scale=args.scale)
    clients, descriptors = [], []
    def connect(register=False):
        c = Gio.DBusConnection.new_for_address_sync(session.env['DBUS_SESSION_BUS_ADDRESS'],
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        clients.append(c)
        if register: call(c, FRONT, PATH, 'org.freedesktop.host.portal.Registry', 'Register', '(sa{sv})', (APP, {}))
        return c
    def call(c, destination, path, interface, method, signature='()', values=(), with_fds=False):
        if with_fds:
            return c.call_with_unix_fd_list_sync(destination,path,interface,method,GLib.Variant(signature,values),None,Gio.DBusCallFlags.NO_AUTO_START,10000,None,None)
        return c.call_sync(destination,path,interface,method,GLib.Variant(signature,values),None,Gio.DBusCallFlags.NO_AUTO_START,10000,None).unpack()
    def drain():
        while GLib.MainContext.default().pending(): GLib.MainContext.default().iteration(False)
    def wait(condition, label, timeout=20):
        return session.wait_for(lambda: (drain(), condition())[1], label, timeout=timeout)
    def rejected(operation, expected=None):
        try: operation()
        except GLib.Error as error:
            name=Gio.DBusError.get_remote_error(error)
            assert name and name not in ('org.freedesktop.DBus.Error.NoReply','org.freedesktop.DBus.Error.Timeout','org.freedesktop.DBus.Error.TimedOut'), str(error)
            if expected: assert name==expected,(name,expected)
            return
        raise AssertionError('Unapproved USB operation succeeded')
    def passed(name):
        result['checks'].append(name)
        (session.output/'progress.json').write_text(json.dumps(result,indent=2)+'\n')
        print(name,flush=True)
    observer=connect()
    backend=session.start([str(args.binary),'--canopa-config',str(config)],'backend.log')
    def owner(name): return call(observer,'org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','NameHasOwner','(s)',(name,))[0]
    wait(lambda:owner(BACK),'Rust backend')
    permission=next(p for p in (Path('/usr/lib/xdg-permission-store'),Path('/usr/libexec/xdg-permission-store')) if p.is_file())
    session.start([str(permission)],'permission-store.log')
    saved=session.env.copy()
    session.env.update(LD_LIBRARY_PATH=str(args.umockdev/'usr/lib'),LD_PRELOAD=str(args.umockdev/'usr/lib/libumockdev-preload.so.0'),UMOCKDEV_DIR=testbed.get_root_dir())
    try: frontend=session.start([str(args.frontend),'--verbose'],'frontend.log')
    finally: session.env=saved
    wait(lambda:owner(FRONT),'canonical frontend')
    caller=connect(True); outsider=connect(True)
    try:
        xml=call(observer,BACK,PATH,'org.freedesktop.DBus.Introspectable','Introspect')[0]
        interface=next(i for i in ET.fromstring(xml).findall('interface') if i.get('name')==IMPL)
        methods={m.get('name'):(''.join(a.get('type') for a in m.findall('arg') if a.get('direction')!='out'),''.join(a.get('type') for a in m.findall('arg') if a.get('direction')=='out')) for m in interface.findall('method')}
        assert methods=={'AcquireDevices':('ossa(sa{sv}a{sv})a{sv}','ua{sv}')},methods
        assert call(observer,BACK,PATH,'org.freedesktop.DBus.Properties','Get','(ss)',(IMPL,'version'))[0]==1
        assert call(observer,FRONT,PATH,'org.freedesktop.DBus.Properties','Get','(ss)',(PUBLIC,'version'))[0]==1
        assert call(caller,FRONT,PATH,PUBLIC,'EnumerateDevices','(a{sv})',({},))[0]==[]
        passed('exact-v1-wire-and-empty-synthetic-inventory-before-fixture-insertion')
        read_fd,write_fd=os.pipe()
        try:
            incoming=Gio.UnixFDList.new();index=incoming.append(read_fd)
            rejected(lambda:caller.call_with_unix_fd_list_sync(BACK,PATH,IMPL,'AcquireDevices',
                GLib.Variant('(ossa(sa{sv}a{sv})a{sv})',(PATH+'/request/fixture/attached','',APP,[('fixture',{}, {})],{'future-fd':GLib.Variant('h',index)})),
                None,Gio.DBusCallFlags.NO_AUTO_START,10000,incoming,None),'org.freedesktop.DBus.Error.InvalidArgs')
        finally:os.close(read_fd);os.close(write_fd)
        passed('unexpected-input-fd-is-rejected-before-consent')
        for index,name in enumerate(('Research controller','Measurement device','Backup controller')):
            node=f'bus/usb/001/{100+index:03d}'
            testbed.add_from_string(f'P: /devices/usbfixture{index}\nN: {node}\nE: DEVNAME=/dev/{node}\nE: DEVTYPE=usb_device\nE: DRIVER=usb\nE: ID_BUS=usb\nE: ID_MODEL_ID=abcd\nE: ID_MODEL_FROM_DATABASE={name}\nE: ID_VENDOR_FROM_DATABASE=Canopa test devices\nE: ID_SERIAL_SHORT=fixture-{index}\nE: ID_VENDOR_ID=1234\nE: SUBSYSTEM=usb\nA: idProduct=abcd\nA: idVendor=1234\n')
            (Path(testbed.get_root_dir())/'dev'/node).write_bytes(f'fixture-{index}'.encode())
        def inventory():
            values=call(caller,FRONT,PATH,PUBLIC,'EnumerateDevices','(a{sv})',({},))[0]
            return values if len(values)==3 else None
        devices=wait(inventory,'synthetic inventory')
        ids={info['properties']['ID_SERIAL_SHORT']:key for key,info in devices}; first=ids['fixture-0'];second=ids['fixture-1']
        events=[];foreign_events=[]
        caller.signal_subscribe(FRONT,PUBLIC,'DeviceEvents',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:events.append(v.unpack()))
        outsider.signal_subscribe(FRONT,PUBLIC,'DeviceEvents',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:foreign_events.append(v.unpack()))
        monitor=call(caller,FRONT,PATH,PUBLIC,'CreateSession','(a{sv})',({'session_handle_token':GLib.Variant('s','monitor')},))[0]
        wait(lambda:any(key==monitor and {device for action,device,info in changes if action=='add'}==set(ids.values()) for key,changes in events),'initial directed monitoring inventory')
        rejected(lambda:call(outsider,FRONT,monitor,'org.freedesktop.portal.Session','Close'),'org.freedesktop.DBus.Error.AccessDenied')
        assert not foreign_events
        passed('frontend-monitoring-sends-fixture-inventory-only-to-original-session-owner')
        protocols=Path('/usr/share/wayland-protocols')
        for name,xml_path in [('xdg-shell',protocols/'stable/xdg-shell/xdg-shell.xml'),('xdg-foreign',protocols/'unstable/xdg-foreign/xdg-foreign-unstable-v2.xml')]:
            for mode,suffix in [('client-header','.h'),('private-code','.c')]:
                subprocess.run(['wayland-scanner',mode,str(xml_path),str(session.output/(name+suffix))],env=session.env,check=True,timeout=15)
        parent=session.output/'portal-parent'
        subprocess.run(['cc','-Wall','-Wextra','-Werror','-I',str(session.output),str(ROOT/'scripts/desktop-fixture/portal-parent.c'),str(session.output/'xdg-shell.c'),str(session.output/'xdg-foreign.c'),'-lwayland-client','-o',str(parent)],env=session.env,check=True,timeout=30)
        session.start([str(parent)],'parent.log')
        exported=wait(lambda:next((line for line in (session.output/'parent.log').read_text().splitlines() if line.startswith('wayland:')),None),'exported parent')
        # Keep one virtual keyboard alive between short event-producing clients.
        session.start(['wtype','-s','600000'],'keyboard.log')
        pointer=session.build_pointer()
        outputs=json.loads(session.run(['niri','msg','--json','outputs']))
        logical=next(o['logical'] for o in outputs.values() if o.get('logical'))
        def windows(): return json.loads(session.run(['niri','msg','--json','windows']))
        def action(*args): return session.run(['niri','msg','action',*map(str,args)])
        def keys(*args):
            session.run(['wtype',*args]);time.sleep(.15);drain()
        def alt(key): keys('-M','alt','-k',key,'-m','alt')
        def click(x,y):
            session.run([str(pointer),str(round(x/logical['width']*1600)),str(round(y/logical['height']*1000))]);time.sleep(.15);drain()
        def gone(window):
            wait(lambda:not any(w['id']==window['id'] for w in windows()),'USB dialog closes')
            terminal=bool(select.select([window['pidfd']],[],[],5)[0]);os.close(window['pidfd']);window['pidfd']=None
            result.setdefault('helpers',[]).append(dict(pid=window['pid'],terminal=terminal));assert terminal
        responses={};pending={};count=0
        def subscribe(c):
            c.signal_subscribe(FRONT,'org.freedesktop.portal.Request','Response',None,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:responses.__setitem__(p,v.unpack()))
        subscribe(caller);subscribe(outsider)
        def begin(offered=None,client=caller,direct=None):
            nonlocal count
            count+=1;before={w['id'] for w in windows()}
            offered=offered if offered is not None else [(first,{'writable':GLib.Variant('b',True)}),(second,{})]
            if direct is None:
                path=call(client,FRONT,PATH,PUBLIC,'AcquireDevices','(sa(sa{sv})a{sv})',(exported,offered,{'handle_token':GLib.Variant('s','usb'+str(count))}))[0]
            else:
                path=PATH+'/request/fixture/direct'+str(count)
                def done(c,reply):
                    try: pending[path]=c.call_finish(reply).unpack()
                    except GLib.Error as error: pending[path]=error
                client.call(BACK,PATH,IMPL,'AcquireDevices',GLib.Variant('(ossa(sa{sv}a{sv})a{sv})',(path,exported,APP,direct,{})),None,Gio.DBusCallFlags.NO_AUTO_START,60000,None,done)
            window=wait(lambda:next((w for w in windows() if w['app_id']=='org.canopa.Portal' and w['id'] not in before),None),'Iced USB dialog')
            assert window['is_floating'] and abs(window['layout']['window_size'][0]-680)<=1,window
            command=Path(f"/proc/{window['pid']}/cmdline").read_bytes().split(b'\0');assert b'--usb-dialog' in command,command
            window['pidfd']=os.pidfd_open(window['pid']);session.usb_helpers.append(window)
            action('move-floating-window','--id',window['id'],'--x',20,'--y',10);action('focus-window','--id',window['id']);time.sleep(.3)
            return path,window
        def response(path,status=0,direct=False):
            values=wait(lambda:(pending if direct else responses).get(path),'USB response')
            assert values[0]==status,values;return values[1]
        def finish(path,client=caller):
            value,fdlist=call(client,FRONT,PATH,PUBLIC,'FinishAcquireDevices','(oa{sv})',(path,{}),True)
            values,finished=value.unpack();assert finished
            owned=fdlist.steal_fds() if fdlist else [];descriptors.extend(owned)
            assert len(owned)==sum(bool(options['success']) for key,options in values)
            for key,options in values:
                if not options['success']:continue
                target=os.readlink(f"/proc/self/fd/{owned[options['fd']]}")
                assert target.startswith(testbed.get_root_dir()+'/dev/'),target
            return values,owned
        def release(values,client=caller):call(client,FRONT,PATH,PUBLIC,'ReleaseDevices','(asa{sv})',([key for key,options in values],{}))
        def cached():
            targets=[]
            for p in Path(f'/proc/{frontend.pid}/fd').glob('*'):
                try:
                    target=os.readlink(p)
                    if target.startswith(testbed.get_root_dir()+'/dev/'):targets.append(target)
                except FileNotFoundError:pass
            return targets
        path,window=begin();keys('-k','Return');alt('a')
        assert path not in responses and not cached()
        session.capture('01-unchecked-'+args.scheme+'.png')
        # Use the real fixed footer button for explicit denial.
        click(190,10+window['layout']['window_size'][1]-42)
        assert response(path,1)=={};gone(window)
        rejected(lambda:finish(path));assert not cached()
        passed('initial-enter-and-empty-allow-do-not-grant-explicit-deny-returns-no-device')
        path,window=begin();click(64,233);click(64,372)
        session.capture('02-read-only-'+args.scheme+'.png');click(530,10+window['layout']['window_size'][1]-42);assert response(path)=={};gone(window)
        values,owned=finish(path);assert len(values)==1 and values[0][0]==first and fcntl.fcntl(owned[0],fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDONLY
        assert os.read(owned[0],100)==b'fixture-0';release(values);assert not cached()
        passed('pointer-selects-one-of-two-devices-and-read-only-downgrade-delivers-only-that-synthetic-fd')
        path,window=begin();alt('s');alt('a');response(path);gone(window)
        rejected(lambda:finish(path,outsider),'org.freedesktop.portal.Error.NotAllowed')
        values,owned=finish(path);assert len(values)==2
        modes={key:fcntl.fcntl(owned[options['fd']],fcntl.F_GETFL)&os.O_ACCMODE for key,options in values}
        assert modes=={first:os.O_RDWR,second:os.O_RDONLY},modes
        release(values);assert not cached()
        passed('selected-devices-retain-individual-access-modes-and-foreign-finish-is-denied')
        path,window=begin();alt('s');alt('s');alt('a');assert path not in responses
        keys('-k','Escape');response(path,1);gone(window);assert not cached()
        passed('clearing-selection-disables-grant-and-escape-denies')
        path,window=begin();action('close-window','--id',window['id']);response(path,1);gone(window)
        assert not cached();passed('ordinary-window-close-denies-without-device-access')
        path,window=begin();rejected(lambda:call(outsider,FRONT,path,'org.freedesktop.portal.Request','Close'),'org.freedesktop.DBus.Error.AccessDenied')
        call(caller,FRONT,path,'org.freedesktop.portal.Request','Close');gone(window)
        rejected(lambda:finish(path));assert path not in responses and not cached()
        passed('foreign-close-is-denied-and-owner-close-retires-helper-without-pending-access')
        lost=connect(True);path,window=begin(client=lost);lost.close_sync(None);gone(window);assert not cached()
        passed('original-caller-loss-retires-owned-iced-helper-without-device-access')
        first_path,first_window=begin();second_path,second_window=begin(client=outsider)
        alt('s');alt('a');response(second_path);gone(second_window)
        values,owned=finish(second_path,outsider);release(values,outsider)
        assert any(w['id']==first_window['id'] for w in windows())
        action('focus-window','--id',first_window['id']);keys('-k','Escape');response(first_path,1);gone(first_window)
        assert not cached();passed('concurrent-callers-keep-independent-decisions-and-owned-fds')
        # More rows than fit exercise scrolling without touching any device node.
        direct=[('long'+str(i),{'properties':GLib.Variant('a{sv}',{'ID_MODEL_FROM_DATABASE':GLib.Variant('s','Synthetic long device label '+str(i)+' '+'detail '*12),'ID_SERIAL_SHORT':GLib.Variant('s','serial-'+str(i))})},{'writable':GLib.Variant('b',True)}) for i in range(32)]
        path,window=begin(direct=direct)
        action('set-window-width','--id',window['id'],440);action('set-window-height','--id',window['id'],360);time.sleep(.3)
        session.capture('03-narrow-'+args.scheme+'.png');keys('-k','Escape');response(path,1,True);gone(window)
        passed('long-device-list-keeps-denial-reachable-at-minimum-window-size')
        path,window=begin([(first,{})]);alt('s')
        testbed.remove_device('/sys/devices/usbfixture0')
        wait(lambda:len(call(caller,FRONT,PATH,PUBLIC,'EnumerateDevices','(a{sv})',({},))[0])==2,'synthetic unplug during consent')
        alt('a');response(path);gone(window);values,owned=finish(path)
        assert len(values)<=1 and not any(options['success'] for key,options in values) and not owned
        passed('unplug-during-consent-cannot-open-a-stale-device')
        wait(lambda:any(key==monitor and any(action=='remove' and device==first for action,device,info in changes) for key,changes in events),'directed unplug event')
        assert not foreign_events
        call(caller,FRONT,monitor,'org.freedesktop.portal.Session','Close')
        passed('frontend-unplug-monitoring-remains-directed-and-owner-can-close-it')
    finally:
        for fd in descriptors:os.close(fd)
        for c in clients:
            if not c.is_closed():c.close_sync(None)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('binary','niri','frontend'):
        parser.add_argument('--'+name,type=Path,required=True);parser.add_argument('--'+name+'-sha256',required=True)
    parser.add_argument('--umockdev',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--scheme',choices=('dark','light'),default='dark')
    parser.add_argument('--scale',type=float,default=1)
    parser.add_argument('--shipped-descriptor',action='store_true')
    parser.add_argument('--portal-descriptor',type=Path,help='Use an exact optional package descriptor')
    parser.add_argument('--mocked',action='store_true',help=argparse.SUPPRESS)
    args=parser.parse_args();args.umockdev=args.umockdev.resolve(strict=True)
    if not args.mocked:
        env=dict(os.environ,LD_LIBRARY_PATH=str(args.umockdev/'usr/lib'),GI_TYPELIB_PATH=str(args.umockdev/'usr/lib/girepository-1.0'),LD_PRELOAD=str(args.umockdev/'usr/lib/libumockdev-preload.so.0'))
        os.execve(sys.executable,[sys.executable,__file__,*sys.argv[1:],'--mocked'],env)
    import gi
    gi.require_version('UMockdev','1.0');gi.require_version('GUdev','1.0')
    from gi.repository import UMockdev,GUdev
    testbed=UMockdev.Testbed.new();assert not GUdev.Client.new(['usb']).query_by_subsystem('usb')
    from native_compositor import freeze_niri
    spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py');desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)
    for name in ('binary','niri','frontend'):
        p=getattr(args,name).resolve(strict=True);setattr(args,name,p);assert digest(p)==getattr(args,name+'_sha256')
    frozen=freeze_niri(args.niri,args.niri_sha256)
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    pins={str(p):digest(p) for p in (Path(__file__),args.binary,args.frontend,frozen,ROOT/'scripts/validate-desktop.py',ROOT/'scripts/native_compositor.py',ROOT/'packaging/portal/canopa.portal',args.umockdev/'usr/lib/libumockdev-preload.so.0')}
    if args.portal_descriptor:
        args.portal_descriptor=args.portal_descriptor.resolve(strict=True);pins[str(args.portal_descriptor)]=digest(args.portal_descriptor)
    started=time.monotonic();result=dict(status='running',scope='Private canonical frontend, Rust/Iced consent, synthetic devices only. No real USB/Flatpak, installed routing or distribution acceptance.',checks=[],pins=pins,scheme=args.scheme,scale=args.scale)
    with tempfile.TemporaryDirectory(prefix='canopa-usb-portal-') as directory:
        session=desktop.Session(Path(directory),args.output);session.usb_helpers=[]
        for name in ('LD_PRELOAD','LD_LIBRARY_PATH','GI_TYPELIB_PATH','UMOCKDEV_DIR'):session.env.pop(name,None)
        session.env['PATH']=str(frozen.parent)+os.pathsep+session.env['PATH']
        try:verify(args,session,result,testbed);result['status']='passed'
        except Exception as error:
            result.update(status='failed',error=repr(error),traceback=traceback.format_exc())
            try:session.capture('failure.png')
            except Exception:pass
        finally:
            for process in reversed(session.processes):
                if process.poll() is None:
                    # Each live Popen is the owned leader of this fixture's group.
                    assert os.getpgid(process.pid)==process.pid
                    os.killpg(process.pid,signal.SIGTERM)
                    try:process.wait(timeout=5)
                    except subprocess.TimeoutExpired:os.killpg(process.pid,signal.SIGKILL);process.wait()
            result['processes']=[dict(pid=p.pid,command=p.args,exit_code=p.poll()) for p in session.processes]
            for window in session.usb_helpers:
                if window['pidfd'] is not None:
                    terminal=bool(select.select([window['pidfd']],[],[],5)[0]);os.close(window['pidfd'])
                    result.setdefault('helpers',[]).append(dict(pid=window['pid'],terminal=terminal))
            if not all(h['terminal'] for h in result.get('helpers',[])):result['status']='failed'
    result.update(seconds=round(time.monotonic()-started,2),changed_inputs=[p for p,h in pins.items() if digest(Path(p))!=h])
    if result['changed_inputs'] or any(p['exit_code'] is None for p in result['processes']):result['status']='failed'
    (args.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:result.get(k) for k in ('status','error','seconds')}))
    raise SystemExit(0 if result['status']=='passed' else 1)


if __name__=='__main__':main()
