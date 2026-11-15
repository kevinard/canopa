#!/usr/bin/env python3
"""Private Niri/Cage/Weston greeter acceptance against a test-owned greetd peer."""
import argparse,importlib.util,json,os,subprocess,sys,tempfile,time
from pathlib import Path
from PIL import Image
import native_accessibility as a11y
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('desktop',ROOT/'scripts/validate-desktop.py');desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)
def verify(binary,session,compositor,compositor_binary):
    session.start_niri('prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\nhotkey-overlay { skip-at-startup; }\nwindow-rule { open-fullscreen true; }\n')
    a11y.start(session)
    root=session.runtime;config=root/'greeter.kdl';marker=root/'widget-runs'
    faces=root/'faces';faces.mkdir()
    for name,color in [('alice',(210,60,70)),('bob',(40,120,220))]:
        Image.new('RGBA',(96,96),color+(255,)).save(root/('avatar-'+name),format='PNG')
    Image.new('RGBA',(96,96),(220,170,40,255)).save(faces/'bob',format='PNG')
    Image.new('RGBA',(96,96),(40,180,90,255)).save(root/'override.png')
    session.start(['/usr/bin/python3',str(ROOT/'scripts/desktop-fixture/accounts.py'),str(root)],'accounts.log')
    session.wait_for(lambda:(root/'accounts-ready').exists(),'private AccountsService')
    session.env['DBUS_SYSTEM_BUS_ADDRESS']=session.env['DBUS_SESSION_BUS_ADDRESS']
    script=root/'status.py';script.write_text('from pathlib import Path\nimport os\np=Path(__file__).with_name("widget-runs")\nassert "GREETD_SOCK" not in os.environ\nwith p.open("a") as f:f.write(str(os.getpid())+"\\n")\nprint("Login widget ready")\n')
    sessions=root/'sessions';sessions.mkdir()
    (sessions/'canopa.desktop').write_text('[Desktop Entry]\nType=Application\nName=Canopa\nExec=niri-session\nDesktopNames=canopa;niri;\n')
    (sessions/'other.desktop').write_text('[Desktop Entry]\nType=Application\nName=Other desktop\nExec=/usr/bin/true "arg with spaces"\nDesktopNames=Other;\n')
    config.write_text(f'''schema-version 1
greeter {{
 remember-selection #false
 user-source "accountsservice"
 avatar-dir "{faces}"
 users "alice" "bob"
 session-dirs "{sessions}"
 view {{
  auth-timeout 8
  widgets "avatar" "status"
  layout {{ top-left "clock" "date"; center "avatar" "authentication"; bottom-right "status"; }}
  widget "status" {{ command {{ argv "python3" "{script}"; }}; interval 1; preview "Login widget preview"; }}
 }}
}}
''')
    catalog=json.loads(session.run([str(binary),'--canopa-config',str(config),'list-sessions']));assert [s['id'] for s in catalog]==['canopa','other'];assert all(s['desktop_file'].startswith(str(sessions)) for s in catalog)
    users=json.loads(session.run([str(binary),'--canopa-config',str(config),'list-users']));assert users[0]['username']=='alice' and users[0]['display_name']=='Alice Example';assert not any(u['username'] in ['root','daemon','bad user'] for u in users)
    preview=session.output/'preview.png';session.run([str(binary),'--canopa-config',str(config),'preview',str(preview)])
    assert not marker.exists();assert Image.open(preview).size==(1280,800)
    session.start([sys.executable,str(ROOT/'scripts/desktop-fixture/greetd.py'),str(root)],'greetd.log')
    session.wait_for(lambda:(root/'greetd.sock').exists(),'private greetd socket');session.env['GREETD_SOCK']=str(root/'greetd.sock')
    protocol=ROOT/'scripts/desktop-fixture/virtual-keyboard-unstable-v1.xml'
    for mode,name in [('client-header','virtual-keyboard.h'),('private-code','virtual-keyboard.c')]:
        subprocess.run(['wayland-scanner',mode,str(protocol),str(session.output/name)],check=True)
    keyboard=session.output/'keyboard'
    subprocess.run(['cc','-Wall','-Wextra','-Wno-misleading-indentation','-I',str(session.output),str(ROOT/'scripts/desktop-fixture/session-keyboard.c'),str(session.output/'virtual-keyboard.c'),'-lwayland-client','-lxkbcommon','-o',str(keyboard)],check=True)
    nss_library=session.output/'nss-fixture.so'
    subprocess.run(['cc','-shared','-fPIC','-Wall','-Wextra','-Werror',str(ROOT/'scripts/desktop-fixture/nss.c'),'-ldl','-o',str(nss_library)],check=True)
    session.start([str(keyboard),'hold'],'keyboard.log')
    click=session.build_pointer('click')
    def events():
        try:return json.loads((root/'greetd-events.json').read_text())
        except FileNotFoundError:return []
    def wait(predicate,label,timeout=15):
        end=time.monotonic()+timeout
        while time.monotonic()<end:
            value=predicate()
            if value:return value
            time.sleep(.04)
        raise AssertionError('Timed out: '+label)
    def event(kind,after=0,**fields):return next((e for e in events()[after:] if e['event']==kind and all(e.get(k)==v for k,v in fields.items())),None)
    def avatar(name,color,visible=True):
        def check():
            image=session.capture(name)
            pixels=image.get_flattened_data() if hasattr(image,'get_flattened_data') else image.getdata()
            count=sum(1 for p in pixels if tuple(p[:3])==color)
            return count>2000 if visible else count==0
        wait(check,'selected avatar '+name,timeout=5)
    def mode(value,**options):
        (root/'device-touch').unlink(missing_ok=True);(root/'greetd-control.json').write_text(json.dumps(dict(mode=value,**options)))
    def key(value):session.run([str(keyboard),'key',value])
    def answer(value):session.run([str(keyboard),'text',value])
    def prompt(text,after):
        wait(lambda:event('prompt',after,text=text),text)
        a11y.request(session,'--window','Canopa Login','--name',text)
    def start(name,backend='otp',accounts_mode='ok',accounts_delay=0,nss_mode=None,**options):
        (root/'accounts-control.json').write_text(json.dumps({'mode':accounts_mode,'delay_ms':accounts_delay}))
        mode(backend,**options);before=len(events())
        command=[str(binary),'--canopa-config',str(config)]
        if compositor!='direct':
            command+=['session','--compositor',compositor,'--renderer','software']
            if compositor_binary:command+=['--compositor-bin',str(compositor_binary)]
            if compositor=='weston':command+=['--compositor-arg='+v for v in ['--backend=wayland','--fullscreen','--width=1280','--height=800']]
        if os.environ.get('CANOPA_GREETER_TRACE'):session.env['WAYLAND_DEBUG']='client'
        saved={k:session.env.get(k) for k in ['LD_PRELOAD','CANOPA_NSS_FIXTURE','CANOPA_NSS_EVENTS']}
        if nss_mode:
            session.env.update(LD_PRELOAD=str(nss_library),CANOPA_NSS_FIXTURE=nss_mode,CANOPA_NSS_EVENTS=str(root/'nss-events'))
        process=session.start(command,name+'.log')
        for k,v in saved.items():
            if v is None:session.env.pop(k,None)
            else:session.env[k]=v
        session.env.pop('WAYLAND_DEBUG',None)
        wait(lambda:event('cancel',before),'initial cancellation acknowledgement')
        time.sleep(1.5)
        windows=json.loads(session.run(['niri','msg','--json','windows']))
        window=max(windows,key=lambda w:w['id'])
        session.run(['niri','msg','action','focus-window','--id',str(window['id'])])
        time.sleep(.2)
        if name=='otp':
            session.capture('startup.png')
            (session.output/'windows.json').write_text(session.run(['niri','msg','--json','windows']))
        return process,before
    def done(process):assert process.wait(timeout=15)==0;session.processes.remove(process)
    def failed(process,name):
        code=process.wait(timeout=12)
        # Weston 15 exits zero when its child fails. Verify the frontend error
        # and absence of greetd handoff, rather than inferring login from that code.
        assert code!=0 or compositor=='weston'
        assert 'canopa-greeter: greeter ended without an authenticated session handoff' in (session.output/(name+'.log')).read_text()
        session.processes.remove(process)
    def no_handoff(after):assert not event('handoff',after),'unauthorized session handoff'

    app,before=start('otp')
    def selected_row():
        image=session.capture('startup.png')
        w,h=image.size
        # The selected account row carries the only accent-filled submit button.
        pixels=image.load()
        points=[(x,y) for y in range(0,h,2) for x in range(0,w,2) if pixels[x,y][:3]==(176,222,195)]
        if len(points)<100:return None
        ys=sorted(y for _,y in points)
        return w,h,ys[len(ys)//2]
    w,h,selected=wait(selected_row,'account list presented')
    scale=min(w/1280,h/800)
    # The desktop chooser by its accessible action, then the pointer on Bob's row.
    a11y.request(session,'--window','Canopa Login','--name','Desktop: Canopa','--click')
    time.sleep(.2)
    session.capture('desktop-chooser.png')
    key('Down');key('Return')
    # Back on the account list, whose manual-entry action only it exposes.
    def listed():
        try:return a11y.request(session,'--window','Canopa Login','--name','Not listed?')
        except RuntimeError:return None
    wait(listed,'account list after choosing a desktop')
    w,h,selected=wait(selected_row,'selected account row')
    # The click fixture spans 1600x1000 units: 800 is the horizontal centre.
    session.run([str(click),'800',str(round((selected+86*scale)/h*1000))])
    prompt('Password:',before)
    assert event('begin',before,user='bob')
    answer('wrong');time.sleep(2.3);no_handoff(before);assert app.poll() is None;assert event('failed-worker-cleanup',before)
    key('Return');prompt('Password:',before+len(events()[before:])-1)
    a11y.request(session,'--window','Canopa Login','--name','Password:','--value','canopa-fixture-password')
    time.sleep(.2)
    protected=json.loads(a11y.request(session,'--window','Canopa Login','--name','Password:'))
    assert protected['role']=='password text' and 'canopa-fixture-password' not in protected['text'],protected
    a11y.request(session,'--window','Canopa Login','--name','Submit response','--click')
    prompt('One-time code:',before)
    answer('123456');done(app)
    handoff=event('handoff',before);assert handoff['user']=='bob' and handoff['cmd']==['/usr/bin/true','arg with spaces'] and handoff['env']==['XDG_SESSION_TYPE=wayland','XDG_SESSION_DESKTOP=other','XDG_CURRENT_DESKTOP=Other']
    assert marker.exists();runs=len(marker.read_text().splitlines());time.sleep(1.2);assert len(marker.read_text().splitlines())==runs

    app,before=start('token','token',**{'late-success':True})
    key('Down');key('Up');key('Tab');key('Tab');key('Return');prompt('Password:',before);assert event('begin',before,user='alice');answer('canopa-fixture-password');wait(lambda:event('device-wait',before),'pending device request');time.sleep(.4)
    session.capture('touch.png')
    cancel_at=len(events());key('Escape');wait(lambda:event('cancel',cancel_at),'cancel blocked hardware request');time.sleep(.7);no_handoff(before)
    (root/'device-touch').touch();time.sleep(.4);no_handoff(before)
    (root/'device-touch').unlink();retry_at=len(events());key('Return');prompt('Password:',retry_at)
    answer('canopa-fixture-password');wait(lambda:event('device-wait',retry_at),'fresh device request');(root/'device-touch').touch();done(app)

    configured=config.read_text();config.write_text(configured.replace('users "alice" "bob"','users;'))
    app,before=start('fingerprint','fingerprint',accounts_mode='denied');session.run([str(keyboard),'type','discard']);session.run([str(keyboard),'held','BackSpace']);answer('alice');wait(lambda:event('device-wait',before),'fingerprint without password')
    assert event('begin',before,user='alice') and not event('prompt',before,text='Password:');(root/'device-touch').touch();done(app);config.write_text(configured)

    app,before=start('timeout','token');key('Return');prompt('Password:',before);answer('canopa-fixture-password');wait(lambda:event('device-wait',before),'device wait for timeout')
    after=len(events());wait(lambda:event('cancel',after),'authentication timeout cancellation',12);time.sleep(.5);no_handoff(before)
    (root/'device-touch').touch();time.sleep(.3);no_handoff(before)
    retry_at=len(events());key('Return');prompt('Password:',retry_at);answer('canopa-fixture-password');wait(lambda:event('device-wait',retry_at),'retry after timeout');done(app);assert event('handoff',before)

    for backend in ['oversized','disconnect']:
        app,before=start(backend,backend);key('Return');failed(app,backend);no_handoff(before)

    app,before=start('launch-refused','password',**{'start-refused':True});key('Return');prompt('Password:',before);answer('canopa-fixture-password');wait(lambda:event('start-refused',before),'rejected session start');time.sleep(2.3);assert app.poll() is None;no_handoff(before)
    mode('password');retry_at=len(events());key('Return');prompt('Password:',retry_at);answer('canopa-fixture-password');done(app)
    app,before=start('cancel-blocked','token');key('Return');prompt('Password:',before);answer('canopa-fixture-password');wait(lambda:event('device-wait',before),'blocked backend request')
    (root/'greetd-control.json').write_text(json.dumps({'mode':'token','cancel-blocked':True}))
    drain_after=len(events());key('Escape');failed(app,'cancel-blocked');no_handoff(before)
    # The intentionally stalled peer still owns its old requests after the
    # frontend's bounded exit. Drain it before starting a different conversation;
    # its late cancellation must not become input to the next test case.
    wait(lambda:event('client-close',drain_after,active=0),'blocked peer connection cleanup')
    config.write_text(configured.replace('users "alice" "bob"','users;'))
    app,before=start('accounts-service','password');avatar('avatar-alice.png',(210,60,70));key('Down');avatar('accounts-chooser.png',(40,120,220));key('Return');prompt('Password:',before);assert event('begin',before,user='bob');avatar('avatar-password.png',(40,120,220));answer('canopa-fixture-password');done(app)
    config.write_text(configured.replace('user-source "accountsservice"',f'user-source "accountsservice"; user-avatar "bob" "{root / "override.png"}"'))
    app,before=start('avatar-override','password');key('Down');avatar('avatar-override.png',(40,180,90));key('Return');prompt('Password:',before);assert event('begin',before,user='bob');answer('canopa-fixture-password');done(app)
    config.write_text(configured.replace('user-source "accountsservice"','user-source "accountsservice"; user-avatars #false'))
    app,before=start('avatar-disabled','password');avatar('avatar-disabled.png',(210,60,70),False);key('Return');prompt('Password:',before);answer('canopa-fixture-password');done(app)
    config.write_text(configured.replace('users "alice" "bob"','users;'))
    app,before=start('accounts-late','password',accounts_delay=1800);key('Escape');answer('uncached');prompt('Password:',before);assert event('begin',before,user='uncached');answer('canopa-fixture-password');done(app)
    app,before=start('accounts-timeout','password',accounts_delay=3000);key('Escape');answer('uncached');prompt('Password:',before);assert event('begin',before,user='uncached');answer('canopa-fixture-password');done(app)
    account_events=json.loads((root/'accounts-events.json').read_text());assert all(e.get('name') in ['ListCachedUsers','Uid','UserName','RealName','SystemAccount','IconFile',None] for e in account_events)
    (session.output/'accounts-events.json').write_text(json.dumps(account_events,indent=2))
    nss_config=configured.replace('users "alice" "bob"','users;').replace('user-source "accountsservice"','user-source "nss"; nss-uid-min 1000; nss-uid-max 300000')
    config.write_text(nss_config)
    app,before=start('nss-users','password',nss_mode='ok');key('Down');avatar('nss-chooser.png',(220,170,40));key('Return');prompt('Password:',before);assert event('begin',before,user='bob');answer('canopa-fixture-password');done(app)
    config.write_text(nss_config.replace('user-source "nss"','user-source "auto"'))
    app,before=start('nss-fallback','password',accounts_mode='denied',nss_mode='ok');key('Return');prompt('Password:',before);assert event('begin',before,user='alice');answer('canopa-fixture-password');done(app)
    config.write_text(nss_config)
    app,before=start('nss-timeout','password',nss_mode='block');answer('uncached');prompt('Password:',before);assert event('begin',before,user='uncached');answer('canopa-fixture-password');done(app)
    nss_events=(root/'nss-events').read_text()
    for line in nss_events.splitlines():
        assert not Path('/proc/'+line.split()[0]).exists(),'NSS helper was not reaped'
    (session.output/'nss-events').write_text(nss_events)
    config.write_text(configured)
    # A remembered user opens straight onto their password and desktop.
    state=root/'state';(state/'canopa').mkdir(parents=True,mode=0o700);state.chmod(0o700)
    saved=state/'canopa'/'greeter.json';saved.write_text(json.dumps({'last_user':'bob','sessions':{'bob':'other'}}));saved.chmod(0o600)
    config.write_text(configured.replace('remember-selection #false','remember-selection #true'));session.env['XDG_STATE_HOME']=str(state)
    app,before=start('remembered','password');prompt('Password:',before);assert event('begin',before,user='bob')
    session.capture('remembered.png');answer('canopa-fixture-password');done(app)
    handoff=event('handoff',before);assert handoff['user']=='bob' and handoff['cmd']==['/usr/bin/true','arg with spaces']
    session.env.pop('XDG_STATE_HOME');config.write_text(configured)
    empty=root/'empty-sessions';empty.mkdir();config.write_text(configured.replace(str(sessions),str(empty)))
    app,before=start('no-sessions','password');key('Return');time.sleep(.5);no_handoff(before);assert not event('begin',before);assert app.poll() is None;session.retire(app)
    result=dict(status='passed',compositor=compositor,checks=['AccountsService avatars follow selection and password prompts','per-user avatar override takes priority','avatar directory works with NSS','disabled avatars leave login available','NSS username selection and labels','automatic NSS fallback after service denial','NSS timeout keeps manual authentication available and reaps helpers','AccountsService cached user filtering and labels','AccountsService-only username selection','late account metadata preserves manual username','AccountsService timeout keeps manual authentication available','installed desktop session discovery','empty catalog stays in greeter','command-free preview','fullscreen software rendering','configured user selection','pointer account selection and submit','keyboard user/session selection','manual username entry','held Backspace edits username','session selection and argv/environment boundaries','fresh password and visible OTP prompts','failed authentication retry','authenticated exit/handoff','custom widget environment and cleanup','hardware cue','cancel in-flight PAM and drain late success','fresh authentication after cancellation','fingerprint-style no-password authentication','bounded transaction timeout','retry after timeout','oversized protocol rejection','disconnect fails closed','failed session launch stays in greeter','remembered user opens on their password and desktop','unacknowledged cancellation fails closed'])
    (session.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n');(session.output/'greetd-events.json').write_text(json.dumps(events(),indent=2));print(json.dumps(result,indent=2))
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,default=ROOT/'target/debug/canopa-greeter');p.add_argument('--output',type=Path,default=ROOT/'target/native-runs/verify-greeter');p.add_argument('--cage',type=Path,help='Compatibility alias for --compositor cage --compositor-bin PATH');p.add_argument('--compositor',choices=['direct','cage','weston'],default='direct');p.add_argument('--compositor-bin',type=Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
    if a.cage:a.compositor='cage';a.compositor_bin=a.cage
    with tempfile.TemporaryDirectory(prefix='canopa-greeter-') as temp:
        runtime=Path(temp);runtime.chmod(0o700);session=desktop.Session(runtime,a.output.resolve())
        try:verify(a.binary.resolve(),session,a.compositor,a.compositor_bin)
        finally:
            if (runtime/'greetd-events.json').exists():(a.output/'greetd-events.json').write_bytes((runtime/'greetd-events.json').read_bytes())
            session.close()
if __name__=='__main__':main()
