#!/usr/bin/env python3
"""Test the canonical touch contact registry without another Cargo build graph."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time

sys.dont_write_bytecode = True
import vendor_sources

ROOT = Path(__file__).resolve().parents[1]
BASE = 'niri-input-capture-keyboard'
OVERLAY = 'niri-remote-touch'
MODULE = 'src/input/remote_touch/contacts.rs'
WIRE = 'src/dbus/remote_input/touch/wire.rs'
DELTA = sorted([MODULE, WIRE, 'src/input/remote_touch.rs', 'src/input/mod.rs',
                'src/dbus/remote_input.rs', 'src/dbus/remote_input/touch.rs'])
WORK = ROOT / 'target/vendor-tests/remote-touch'


def inventory(root):
    return {str(p.relative_to(root)): vendor_sources.digest(p.read_bytes()) for p in root.rglob('*') if p.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archives', type=Path, help='Verify complete old/new materializations using cached archives')
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    started = time.monotonic()
    errors = vendor_sources.check(ROOT)
    if errors:raise ValueError('\n'.join(errors))
    record = vendor_sources.records(ROOT)[OVERLAY]
    source = vendor_sources.added_file(ROOT, record, MODULE)
    wire = vendor_sources.added_file(ROOT, record, WIRE)
    result = dict(status='running', source_sha256=vendor_sources.digest(source), source_bytes=len(source),
                  recipe_sha256=vendor_sources.digest(json.dumps(record,sort_keys=True).encode()),
                  rustc=subprocess.check_output(['rustc','--version'],text=True).strip(), cargo_build=False,
                  wire_sha256=vendor_sources.digest(wire), native_touch_verified=False, advertised=False)
    if args.archives:
        with tempfile.TemporaryDirectory(prefix='canopa-remote-touch-sources-') as temporary:
            base = vendor_sources.prepare(ROOT,args.archives.resolve(),Path(temporary)/'base',BASE)
            candidate = vendor_sources.prepare(ROOT,args.archives.resolve(),Path(temporary)/'candidate',OVERLAY)
            delta = {};counts = {}
            for name in base:
                before,after = inventory(base[name]),inventory(candidate[name])
                delta[name] = sorted(n for n in before.keys()|after.keys() if before.get(n)!=after.get(n))
                counts[name] = len(after)
            assert delta == {'niri':DELTA,'smithay':[]},delta
            assert (candidate['niri']/MODULE).read_bytes()==source
            result['archive_materialization'] = dict(delta=delta,file_counts=counts,original_notices_verified=True)
        result['temporary_sources_removed'] = not Path(temporary).exists()
    WORK.mkdir(parents=True,exist_ok=True)
    for name,body in [('contacts.rs',source),('wire.rs',wire),
                      ('tests.rs',b'pub mod contacts;\npub mod wire;\n')]:
        path=WORK/name
        if not path.exists() or path.read_bytes()!=body:path.write_bytes(body)
    binary=WORK/'contacts-tests'
    subprocess.run(['rustfmt','--edition','2021','--check',str(WORK/'tests.rs')],check=True)
    command = ['rustc','--crate-name','canopa_remote_touch_contacts','--edition','2021','--test',
               '-D','warnings','-C','debuginfo=0',str(WORK/'tests.rs'),'-o',str(binary)]
    subprocess.run(command,check=True,cwd=ROOT)
    log = WORK/'tests.log'
    with log.open('w') as stream:code = subprocess.run([str(binary)],cwd=ROOT,stdout=stream,stderr=subprocess.STDOUT).returncode
    result.update(status='passed' if code==0 else 'failed',exit_code=code,command=command,
                  test_result=next(line for line in log.read_text().splitlines() if line.startswith('test result:')),
                  test_log=str(log.relative_to(ROOT)),binary=str(binary.relative_to(ROOT)),binary_bytes=binary.stat().st_size)
    with binary.open('rb') as stream:result['binary_sha256']=hashlib.file_digest(stream,'sha256').hexdigest()
    result['seconds']=round(time.monotonic()-started,3)
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2));return code


if __name__ == '__main__':raise SystemExit(main())
