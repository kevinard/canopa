#!/usr/bin/env python3
"""Check the canonical keyboard ownership module without another Cargo graph."""
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
OVERLAY = 'niri-input-capture-keyboard'
BASE = 'niri-global-shortcuts'
MODULE = 'src/dbus/input_capture/keyboard.rs'
MASKS = 'src/dbus/input_capture/masks.rs'
DELTA = sorted([MODULE, 'src/dbus/input_capture/keys.rs', 'src/dbus/input_capture.rs',
                'src/dbus/mod.rs', 'src/dbus/global_shortcuts.rs', 'src/input/mod.rs',
                'src/handlers/virtual_keyboard.rs', MASKS])
WORK = ROOT / 'target/vendor-tests/input-capture-keyboard'


def canonical_source(record, module=MODULE):
    return vendor_sources.added_file(ROOT, record, module)


def inventory(source):
    return {str(p.relative_to(source)): vendor_sources.digest(p.read_bytes())
            for p in source.rglob('*') if p.is_file()}


def verify_archives(archives, expected):
    with tempfile.TemporaryDirectory(prefix='canopa-keyboard-source-check-') as directory:
        base = vendor_sources.prepare(ROOT, archives, Path(directory) / 'base', BASE)
        candidate = vendor_sources.prepare(ROOT, archives, Path(directory) / 'candidate', OVERLAY)
        delta = {}
        counts = {}
        for name in base:
            before, after = inventory(base[name]), inventory(candidate[name])
            delta[name] = sorted(p for p in before.keys() | after.keys()
                                 if before.get(p) != after.get(p))
            counts[name] = len(after)
        if delta != {'niri': DELTA, 'smithay': []}:
            raise ValueError(f'Unexpected combined-recipe change: {delta}')
        for module, body in expected.items():
            if (candidate['niri'] / module).read_bytes() != body:
                raise ValueError('Materialized ownership source differs from tested input')
        return {'delta_from_combined_recipe': delta, 'source_file_counts': counts,
                'archives_and_original_notices': 'verified', 'temporary_sources_removed': True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archives', type=Path,
                        help='Also prove the full old/new materialization using cached archives')
    parser.add_argument('--output', type=Path, default=WORK / 'result.json')
    args = parser.parse_args()
    errors = vendor_sources.check(ROOT)
    if errors:
        raise ValueError('\n'.join(errors))
    record = vendor_sources.records(ROOT)[OVERLAY]
    bodies = {module: canonical_source(record, module) for module in (MODULE, MASKS)}
    archive_check = verify_archives(args.archives.resolve(), bodies) if args.archives else None
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
              'overlay': OVERLAY, 'source_sha256': {module: vendor_sources.digest(body) for module, body in bodies.items()},
              'recipe_sha256': vendor_sources.digest(json.dumps(record, sort_keys=True).encode()),
              'compiler': subprocess.check_output(['rustc', '--version'], text=True).strip(),
              'compile_command': command, 'compile_exit_code': code,
              'archive_check': archive_check, 'new_cargo_graph': False,
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
