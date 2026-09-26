#!/usr/bin/env python3
"""Run fork library tests in a disposable workspace seeded from the root lockfile."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']
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
    with tempfile.TemporaryDirectory(prefix='canopa-vendor-tests-') as directory:
        snapshot = Path(directory)
        for name in ['iced_layershell', 'layershellev']:
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
            '[patch.crates-io]\nlayershellev = { path = "layershellev" }\n'
            '[workspace.dependencies.iced]\n'
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
            'cargo', 'test', '--locked', '--offline', '--manifest-path', str(snapshot / 'Cargo.toml'),
            '-p', 'iced_layershell', '-p', 'layershellev', '--lib',
        ], cwd=ROOT, env=env, check=True)


if __name__ == '__main__':
    main()
