#!/usr/bin/env python3
"""Run fork library tests at stable source paths, seeded from the root lockfile."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib

from vendor_test_workspace import sync_tree, workspace as test_workspace, write_if_changed

ROOT = Path(__file__).resolve().parents[1]


def main():
    root_manifest = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    workspace = root_manifest['workspace']
    iced = workspace['dependencies']['iced'].copy()
    features = set(iced['features'])
    for member in workspace['members']:
        manifest = tomllib.loads((ROOT / member / 'Cargo.toml').read_text())
        dependency = manifest.get('dependencies', {}).get('iced', {})
        if isinstance(dependency, dict):
            features.update(dependency.get('features', []))
    locked = tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']
    locked_registry = {(p['name'], p['version'], p.get('source')) for p in locked}
    env = os.environ.copy()
    env['CARGO_TARGET_DIR'] = str(Path(env.get('CARGO_TARGET_DIR', ROOT / 'target')).resolve())
    # The disposable workspace otherwise restores Cargo's full-debug defaults.
    env.setdefault('CARGO_PROFILE_DEV_DEBUG', root_manifest['profile']['dev']['debug'])
    with test_workspace('forks') as snapshot:
        forks = list(json.loads((ROOT / 'vendor/provenance.json').read_text())['crates'])
        # Upstream's macro integration test enables Iced's default desktop stack.
        # These library tests use the same Iced features as Canopa's combined build.
        manifest = ROOT / 'vendor/iced_layershell/Cargo.toml'
        original = '[dev-dependencies.iced]\nversion = "0.14"'
        text = manifest.read_text()
        if text.count(original) != 1:
            raise RuntimeError('Review the upstream Iced dev dependency before updating this fixture')
        for name in forks:
            overrides = {'Cargo.toml': text.replace(original, '[dev-dependencies.iced]\nworkspace = true').encode()} if name == 'iced_layershell' else {}
            sync_tree(ROOT / 'vendor' / name, snapshot / name, overrides=overrides)
        write_if_changed(snapshot / 'Cargo.toml', (
            '[workspace]\nresolver = "3"\nmembers = ["iced_layershell", "layershellev"]\n'
            f'exclude = {json.dumps([name for name in forks if name not in {"iced_layershell", "layershellev"}])}\n'
            '[patch.crates-io]\n'
            + ''.join(f'{name} = {{ path = "{name}" }}\n' for name in forks)
            + '[workspace.dependencies.iced]\n'
            f'version = {json.dumps(iced["version"])}\ndefault-features = false\n'
            f'features = {json.dumps(sorted(features))}\n').encode())
        write_if_changed(snapshot / 'Cargo.lock', (ROOT / 'Cargo.lock').read_bytes())
        # Cargo prunes unrelated workspace packages in this temporary lockfile.
        # Check the resulting active graph before executing any tests. Optional
        # inactive entries may be added by upstream's test-only dependency graph.
        metadata = json.loads(subprocess.check_output([
            'cargo', 'metadata', '--offline', '--format-version', '1',
            '--filter-platform', 'x86_64-unknown-linux-gnu',
            '--manifest-path', str(snapshot / 'Cargo.toml'),
        ], cwd=ROOT, env=env, text=True))
        resolved = {node['id'] for node in metadata['resolve']['nodes']}
        for package in metadata['packages']:
            key = (package['name'], package['version'], package['source'])
            if package['id'] in resolved and package['source'] and key not in locked_registry:
                raise RuntimeError(f'Vendor tests escaped the root locked graph: {key}')
        command = [
            'bash', str(ROOT / 'scripts/cargo.sh'), 'test', '--locked', '--offline', '--message-format=json', '-j1', '--manifest-path', str(snapshot / 'Cargo.toml'),
            '-p', 'iced_layershell', '-p', 'layershellev', '--lib',
        ]
        futures_library = None
        with subprocess.Popen(command, cwd=ROOT, env=env, stdout=subprocess.PIPE, text=True) as process:
            for line in process.stdout:
                try:
                    message = json.loads(line)
                except ValueError:
                    print(line, end='', flush=True)
                    continue
                if message.get('reason') == 'compiler-artifact' and message.get('target', {}).get('name') == 'iced_futures':
                    libraries = [Path(p) for p in message['filenames'] if p.endswith('.rlib')]
                    if len(libraries) == 1:
                        futures_library = libraries[0]
                elif message.get('reason') == 'compiler-message':
                    print(message['message'].get('rendered', ''), end='', flush=True)
            code = process.wait()
        if code:
            raise subprocess.CalledProcessError(code, command)
        # This adapter seam is std-only. Exercise its actual source without
        # introducing an excluded crate's optional test dependency graph.
        event_tests = snapshot / 'iced-winit-event-batch-tests'
        subprocess.run([
            'rustc', '--edition=2024', '--test',
            str(snapshot / 'iced_winit/src/event_batch.rs'), '-o', str(event_tests),
        ], cwd=ROOT, env=env, check=True)
        subprocess.run([str(event_tests)], cwd=ROOT, env=env, check=True)
        # Use the actual pure policy and runtime source with the existing cached
        # futures library, without introducing excluded crates' dev graphs.
        policy_tests=snapshot/'winit-shortcut-policy-tests'
        subprocess.run(['rustc','--edition=2021','--test',str(snapshot/'winit/src/platform_impl/linux/wayland/keyboard_shortcuts/policy.rs'),'-o',str(policy_tests)],cwd=ROOT,env=env,check=True)
        subprocess.run([str(policy_tests)],cwd=ROOT,env=env,check=True)
        recording_tests=snapshot/'winit-shortcut-recording-tests'
        subprocess.run(['rustc','--edition=2021','--test',str(snapshot/'winit/src/platform_impl/linux/wayland/keyboard_shortcuts/recording.rs'),'-o',str(recording_tests)],cwd=ROOT,env=env,check=True)
        subprocess.run([str(recording_tests)],cwd=ROOT,env=env,check=True)
        dependencies=Path(env['CARGO_TARGET_DIR'])/'debug/deps'
        if futures_library is None or not futures_library.is_file() or not futures_library.resolve().is_relative_to(dependencies.resolve()):
            raise RuntimeError('Cargo did not identify the current locked futures library for the shortcut tests')
        shortcut_entry=snapshot/'iced-runtime-shortcut-tests.rs'
        write_if_changed(shortcut_entry,('pub extern crate iced_futures as futures;\n#[path = '+json.dumps(str(snapshot/'iced_runtime/src/window/keyboard_shortcuts.rs'))+']\nmod keyboard_shortcuts;\n').encode())
        shortcut_tests=snapshot/'iced-runtime-shortcut-tests'
        subprocess.run(['rustc','--edition=2021','--test',str(shortcut_entry),'--extern','iced_futures='+str(futures_library),'-L','dependency='+str(dependencies),'-o',str(shortcut_tests)],cwd=ROOT,env=env,check=True)
        subprocess.run([str(shortcut_tests)],cwd=ROOT,env=env,check=True)
        subprocess.run([sys.executable, str(ROOT / 'scripts/test-vendor-pipes.py')],
                       cwd=ROOT, env=env, check=True)


if __name__ == '__main__':
    main()
