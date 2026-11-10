#!/usr/bin/env python3
"""Check the keyboard ownership modules of the pinned Niri fork without another Cargo graph.

The std-only modules are read from the pinned kevinard/niri archive
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
MODULE = 'src/dbus/input_capture/keyboard.rs'
MASKS = 'src/dbus/input_capture/masks.rs'
WORK = ROOT / 'target/fork-tests/input-capture-keyboard'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archives', type=Path, default=ROOT / 'target/fork-archives',
                        help='Directory holding (or receiving) the pinned Niri fork archive')
    parser.add_argument('--output', type=Path, default=WORK / 'result.json')
    args = parser.parse_args()
    origin, archive = fork_sources.origin('niri', args.archives.resolve(), fetch=True)
    bodies = {module: fork_sources.archive_member(archive, origin['archive_root'], module)
              for module in (MODULE, MASKS)}
    WORK.mkdir(parents=True, exist_ok=True)
    for module, body in bodies.items():
        source = WORK / Path(module).name
        if not source.exists() or source.read_bytes() != body:
            source.write_bytes(body)
    source = WORK / 'ownership-tests.rs'
    source.write_text('pub mod keyboard;\npub mod masks;\n')
    binary, build_log, test_log = (WORK / name for name in ('core-tests', 'build.log', 'tests.log'))
    command = ['rustc', '--edition=2021', '--test', '-D', 'warnings', str(source), '-o', str(binary)]
    started = time.monotonic()
    with build_log.open('w') as log:
        built = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    code = built.returncode
    result = {'scope': 'pure compositor ownership core; no runtime keyboard capture claim',
              'fork': origin['repository'], 'commit': origin['commit'], 'archive_sha256': origin['archive_sha256'],
              'source_sha256': {module: fork_sources.digest(body) for module, body in bodies.items()},
              'compiler': subprocess.check_output(['rustc', '--version'], text=True).strip(),
              'compile_command': command, 'compile_exit_code': code,
              'new_cargo_graph': False,
              'live_session_changed': False}
    if not code:
        with test_log.open('w') as log:
            tested = subprocess.run([str(binary)], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
        code = tested.returncode
        result.update(test_exit_code=code, test_log=str(test_log.relative_to(ROOT)),
                      test_result=next((line for line in test_log.read_text().splitlines()
                                        if line.startswith('test result:')), None))
        with binary.open('rb') as stream:
            result['binary_sha256'] = hashlib.file_digest(stream, 'sha256').hexdigest()
        result['binary_bytes'] = binary.stat().st_size
    result.update(seconds=round(time.monotonic() - started, 3), exit_code=code)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))
    return code


if __name__ == '__main__':
    raise SystemExit(main())
