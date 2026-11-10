#!/usr/bin/env python3
"""Check relocatable IPP backend packaging and its matching client requirement."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TOOL = ROOT / 'scripts/package-print-backend.py'


def run(args):
    out = args.out.resolve()
    out.mkdir(parents=True)
    checks = []
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1', LC_ALL='C')
    for key in ['LD_LIBRARY_PATH', 'LD_PRELOAD', 'LD_AUDIT', 'CANOPA_PRINT_CUPS_LIBRARY']:
        env.pop(key, None)

    def invoke(bundle, *options, refusal=None):
        result = subprocess.run(['python3', str(TOOL), str(bundle), *map(str, options)],
                                env=env, cwd=ROOT, capture_output=True, text=True, timeout=30)
        with (out / 'checks.log').open('a') as log:
            log.write(result.stdout + result.stderr)
        if refusal:
            assert result.returncode == 1 and refusal in result.stderr, result.stderr
        else:
            assert result.returncode == 0, result.stderr
        return result

    with tempfile.TemporaryDirectory(prefix='canopa-backend-package-') as directory:
        base = Path(directory)
        empty = base / 'empty'
        invoke(empty, '--check')
        invoke(empty, '--check', '--required', refusal='Missing packaged IPP backend')
        checks.append('optional-absence-and-required-refusal')
        invoke(empty, '--build-receipt', args.build_receipt.resolve(), refusal='Missing packaged Print runtime')
        assert not empty.exists()
        checks.append('matching-client-required-before-staging')
        bundle = base / 'prefix'
        subprocess.run(['python3', str(ROOT / 'scripts/package-print-runtime.py'), str(bundle),
                        '--build-receipt', str(args.client_build.resolve())], env=env, cwd=ROOT,
                       capture_output=True, text=True, check=True, timeout=30)
        invoke(bundle, '--build-receipt', args.build_receipt.resolve())
        invoke(bundle, '--check', '--required')
        manifest = json.loads((bundle / 'share/canopa/print-backend.json').read_text())
        assert str(ROOT) not in json.dumps(manifest) and str(base) not in json.dumps(manifest)
        relocated = base / 'relocated prefix'
        bundle.rename(relocated)
        bundle = relocated
        invoke(bundle, '--check', '--required')
        checks.append('complete-relocatable-backend-client-and-notices')
        binary = bundle / manifest['backend']['path']
        discovery = subprocess.run([str(binary)], env=dict(env, HOME=str(base),
                                   CUPS_SERVER=str(base / 'no-scheduler.sock'), LD_DEBUG='libs'),
                                   capture_output=True, text=True, check=True, timeout=10)
        assert discovery.stdout.startswith('network ipp ')
        expected = 'calling init: ' + str(bundle / 'lib/canopa/print/libcups.so.2')
        assert expected in discovery.stderr, discovery.stderr
        (out / 'loader.log').write_text(discovery.stderr)
        checks.append('actual-default-loader-selects-relocated-sibling-client')
        isolated = base / 'isolated/ipp'
        isolated.parent.mkdir()
        shutil.copyfile(binary, isolated)
        isolated.chmod(0o700)
        refused = subprocess.run([str(isolated)], env=dict(env, HOME=str(base),
                                 CUPS_SERVER=str(base / 'no-scheduler.sock'), LD_LIBRARY_PATH='/usr/lib'),
                                 capture_output=True, text=True, timeout=10)
        assert refused.returncode != 0 and not refused.stdout
        assert 'canopaCupsAuthenticationVersion' in refused.stderr, refused.stderr
        (out / 'missing-client.log').write_text(refused.stderr)
        checks.append('stock-client-fallback-refused-before-discovery-or-private-API-use')
        invoke(bundle, '--build-receipt', args.build_receipt.resolve(), refusal='destination already exists')
        checks.append('existing-backend-refused')
        for name in manifest['files']:
            path = bundle / name
            original = path.read_bytes()
            path.write_bytes(original + b'changed')
            invoke(bundle, '--check', '--required', refusal='differs from its inventory')
            path.write_bytes(original)
        checks.append('binary-changes-and-original-notice-tampering-refused')
        path = bundle / 'share/canopa/print-backend.json'
        original = path.read_bytes()
        path.write_text(json.dumps({**manifest, 'client_sha256': '0' * 64}))
        invoke(bundle, '--check', '--required', refusal='backend and client do not match')
        path.write_bytes(original)
        checks.append('different-client-binding-refused')
        backend = bundle / manifest['backend']['path']
        original = backend.read_bytes()
        external = base / 'external-backend'
        external.write_bytes(original)
        backend.unlink()
        backend.symlink_to(external)
        invoke(bundle, '--check', '--required', refusal='Expected a regular file')
        backend.unlink()
        backend.write_bytes(original)
        backend.chmod(0o755)
        checks.append('symlink-backend-refused')
        if args.receiver:
            process = subprocess.run(['python3', str(ROOT / 'scripts/test-print-ipp-backend.py'),
                                      '--backend', str(backend), '--library', str(bundle / 'lib/canopa/print/libcups.so.2'),
                                      '--receiver', str(args.receiver.resolve()), '--out', str(out / 'native')],
                                     env=env, cwd=ROOT, capture_output=True, text=True, timeout=180)
            (out / 'native.log').write_text(process.stdout + process.stderr)
            assert process.returncode == 0, process.stdout + process.stderr
            checks.append('actual-relocated-backend-five-owned-IPP-cases')
        invoke(bundle, '--check', '--required')
    result = dict(status='passed', checks=checks, private_temporary_data_removed=not Path(directory).exists(),
                  backend_sha256=manifest['backend']['sha256'], client_sha256=manifest['client_sha256'])
    (out / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build-receipt', type=Path, required=True)
    parser.add_argument('--client-build', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--receiver', type=Path, help='Also exercise five actual owned IPP jobs after relocation')
    run(parser.parse_args())
