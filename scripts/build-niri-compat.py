#!/usr/bin/env python3
"""Build the pinned Niri compatibility companion without installing it.

Run from the Canopa workspace root. This deliberately builds Niri's independent
upstream workspace from disposable prepared sources. All Cargo outputs remain in
Canopa's target/ or an explicit CARGO_TARGET_DIR. --fetch retrieves locked
dependencies before the offline build; --resume verifies prepared sources again.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tarfile

import vendor_sources
from niri_source_bindings import bind_sources

ROOT = Path(__file__).resolve().parents[1]


def resume_sources(archives, work, record, receipt):
    """Resume only the exact prepared source tree; never trust a receipt alone."""
    errors = vendor_sources.check(ROOT)
    if errors:
        raise ValueError("\n".join(errors))
    recipe_hash = vendor_sources.digest(json.dumps(record, sort_keys=True).encode())
    if receipt.get("recipe_sha256") != recipe_hash:
        raise ValueError("The source recipe changed; select a new work directory")
    sources = {source: work / spec["archive_root"] for source, spec in record["sources"].items()}
    for source, spec in record["sources"].items():
        archive = archives / spec["archive"]
        if vendor_sources.digest(archive.read_bytes()) != spec["archive_sha256"]:
            raise ValueError(f"{source}: archive checksum mismatch")
        base = work / spec["archive_root"]
        if receipt["sources"].get(source) != str(base):
            raise ValueError("Prepared source location differs from its receipt")
        patched = {path: hashes["patched"] for patch in record["patches"] if patch["source"] == source for path, hashes in patch["files"].items()}
        with tarfile.open(archive) as contents:
            if source == "niri":
                originals = tuple(contents.extractfile(spec["archive_root"] + "/" + name).read().decode() for name in ("Cargo.toml", "Cargo.lock"))
                binding = bind_sources(sources, record, originals, write=False)
                if binding != receipt["binding"]:
                    raise ValueError("Prepared source bindings differ from the canonical transformation")
            expected_files = set()
            for member in contents:
                if not (member.isfile() or member.issym()):
                    continue
                relative = member.name.removeprefix(spec["archive_root"] + "/")
                expected_files.add(relative)
                if member.issym():
                    path = base / relative
                    if not path.is_symlink() or os.readlink(path) != member.linkname:
                        raise ValueError(f"Prepared source symlink changed: {source}/{relative}")
                    continue
                expected = patched.get(relative) or vendor_sources.digest(contents.extractfile(member).read())
                if source == "niri" and relative in ("Cargo.toml", "Cargo.lock"):
                    key = "manifest" if relative == "Cargo.toml" else "lock"
                    expected = binding[f"{key}_prepared_sha256"]
                path = base / relative
                if path.is_symlink() or vendor_sources.digest(path.read_bytes()) != expected:
                    raise ValueError(f"Prepared source changed: {source}/{relative}")
            actual_files = {str(path.relative_to(base)) for path in base.rglob("*") if path.is_file() or path.is_symlink()}
            if actual_files != expected_files:
                raise ValueError(f"Prepared source has unexpected files: {source}")
        sources[source] = base
    return sources


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archives", type=Path, required=True, help="Directory containing the pinned upstream tar.gz archives")
    parser.add_argument("--work-dir", type=Path, required=True, help="Prepared sources beneath the Cargo target root or /tmp")
    build_root = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
    parser.add_argument("--target-dir", type=Path, default=build_root / "niri-compat")
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--release", action="store_true", help="Build the optimized compositor for distribution")
    parser.add_argument("--fetch", action="store_true", help="Fetch missing locked dependencies before the offline build")
    parser.add_argument("--resume", action="store_true", help="Verify and reuse the exact existing prepared sources")
    args = parser.parse_args()
    work = args.work_dir.resolve()
    roots = {(ROOT / "target").resolve(), build_root}
    if Path("/") in roots:
        parser.error("the Cargo target root cannot be /")
    if not (any(work.is_relative_to(root) for root in roots) or work.is_relative_to(Path("/tmp"))):
        parser.error("--work-dir must be beneath the workspace/configured target or /tmp")
    if not any(args.target_dir.resolve().is_relative_to(root) for root in roots):
        parser.error("--target-dir must be beneath the workspace/configured target")
    name = "niri-source-cancellation"
    record = vendor_sources.records(ROOT)[name]
    receipt_path = work / "canopa-build.json"
    if args.resume:
        previous = json.loads(receipt_path.read_text())
        sources = resume_sources(args.archives.resolve(), work, record, previous)
        binding = previous["binding"]
    else:
        sources = vendor_sources.prepare(ROOT, args.archives.resolve(), work, name)
        binding = bind_sources(sources, record)
    receipt = {"overlay": name, "profile": "release" if args.release else "debug", "sources": {key: str(value) for key, value in sources.items()}, "binding": binding,
               "recipe_sha256": vendor_sources.digest(json.dumps(record, sort_keys=True).encode()), "installed": False}
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2), flush=True)
    if args.prepare_only:
        return
    env = os.environ.copy()
    env.update(CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0", CARGO_TARGET_DIR=str(args.target_dir.resolve()),
               GIT_CEILING_DIRECTORIES=str(work))
    if args.fetch:
        subprocess.run(["bash", str(ROOT / "scripts/cargo.sh"), "fetch", "--manifest-path", str(sources["niri"] / "Cargo.toml"), "--locked"], cwd=ROOT, env=env, check=True)
    command = ["bash", str(ROOT / "scripts/cargo.sh"), "build", "--manifest-path", str(sources["niri"] / "Cargo.toml"), "-p", "niri", "--locked", "--offline", "-j1"]
    if args.release:
        env["CARGO_PROFILE_RELEASE_STRIP"] = "symbols"
        command.append("--release")
    subprocess.run(command, cwd=ROOT, env=env, check=True)
    binary = args.target_dir.resolve() / ("release" if args.release else "debug") / "niri"
    receipt["binary"] = str(binary)
    receipt["binary_sha256"] = vendor_sources.digest(binary.read_bytes())
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"Built private companion: {binary}", flush=True)


if __name__ == "__main__":
    main()
