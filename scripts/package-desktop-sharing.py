#!/usr/bin/env python3
"""Stage or verify optional Niri/portal companions and explicit sharing routing."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import struct
import subprocess

import fork_sources

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = Path('share/canopa/desktop-sharing.json')
PROGRAMS = Path('libexec/canopa/desktop-sharing')
NOTICES = Path('share/licenses/canopa-desktop-sharing')
PROFILE = Path('share/canopa/desktop-sharing')
DESCRIPTOR = Path('share/xdg-desktop-portal/portals/canopa.portal')
INTERFACES = ('ScreenCast', 'RemoteDesktop', 'Clipboard')
EXTRA_INTERFACES = ('InputCapture', 'GlobalShortcuts', 'Usb')
FORKS = {'niri': 'niri', 'frontend': 'xdg-desktop-portal'}
SCHEMA = 2
EXECUTABLES = {'niri': PROGRAMS / 'niri', 'frontend': PROGRAMS / 'xdg-desktop-portal'}


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def regular(path):
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError(f'Expected a regular file: {path}')


def expected_notices(kind):
    """Original license texts each pinned companion must ship, as {relative path: sha256}."""
    fork = FORKS[kind]
    pin = fork_sources.pin(fork)
    notices = {f'{fork}/{name}': checksum for name, checksum in pin['notices'].items()}
    for name, spec in pin.get('cargo_git', {}).items():
        notices.update({f'{name}/{file}': checksum for file, checksum in spec['notices'].items()})
    return notices


def source_record(kind, component):
    """Validate a packaged component's source identity against the current pin."""
    fork = FORKS[kind]
    pin = fork_sources.pin(fork)
    expected = dict(fork=fork, upstream={key: pin[key] for key in fork_sources.ORIGIN_FIELDS})
    if 'cargo_git' in pin:
        expected['cargo_git'] = {name: {key: spec[key] for key in ('repository', 'commit', 'packages')}
                                 for name, spec in pin['cargo_git'].items()}
    if 'subprojects' in pin:
        expected['subprojects'] = {key: value for key, value in pin['subprojects'].items() if key != 'description'}
    if {key: component.get(key) for key in expected} != expected:
        raise ValueError(f'Packaged {kind} companion is not the pinned fork build')
    return expected


def inspect_program(path):
    regular(path)
    if not os.access(path, os.X_OK):
        raise ValueError(f'Companion is not executable: {path}')
    with path.open('rb') as stream:
        header = stream.read(20)
    if (header[:7] != b'\x7fELF\x02\x01\x01' or len(header) < 20
            or struct.unpack_from('<HH', header, 16) not in [(2, 62), (3, 62)]):
        raise ValueError('Companion must be an ELF64 little-endian x86-64 executable')
    dynamic = subprocess.check_output(['readelf', '--dynamic', '--wide', str(path)],
                                      text=True, env={**os.environ, 'LC_ALL': 'C'}, timeout=15)
    if re.search(r'\((?:RPATH|RUNPATH)\)', dynamic):
        raise ValueError('Companion embeds a build-specific runtime search path')
    needed = sorted(re.findall(r'\(NEEDED\).*\[([^]]+)\]', dynamic))
    if any('/' in name for name in needed):
        raise ValueError('Companion has an absolute library dependency')
    return dict(architecture='x86_64', needed=needed)


def verified_build(path, kind):
    """Return (executable, metadata, {notice relative path: file}) for a pinned fork build."""
    path = path.resolve(strict=True)
    receipt = json.loads(path.read_text())
    fork = FORKS[kind]
    if receipt.get('fork') != fork or receipt.get('status') != 'built' or receipt.get('exit_code') != 0:
        raise ValueError(f'{kind} build is incomplete or belongs to another fork')
    pin = fork_sources.check_origin(receipt.get('origin', {}), fork)
    binary = Path(receipt['binary'])
    if not binary.is_absolute():
        raise ValueError('Build receipt must name an absolute executable path')
    regular(binary)
    if digest(binary) != receipt.get('binary_sha256'):
        raise ValueError(f'{kind} executable differs from its build receipt')
    source = Path(receipt['source'])
    if not source.is_absolute() or source.is_symlink() or not source.is_dir():
        raise ValueError('Companion source root is unavailable')
    inputs = fork_sources.read_inputs(path, receipt)
    ignore = set()
    if kind == 'frontend':
        wraplock = source / 'subprojects/.wraplock'
        if wraplock.exists() and wraplock.stat().st_size == 0 and not wraplock.is_symlink():
            ignore.add('subprojects/.wraplock')
    if fork_sources.inventory(source, ignore) != inputs:
        raise ValueError(f'{fork} sources changed after the companion build')
    metadata = dict(fork=fork, upstream={key: receipt['origin'][key] for key in fork_sources.ORIGIN_FIELDS})
    if kind == 'niri':
        metadata['cargo_git'] = {name: {'repository': spec['repository'], 'commit': spec['commit'],
                                        'packages': spec['packages']} for name, spec in pin['cargo_git'].items()}
        locked = {package: {'repository': spec['repository'], 'commit': spec['commit'], 'query': 'rev=' + spec['commit']}
                  for spec in pin['cargo_git'].values() for package in spec['packages']}
        if receipt.get('cargo_git') != locked or inputs.get('Cargo.lock') != receipt.get('lock_sha256'):
            raise ValueError('Niri lockfile git sources differ from the pinned forks')
    else:
        metadata['subprojects'] = receipt.get('subprojects')
    source_record(kind, metadata)
    notices = receipt.get('notices', {})
    if {name: value.get('sha256') for name, value in notices.items()} != expected_notices(kind):
        raise ValueError(f'{kind} build notices differ from the pinned forks')
    files = {}
    for relative, value in notices.items():
        notice = Path(value['path'])
        if relative.startswith(fork + '/') and (notice != source / relative.split('/', 1)[1]
                                                 or inputs.get(notice.name) != value['sha256']):
            raise ValueError('Companion notice is not part of its verified source tree')
        regular(notice)
        if digest(notice) != value['sha256']:
            raise ValueError('Companion notice differs from its build receipt')
        files[Path(relative)] = notice
    metadata.update(profile=receipt['profile'], sha256=receipt['binary_sha256'], **inspect_program(binary))
    return binary, metadata, files


def descriptor(interfaces=INTERFACES):
    base = (ROOT / 'packaging/portal/canopa.portal').read_text()
    additions = ''.join(f'org.freedesktop.impl.portal.{name};' for name in interfaces)
    return base.replace('Interfaces=', 'Interfaces=' + additions, 1).encode()


def assets(components, interfaces=INTERFACES):
    """Repository routing files and generated data; notices come from the verified builds."""
    if interfaces not in (INTERFACES, INTERFACES + EXTRA_INTERFACES):
        raise ValueError('Unsupported optional portal selection')
    files = {}
    fragment = (ROOT / 'packaging/portal/desktop-sharing/portals.conf.fragment').read_bytes()
    if EXTRA_INTERFACES[0] in interfaces:
        fragment += ''.join(f'org.freedesktop.impl.portal.{name}=canopa\n' for name in EXTRA_INTERFACES).encode()
    generated = {DESCRIPTOR: descriptor(interfaces), PROFILE / 'portals.conf.fragment': fragment}
    files[PROFILE / 'xdg-desktop-portal.service.conf'] = ROOT / 'packaging/portal/desktop-sharing/xdg-desktop-portal.service.conf'
    for name in ('canopa-screencast.lua', '90-canopa-screencast.conf'):
        files[PROFILE / 'wireplumber' / name] = ROOT / 'packaging/portal/desktop-sharing' / name
    for kind, component in components.items():
        record = source_record(kind, component)
        generated[NOTICES / kind / 'source.json'] = (json.dumps(record, indent=2, sort_keys=True) + '\n').encode()
    return files, generated


def notice_inventory(components):
    return {NOTICES / kind / relative: checksum for kind in components
            for relative, checksum in expected_notices(kind).items()}


def safe_destination(bundle, relative):
    path = bundle / relative
    for parent in [path, *path.parents]:
        if not parent.is_relative_to(bundle):
            break
        if parent.is_symlink():
            raise ValueError('Packaged companion destination contains a symlink')
    return path


def stage(bundle, niri, frontend, extra_portals=False):
    builds = {kind: verified_build(path, kind) for kind, path in [('niri', niri), ('frontend', frontend)]}
    components = {kind: metadata for kind, (_, metadata, _) in builds.items()}
    interfaces = INTERFACES + EXTRA_INTERFACES if extra_portals else INTERFACES
    files, generated = assets(components, interfaces)
    files.update({EXECUTABLES[kind]: binary for kind, (binary, _, _) in builds.items()})
    for kind, (_, _, notices) in builds.items():
        files.update({NOTICES / kind / relative: notice for relative, notice in notices.items()})
    for relative in [PROGRAMS, PROFILE, MANIFEST, NOTICES]:
        path = safe_destination(bundle, relative)
        if path.exists():
            raise ValueError('Desktop-sharing component destination already exists')
    current = safe_destination(bundle, DESCRIPTOR)
    if current.exists() and current.read_bytes() != (ROOT / 'packaging/portal/canopa.portal').read_bytes():
        raise ValueError('Refusing to replace a different portal descriptor')
    for relative in [*files, *generated, MANIFEST]:
        safe_destination(bundle, relative)
    for relative, source in files.items():
        target = bundle / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        target.chmod(0o755 if relative in EXECUTABLES.values() else 0o644)
    for relative, data in generated.items():
        target = bundle / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        target.chmod(0o644)
    manifest = dict(schema_version=SCHEMA, interfaces=list(interfaces), components=components,
                    files={str(relative): digest(bundle / relative) for relative in [*files, *generated]})
    (bundle / MANIFEST).parent.mkdir(parents=True, exist_ok=True)
    (bundle / MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    check(bundle, required=True)


def check(bundle, required=False, require_extra_portals=False):
    if not any((bundle / path).exists() or (bundle / path).is_symlink()
               for path in [PROGRAMS, PROFILE, MANIFEST, NOTICES]):
        if required or require_extra_portals or ((bundle / DESCRIPTOR).exists() and any(
                f'org.freedesktop.impl.portal.{name};' in (bundle / DESCRIPTOR).read_text() for name in INTERFACES + EXTRA_INTERFACES)):
            raise ValueError('Missing packaged desktop-sharing companions')
        print('Release desktop sharing: absent; existing routing retained.')
        return
    regular(safe_destination(bundle, MANIFEST))
    manifest = json.loads((bundle / MANIFEST).read_text())
    if (manifest.get('schema_version') != SCHEMA or manifest.get('interfaces') not in [list(INTERFACES), list(INTERFACES + EXTRA_INTERFACES)]
            or set(manifest.get('components', {})) != set(EXECUTABLES)):
        raise ValueError('Incomplete desktop-sharing manifest')
    interfaces = tuple(manifest['interfaces'])
    if require_extra_portals and interfaces != INTERFACES + EXTRA_INTERFACES:
        raise ValueError('Missing optional InputCapture/GlobalShortcuts/Usb selection')
    files, generated = assets(manifest['components'], interfaces)
    notices = notice_inventory(manifest['components'])
    expected = {str(p) for p in [*files, *generated, *notices, *EXECUTABLES.values()]}
    if set(manifest.get('files', {})) != expected:
        raise ValueError('Incomplete desktop-sharing file inventory')
    for name, checksum in manifest['files'].items():
        path = safe_destination(bundle, Path(name))
        regular(path)
        if digest(path) != checksum or checksum != notices.get(Path(name), checksum):
            raise ValueError(f'Packaged desktop-sharing file changed: {name}')
    for relative, source in files.items():
        if (bundle / relative).read_bytes() != source.read_bytes():
            raise ValueError(f'Packaged companion notice or routing file changed: {relative}')
    for relative, data in generated.items():
        if (bundle / relative).read_bytes() != data:
            raise ValueError(f'Packaged companion source record or descriptor changed: {relative}')
    for kind, relative in EXECUTABLES.items():
        metadata = manifest['components'][kind]
        actual = inspect_program(bundle / relative)
        if metadata['sha256'] != digest(bundle / relative) or any(metadata.get(k) != v for k, v in actual.items()):
            raise ValueError('Packaged companion executable metadata changed')
    print('Release desktop sharing: both pinned fork companions, routing, source records and original notices verified.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--niri-build', type=Path)
    parser.add_argument('--frontend-build', type=Path)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--required', action='store_true')
    parser.add_argument('--extra-portals', action='store_true', help='Also select InputCapture, GlobalShortcuts and Usb')
    parser.add_argument('--require-extra-portals', action='store_true', help='Require all six optional interfaces when checking')
    args = parser.parse_args()
    if args.check:
        if args.niri_build or args.frontend_build or args.extra_portals:
            parser.error('--check does not accept build receipts')
    elif args.required or args.require_extra_portals or not (args.niri_build and args.frontend_build):
        parser.error('provide both --niri-build and --frontend-build, or --check [--required]')
    try:
        if args.check:
            check(args.bundle.resolve(), args.required, args.require_extra_portals)
        else:
            stage(args.bundle.resolve(), args.niri_build, args.frontend_build, args.extra_portals)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        parser.exit(1, f'Desktop-sharing packaging: {error}\n')


if __name__ == '__main__':
    main()
