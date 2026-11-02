#!/usr/bin/env python3
"""Prepare a checksummed Arch source package from this checkout; optionally build."""
import argparse
import gzip
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def prepare(output):
    version = tomllib.loads((ROOT / 'crates/canopa-settings/Cargo.toml').read_text())['package']['version']
    output.mkdir(parents=True, exist_ok=False)
    archive = output / f'canopa-{version}.tar.gz'
    epoch = int(subprocess.check_output(['git', 'show', '-s', '--format=%ct', 'HEAD'], cwd=ROOT, text=True))
    files = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=ROOT).decode().split('\0')
    with archive.open('wb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=epoch) as compressed, tarfile.open(fileobj=compressed, mode='w|') as bundle:
        for name in sorted(set(files) - {''}):
            if name.startswith(('.scratch/', 'target/', '.worktrees/', 'experiments/')):
                continue
            path = ROOT / name
            if path.resolve().is_relative_to(output):
                continue
            if not path.exists() and not path.is_symlink():
                continue
            info = bundle.gettarinfo(str(path), arcname=f'canopa-{version}/{name}')
            info.uid = info.gid = 0
            info.uname = info.gname = ''
            info.mtime = epoch
            with path.open('rb') if info.isfile() else io.BytesIO() as source:
                bundle.addfile(info, source if info.isfile() else None)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    recipe = (ROOT / 'packaging/arch/PKGBUILD.in').read_text().replace('@VERSION@', version).replace('@SOURCE_SHA256@', digest)
    (output / 'PKGBUILD').write_text(recipe)
    result = subprocess.check_output(['makepkg', '--printsrcinfo'], cwd=output, text=True)
    (output / '.SRCINFO').write_text(result)
    print(f'Prepared {archive} (sha256 {digest})', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path, help='new package directory')
    parser.add_argument('--build', action='store_true', help='run makepkg without installing packages/dependencies')
    args = parser.parse_args()
    output = args.output.resolve()
    prepare(output)
    if args.build:
        env = dict(os.environ, CANOPA_PACKAGE_TARGET_DIR=str(ROOT / 'target'))
        subprocess.run(['python3', str(ROOT / 'scripts/build-guard.py'), '--path', str(output), '--', 'makepkg'], cwd=output, env=env, check=True)


if __name__ == '__main__':
    main()
