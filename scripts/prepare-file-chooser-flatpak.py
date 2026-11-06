#!/usr/bin/env python3
"""Deploy the tiny chooser probe into an existing fixture-owned Flatpak store.

No runtime download or SDK installation is performed. The store must already
contain its private Flatpak tools, canopa-fixture remote and selected runtime.
"""
import argparse
import configparser
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
APP = 'org.canopa.PortalFileChooserProbe'
SOURCE = ROOT / 'scripts/desktop-fixture/file-chooser-flatpak.c'


def digest(path):
    with path.open('rb') as stream: return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--store', required=True, type=Path)
    parser.add_argument('--runtime', required=True, help='Existing runtime ID/architecture/branch')
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--client-build', type=Path, help='Reuse an existing verified probe build')
    args = parser.parse_args()
    store, output = args.store.resolve(strict=True), args.output.resolve()
    if not store.is_relative_to(ROOT / 'target') or not output.is_relative_to(ROOT / 'target'):
        parser.error('Use fixture-owned store/output paths under root target/')
    if output.exists(): parser.error('--output must be a new receipt path')
    parts = args.runtime.split('/')
    if len(parts) != 3 or any(not part or part in ['.', '..'] for part in parts): parser.error('Invalid runtime ref')
    runtime = store / 'installation/runtime' / args.runtime / 'active'
    if not runtime.is_dir(): parser.error('The selected runtime must already be deployed')
    output.parent.mkdir(parents=True, exist_ok=True)
    assert shutil.disk_usage(ROOT).free > 3 * 1024**3
    if args.client_build:
        build = json.loads(args.client_build.read_text())
        assert build['status'] == 'built' and build['source_sha256'] == digest(SOURCE)
        program = Path(build['program']); assert digest(program) == build['sha256']
        build_path = args.client_build.resolve()
    else:
        program = output.parent / 'chooser-probe'
        if program.exists(): parser.error('Existing probe: reuse its build receipt or select its stable output directory')
        command = ['cc', '-D_GNU_SOURCE', '-std=c11', '-Wall', '-Wextra', '-Werror', '-O1', str(SOURCE), '-o', str(program),
                   *shlex.split(subprocess.check_output(['pkg-config', '--cflags', '--libs', 'gio-2.0'], text=True))]
        subprocess.run(command, check=True, timeout=30)
        build = dict(status='built', command=command, program=str(program), sha256=digest(program), source_sha256=digest(SOURCE), bytes=program.stat().st_size)
        build_path = output.parent / 'client-build.json'
        build_path.write_text(json.dumps(build, indent=2) + '\n')
    result = dict(status='running', app=APP, client_build=str(build_path), program_sha256=build['sha256'],
                  runtime_commit=runtime.resolve().name, runtime_rebuilt=False, system_installation_changed=False, commands=[])
    deployed = store / 'installation/app' / APP / 'x86_64/1/active/files/bin/chooser-probe'
    if deployed.is_file() and digest(deployed) == build['sha256']:
        metadata = configparser.ConfigParser()
        metadata.read(deployed.parents[2] / 'metadata')
        assert metadata['Application']['name'] == APP and metadata['Application']['runtime'] == args.runtime
        assert not metadata.get('Context', 'filesystems', fallback='')
        result.update(status='passed', existing_deployment_verified=True)
    else:
        app = store / 'flatpak-build/file-chooser-app'
        (app / 'files/bin').mkdir(parents=True, exist_ok=True)
        (app / 'files/share/applications').mkdir(parents=True, exist_ok=True)
        target = app / 'files/bin/chooser-probe'
        if target.exists(): target.unlink()
        shutil.copyfile(program, target); target.chmod(0o755)
        (app / 'metadata').write_text('[Application]\nname=' + APP + '\nruntime=' + args.runtime + '\nsdk=' + args.runtime + '\ncommand=chooser-probe\n')
        (app / 'files/share/applications' / (APP + '.desktop')).write_text('[Desktop Entry]\nType=Application\nName=Canopa sandbox file chooser probe\nExec=chooser-probe\n')
        env = {**os.environ, 'FLATPAK_USER_DIR': str(store / 'installation'), 'FLATPAK_SYSTEM_DIR': str(store / 'empty-system'),
               'FLATPAK_CONFIG_DIR': str(store / 'config'), 'FLATPAK_SYSTEM_CACHE_DIR': str(store / 'system-cache'),
               'HOME': str(store / 'home'), 'XDG_DATA_HOME': str(store / 'data'), 'XDG_CONFIG_HOME': str(store / 'config'),
               'XDG_CACHE_HOME': str(store / 'cache'), 'XDG_RUNTIME_DIR': str(store / 'runtime'),
               'DBUS_SESSION_BUS_ADDRESS': 'unix:path=' + str(store / 'no-session-bus'),
               'DBUS_SYSTEM_BUS_ADDRESS': 'unix:path=' + str(store / 'no-system-bus'), 'LC_ALL': 'C'}
        for key in ['LD_PRELOAD', 'LD_AUDIT', 'LD_LIBRARY_PATH']: env.pop(key, None)
        command = ['/usr/lib/ld-linux-x86-64.so.2', '--library-path', str(store / 'flatpak-tools/usr/lib'), str(store / 'flatpak-tools/usr/bin/flatpak')]
        for name, flags in [('finish', ['build-finish', '--command=chooser-probe', str(app)]),
                            ('export', ['build-export', str(store / 'repository'), str(app), '1']),
                            ('install', ['install', '--user', '--or-update', '--no-deps', '--no-related', '--noninteractive', '--assumeyes', 'canopa-fixture', 'app/' + APP + '/x86_64/1'])]:
            with output.with_name(output.stem + '-' + name + '.log').open('w') as log:
                answer = subprocess.run([*command, *flags], env=env, stdout=log, stderr=subprocess.STDOUT, timeout=45)
            result['commands'].append(dict(name=name, exit_code=answer.returncode))
            output.write_text(json.dumps(result, indent=2) + '\n')
            if answer.returncode: raise SystemExit(answer.returncode)
        assert digest(deployed) == build['sha256']
        result['status'] = 'passed'
    output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({key: result.get(key) for key in ['status', 'existing_deployment_verified', 'client_build']}))


if __name__ == '__main__': main()
