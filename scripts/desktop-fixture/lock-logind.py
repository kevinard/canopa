#!/usr/bin/env python3
"""Private logind protocol fixture, served only on the desktop test's isolated bus."""
import json
import os
from pathlib import Path
import sys
import time
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib

root=Path(sys.argv[1]);state=root/'logind-state.json';control=root/'logind-control.json'
connection=Gio.bus_get_sync(Gio.BusType.SESSION,None)
manager='/org/freedesktop/login1';session=manager+'/session/fixture';other=manager+'/session/other'
events=[];locked=False

def save():
    pending=state.with_suffix('.new');pending.write_text(json.dumps({'locked':locked,'events':events}));pending.replace(state)

def settings():
    try:return json.loads(control.read_text())
    except (OSError,ValueError):return {}

def variant(signature,*values):return GLib.Variant(signature,values)

def method(conn,sender,path,interface,name,parameters,invocation):
    global locked
    args=parameters.unpack()
    if interface=='org.freedesktop.login1.Manager':
        if name=='GetSession':
            if args[0] not in ('canopa-fixture','other-user'):
                invocation.return_dbus_error('org.freedesktop.login1.NoSuchSession','no fixture session');return
            invocation.return_value(variant('(o)',session if args[0]=='canopa-fixture' else other));return
        if name=='GetSessionByPID':
            if settings().get('no-pid-session'):
                invocation.return_dbus_error('org.freedesktop.login1.NoSessionForPID','fixture user service');return
            invocation.return_value(variant('(o)',session));return
        if name=='ListSessions':
            sessions=[('canopa-fixture',os.getuid(),'fixture','seat0',session),('other-user',os.getuid()+1,'other','seat1',other)]
            if settings().get('ambiguous'):sessions.append(('second',os.getuid(),'fixture','seat1',manager+'/session/second'))
            invocation.return_value(variant('(a(susso))',sessions));return
        if name in ('LockSession','LockSessions','UnlockSession'):
            target=other if args and args[0]=='other-user' else session
            connection.emit_signal(None,target,'org.freedesktop.login1.Session','Unlock' if name=='UnlockSession' else 'Lock',None)
            events.append({'request':name,'target':target,'time':time.monotonic()});save()
            invocation.return_value(None);return
    if interface=='org.freedesktop.login1.Session':
        if name=='SetLockedHint':
            assert path==session, 'hint was sent to another user session'
            locked=args[0];events.append({'locked':locked,'time':time.monotonic()});save()
            invocation.return_value(None);return
        if name in ('Lock','Unlock'):
            connection.emit_signal(None,path,interface,name,None);invocation.return_value(None);return
    invocation.return_dbus_error('org.freedesktop.DBus.Error.UnknownMethod','unknown fixture method')

def get_property(conn,sender,path,interface,name):
    if name=='User':return variant('(uo)',os.getuid()+(path==other),'/org/freedesktop/login1/user/fixture')
    if name=='Id':return GLib.Variant('s','other-user' if path==other else 'canopa-fixture')
    if name=='Type':return GLib.Variant('s','wayland')
    if name=='Class':return GLib.Variant('s','user')
    if name=='Active':return GLib.Variant('b',True)
    if name=='LockedHint':return GLib.Variant('b',locked)
    return None

xml='''<node><interface name="org.freedesktop.login1.Manager">
<method name="GetSession"><arg type="s" direction="in"/><arg type="o" direction="out"/></method>
<method name="GetSessionByPID"><arg type="u" direction="in"/><arg type="o" direction="out"/></method>
<method name="ListSessions"><arg type="a(susso)" direction="out"/></method>
<method name="LockSession"><arg type="s" direction="in"/></method><method name="LockSessions"/>
<method name="UnlockSession"><arg type="s" direction="in"/></method>
</interface><interface name="org.freedesktop.login1.Session">
<method name="SetLockedHint"><arg type="b" direction="in"/></method><method name="Lock"/><method name="Unlock"/>
<signal name="Lock"/><signal name="Unlock"/>
<property name="Id" type="s" access="read"/><property name="User" type="(uo)" access="read"/>
<property name="Type" type="s" access="read"/><property name="Class" type="s" access="read"/>
<property name="Active" type="b" access="read"/><property name="LockedHint" type="b" access="read"/>
</interface></node>'''
node=Gio.DBusNodeInfo.new_for_xml(xml)
connection.register_object(manager,node.interfaces[0],method,None,None)
for path in (session,other,manager+'/session/second'):
    connection.register_object(path,node.interfaces[1],method,get_property,None)
connection.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','RequestName',
                     variant('(su)','org.freedesktop.login1',0),GLib.VariantType.new('(u)'),Gio.DBusCallFlags.NONE,-1,None)
save();(root/'logind-ready').touch();GLib.MainLoop().run()
