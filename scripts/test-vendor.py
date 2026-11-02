#!/usr/bin/env python3
"""Run fork library tests in a disposable workspace seeded from the root lockfile."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib

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
    with tempfile.TemporaryDirectory(prefix='canopa-vendor-tests-') as directory:
        snapshot = Path(directory)
        forks = list(json.loads((ROOT / 'vendor/provenance.json').read_text())['crates'])
        for name in forks:
            shutil.copytree(ROOT / 'vendor' / name, snapshot / name)
        # Upstream's macro integration test enables Iced's default desktop stack.
        # These library tests use the same Iced features as Canopa's combined build.
        manifest = snapshot / 'iced_layershell/Cargo.toml'
        original = '[dev-dependencies.iced]\nversion = "0.14"'
        text = manifest.read_text()
        if text.count(original) != 1:
            raise RuntimeError('Review the upstream Iced dev dependency before updating this fixture')
        manifest.write_text(text.replace(original, '[dev-dependencies.iced]\nworkspace = true'))
        (snapshot / 'Cargo.toml').write_text(
            '[workspace]\nresolver = "3"\nmembers = ["iced_layershell", "layershellev"]\n'
            f'exclude = {json.dumps([name for name in forks if name not in {"iced_layershell", "layershellev"}])}\n'
            '[patch.crates-io]\n'
            + ''.join(f'{name} = {{ path = "{name}" }}\n' for name in forks)
            + '[workspace.dependencies.iced]\n'
            f'version = {json.dumps(iced["version"])}\ndefault-features = false\n'
            f'features = {json.dumps(sorted(features))}\n')
        shutil.copyfile(ROOT / 'Cargo.lock', snapshot / 'Cargo.lock')
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
        subprocess.run([
            'bash', str(ROOT / 'scripts/cargo.sh'), 'test', '--locked', '--offline', '-j1', '--manifest-path', str(snapshot / 'Cargo.toml'),
            '-p', 'iced_layershell', '-p', 'layershellev', '--lib',
        ], cwd=ROOT, env=env, check=True)
        # This adapter seam is std-only. Exercise its actual source without
        # introducing an excluded crate's optional test dependency graph.
        event_tests = snapshot / 'iced-winit-event-batch-tests'
        subprocess.run([
            'rustc', '--edition=2024', '--test',
            str(snapshot / 'iced_winit/src/event_batch.rs'), '-o', str(event_tests),
        ], cwd=ROOT, env=env, check=True)
        subprocess.run([str(event_tests)], cwd=ROOT, env=env, check=True)
        subprocess.run([sys.executable, str(ROOT / 'scripts/test-vendor-pipes.py')],
                       cwd=ROOT, env=env, check=True)


if __name__ == '__main__':
    main()
