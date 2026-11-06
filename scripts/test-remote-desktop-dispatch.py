#!/usr/bin/env python3
"""Check that advertised Touch methods reach the real ordered backend dispatcher."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    from gi.repository import Gio,GLib
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();binary=args.binary.resolve(strict=True);args.output.parent.mkdir(parents=True,exist_ok=True)
    rows=[]
    with tempfile.TemporaryDirectory(prefix='canopa-touch-dispatch-') as folder:
        root=Path(folder);config=root/'canopa.kdl';config.write_text('schema-version 1\n')
        bus=subprocess.Popen(['dbus-daemon','--session','--nofork','--print-address=1'],stdout=subprocess.PIPE,text=True)
        backend=None;connection=None
        try:
            address=bus.stdout.readline().strip();assert address
            connection=Gio.DBusConnection.new_for_address_sync(address,Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
            env=dict(os.environ)
            for name in ['NIRI_SOCKET','WAYLAND_DISPLAY','DISPLAY']:env.pop(name,None)
            env.update(DBUS_SESSION_BUS_ADDRESS=address,DBUS_SYSTEM_BUS_ADDRESS='unix:path='+str(root/'none'),
                XDG_CONFIG_HOME=str(root/'config'),XDG_STATE_HOME=str(root/'state'),XDG_DATA_HOME=str(root/'data'),XDG_CACHE_HOME=str(root/'cache'))
            with args.output.with_suffix('.log').open('w') as stream:
                backend=subprocess.Popen([str(binary),'--canopa-config',str(config)],env=env,stdout=stream,stderr=stream)
            service='org.freedesktop.impl.portal.desktop.canopa';path='/org/freedesktop/portal/desktop'
            deadline=time.monotonic()+10
            while True:
                assert backend.poll() is None,'Backend exited before dispatch check'
                ready=connection.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','NameHasOwner',
                    GLib.Variant('(s)',(service,)),None,Gio.DBusCallFlags.NONE,1000,None).unpack()[0]
                if ready:break
                assert time.monotonic()<deadline,'Backend startup timeout'
                time.sleep(.05)
            xml=connection.call_sync(service,path,'org.freedesktop.DBus.Introspectable','Introspect',None,None,Gio.DBusCallFlags.NONE,3000,None).unpack()[0]
            key='/org/freedesktop/portal/desktop/session/probe/touch'
            for method,signature,values in [
                ('NotifyTouchDown','(oa{sv}uudd)',(key,{},41,7,1.,2.)),
                ('NotifyTouchMotion','(oa{sv}uudd)',(key,{},41,7,2.,3.)),
                ('NotifyTouchUp','(oa{sv}u)',(key,{},7)),
            ]:
                assert f'<method name="{method}">' in xml,method
                try:connection.call_sync(service,path,'org.freedesktop.impl.portal.RemoteDesktop',method,
                    GLib.Variant(signature,values),None,Gio.DBusCallFlags.NONE,3000,None)
                except GLib.Error as error:rows.append(dict(method=method,error=Gio.DBusError.get_remote_error(error)))
                else:rows.append(dict(method=method,error=None))
        finally:
            if connection:connection.close_sync(None)
            if backend:
                backend.terminate();backend.wait(timeout=10)
            bus.terminate();bus.wait(timeout=10)
    passed=all(r['error']=='org.freedesktop.DBus.Error.InvalidArgs' for r in rows) and len(rows)==3
    with binary.open('rb') as stream:checksum=hashlib.file_digest(stream,'sha256').hexdigest()
    result=dict(status='passed' if passed else 'failed',binary_sha256=checksum,checks=rows,
        scope='Real advertised methods must reach session validation; UnknownMethod reveals missing ordered dispatch. No consent/input success claim.')
    args.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));return 0 if passed else 1


if __name__=='__main__':raise SystemExit(main())
