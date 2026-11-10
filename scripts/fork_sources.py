#!/usr/bin/env python3
"""Pinned GitHub fork sources for Canopa's separately built programs.

`packaging/forks.json` pins each fork commit, its GitHub source archive and the
archive's SHA-256. Builders verify an archive before extracting it into a
disposable source tree beneath target/ or /tmp, record that tree's file
inventory and refuse to reuse it after any change. Nothing here installs or
selects a program.

Run `python3 scripts/fork_sources.py fetch NAME... --archives DIR` to download
and verify the pinned archives.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "packaging/forks.json"
NOTICE_PREFIXES = ("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE")
# Fields of a pin that identify the source; receipts and provenance copy them.
ORIGIN_FIELDS = ("repository", "upstream", "branch", "commit", "base", "url", "archive", "archive_sha256")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def json_digest(value):
    return digest(json.dumps(value, sort_keys=True).encode())


def pins(manifest=None):
    return json.loads(Path(manifest or MANIFEST).read_text())


def pin(name, manifest=None):
    values = pins(manifest)
    if name not in values:
        raise ValueError(f"Unknown fork: {name}")
    return values[name]


def resolve_ref(repository, ref):
    """Resolve a fork branch or tag to a commit; a full commit is used as given."""
    if re.fullmatch(r"[0-9a-f]{40}", ref):
        return ref
    output = subprocess.check_output(["git", "ls-remote", repository + ".git", ref],
                                     text=True, timeout=60, env=dict(os.environ, GIT_TERMINAL_PROMPT="0"))
    matches = {line.split("\t")[1]: line.split("\t")[0] for line in output.splitlines() if "\t" in line}
    for candidate in (ref, "refs/heads/" + ref, "refs/tags/" + ref + "^{}", "refs/tags/" + ref):
        if candidate in matches:
            return matches[candidate]
    raise ValueError(f"{repository} has no ref {ref}")


def download(url, path, expected=None):
    """Download into the archive directory atomically, verifying a known checksum."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".download-", delete=False) as temporary:
        partial = Path(temporary.name)
        try:
            with urllib.request.urlopen(url, timeout=120) as source:
                while chunk := source.read(1024 * 1024):
                    temporary.write(chunk)
            temporary.flush()
            actual = file_digest(partial)
            if expected is not None and actual != expected:
                raise ValueError(f"{url}: checksum {actual} differs from the pinned {expected}")
            partial.chmod(0o644)
            partial.rename(path)
        finally:
            partial.unlink(missing_ok=True)


def archive_file(archives, archive, url, expected, fetch=False):
    path = Path(archives) / archive
    if not path.exists():
        if not fetch:
            raise ValueError(f"Missing source archive {path}; download it with --download")
        download(url, path, expected)
    actual = file_digest(path)
    if expected is not None and actual != expected:
        raise ValueError(f"{path.name}: archive checksum mismatch")
    return path, actual


def origin(name, archives, ref=None, fetch=False, manifest=None):
    """Return (origin record, archive path) for the pinned commit or an explicit fork ref.

    The pinned commit must match its recorded archive checksum. Another ref is
    resolved to a commit, downloaded if absent, and recorded as unpinned with
    the checksum of the archive actually used.
    """
    value = pin(name, manifest)
    commit = value["commit"] if ref is None else resolve_ref(value["repository"], ref)
    if commit == value["commit"]:
        record = {key: value[key] for key in ORIGIN_FIELDS}
        record.update(fork=name, ref=ref or value["branch"], pinned=True, archive_root=value["archive_root"])
    else:
        record = dict(fork=name, repository=value["repository"], upstream=value["upstream"], ref=ref,
                      commit=commit, url=f"https://codeload.github.com/{value['repository'].removeprefix('https://github.com/')}/tar.gz/{commit}",
                      archive=f"{name}-{commit}.tar.gz", archive_sha256=None, pinned=False,
                      archive_root=f"{name}-{commit}")
    path, actual = archive_file(archives, record["archive"], record["url"], record["archive_sha256"],
                                fetch or not record["pinned"])
    record["archive_sha256"] = actual
    return record, path


def _members(contents, root, prefixes=None):
    selected = []
    for member in contents:
        name = PurePosixPath(member.name)
        if name.is_absolute() or ".." in name.parts or name.parts[:1] != (root,):
            raise ValueError(f"Unexpected archive member outside {root}: {member.name}")
        if not (member.isfile() or member.isdir()):
            raise ValueError(f"Unsupported archive member type: {member.name}")
        relative = PurePosixPath(*name.parts[1:]) if len(name.parts) > 1 else None
        if prefixes is not None and (relative is None or not any(
                relative == PurePosixPath(prefix) or PurePosixPath(prefix) in relative.parents for prefix in prefixes)):
            continue
        selected.append((member, str(relative) if relative else None))
    return selected


def archive_inventory(archive, root, prefixes=None):
    """File checksums inside an archive without extracting it."""
    with tarfile.open(archive) as contents:
        return {relative: digest(contents.extractfile(member).read())
                for member, relative in _members(contents, root, prefixes) if member.isfile()}


def archive_member(archive, root, relative):
    with tarfile.open(archive) as contents:
        return contents.extractfile(f"{root}/{relative}").read()


def extract(archive, destination, root, prefixes=None):
    """Extract one verified archive root (or only PREFIXES within it) into DESTINATION."""
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive) as contents:
        members = [member for member, _ in _members(contents, root, prefixes)]
        contents.extractall(destination, members=members, filter="data")
    return destination / root


def inventory(source, ignore=()):
    """Checksums of every regular file in SOURCE; symlinks are refused."""
    source = Path(source)
    if source.is_symlink() or not source.is_dir():
        raise ValueError(f"Prepared source root must be a real directory: {source}")
    files = {}
    for path in source.rglob("*"):
        if path.is_symlink():
            raise ValueError(f"Unexpected symlink in prepared sources: {path}")
        if path.is_file():
            relative = str(path.relative_to(source))
            if relative not in ignore:
                files[relative] = file_digest(path)
    return files


def notices(source, expected=None):
    """Top-level license/notice files of SOURCE, checked against EXPECTED {name: sha256}."""
    source = Path(source)
    found = {path.name: file_digest(path) for path in sorted(source.iterdir())
             if path.is_file() and not path.is_symlink() and path.name.upper().startswith(NOTICE_PREFIXES)}
    if expected is not None and found != expected:
        raise ValueError(f"{source}: license and notice files differ from the pinned fork")
    if not found:
        raise ValueError(f"{source}: no license or notice files")
    return {name: {"path": str(source / name), "sha256": checksum} for name, checksum in found.items()}


def write_inputs(work, inputs):
    (Path(work) / "source-inputs.json").write_text(json.dumps(inputs, indent=2, sort_keys=True) + "\n")
    return json_digest(inputs)


def read_inputs(receipt_path, receipt):
    """Load and authenticate the source inventory written beside a build receipt."""
    inputs = json.loads((Path(receipt_path).parent / "source-inputs.json").read_text())
    if len(inputs) != receipt.get("source_file_count") or json_digest(inputs) != receipt.get("source_inputs_sha256"):
        raise ValueError("Source inventory differs from its build receipt")
    return inputs


def check_origin(record, name, manifest=None):
    """Require a receipt or manifest origin to be exactly the current pin."""
    value = pin(name, manifest)
    if (record.get("fork") != name or record.get("pinned") is not True
            or any(record.get(key) != value[key] for key in ORIGIN_FIELDS)):
        raise ValueError(f"{name} build is not the pinned fork commit {value['commit']}")
    return value


def work_roots():
    roots = {(ROOT / "target").resolve(), Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()}
    if Path("/") in roots:
        raise ValueError("the Cargo target root cannot be /")
    return roots


def check_work_dir(path, allow_tmp=True):
    path = Path(path).resolve()
    if not (any(path.is_relative_to(root) for root in work_roots())
            or (allow_tmp and path.is_relative_to(Path("/tmp")))):
        raise ValueError("--work-dir must be beneath the workspace/configured target or /tmp")
    return path


def fetch_pinned(name, archives, manifest=None):
    value = pin(name, manifest)
    paths = [archive_file(archives, value["archive"], value["url"], value["archive_sha256"], True)[0]]
    if "subprojects" in value:
        extra = value["subprojects"]
        paths.append(archive_file(archives, extra["archive"], extra["url"], extra["archive_sha256"], True)[0])
    return paths


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    fetch = commands.add_parser("fetch", help="download and verify pinned archives")
    fetch.add_argument("names", nargs="+", choices=sorted(pins()))
    fetch.add_argument("--archives", type=Path, required=True)
    args = parser.parse_args()
    try:
        for name in args.names:
            for path in fetch_pinned(name, args.archives):
                print(f"{path}: verified", flush=True)
    except (OSError, ValueError) as error:
        parser.exit(1, f"Fork sources: {error}\n")


if __name__ == "__main__":
    main()
