#!/usr/bin/env python3
"""Independent libatspi client, only used against fixture-owned accessibility buses."""
import argparse
import json
import time
import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi, GLib

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--window',required=True)
parser.add_argument('--name',required=True)
parser.add_argument('--value')
parser.add_argument('--select',nargs=2,type=int)
parser.add_argument('--click',action='store_true')
args=parser.parse_args()

def find(node,name,depth=0):
    if node is None or depth>15:return None
    try:
        if node.get_name()==name:return node
        for i in range(min(node.get_child_count(),256)):
            result=find(node.get_child_at_index(i),name,depth+1)
            if result:return result
    except GLib.Error:pass
    return None

end=time.monotonic()+6
node=None
while time.monotonic()<end:
    window=find(Atspi.get_desktop(0),args.window)
    if window:node=find(window,args.name)
    if node:break
    while GLib.MainContext.default().iteration(False):pass
    time.sleep(.05)
if not node:raise RuntimeError('requested fixture accessibility node missing')
if args.value is not None:
    assert Atspi.EditableText.set_text_contents(node, args.value)
elif args.select:
    assert Atspi.Text.set_selection(node, 0, *args.select)
elif args.click:
    assert Atspi.Action.do_action(node, 0)
else:
    text=node.get_text_iface()
    value=Atspi.Text.get_text(node,0,-1) if text else ''
    print(json.dumps({'name':node.get_name(),'role':node.get_role_name(),'text':value}))
