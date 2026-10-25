#!/usr/bin/env python3
"""Check shared dependency forks and patches against their recorded provenance."""

import argparse
import difflib
import hashlib
import json
from pathlib import Path
import tarfile

import vendor_sources


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
        patch = root / record["patch"]
        if patch.is_symlink() or digest(patch.read_bytes()) != record["patch_sha256"]:
            errors.append(f"{name}: patch differs from provenance record")
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
            source = {}
            with tarfile.open(archive) as contents:
                for member in contents:
                    if member.isdir():
                        continue
                    if not member.isfile() or not member.name.startswith(prefix):
                        errors.append(f"{name}: unexpected archive member {member.name}")
                        continue
                    relative = member.name.removeprefix(prefix)
                    data = contents.extractfile(member).read()
                    upstream[relative] = digest(data)
                    if relative.startswith("src/") or relative in {"Cargo.toml", "Cargo.toml.orig"}:
                        source[relative] = data.decode("utf-8")
            expected = {
                path: hashes["upstream"]
                for path, hashes in record["files"].items()
                if hashes["upstream"] is not None
            }
            errors.extend(compare(f"{name} upstream", expected, upstream))
            actual_patch = ""
            for relative in sorted(upstream.keys() | local.keys()):
                if (not relative.startswith("src/") and relative not in {"Cargo.toml", "Cargo.toml.orig"}) or upstream.get(relative) == local.get(relative):
                    continue
                path = root / "vendor" / name / relative
                before = source.get(relative, "")
                after = path.read_text() if path.is_file() else ""
                actual_patch += "".join(difflib.unified_diff(
                    before.splitlines(keepends=True), after.splitlines(keepends=True),
                    fromfile=f"a/{relative}", tofile=f"b/{relative}",
                ))
            if patch.read_text() != actual_patch:
                errors.append(f"{name}: patch does not describe the actual upstream source changes")
    errors.extend(vendor_sources.check(root, archives))
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--archives", type=Path,
        help="also verify published crate and pinned source archives in this directory (no downloads)",
    )
    args = parser.parse_args()
    try:
        errors = check(Path(__file__).resolve().parents[1], args.archives)
    except (OSError, ValueError, tarfile.TarError) as error:
        parser.exit(1, f"Vendor verification failed: {error}\n")
    if errors:
        parser.exit(1, "\n".join(errors) + "\n")
    print("Shared dependency forks verified" + (
        " against published archives." if args.archives else " against provenance manifest."
    ))


if __name__ == "__main__":
    main()
