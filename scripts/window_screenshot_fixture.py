"""Cache an exact-source frame harness against existing workspace dependencies."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
ROOT=Path(__file__).resolve().parents[1]
TOOLS=ROOT/'target/native-test-tools/window-screenshot'


def digest(path):
    with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()


def frame_reader(artifacts):
    source=ROOT/'crates/canopa-portal/src/capture/window/frame.rs'
    artifacts=Path(artifacts).resolve(strict=True)
    wanted={'pipewire':None,'image':None,'tokio':None,'rustix':None,'libc':None,'serde_json':None,'anyhow':None}
    versions={'pipewire':'0.9.2','image':'0.25.','rustix':'1.'}
    for line in artifacts.read_text().splitlines():
        value=json.loads(line)
        if value.get('reason')!='compiler-artifact':continue
        name=value['target']['name']
        if name not in wanted:continue
        if name=='tokio' and 'full' not in value['features']:continue
        version=versions.get(name)
        if version and '@'+version not in value['package_id']:continue
        for filename in value['filenames']:
            if filename.endswith('.rlib'):wanted[name]=Path(filename)
    assert all(wanted.values()),wanted
    constants=ROOT/'crates/canopa-portal/src/capture.rs'
    declarations=re.findall(r'^pub\(crate\) const (?:MAX_SIDE|MAX_PIXELS|MAX_BYTES):[^;]+;',constants.read_text(),re.M)
    assert len(declarations)==3
    TOOLS.mkdir(parents=True,exist_ok=True);module=TOOLS/'capture/window/frame.rs';module.parent.mkdir(parents=True,exist_ok=True)
    if not module.exists() or module.read_bytes()!=source.read_bytes():module.write_bytes(source.read_bytes())
    wrapper=TOOLS/'tests.rs';body='#[allow(dead_code)]\nmod capture {\n'+'\n'.join(declarations)+'\npub mod window { mod frame; }\n}\n'
    if not wrapper.exists() or wrapper.read_text()!=body:wrapper.write_text(body)
    binary=TOOLS/'frame-tests';receipt=TOOLS/'build.json'
    inputs=dict(source=digest(source),wrapper=digest(wrapper),dependencies={name:dict(path=str(p),sha256=digest(p)) for name,p in wanted.items()},compiler=subprocess.check_output(['rustc','--version'],cwd=ROOT,text=True).strip())
    old=json.loads(receipt.read_text()) if receipt.exists() else {}
    if binary.exists() and old.get('inputs')==inputs and old.get('sha256')==digest(binary):return binary
    command=['rustc','--edition=2024','--test','--crate-name','canopa_window_frame','-C','debuginfo=2','-C','incremental='+str(TOOLS/'incremental'),'-L','dependency='+str(ROOT/'target/debug/deps'),str(wrapper),'-o',str(binary)]
    for name,path in wanted.items():command.extend(['--extern',name+'='+str(path)])
    subprocess.run(command,cwd=ROOT,check=True)
    receipt.write_text(json.dumps(dict(inputs=inputs,command=command,sha256=digest(binary),bytes=binary.stat().st_size,cargo_dependency_build=False),indent=2)+'\n')
    return binary


def receiver():
    spec=importlib.util.spec_from_file_location('screenshot_touch_receiver',ROOT/'scripts/test-remote-touch-lease.py')
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module.receiver()


def pointer():
    """Reuse the desktop fixture's click caller in one stable tool cache."""
    from screencast_media import desktop
    sources=[ROOT/'scripts/desktop-fixture/click.c',
             ROOT/'scripts/desktop-fixture/wlr-virtual-pointer-unstable-v1.xml',
             ROOT/'scripts/validate-desktop.py']
    inputs={str(p):digest(p) for p in sources}
    inputs['compiler']=subprocess.check_output(['cc','--version'],text=True).splitlines()[0]
    TOOLS.mkdir(parents=True,exist_ok=True)
    binary=TOOLS/'click';receipt=TOOLS/'pointer-build.json'
    if binary.exists() and receipt.exists():
        old=json.loads(receipt.read_text())
        if old.get('inputs')==inputs and old.get('sha256')==digest(binary):return binary
    binary=desktop.Session(TOOLS/'unused-runtime',TOOLS).build_pointer()
    receipt.write_text(json.dumps(dict(inputs=inputs,sha256=digest(binary),bytes=binary.stat().st_size),indent=2)+'\n')
    return binary


if __name__=='__main__':
    import argparse,sys
    sys.dont_write_bytecode=True
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--artifacts',required=True,type=Path,help='JSON output from the existing workspace --all-targets --no-run build')
    args=parser.parse_args();print(frame_reader(args.artifacts))
