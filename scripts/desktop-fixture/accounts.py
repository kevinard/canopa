#!/usr/bin/env python3
"""AccountsService read-only fixture on a private session bus, never the host bus."""
import json
from pathlib import Path
import sys
import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib

root = Path(sys.argv[1])
events = []
connection = Gio.bus_get_sync(Gio.BusType.SESSION, None)
manager = '/org/freedesktop/Accounts'
data = {
    '1000': dict(Uid=1000, UserName='alice', RealName='Alice Example', SystemAccount=False),
    '1001': dict(Uid=1001, UserName='bob', RealName='Bob Builder', SystemAccount=False),
    '1002': dict(Uid=1002, UserName='remote', RealName='Remote User', SystemAccount=False),
    '1003': dict(Uid=1003, UserName='locked', RealName='Locked Account', SystemAccount=False),
    '1004': dict(Uid=1004, UserName='bad user', RealName='Invalid', SystemAccount=False),
    '1005': dict(Uid=1005, UserName='fallback', RealName='Bad\nName', SystemAccount=False),
    '1006': dict(Uid=1006, UserName='missing', SystemAccount=False),
    '1007': dict(Uid=1007, UserName='carol', RealName='Carol Example', SystemAccount=False),
    '0': dict(Uid=0, UserName='root', RealName='Root', SystemAccount=False),
    '42': dict(Uid=42, UserName='daemon', RealName='Daemon', SystemAccount=True),
}


def record(event, **fields):
    events.append(dict(event=event, **fields))
    pending = root / 'accounts-events.new'
    pending.write_text(json.dumps(events))
    pending.replace(root / 'accounts-events.json')


def settings():
    try:
        return json.loads((root / 'accounts-control.json').read_text())
    except (OSError, ValueError):
        return {}


def method(conn, sender, path, interface, name, parameters, invocation):
    record('method', name=name)
    assert name in ('ListCachedUsers', 'FindUserById'), 'account discovery attempted a mutation'
    if name == 'FindUserById':
        uid = str(parameters.unpack()[0])
        record('find-user', uid=int(uid))
        if uid in data:
            invocation.return_value(GLib.Variant('(o)', (manager + '/User' + uid,)))
        else:
            invocation.return_dbus_error('org.freedesktop.Accounts.Error.Failed', 'no such user')
        return
    config = settings()
    if config.get('mode') == 'denied':
        invocation.return_dbus_error('org.freedesktop.DBus.Error.AccessDenied', 'fixture denial')
        return
    if config.get('mode') == 'empty':
        paths = []
    elif config.get('mode') == 'oversized':
        paths = [manager + '/User' + str(i) for i in range(129)]
    else:
        paths = [manager + '/User' + uid for uid in data if uid != '1007' or config.get('extra_user')] + [manager + '/User1000']
    def reply():
        invocation.return_value(GLib.Variant('(ao)', (paths,)))
        return False
    delay = config.get('delay_ms', 0)
    if delay:
        GLib.timeout_add(delay, reply)
    else:
        reply()


def get_property(conn, sender, path, interface, name):
    record('property', path=path, name=name)
    assert name in ('Uid', 'UserName', 'RealName', 'SystemAccount', 'IconFile'), 'unexpected account metadata read'
    uid = path.rsplit('User', 1)[1]
    if name == 'IconFile':
        mode = settings().get('icon_mode')
        if mode == 'missing':
            return None
        if mode == 'invalid':
            return GLib.Variant('s', 'https://invalid.example/avatar.png')
        return GLib.Variant('s', str(root / ('avatar-' + data[uid]['UserName'])) if uid in ('1000', '1001') else '')
    if name not in data[uid]:
        return None
    value = settings().get('real_names', {}).get(data[uid]['UserName'], data[uid][name]) if name == 'RealName' else data[uid][name]
    signature = 't' if name == 'Uid' else 'b' if name == 'SystemAccount' else 's'
    return GLib.Variant(signature, value)


xml = '''<node><interface name="org.freedesktop.Accounts">
<method name="FindUserById"><arg type="x" direction="in"/><arg type="o" direction="out"/></method>
<method name="ListCachedUsers"><arg type="ao" direction="out"/></method>
<signal name="UserAdded"><arg type="o"/></signal>
</interface><interface name="org.freedesktop.Accounts.User">
<property name="Uid" type="t" access="read"/><property name="UserName" type="s" access="read"/>
<property name="RealName" type="s" access="read"/><property name="SystemAccount" type="b" access="read"/>
<property name="IconFile" type="s" access="read"/>
<property name="HomeDirectory" type="s" access="read"/><property name="PasswordHint" type="s" access="read"/>
<property name="LocalAccount" type="b" access="read"/><property name="Locked" type="b" access="read"/>
</interface></node>'''
node = Gio.DBusNodeInfo.new_for_xml(xml)
connection.register_object(manager, node.interfaces[0], method, None, None)
for uid in data:
    connection.register_object(manager + '/User' + uid, node.interfaces[1], None, get_property, None)
connection.call_sync('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
                     'RequestName', GLib.Variant('(su)', ('org.freedesktop.Accounts', 0)),
                     GLib.VariantType.new('(u)'), Gio.DBusCallFlags.NONE, -1, None)
record('ready')
(root / 'accounts-ready').touch()
last_revision = None
def changes():
    global last_revision
    config = settings()
    revision = config.get('revision', 0)
    if revision != last_revision:
        last_revision = revision
        connection.emit_signal(None, manager, 'org.freedesktop.Accounts', 'UserAdded',
                               GLib.Variant('(o)', (manager+'/User1007',)))
        connection.emit_signal(None, manager+'/User1000', 'org.freedesktop.DBus.Properties', 'PropertiesChanged',
                               GLib.Variant('(sa{sv}as)', ('org.freedesktop.Accounts.User', {}, ['RealName', 'IconFile'])))
    return True
GLib.timeout_add(50, changes)
GLib.MainLoop().run()
