#!/usr/bin/env python3
"""Exercise canonical USB frontend ownership using only private synthetic UMockdev devices.

The consent service is controllable to test frontend trust boundaries. Real Rust
and Iced acceptance is separate. No host bus, USB ioctls or real device opens.
"""
from pathlib import Path
import argparse
import fcntl
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
import traceback

sys.dont_write_bytecode = True
root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--frontend', type=Path, required=True)
parser.add_argument('--source', type=Path, required=True, help='Verified frontend source with backend XML')
parser.add_argument('--umockdev', type=Path, required=True, help='Private UMockdev prefix; no system installation required')
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--observe-baseline', action='store_true', help='Record defective baseline observations without asserting fixed behavior')
parser.add_argument('--mocked', action='store_true', help=argparse.SUPPRESS)
args = parser.parse_args()
frontend = args.frontend.resolve(strict=True)
tools = args.umockdev.resolve(strict=True)
if '--mocked' not in sys.argv:
    env = dict(os.environ, LD_LIBRARY_PATH=str(tools / 'usr/lib'),
               GI_TYPELIB_PATH=str(tools / 'usr/lib/girepository-1.0'),
               LD_PRELOAD=str(tools / 'usr/lib/libumockdev-preload.so.0'))
    os.execve(sys.executable, [sys.executable, __file__, *sys.argv[1:], '--mocked'], env)
from gi.repository import Gio, GLib
import gi
gi.require_version('UMockdev', '1.0')
gi.require_version('GUdev', '1.0')
from gi.repository import UMockdev, GUdev

output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
FRONT = 'org.freedesktop.portal.Desktop'
BACK = 'org.freedesktop.impl.portal.desktop.canopa'
PATH = '/org/freedesktop/portal/desktop'
USB = 'org.freedesktop.portal.Usb'
APP = 'org.canopa.UsbFixture'
impl = args.source.resolve(strict=True) / 'data/org.freedesktop.impl.portal.Usb.xml'
def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()
pins = {str(path): digest(path) for path in (Path(__file__), frontend, impl, tools / 'usr/lib/libumockdev-preload.so.0', tools / 'usr/lib/libumockdev.so.0', tools / 'usr/lib/girepository-1.0/UMockdev-1.0.typelib')}
result = dict(status='running', scope='Canonical frontend with controllable fake consent, synthetic UMockdev devices only; no Rust/UI/hardware acceptance.', pins=pins, checks=[], observations={})
processes = []
connections = []
fds = []
testbed = UMockdev.Testbed.new()
assert not GUdev.Client.new(['usb']).query_by_subsystem('usb')
started = time.monotonic()
try:
    with tempfile.TemporaryDirectory(prefix='canopa-usb-front-') as directory:
        runtime = Path(directory)
        env = dict(os.environ, XDG_RUNTIME_DIR=str(runtime), HOME=str(runtime / 'home'),
                   XDG_CONFIG_HOME=str(runtime / 'config'), XDG_DATA_HOME=str(runtime / 'data'),
                   XDG_CACHE_HOME=str(runtime / 'cache'), XDG_CURRENT_DESKTOP='Canopa',
                   XDG_DATA_DIRS=str(runtime / 'data'), GIO_USE_VFS='local', GSETTINGS_BACKEND='memory',
                   GSETTINGS_SCHEMA_DIR='/usr/share/glib-2.0/schemas', UMOCKDEV_DIR=testbed.get_root_dir(),
                   DBUS_SYSTEM_BUS_ADDRESS='unix:path=' + str(runtime / 'no-system-bus'))
        env['XDG_STATE_HOME'] = str(runtime / 'state')
        for key in ('DISPLAY', 'WAYLAND_DISPLAY', 'DBUS_STARTER_ADDRESS', 'DBUS_STARTER_BUS_TYPE'):
            env.pop(key, None)
        for name in ('home', 'config', 'data/applications', 'cache', 'portals'):
            (runtime / name).mkdir(parents=True)
        (runtime / 'data/applications' / (APP + '.desktop')).write_text('[Desktop Entry]\nType=Application\nName=USB fixture\nExec=true\n')
        (runtime / 'portals/canopa.portal').write_text('[portal]\nDBusName=' + BACK + '\nInterfaces=org.freedesktop.impl.portal.Usb;\nUseIn=Canopa;\n')
        (runtime / 'portals/portals.conf').write_text('[preferred]\ndefault=none\norg.freedesktop.impl.portal.Usb=canopa\n')
        env['XDG_DESKTOP_PORTAL_DIR'] = str(runtime / 'portals')
        def start(command, name):
            log = (output / name).open('w')
            process = subprocess.Popen(command, env=env, stdout=log, stderr=log)
            log.close(); processes.append(process); return process
        bus_config = runtime / 'bus.conf'
        bus_config.write_text('<busconfig><type>session</type><listen>unix:tmpdir=' + str(runtime) + '</listen><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        bus = subprocess.Popen(['dbus-daemon', '--config-file=' + str(bus_config), '--nofork', '--print-address=1'], env=env, stdout=subprocess.PIPE, stderr=(output / 'bus.log').open('w'), text=True)
        processes.append(bus); env['DBUS_SESSION_BUS_ADDRESS'] = bus.stdout.readline().strip()
        def connect():
            c = Gio.DBusConnection.new_for_address_sync(env['DBUS_SESSION_BUS_ADDRESS'], Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
            connections.append(c); return c
        def wait(condition, label, timeout=10):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                while GLib.MainContext.default().pending(): GLib.MainContext.default().iteration(False)
                assert all(p.poll() is None for p in processes), label
                value = condition()
                if value: return value
                time.sleep(.01)
            raise AssertionError('Timed out: ' + label)
        def call(c, destination, path, interface, method, signature='()', values=(), with_fds=False):
            done = []
            def reply(source, pending):
                try: done.append(source.call_with_unix_fd_list_finish(pending) if with_fds else source.call_finish(pending))
                except GLib.Error as error: done.append(error)
            if with_fds:
                c.call_with_unix_fd_list(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 10000, None, None, reply)
            else:
                c.call(destination, path, interface, method, GLib.Variant(signature, values), None, Gio.DBusCallFlags.NONE, 10000, None, reply)
            wait(lambda: done, method)
            if isinstance(done[0], GLib.Error): raise done[0]
            return done[0] if with_fds else done[0].unpack()
        server = connect(); caller = connect()
        call(server, 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'RequestName', '(su)', (BACK, 0))
        queued = []
        info = Gio.DBusNodeInfo.new_for_xml(impl.read_text()).interfaces[0]
        request_info = Gio.DBusNodeInfo.new_for_xml('<node><interface name="org.freedesktop.impl.portal.Request"><method name="Close"/></interface></node>').interfaces[0]
        def request_method(c,s,p,i,m,v,invocation): invocation.return_value(None)
        def method(c,s,p,i,m,v,invocation):
            handle,parent,app,offered,options = v.unpack()
            assert app == APP and offered
            registration = server.register_object(handle, request_info, request_method, None, None)
            queued.append(dict(handle=handle, offered=offered, invocation=invocation, registration=registration))
        server.register_object(PATH, info, method, lambda c,s,p,i,name: GLib.Variant('u', 1), None)
        permission_binary = next(path for path in (Path('/usr/lib/xdg-permission-store'), Path('/usr/libexec/xdg-permission-store')) if path.exists())
        start([str(permission_binary)], 'permission-store.log')
        front = start([str(frontend), '--verbose'], 'frontend.log')
        wait(lambda: call(caller, 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', '(s)', (FRONT,))[0], 'frontend owner')
        call(caller, FRONT, PATH, 'org.freedesktop.host.portal.Registry', 'Register', '(sa{sv})', (APP, {}))
        assert call(caller, FRONT, PATH, USB, 'EnumerateDevices', '(a{sv})', ({},))[0] == []
        result['checks'].append('empty synthetic inventory before fixture insertion')
        for index in range(10):
            node = f'bus/usb/001/{100+index:03d}'
            testbed.add_from_string(f'P: /devices/usbfixture{index}\nN: {node}\nE: DEVNAME=/dev/{node}\nE: DEVTYPE=usb_device\nE: DRIVER=usb\nE: ID_BUS=usb\nE: ID_MODEL_ID=abcd\nE: ID_SERIAL_SHORT=fixture-{index}\nE: ID_SERIAL=canopa-fixture-{index}\nE: ID_VENDOR_ID=1234\nE: SUBSYSTEM=usb\nA: idProduct=abcd\nA: idVendor=1234\n')
            (Path(testbed.get_root_dir()) / 'dev' / node).write_bytes(f'fixture-{index}'.encode())
        def inventory():
            values = call(caller, FRONT, PATH, USB, 'EnumerateDevices', '(a{sv})', ({},))[0]
            return values if len(values) == 10 else None
        devices = wait(inventory, 'ten synthetic devices')
        ids = {info['properties']['ID_SERIAL_SHORT']: key for key,info in devices}
        responses = {}
        caller.signal_subscribe(FRONT, 'org.freedesktop.portal.Request', 'Response', None, None, Gio.DBusSignalFlags.NONE,
            lambda c,s,p,i,n,v: responses.__setitem__(p, v.unpack()))
        def begin_many(devices):
            before = len(queued)
            handle = call(caller, FRONT, PATH, USB, 'AcquireDevices', '(sa(sa{sv})a{sv})', ('', devices, {'handle_token': GLib.Variant('s', 'fixture' + str(before))}))[0]
            responses.pop(handle, None)
            pending = wait(lambda: queued[before] if len(queued)>before else None, 'consent call')
            assert pending['handle'] == handle
            return handle,pending
        def begin(key, writable):
            return begin_many([(key, {'writable': GLib.Variant('b', writable)})])
        def reply(pending, code, values):
            pending['invocation'].return_value(GLib.Variant('(ua{sv})', (code, values)))
            server.unregister_object(pending['registration'])
        def approve(pending, choices=None):
            if choices is None:
                choices = [(key, {'writable': GLib.Variant('b', options.get('writable', False))}) for key,info,options in pending['offered']]
            reply(pending, 0, {'devices': GLib.Variant('a(sa{sv})', choices)})
        def finish(handle):
            value, descriptors = call(caller, FRONT, PATH, USB, 'FinishAcquireDevices', '(oa{sv})', (handle, {}), True)
            values, finished = value.unpack(); assert finished and len(values)==1 and values[0][1]['success']
            owned = descriptors.steal_fds(); index=values[0][1]['fd']; assert len(owned)==1 and index==0
            fd=owned[0];fds.append(fd)
            target=os.readlink(f'/proc/self/fd/{fd}');assert target.startswith(testbed.get_root_dir()+'/dev/'),target
            return fd
        def acquire(key, writable):
            handle,pending=begin(key,writable);approve(pending);assert wait(lambda: responses.get(handle), 'successful response')[0]==0;return finish(handle)
        first = ids['fixture-0']; second = ids['fixture-1']; third = ids['fixture-2']
        ro=acquire(first,False);rw=acquire(second,True)
        assert fcntl.fcntl(ro,fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDONLY
        assert fcntl.fcntl(rw,fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDWR
        assert os.read(ro,100)==b'fixture-0' and os.read(rw,100)==b'fixture-1'
        result['checks'].append('fresh read-only/read-write acquisitions transfer fixture-only FDs with the requested mode')
        upgraded=acquire(first,True);downgraded=acquire(second,False)
        result['observations']['reacquired_modes']={'read_only_then_write':fcntl.fcntl(upgraded,fcntl.F_GETFL)&os.O_ACCMODE,'write_then_read_only':fcntl.fcntl(downgraded,fcntl.F_GETFL)&os.O_ACCMODE}
        for key in (first,second,first,second):call(caller,FRONT,PATH,USB,'ReleaseDevices','(asa{sv})',([key],{}))
        targets=[]
        for path in Path(f'/proc/{front.pid}/fd').glob('*'):
            try:
                target=os.readlink(path)
                if target.startswith(testbed.get_root_dir()+'/dev/'):targets.append(target)
            except FileNotFoundError:pass
        result['observations']['frontend_device_fd_copies_after_matching_releases']=targets
        handle,pending=begin(third,False)
        call(caller,FRONT,handle,'org.freedesktop.portal.Request','Close')
        try:
            call(caller,FRONT,handle,'org.freedesktop.portal.Request','Close')
            raise AssertionError('Closed public request is still exported')
        except GLib.Error as error:
            assert Gio.DBusError.get_remote_error(error) not in ('org.freedesktop.DBus.Error.NoReply','org.freedesktop.DBus.Error.Timeout')
        approve(pending)
        deadline=time.monotonic()+.3
        while time.monotonic()<deadline:
            while GLib.MainContext.default().pending():GLib.MainContext.default().iteration(False)
            time.sleep(.01)
        try:
            finish(handle);result['observations']['late_success_after_close_can_still_finish']=True
        except GLib.Error as error:
            result['observations']['late_success_after_close_can_still_finish']=False
            result['observations']['late_finish_error']=str(error)
        result['observations']['late_public_response']=handle in responses
        invalid = {}
        invalid_grants = {
            'unrequested-device': [(third, {})],
            'duplicate-device': [(first, {}), (first, {})],
            'write-escalation': [(first, {'writable': GLib.Variant('b', True)})],
            'wrong-writable-type': [(first, {'writable': GLib.Variant('s', 'true')})],
            'valid-prefix-then-unrequested': [(first, {}), (third, {})],
        }
        for name, choices in invalid_grants.items():
            handle,pending=begin_many([(first,{}),(second,{})]) if name in ('duplicate-device','valid-prefix-then-unrequested') else begin(first,False)
            approve(pending,choices)
            response=wait(lambda: responses.get(handle), name+' response')
            try:
                value,descriptors=call(caller,FRONT,PATH,USB,'FinishAcquireDevices','(oa{sv})',(handle,{}),True)
                owned=descriptors.steal_fds() if descriptors else [];fds.extend(owned)
                invalid[name]={'response':response[0],'finish_allowed':True,'fd_count':len(owned)}
                call(caller,FRONT,PATH,USB,'ReleaseDevices','(asa{sv})',([first,third],{}))
            except GLib.Error:
                invalid[name]={'response':response[0],'finish_allowed':False}
        for name, values in {'missing-devices':{},'wrong-devices-type':{'devices':GLib.Variant('s','invalid')}}.items():
            handle,pending=begin(first,False);reply(pending,0,values)
            response=wait(lambda:responses.get(handle),name+' response')
            invalid[name]={'response':response[0]}
        result['observations']['invalid_grants']=invalid
        handle,pending=begin_many([(first,{'writable':GLib.Variant('b',True)}),(second,{})])
        approve(pending,[(first,{'writable':GLib.Variant('b',False),'future-extension':GLib.Variant('s','ignored')})])
        assert wait(lambda:responses.get(handle),'valid subset response')[0]==0
        assert fcntl.fcntl(finish(handle),fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDONLY
        call(caller,FRONT,PATH,USB,'ReleaseDevices','(asa{sv})',([first],{}))
        result['checks'].append('valid subset/read-only downgrade and unknown result option retain exact requested FD mode')
        handle,pending=begin(first,False);approve(pending,[])
        assert wait(lambda:responses.get(handle),'empty subset')[0]==0
        value,descriptors=call(caller,FRONT,PATH,USB,'FinishAcquireDevices','(oa{sv})',(handle,{}),True)
        assert value.unpack()==([],True) and (descriptors is None or descriptors.get_length()==0)
        result['checks'].append('empty valid subset finishes without descriptors')
        handle,pending=begin_many([(key,{}) for key,info in devices]);approve(pending)
        assert wait(lambda:responses.get(handle),'batched approval')[0]==0
        batches=[];received=[]
        while True:
            value,descriptors=call(caller,FRONT,PATH,USB,'FinishAcquireDevices','(oa{sv})',(handle,{}),True)
            values,done=value.unpack();owned=descriptors.steal_fds() if descriptors else [];fds.extend(owned)
            assert len(owned)<=8 and len(owned)==len(values) and all(options['success'] for key,options in values)
            for key,options in values:
                fd=owned[options['fd']];target=os.readlink(f'/proc/self/fd/{fd}')
                assert target.startswith(testbed.get_root_dir()+'/dev/'),target
                assert fcntl.fcntl(fd,fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDONLY
                received.append(key)
            batches.append(len(owned))
            if done:break
        assert batches==[8,2] and set(received)=={key for key,info in devices} and len(received)==10
        call(caller,FRONT,PATH,USB,'ReleaseDevices','(asa{sv})',(received,{}))
        result['checks'].append('ten synthetic devices retain eight-FD batching and exact descriptor indexes')
        acquire(first,False)
        handle,pending=begin(first,False);approve(pending)
        assert wait(lambda:responses.get(handle),'approval before permission change')[0]==0
        store='org.freedesktop.impl.portal.PermissionStore'
        call(caller,store,'/org/freedesktop/impl/portal/PermissionStore',store,'SetPermission','(sbssas)',('usb',True,'usb:1234/abcd/canopa-fixture-0',APP,['no']))
        value,descriptors=call(caller,FRONT,PATH,USB,'FinishAcquireDevices','(oa{sv})',(handle,{}),True)
        values,done=value.unpack();owned=descriptors.steal_fds() if descriptors else [];fds.extend(owned)
        assert done and len(values)==1
        result['observations']['cached_device_permission_revoked']={'success':values[0][1]['success'],'fd_count':len(owned)}
        call(caller,FRONT,PATH,USB,'ReleaseDevices','(asa{sv})',([first],{}))
        call(caller,store,'/org/freedesktop/impl/portal/PermissionStore',store,'SetPermission','(sbssas)',('usb',True,'usb:1234/abcd/canopa-fixture-0',APP,['yes']))
        acquire(first,False)
        handle,pending=begin(first,False);approve(pending)
        assert wait(lambda:responses.get(handle),'approval before unplug')[0]==0
        testbed.remove_device('/sys/devices/usbfixture0')
        wait(lambda: len(call(caller,FRONT,PATH,USB,'EnumerateDevices','(a{sv})',({},))[0])==9,'synthetic unplug')
        value,descriptors=call(caller,FRONT,PATH,USB,'FinishAcquireDevices','(oa{sv})',(handle,{}),True)
        values,done=value.unpack();owned=descriptors.steal_fds() if descriptors else [];fds.extend(owned)
        assert done and len(values)==1
        result['observations']['cached_device_unplug']={'success':values[0][1]['success'],'fd_count':len(owned)}
        call(caller,FRONT,PATH,USB,'ReleaseDevices','(asa{sv})',([first],{}))
        if not args.observe_baseline:
            assert result['observations']['reacquired_modes']=={'read_only_then_write':os.O_RDWR,'write_then_read_only':os.O_RDONLY}
            assert not result['observations']['frontend_device_fd_copies_after_matching_releases']
            assert not result['observations']['late_success_after_close_can_still_finish']
            assert not result['observations']['late_public_response']
            assert all(row['response']==2 and not row.get('finish_allowed',False) for row in invalid.values()),invalid
            assert result['observations']['cached_device_unplug']=={'success':False,'fd_count':0}
            assert result['observations']['cached_device_permission_revoked']=={'success':False,'fd_count':0}
        result['status']='passed'
except Exception as error:
    result.update(status='failed',error=repr(error),traceback=traceback.format_exc())
finally:
    for fd in fds:os.close(fd)
    for c in connections:
        if not c.is_closed():c.close_sync(None)
    for process in reversed(processes):
        if process.poll() is None:process.terminate()
        try:process.wait(timeout=5)
        except subprocess.TimeoutExpired:process.kill();process.wait()
    result['processes']=[dict(pid=p.pid,exit_code=p.poll()) for p in processes]
result.update(seconds=round(time.monotonic()-started,2),changed_inputs=[name for name,value in pins.items() if digest(Path(name))!=value])
(output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({key:result.get(key) for key in ('status','error','observations','seconds')}))
raise SystemExit(0 if result['status']=='passed' else 1)
