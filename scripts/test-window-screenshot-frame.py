#!/usr/bin/env python3
"""Exercise the exact screenshot frame reader on a real private window producer."""
import argparse,json,os,subprocess,sys,tempfile,time,traceback
from pathlib import Path
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'scripts'))
from screencast_media import MediaSession,digest
from window_screenshot_fixture import TOOLS,receiver


def verify(s,result,binary):
 from gi.repository import Gio,GLib
 from PIL import Image,ImageChops
 s.start_niri('debug { dbus-interfaces-in-non-session-instances; }\nxwayland-satellite { off; }\nanimations { off; }\nhotkey-overlay { skip-at-startup; }\nlayout { border { off; }; focus-ring { off; }; }\n')
 s.start([str(receiver())],'source.log')
 windows=lambda:json.loads(s.run(['niri','msg','--json','windows']))
 w=s.wait_for(lambda:next((v for v in windows() if v['app_id']=='org.canopa.TouchReceiver'),None),'source window')
 def action(*args):return s.run(['niri','msg','action',*map(str,args)])
 action('move-window-to-floating','--id',w['id']);action('set-window-width','--id',w['id'],400);action('set-window-height','--id',w['id'],300)
 s.wait_for(lambda:'configured 400 300' in (s.output/'source.log').read_text(),'window size')
 action('move-window-to-workspace','--window-id',w['id'],'--focus','false','2')
 c=Gio.DBusConnection.new_for_address_sync(s.env['DBUS_SESSION_BUS_ADDRESS'],Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,None,None)
 def call(p,i,m,signature='()',values=()):return c.call_sync('org.gnome.Mutter.ScreenCast',p,i,m,GLib.Variant(signature,values),None,Gio.DBusCallFlags.NONE,10000,None).unpack()
 session=call('/org/gnome/Mutter/ScreenCast','org.gnome.Mutter.ScreenCast','CreateSession','(a{sv})',({},))[0]
 stream=call(session,'org.gnome.Mutter.ScreenCast.Session','RecordWindow','(a{sv})',({'window-id':GLib.Variant('t',w['id']),'canopa-source-id':GLib.Variant('t',w['id']),'cursor-mode':GLib.Variant('u',0)},))[0]
 nodes=[];c.signal_subscribe(None,'org.gnome.Mutter.ScreenCast.Stream','PipeWireStreamAdded',stream,None,Gio.DBusSignalFlags.NONE,lambda c,s,p,i,n,v:nodes.append(v.unpack()[0]))
 call(session,'org.gnome.Mutter.ScreenCast.Session','Start')
 def ready():
  while GLib.MainContext.default().pending():GLib.MainContext.default().iteration(False)
  return nodes
 s.wait_for(ready,'window producer')
 node=next(v for v in s.registry() if v['id']==nodes[0]);serial=int(node['info']['props']['object.serial'])
 pid=next(p.pid for p in s.processes if str(s.compositor) in p.args)
 log=s.output/'reader.log'
 with log.open('w') as output:
  reader=subprocess.Popen([str(binary),'--exact','capture::window::frame::tests::native_window_frame_reader','--ignored','--nocapture','--test-threads=1'],stdin=subprocess.PIPE,stdout=output,stderr=output,cwd=ROOT/'crates/canopa-portal',env={**s.env,'CANOPA_WINDOW_FRAME_FIXTURE':'1'},start_new_session=True)
 s.processes.append(reader)
 def read(pid,name):
  before=sum('FRAME ' in v for v in log.read_text().splitlines());path=s.output/name
  reader.stdin.write((json.dumps(dict(serial=serial,pid=pid,output=str(path)))+'\n').encode());reader.stdin.flush()
  def response():
   values=[json.loads(v.split('FRAME ',1)[1]) for v in log.read_text().splitlines() if 'FRAME ' in v]
   return values[before] if len(values)>before else None
  return s.wait_for(response,'native frame reply',timeout=20),path
 wrong,path=read(pid+1,'wrong.png');assert 'error' in wrong and not path.exists(),wrong
 result['checks'].append('foreign producer PID rejected before reading')
 correct,path=read(pid,'frame.png');assert correct==dict(width=400,height=300),correct
 image=Image.open(path).convert('RGB');inner=image.crop((4,4,396,296));diff=ImageChops.difference(inner,Image.new('RGB',inner.size,(32,144,192)))
 assert all(high<=2 for low,high in diff.getextrema()),diff.getextrema()
 result['checks'].append('actual off-workspace window pixels copied through exact Rust reader')
 reader.stdin.close();assert reader.wait(timeout=10)==0;s.processes.remove(reader)
 result['reader']=dict(pid=reader.pid,exit_code=reader.returncode,binary=str(binary),sha256=digest(binary),source_sha256=digest(ROOT/'crates/canopa-portal/src/capture/window/frame.rs'))
 call(session,'org.gnome.Mutter.ScreenCast.Session','Stop');c.close_sync(None)


def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',required=True,type=Path);p.add_argument('--reader',type=Path,default=TOOLS/'frame-tests');args=p.parse_args();out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
 result=dict(status='running',checks=[]);code=1;started=time.monotonic()
 with tempfile.TemporaryDirectory(prefix='canopa-window-frame-') as folder:
  s=MediaSession(Path(folder),out,ROOT/'target/niri-screencast-native/debug/niri','cd042195bd1fa82a833f06b34e6f825f3c8eb71a5283261fece68dadb86f9b6a')
  try:verify(s,result,args.reader.resolve(strict=True));result['status']='passed';code=0
  except Exception as error:result.update(status='failed',error=repr(error),traceback=traceback.format_exc())
  finally:
   processes=list(s.processes);s.close();result['processes']=[dict(pid=v.pid,command=v.args,exit_code=v.poll()) for v in processes]
 result.update(seconds=round(time.monotonic()-started,2),exit_code=code);(out/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));return code


if __name__=='__main__':raise SystemExit(main())
