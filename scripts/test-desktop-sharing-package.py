#!/usr/bin/env python3
"""Exercise optional sharing packaging with real build receipts and relocation."""
import argparse
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import traceback

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('sharing_package', ROOT / 'scripts/package-desktop-sharing.py')
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


def run(args, result):
    def passed(name):
        result['checks'].append(name)
        print(name, flush=True)

    def refused(action):
        try:
            action()
        except (ValueError, OSError, KeyError):
            return
        raise AssertionError('Invalid or incomplete component was accepted')

    with tempfile.TemporaryDirectory(prefix='canopa-sharing-package-') as directory:
        root = Path(directory)
        bundle = root / 'bundle'
        package.check(bundle)
        refused(lambda: package.check(bundle, required=True))
        passed('optional-absence-accepted-required-absence-rejected')

        original = json.loads(args.niri_build.read_text())
        for label, edit in [
                ('old-capture-recipe', lambda r: r.update(overlay='niri-desktop-compat')),
                ('stale-recipe', lambda r: r.update(recipe_sha256='0' * 64)),
                ('wrong-executable', lambda r: r.update(binary_sha256='0' * 64)),
                ('changed-source', lambda r: r['inputs']['niri'].update({'Cargo.toml': '0' * 64}))]:
            changed = copy.deepcopy(original)
            edit(changed)
            receipt = root / (label + '.json')
            receipt.write_text(json.dumps(changed))
            refused(lambda: package.stage(bundle, receipt, args.frontend_build))
            assert not bundle.exists(), 'Refused build must not create a partial component'
            passed(label + '-refused-before-staging')

        half = subprocess.run(['bash', str(ROOT / 'scripts/package-release.sh'),
                               str(root / 'no-build'), str(root / 'no-output'),
                               '--desktop-niri', str(args.niri_build)],
                              cwd=ROOT, capture_output=True, text=True)
        assert half.returncode == 2 and 'must be supplied together' in half.stderr, half
        assert not (root / 'no-output').exists()
        passed('release-requires-both-companions-before-staging')
        extra_only = subprocess.run(['bash', str(ROOT / 'scripts/package-release.sh'),
                                     str(root / 'no-build'), str(root / 'no-output'), '--extra-portals'],
                                    cwd=ROOT, capture_output=True, text=True)
        assert extra_only.returncode == 2 and '--extra-portals requires' in extra_only.stderr, extra_only
        assert not (root / 'no-output').exists()
        passed('extra-portals-require-companion-receipts-before-staging')

        package.stage(bundle, args.niri_build, args.frontend_build)
        refused(lambda: package.check(bundle, require_extra_portals=True))
        passed('base-sharing-package-does-not-claim-extra-portals')
        relocated = root / 'relocated prefix'
        bundle.rename(relocated)
        package.check(relocated, required=True)
        for kind, relative in package.EXECUTABLES.items():
            version = subprocess.check_output([str(relocated / relative), '--version'], text=True, timeout=15).strip()
            result.setdefault('versions', {})[kind] = version
        passed('real-companions-load-after-relocation-to-path-with-spaces')

        policy = relocated / package.PROFILE / 'wireplumber/canopa-screencast.lua'
        policy_bytes = policy.read_bytes()
        policy.unlink()
        refused(lambda: package.check(relocated, required=True))
        policy.write_bytes(policy_bytes)
        package.check(relocated, required=True)
        passed('missing-screencast-access-policy-refused')

        frontend = relocated / package.EXECUTABLES['frontend']
        hidden = frontend.with_suffix('.saved')
        frontend.rename(hidden)
        refused(lambda: package.check(relocated, required=True))
        hidden.rename(frontend)
        passed('missing-frontend-refused')

        with frontend.open('r+b') as stream:
            stream.seek(-1, 2)
            byte = stream.read(1)
            stream.seek(-1, 2)
            stream.write(bytes([byte[0] ^ 1]))
        refused(lambda: package.check(relocated, required=True))
        with frontend.open('r+b') as stream:
            stream.seek(-1, 2)
            stream.write(byte)
        passed('changed-executable-refused')

        descriptor = relocated / package.DESCRIPTOR
        manifest_path = relocated / package.MANIFEST
        saved_descriptor, saved_manifest = descriptor.read_bytes(), manifest_path.read_bytes()
        descriptor.write_bytes(saved_descriptor.replace(b'org.freedesktop.impl.portal.RemoteDesktop;', b''))
        manifest = json.loads(saved_manifest)
        manifest['files'][str(package.DESCRIPTOR)] = package.digest(descriptor)
        manifest_path.write_text(json.dumps(manifest))
        refused(lambda: package.check(relocated, required=True))
        descriptor.write_bytes(saved_descriptor)
        manifest_path.write_bytes(saved_manifest)
        package.check(relocated, required=True)
        passed('self-consistent-but-incomplete-routing-refused')
        result['manifest'] = json.loads(saved_manifest)
        incomplete = copy.deepcopy(result['manifest']['components'])
        incomplete['niri']['overlay'] = 'niri-desktop-pointer'
        refused(lambda: package.assets(incomplete, package.INTERFACES + package.EXTRA_INTERFACES))
        passed('extra-portals-refuse-a-compositor-without-shortcut-and-capture-leases')
        if original['overlay'] in package.CAPTURE_SHORTCUT_OVERLAYS:
            extra = root / 'extra'
            package.stage(extra, args.niri_build, args.frontend_build, extra_portals=True)
            package.check(extra, require_extra_portals=True)
            manifest = json.loads((extra / package.MANIFEST).read_text())
            assert manifest['interfaces'] == list(package.INTERFACES + package.EXTRA_INTERFACES)
            fragment = (extra / package.PROFILE / 'portals.conf.fragment').read_text()
            assert 'default=' not in fragment and 'org.freedesktop.impl.portal.Print=' not in fragment
            result['extra_manifest'] = manifest
            passed('all-six-optional-interfaces-and-explicit-routing-verified')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--niri-build', required=True, type=Path)
    parser.add_argument('--frontend-build', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    args.niri_build = args.niri_build.resolve(strict=True)
    args.frontend_build = args.frontend_build.resolve(strict=True)
    if args.output.exists():
        parser.error('--output must be a new receipt path')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    result = dict(status='running', checks=[])
    try:
        run(args, result)
        result['status'] = 'passed'
    except Exception:
        result.update(status='failed', traceback=traceback.format_exc())
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
    if result['status'] != 'passed':
        print(result['traceback'])
        raise SystemExit(1)


if __name__ == '__main__':
    main()
