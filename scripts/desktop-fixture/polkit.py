#!/usr/bin/env python3
"""Test-owned polkit authority on a private bus; never grants host privileges."""
import json, os, sys
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio,GLib
root=Path(sys.argv[1]); bus=Gio.bus_get_sync(Gio.BusType.SESSION,None)
path='/org/freedesktop/PolicyKit1/Authority'; events=[]; agent=None; pending={}; sequence=0; nonce=None

def record(event,**fields):
    events.append(dict(event=event,**fields)); tmp=root/'polkit-events.new';tmp.write_text(json.dumps(events));tmp.replace(root/'polkit-events.json')
def settings():
    try:return json.loads((root/'polkit-control.json').read_text())
    except (OSError,ValueError):return {}
def variant(signature,*values):return GLib.Variant(signature,values)
def identities(uids):return [('unix-user',{'uid':GLib.Variant('u',uid)}) for uid in uids]
def start_request(config):
    global sequence
    if not agent:return
    sequence+=1; key=sequence;cookie=f'canopa-private-cookie-{key}';pending[cookie]=dict(key=key,receipt=False)
    (root/'polkit-mode').write_text(config.get('mode','password'))
    record('begin',id=key,uids=config.get('uids',[os.getuid()]))
    def completed(connection,result):
        try:
            connection.call_finish(result);error='';success=pending[cookie]['receipt']
        except GLib.Error as e:
            error=Gio.DBusError.get_remote_error(e) or 'error';success=False
        record('done',id=key,authorized=success,error=error);pending[cookie]['done']=True
    bus.call(agent[0],agent[1],'org.freedesktop.PolicyKit1.AuthenticationAgent','BeginAuthentication',
             variant('(sssa{ss}sa(sa{sv}))',config.get('action','org.canopa.test.authorize'),config.get('message','Authentication is required to change a test setting.'),'',{},cookie,identities(config.get('uids',[os.getuid()]))),
             GLib.VariantType.new('()'),Gio.DBusCallFlags.NONE,20000,None,completed)
def foreign():
    if not agent:return
    connection=Gio.DBusConnection.new_for_address_sync(os.environ['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
    try:
        connection.call_sync(agent[0],agent[1],'org.freedesktop.PolicyKit1.AuthenticationAgent','BeginAuthentication',
          variant('(sssa{ss}sa(sa{sv}))','org.canopa.test.spoof','Spoof request','',{},'spoof-cookie',identities([os.getuid()])),GLib.VariantType.new('()'),Gio.DBusCallFlags.NONE,1000,None)
        record('foreign',denied=False)
    except GLib.Error as e:record('foreign',denied=Gio.DBusError.get_remote_error(e)=='org.freedesktop.DBus.Error.AccessDenied')
    try:
        # D-Bus permits calls without an interface header; the owner filter must still apply.
        message=Gio.DBusMessage.new_method_call(agent[0],agent[1],None,'BeginAuthentication')
        message.set_body(variant('(sssa{ss}sa(sa{sv}))','org.canopa.test.spoof','Spoof request','',{},'spoof-cookie',identities([os.getuid()])))
        result=connection.send_message_with_reply_sync(message,Gio.DBusSendMessageFlags.NONE,1000,None)
        reply=result[0] if isinstance(result,tuple) else result
        record('foreign-no-interface',denied=reply.get_error_name()=='org.freedesktop.DBus.Error.AccessDenied')
    finally:connection.close_sync(None)
def method(connection,sender,object_path,interface,name,parameters,invocation):
    global agent
    args=parameters.unpack()
    if name in ('RegisterAuthenticationAgent','RegisterAuthenticationAgentWithOptions'):
        if agent:
            invocation.return_dbus_error('org.freedesktop.PolicyKit1.Error.Failed','Agent already registered');return
        subject,locale,object_path=args[:3]
        assert subject[0]=='unix-session' and subject[1]['session-id']=='canopa-fixture'
        agent=(sender,object_path);record('registered',subject='unix-session',locale=locale)
        invocation.return_value(None)
        if settings().get('auto'):
            GLib.timeout_add(50,lambda:(foreign() if settings().get('foreign') else None,start_request(settings()),False)[-1])
        return
    if name=='UnregisterAuthenticationAgent':
        assert agent and sender==agent[0];agent=None;record('unregistered');invocation.return_value(None);return
    if name in ('AuthenticationAgentResponse','AuthenticationAgentResponse2'):
        caller_uid,cookie,identity=args if name.endswith('2') else (os.getuid(),*args)
        if cookie not in pending or pending[cookie].get('done'):
            invocation.return_dbus_error('org.freedesktop.PolicyKit1.Error.Failed','No pending cookie');return
        pending[cookie]['receipt']=True
        record('receipt',id=pending[cookie]['key'],uid=identity[1]['uid'],agent_uid=caller_uid)
        invocation.return_value(None);return
    invocation.return_dbus_error('org.freedesktop.DBus.Error.UnknownMethod','unsupported fixture method')
def property_(connection,sender,object_path,interface,name):
    values={'DaemonVersion':GLib.Variant('s','128'),'BackendName':GLib.Variant('s','local'),'BackendVersion':GLib.Variant('s','128'),'BackendFeatures':GLib.Variant('u',1)}
    return values.get(name)
xml='''<node><interface name="org.freedesktop.PolicyKit1.Authority">
<method name="RegisterAuthenticationAgent"><arg type="(sa{sv})" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/></method>
<method name="RegisterAuthenticationAgentWithOptions"><arg type="(sa{sv})" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/></method>
<method name="UnregisterAuthenticationAgent"><arg type="(sa{sv})" direction="in"/><arg type="s" direction="in"/></method>
<method name="AuthenticationAgentResponse2"><arg type="u" direction="in"/><arg type="s" direction="in"/><arg type="(sa{sv})" direction="in"/></method>
<method name="AuthenticationAgentResponse"><arg type="s" direction="in"/><arg type="(sa{sv})" direction="in"/></method>
<property name="DaemonVersion" type="s" access="read"/><property name="BackendName" type="s" access="read"/><property name="BackendVersion" type="s" access="read"/><property name="BackendFeatures" type="u" access="read"/>
<signal name="Changed"/>
</interface></node>'''
bus.register_object(path,Gio.DBusNodeInfo.new_for_xml(xml).interfaces[0],method,property_,None)
bus.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','RequestName',variant('(su)','org.freedesktop.PolicyKit1',0),GLib.VariantType.new('(u)'),Gio.DBusCallFlags.NONE,-1,None)
def owner_changed(connection,sender,object_path,interface,signal,parameters):
    global agent
    name,old,new=parameters.unpack()
    if agent and name==agent[0] and not new:
        agent=None;record('agent-disconnected')
bus.signal_subscribe('org.freedesktop.DBus','org.freedesktop.DBus','NameOwnerChanged','/org/freedesktop/DBus',None,Gio.DBusSignalFlags.NONE,owner_changed)
record('ready');(root/'polkit-ready').touch()
def poll():
    global nonce
    config=settings()
    if agent and not config.get('auto') and config.get('nonce')!=nonce:
        nonce=config.get('nonce')
        if nonce is not None:start_request(config)
    cancel=root/'polkit-cancel'
    if agent and cancel.exists():
        cancel.unlink()
        for cookie,value in list(pending.items()):
            if not value.get('done'):
                bus.call(agent[0],agent[1],'org.freedesktop.PolicyKit1.AuthenticationAgent','CancelAuthentication',variant('(s)',cookie),None,Gio.DBusCallFlags.NONE,2000,None,None)
    overlap=root/'polkit-overlap'
    if agent and overlap.exists():overlap.unlink();start_request({'mode':'password','uids':[os.getuid()]})
    return True
GLib.timeout_add(20,poll);GLib.MainLoop().run()
