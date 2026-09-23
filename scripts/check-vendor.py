#!/usr/bin/env python3
"""Check layer-shell snapshots, optionally against downloaded release archives."""

import argparse
import hashlib
import json
from pathlib import Path
import tarfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def compare(label, expected, actual):
    errors = []
    for path in sorted(expected.keys() | actual.keys()):
        if path not in actual:
            errors.append(f"{label}: missing {path}")
        elif path not in expected:
            errors.append(f"{label}: unexpected {path}")
        elif actual[path] != expected[path]:
            errors.append(f"{label}: changed {path}")
    return errors


def check(root, archives):
    manifest = json.loads((root / "vendor/provenance.json").read_text())
    errors = []
    for name, record in manifest["crates"].items():
        local = {}
        for path in sorted((root / "vendor" / name).rglob("*")):
            relative = path.relative_to(root / "vendor" / name).as_posix()
            if path.is_symlink():
                errors.append(f"{name}: unexpected symlink {relative}")
            elif path.is_file():
                local[relative] = digest(path.read_bytes())
        expected = {
            path: hashes["local"]
            for path, hashes in record["files"].items()
            if hashes["local"] is not None
        }
        errors.extend(compare(name, expected, local))
        if archives is not None:
            prefix = f"{name}-{record['version']}/"
            archive = archives / f"{name}-{record['version']}.crate"
            if digest(archive.read_bytes()) != record["archive_sha256"]:
                errors.append(f"{name}: archive checksum mismatch")
                continue
            upstream = {}
            with tarfile.open(archive) as contents:
                for member in contents:
                    if member.isdir():
                        continue
                    if not member.isfile() or not member.name.startswith(prefix):
                        errors.append(f"{name}: unexpected archive member {member.name}")
                        continue
                    upstream[member.name.removeprefix(prefix)] = digest(
                        contents.extractfile(member).read()
                    )
            expected = {
                path: hashes["upstream"]
                for path, hashes in record["files"].items()
                if hashes["upstream"] is not None
            }
            errors.extend(compare(f"{name} upstream", expected, upstream))
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--archives", type=Path,
        help="also verify NAME-VERSION.crate files in this directory (no downloads)",
    )
    args = parser.parse_args()
    try:
        errors = check(Path(__file__).resolve().parents[1], args.archives)
    except (OSError, ValueError, tarfile.TarError) as error:
        parser.exit(1, f"Vendor verification failed: {error}\n")
    if errors:
        parser.exit(1, "\n".join(errors) + "\n")
    print("Layer-shell vendor snapshots verified" + (
        " against published archives." if args.archives else " against provenance manifest."
    ))


if __name__ == "__main__":
    main()
