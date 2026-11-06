#!/usr/bin/env python3
"""Stage or verify the optional matching IPP backend; configure no CUPS service."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess

import vendor_sources

ROOT = Path(__file__).resolve().parents[1]
OVERLAY = 'cups-ipp-backend'
BACKEND = Path('lib/canopa/print/ipp')
MANIFEST = Path('share/canopa/print-backend.json')
NOTICES = Path('share/licenses/canopa-print-ipp-backend')
spec = importlib.util.spec_from_file_location('print_runtime_packager', ROOT / 'scripts/package-print-runtime.py')
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def record():
    value = vendor_sources.records(ROOT)[OVERLAY]
    for source in value['sources'].values():
        for asset in vendor_sources.license_assets(source):
            path = ROOT / asset['path']
            runtime.regular(path)
            if runtime.digest(path) != asset['sha256']:
                raise ValueError('Backend notice differs from canonical provenance')
    for patch in value['patches']:
        path = ROOT / patch['path']
        runtime.regular(path)
        if runtime.digest(path) != patch['sha256']:
            raise ValueError('Backend patch differs from canonical provenance')
    return value


def inspect(path, client_sha):
    runtime.regular(path)
    if not 64 <= path.stat().st_size <= 4 * 1024 * 1024:
        raise ValueError('Invalid IPP backend size')
    with path.open('rb') as stream:
        header = stream.read(20)
    if header[:7] != b'\x7fELF\x02\x01\x01' or struct.unpack_from('<HH', header, 16) not in [(2, 62), (3, 62)]:
        raise ValueError('Expected an ELF64 little-endian x86-64 backend')
    dynamic = subprocess.check_output(['readelf', '--dynamic', '--wide', str(path)],
                                      env=dict(os.environ, LC_ALL='C'), text=True, timeout=10)
    needed = sorted(re.findall(r'\(NEEDED\).*\[([^]]+)\]', dynamic))
    runpath = '$ORIGIN:$ORIGIN/../../print-client-by-sha256/' + client_sha
    if ('libcups.so.2' not in needed or any('/' in name for name in needed)
            or re.findall(r'\(RUNPATH\).*\[([^]]+)\]', dynamic) != [runpath]
            or re.search(r'\(RPATH\)', dynamic)):
        raise ValueError('Backend must select its matching sibling client or checksum store')
    symbols = subprocess.check_output(['readelf', '--dyn-syms', '--wide', str(path)],
                                      env=dict(os.environ, LC_ALL='C'), text=True, timeout=10)
    if not re.search(r'\bUND\s+canopaCupsAuthenticationVersion\s*$', symbols, re.M):
        raise ValueError('Backend lacks its required Canopa client capability dependency')
    return dict(format='ELF64', architecture='x86_64', needed=needed, runpath=runpath)


def client_manifest(bundle):
    runtime.check(bundle, required=True)
    return json.loads((bundle / runtime.MANIFEST).read_text())


def stage(bundle, receipt_path):
    recipe = record()
    receipt_path = receipt_path.resolve(strict=True)
    build = json.loads(receipt_path.read_text())
    expected = vendor_sources.digest(json.dumps(recipe, sort_keys=True).encode())
    if (build.get('overlay') != OVERLAY or build.get('status') != 'built'
            or build.get('recipe_sha256') != expected or build.get('inputs_unchanged') is not True
            or build.get('built_targets') != ['ipp'] or build.get('opt_in') is not True):
        raise ValueError('Backend receipt is incomplete or belongs to another recipe')
    client = client_manifest(bundle)
    client_sha = client['library']['sha256']
    if build.get('library_sha256') != client_sha:
        raise ValueError('Backend and packaged client do not match')
    client_receipt = Path(build['client_build'])
    if runtime.digest(client_receipt) != build['client_build_sha256']:
        raise ValueError('Backend client receipt changed')
    _, client_build = runtime.verified_build(client_receipt, runtime.record())
    headers = Path(client_build['source'])
    configuration = build.get('configuration_inputs', {})
    if (configuration != client_build.get('configuration_inputs')
            or set(configuration) != {str(headers / name) for name in ['config.h', 'Makedefs', 'config.status']}
            or any(runtime.digest(Path(path)) != digest for path, digest in configuration.items())):
        raise ValueError('Backend client configuration differs from its build')
    source = Path(build['source'])
    inputs = json.loads((receipt_path.parent / 'source-inputs.json').read_text())
    if (len(inputs) != build['source_file_count']
            or vendor_sources.digest(json.dumps(inputs, sort_keys=True).encode()) != build['source_inputs_sha256']):
        raise ValueError('Backend source inventory differs from its build')
    for name, digest in inputs.items():
        relative = Path(name)
        if relative.is_absolute() or '..' in relative.parts:
            raise ValueError('Invalid backend source inventory path')
        path = source / relative
        runtime.regular(path)
        if runtime.digest(path) != digest:
            raise ValueError('Backend source changed after its build')
    for patch in recipe['patches']:
        if any(inputs.get(name) != hashes['patched'] for name, hashes in patch['files'].items()):
            raise ValueError('Backend build lacks its canonical correction')
    binary = Path(build['binary'])
    metadata = inspect(binary, client_sha)
    if runtime.digest(binary) != build['binary_sha256']:
        raise ValueError('Backend binary differs from its build')
    if any((bundle / path).exists() or (bundle / path).is_symlink() for path in [BACKEND, MANIFEST, NOTICES]):
        raise ValueError('Backend destination already exists')
    for path in [BACKEND, MANIFEST, NOTICES / 'LICENSE']:
        runtime.check_parents(bundle, bundle / path)
    files = {BACKEND: binary}
    source_spec = recipe['sources']['cups']
    for asset in vendor_sources.license_assets(source_spec):
        files[NOTICES / Path(asset['path']).name] = ROOT / asset['path']
    for patch in recipe['patches']:
        files[NOTICES / Path(patch['path']).name] = ROOT / patch['path']
    for name, origin in files.items():
        target = bundle / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(origin, target)
        target.chmod(0o755 if name == BACKEND else 0o644)
    if runtime.digest(bundle / BACKEND) != build['binary_sha256']:
        raise ValueError('Backend changed while staging')
    changes = bundle / NOTICES / 'CHANGES.md'
    changes.write_text('# Canopa IPP backend modifications\n\n' + recipe['description']
                       + '.\n\nModified source: `backend/ipp.c`. The complete correction is '
                       '`resolution.patch`, beside the original LICENSE and NOTICE.\n\n'
                       'This optional program requires the matching packaged client. Staging it '
                       'does not configure a CUPS service, backend directory or printer queue.\n')
    manifest = dict(schema_version=1, overlay=OVERLAY, recipe_sha256=expected,
                    upstream=dict(version=source_spec['revision'], url=source_spec['url'],
                                  archive_sha256=source_spec['archive_sha256']),
                    client_sha256=client_sha,
                    backend=dict(path=str(BACKEND), sha256=runtime.digest(bundle / BACKEND),
                                 bytes=(bundle / BACKEND).stat().st_size, **metadata),
                    files={str(name): runtime.digest(bundle / name) for name in [*files, NOTICES / 'CHANGES.md']})
    (bundle / MANIFEST).parent.mkdir(parents=True, exist_ok=True)
    (bundle / MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')


def check(bundle, required=False):
    if not any((bundle / path).exists() or (bundle / path).is_symlink() for path in [BACKEND, MANIFEST, NOTICES]):
        if required:
            raise ValueError('Missing packaged IPP backend')
        print('Release IPP backend: absent; configured CUPS backend remains authoritative.')
        return
    recipe = record()
    runtime.regular(bundle / MANIFEST)
    manifest = json.loads((bundle / MANIFEST).read_text())
    source = recipe['sources']['cups']
    if (manifest.get('schema_version') != 1 or manifest.get('overlay') != OVERLAY
            or manifest.get('recipe_sha256') != vendor_sources.digest(json.dumps(recipe, sort_keys=True).encode())
            or manifest.get('upstream') != dict(version=source['revision'], url=source['url'], archive_sha256=source['archive_sha256'])):
        raise ValueError('Backend manifest differs from canonical provenance')
    client = client_manifest(bundle)
    if manifest.get('client_sha256') != client['library']['sha256']:
        raise ValueError('Packaged backend and client do not match')
    originals = {str(NOTICES / Path(asset['path']).name): asset['sha256'] for asset in vendor_sources.license_assets(source)}
    originals.update({str(NOTICES / Path(patch['path']).name): patch['sha256'] for patch in recipe['patches']})
    if set(manifest.get('files', {})) != {str(BACKEND), str(NOTICES / 'CHANGES.md'), *originals}:
        raise ValueError('Incomplete backend binary, patch or notice inventory')
    for name, digest in manifest['files'].items():
        path = bundle / name
        runtime.check_parents(bundle, path)
        runtime.regular(path)
        if runtime.digest(path) != digest or (name in originals and digest != originals[name]):
            raise ValueError('Packaged backend file differs from its inventory: ' + name)
    binary = bundle / BACKEND
    actual = dict(path=str(BACKEND), sha256=runtime.digest(binary), bytes=binary.stat().st_size,
                  **inspect(binary, manifest['client_sha256']))
    if manifest.get('backend') != actual:
        raise ValueError('Backend metadata differs from the actual image')
    print('Release IPP backend: matching client, source correction and original notices verified.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--build-receipt', type=Path)
    mode.add_argument('--check', action='store_true')
    parser.add_argument('--required', action='store_true')
    args = parser.parse_args()
    if args.required and not args.check:
        parser.error('--required is only valid with --check')
    try:
        if args.check:
            check(args.bundle.resolve(), args.required)
        else:
            stage(args.bundle.resolve(), args.build_receipt)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        parser.exit(1, f'IPP backend packaging: {error}\n')


if __name__ == '__main__':
    main()
