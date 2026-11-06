"""Integrity and materialization of pinned source overlays in vendor/provenance.json.

Canonical changes live in vendor/. Prepared upstream workspaces are disposable
build inputs, normally beneath target/, and never replace an installed program.
"""
import hashlib
import json
import re
from pathlib import Path
import subprocess
import tarfile
import tempfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def records(root):
    return json.loads((root / "vendor/provenance.json").read_text()).get("source_overlays", {})


def added_file(root, record, relative):
    """Extract an entire explicitly added source, verified against provenance."""
    patches = [patch for patch in record['patches'] if relative in patch['files']]
    if len(patches) != 1:
        raise ValueError('Expected exactly one canonical source addition')
    patch = patches[0]
    expected = patch['files'][relative]
    if expected['upstream'] is not None:
        raise ValueError('Source must be declared absent upstream')
    text = (root / patch['path']).read_text()
    marker = f'diff --git a/{relative} b/{relative}\n'
    if text.count(marker) != 1:
        raise ValueError('Expected one canonical source section')
    section = re.split(r'(?m)^diff --git ', text.split(marker)[1], maxsplit=1)[0]
    header, added = section.split('@@\n', 1)
    match = re.search(r'@@ -0,0 \+1(?:,(\d+))? $', header)
    if ('--- /dev/null\n' not in header or f'+++ b/{relative}\n' not in header or match is None):
        raise ValueError('Expected a complete new-file hunk')
    lines = added.splitlines(keepends=True)
    if len(lines) != int(match[1] or 1) or any(not line.startswith('+') for line in lines):
        raise ValueError('Incomplete source addition')
    source = ''.join(line[1:] for line in lines).encode()
    if digest(source) != expected['patched']:
        raise ValueError('Added source differs from canonical provenance')
    return source


def license_assets(spec):
    return [spec["license"], *spec.get("notices", [])]


def check(root, archives=None):
    errors = []
    for name, record in records(root).items():
        initial_errors = len(errors)
        try:
            originals = {}
            for source, spec in record["sources"].items():
                for license in license_assets(spec):
                    path = root / license["path"]
                    if path.is_symlink() or digest(path.read_bytes()) != license["sha256"]:
                        errors.append(f"{name}: {source} licence differs from provenance ({license['path']})")
                if archives is None:
                    continue
                archive = archives / spec["archive"]
                if digest(archive.read_bytes()) != spec["archive_sha256"]:
                    errors.append(f"{name}: {source} archive checksum mismatch")
                    continue
                prefix = spec["archive_root"] + "/"
                with tarfile.open(archive) as contents:
                    files = {member.name.removeprefix(prefix): contents.extractfile(member).read()
                             for member in contents if member.isfile() and member.name.startswith(prefix)}
                for license in license_assets(spec):
                    if digest(files[license["upstream_path"]]) != license["sha256"]:
                        errors.append(f"{name}: {source} licence does not match upstream ({license['upstream_path']})")
                originals[source] = files
            for patch in record["patches"]:
                path = root / patch["path"]
                if path.is_symlink() or digest(path.read_bytes()) != patch["sha256"]:
                    errors.append(f"{name}: patch differs from provenance")
                for relative, hashes in patch["files"].items():
                    if archives is not None and patch["source"] in originals:
                        original = originals[patch["source"]].get(relative)
                        actual = digest(original) if original is not None else None
                        if actual != hashes["upstream"]:
                            errors.append(f"{name}: {relative} upstream checksum mismatch")
            if archives is not None and len(errors) == initial_errors:
                with tempfile.TemporaryDirectory(prefix="canopa-vendor-source-check-") as directory:
                    _prepare(root, archives, Path(directory) / "sources", record)
        except (OSError, KeyError, ValueError, tarfile.TarError, subprocess.CalledProcessError) as error:
            errors.append(f"{name}: {error}")
    return errors


def prepare(root, archives, destination, name):
    """Verify and apply the canonical patch to a new disposable source directory."""
    errors = check(root)
    if errors:
        raise ValueError("\n".join(errors))
    record = records(root)[name]
    return _prepare(root, archives, destination, record)


def _prepare(root, archives, destination, record):
    destination.mkdir(parents=True, exist_ok=False)
    sources = {}
    for source, spec in record["sources"].items():
        archive = archives / spec["archive"]
        if digest(archive.read_bytes()) != spec["archive_sha256"]:
            raise ValueError(f"{source}: archive checksum mismatch")
        with tarfile.open(archive) as contents:
            prefix = spec["archive_root"]
            if any(member.name != prefix and not member.name.startswith(prefix + "/") for member in contents):
                raise ValueError(f"{source}: unexpected archive root")
            contents.extractall(destination, filter="data")
        sources[source] = destination / spec["archive_root"]
        for license in license_assets(spec):
            if digest((sources[source] / license["upstream_path"]).read_bytes()) != license["sha256"]:
                raise ValueError(f"{source}: upstream licence checksum mismatch ({license['upstream_path']})")
    for patch in record["patches"]:
        source = sources[patch["source"]]
        before = {str(path.relative_to(source)): digest(path.read_bytes())
                  for path in source.rglob("*") if path.is_file()}
        subprocess.run(["patch", "--batch", "--forward", "--fuzz=0", "-p1", "-i", str(root / patch["path"])],
                       cwd=source, check=True)
        after = {str(path.relative_to(source)): digest(path.read_bytes())
                 for path in source.rglob("*") if path.is_file()}
        changed = {path for path in before.keys() | after.keys() if before.get(path) != after.get(path)}
        if changed != set(patch["files"]):
            raise ValueError(f"Unexpected patch scope: {sorted(changed)}")
        for relative, hashes in patch["files"].items():
            if before.get(relative) != hashes["upstream"] or after.get(relative) != hashes["patched"]:
                raise ValueError(f"{relative}: patch result does not match provenance")
    return sources
