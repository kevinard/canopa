#!/usr/bin/env python3
"""Deploy a tiny sharing client in the existing private Flatpak installation.

No runtime or SDK is downloaded or changed. Installed PipeWire client
libraries are copied into the probe application; its temporary build retires.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import stat
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
APP = 'org.canopa.PortalScreenCastProbe'
SOURCES = [ROOT / 'scripts/desktop-fixture' / name for name in
           ('screencast-flatpak.c', 'screencast-consumer.c')]
LIBRARIES = {
    '/usr/lib/libpipewire-0.3.so.0': 'lib/libpipewire-0.3.so.0',
    '/usr/lib/pipewire-0.3/libpipewire-module-protocol-native.so': 'lib/pipewire-0.3/libpipewire-module-protocol-native.so',
    '/usr/lib/pipewire-0.3/libpipewire-module-client-node.so': 'lib/pipewire-0.3/libpipewire-module-client-node.so',
    '/usr/lib/pipewire-0.3/libpipewire-module-adapter.so': 'lib/pipewire-0.3/libpipewire-module-adapter.so',
    '/usr/lib/spa-0.2/support/libspa-support.so': 'lib/spa-0.2/support/libspa-support.so',
    '/usr/lib/spa-0.2/videoconvert/libspa-videoconvert.so': 'lib/spa-0.2/videoconvert/libspa-videoconvert.so',
}
CONFIG = '''context.spa-libs = {
    support.* = support/libspa-support
    video.convert.* = videoconvert/libspa-videoconvert
}
context.modules = [
    { name = libpipewire-module-protocol-native }
    { name = libpipewire-module-client-node }
    { name = libpipewire-module-adapter }
]
'''


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def allocated(root):
    seen, size = set(), 0
    for path in root.rglob('*'):
        entry = path.lstat()
        key = (entry.st_dev, entry.st_ino)
        if stat.S_ISREG(entry.st_mode) and key not in seen:
            seen.add(key)
            size += entry.st_blocks * 512
    return size


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--store', required=True, type=Path)
    parser.add_argument('--runtime', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    store, output = args.store.resolve(strict=True), args.output.resolve()
    if not store.is_relative_to(ROOT / 'target') or not output.is_relative_to(ROOT / 'target'):
        parser.error('Use the existing fixture store and stable output under root target/')
    if output.exists(): parser.error('Use a new receipt path')
    parts = args.runtime.split('/')
    if len(parts) != 3 or any(p in ('', '.', '..') for p in parts): parser.error('Invalid runtime ref')
    runtime = store / 'installation/runtime' / args.runtime / 'active'
    if not runtime.is_dir(): parser.error('Runtime must already be installed')
    assert shutil.disk_usage(ROOT).free > 3 * 1024**3
    output.parent.mkdir(parents=True, exist_ok=True)
    inputs = {str(path): digest(path) for path in SOURCES}
    libs = {path: digest(path) for path in LIBRARIES}
    program, build_path = output.parent / 'sharing-probe', output.parent / 'client-build.json'
    if build_path.exists() and program.exists() and json.loads(build_path.read_text())['source_inputs'] == inputs:
        build = json.loads(build_path.read_text())
        assert digest(program) == build['sha256']
    else:
        flags = shlex.split(subprocess.check_output(
            ['pkg-config', '--cflags', '--libs', 'gio-2.0', 'gio-unix-2.0', 'libpipewire-0.3'], text=True))
        command = ['cc', '-std=c11', '-Wall', '-Wextra', '-Werror', '-O1', str(SOURCES[0]),
                   '-Wl,-rpath,/app/lib', '-o', str(program), *flags]
        subprocess.run(command, check=True, timeout=30)
        build = {'status': 'built', 'command': command, 'program': str(program),
                 'source_inputs': inputs, 'sha256': digest(program), 'bytes': program.stat().st_size}
        build_path.write_text(json.dumps(build, indent=2) + '\n')
    result = {'status': 'running', 'app': APP, 'client_build': str(build_path),
              'runtime_commit': runtime.resolve().name, 'runtime_ref': args.runtime,
              'runtime_rebuilt': False, 'runtime_files_added': 0, 'sdk_downloaded': False,
              'application_library_inputs': libs, 'commands': [], 'store_before': allocated(store)}
    env = {**os.environ, 'FLATPAK_USER_DIR': str(store / 'installation'),
           'FLATPAK_SYSTEM_DIR': str(store / 'empty-system'), 'FLATPAK_CONFIG_DIR': str(store / 'config'),
           'FLATPAK_SYSTEM_CACHE_DIR': str(store / 'system-cache'), 'HOME': str(store / 'home'),
           'XDG_DATA_HOME': str(store / 'data'), 'XDG_CONFIG_HOME': str(store / 'config'),
           'XDG_CACHE_HOME': str(store / 'cache'), 'XDG_RUNTIME_DIR': str(store / 'runtime'),
           'DBUS_SESSION_BUS_ADDRESS': 'unix:path=' + str(store / 'no-session-bus'),
           'DBUS_SYSTEM_BUS_ADDRESS': 'unix:path=' + str(store / 'no-system-bus'), 'LC_ALL': 'C'}
    for key in ('LD_PRELOAD', 'LD_AUDIT', 'LD_LIBRARY_PATH'): env.pop(key, None)
    flatpak = ['/usr/lib/ld-linux-x86-64.so.2', '--library-path', str(store / 'flatpak-tools/usr/lib'),
               str(store / 'flatpak-tools/usr/bin/flatpak')]
    with tempfile.TemporaryDirectory(prefix='canopa-sharing-app-') as directory:
        app = Path(directory)
        for target in ('files/bin', 'files/share/applications', 'files/share/pipewire'):
            (app / target).mkdir(parents=True, exist_ok=True)
        shutil.copyfile(program, app / 'files/bin/sharing-probe')
        (app / 'files/bin/sharing-probe').chmod(0o755)
        for source, target in LIBRARIES.items():
            path = app / 'files' / target
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, path)
            assert digest(path) == libs[source]
        (app / 'files/share/pipewire/client.conf').write_text(CONFIG)
        (app / 'metadata').write_text(f'[Application]\nname={APP}\nruntime={args.runtime}\nsdk={args.runtime}\ncommand=sharing-probe\n')
        (app / 'files/share/applications' / (APP + '.desktop')).write_text(
            f'[Desktop Entry]\nType=Application\nName=Canopa sandbox screen sharing probe\nExec=sharing-probe\n')
        settings = ['--env=PIPEWIRE_MODULE_DIR=/app/lib/pipewire-0.3',
                    '--env=SPA_PLUGIN_DIR=/app/lib/spa-0.2', '--env=PIPEWIRE_CONFIG_DIR=/app/share/pipewire']
        for name, arguments in [
            ('finish', ['build-finish', '--command=sharing-probe', *settings, str(app)]),
            ('export', ['build-export', str(store / 'repository'), str(app), '1']),
            ('install', ['install', '--user', '--or-update', '--no-deps', '--no-related', '--noninteractive',
                         '--assumeyes', 'canopa-fixture', 'app/' + APP + '/x86_64/1'])]:
            with output.with_name(output.stem + '-' + name + '.log').open('w') as log:
                answer = subprocess.run([*flatpak, *arguments], env=env, stdout=log, stderr=subprocess.STDOUT, timeout=45)
            result['commands'].append({'name': name, 'exit_code': answer.returncode})
            output.write_text(json.dumps(result, indent=2) + '\n')
            if answer.returncode: raise SystemExit(answer.returncode)
    deployed = store / 'installation/app' / APP / 'x86_64/1/active/files'
    assert digest(deployed / 'bin/sharing-probe') == build['sha256']
    for source, target in LIBRARIES.items(): assert digest(deployed / target) == libs[source]
    assert runtime.resolve().name == result['runtime_commit']
    result.update(status='passed', store_after=allocated(store), temporary_app_build_removed=True)
    assert result['store_after'] < 136 * 1024**2, result['store_after']
    output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({key: result[key] for key in ['status', 'runtime_files_added', 'store_after', 'temporary_app_build_removed']}))


if __name__ == '__main__': main()
