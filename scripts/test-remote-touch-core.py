#!/usr/bin/env python3
"""Test the touch contact registry of the pinned Niri fork without another Cargo build graph.

The two std-only modules are read from the pinned kevinard/niri archive
(checksummed in packaging/forks.json), downloaded into --archives when absent.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

sys.dont_write_bytecode = True
import fork_sources

ROOT = Path(__file__).resolve().parents[1]
MODULE = 'src/input/remote_touch/contacts.rs'
WIRE = 'src/dbus/remote_input/touch/wire.rs'
WORK = ROOT / 'target/fork-tests/remote-touch'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archives', type=Path, default=ROOT / 'target/fork-archives',
                        help='Directory holding (or receiving) the pinned Niri fork archive')
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    started = time.monotonic()
    origin, archive = fork_sources.origin('niri', args.archives.resolve(), fetch=True)
    source = fork_sources.archive_member(archive, origin['archive_root'], MODULE)
    wire = fork_sources.archive_member(archive, origin['archive_root'], WIRE)
    result = dict(status='running', source_sha256=fork_sources.digest(source), source_bytes=len(source),
                  fork=origin['repository'], commit=origin['commit'], archive_sha256=origin['archive_sha256'],
                  rustc=subprocess.check_output(['rustc','--version'],text=True).strip(), cargo_build=False,
                  wire_sha256=fork_sources.digest(wire), native_touch_verified=False, advertised=False)
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
