#!/usr/bin/env python3
"""Build the Canopa compositor from the pinned kevinard/niri fork without installing it.

Run from the Canopa root. The fork's GitHub archive is verified against
packaging/forks.json and extracted into --work-dir; Cargo builds Niri's own
workspace with its lockfile (--locked), which pins Smithay to the
kevinard/smithay fork by git revision. --fetch downloads locked dependencies
before the offline build. --ref selects another fork branch, tag or commit
(for example a canopa/* topic) for native comparisons; such a build is
recorded as unpinned and cannot be packaged. --reuse keeps the same verified
source paths and incremental cache, and refuses any changed input.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tomllib

import fork_sources

ROOT = Path(__file__).resolve().parents[1]
FORK = "niri"


def cargo_git_sources(lock_text, pinned=None):
    """Locked git packages; for the pinned build they must be exactly the pinned forks."""
    found = {}
    for package in tomllib.loads(lock_text).get("package", []):
        source = package.get("source", "")
        if not source.startswith("git+"):
            continue
        location, _, commit = source.removeprefix("git+").partition("#")
        url, _, query = location.partition("?")
        found[package["name"]] = {"repository": url.removesuffix(".git"), "commit": commit, "query": query}
    if pinned is not None:
        expected = {}
        for name, spec in pinned.items():
            for package in spec["packages"]:
                expected[package] = {"repository": spec["repository"], "commit": spec["commit"],
                                     "query": "rev=" + spec["commit"]}
        if found != expected:
            raise ValueError(f"Niri's lockfile git sources differ from the pinned forks: {found}")
    return found


def git_notices(source, env, git_sources, pinned=None):
    """License texts at the root of each locked git checkout, e.g. Smithay's MIT text."""
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--offline", "--manifest-path",
         str(source / "Cargo.toml")], cwd=source, env=env, text=True))
    roots = {}
    for package in metadata["packages"]:
        if package["name"] in git_sources:
            repository = git_sources[package["name"]]["repository"]
            directory = Path(package["manifest_path"]).parent
            if repository not in roots or len(directory.parts) < len(roots[repository].parts):
                roots[repository] = directory
    result = {}
    for repository, directory in sorted(roots.items()):
        name = next((key for key, spec in (pinned or {}).items() if spec["repository"] == repository),
                    repository.rsplit("/", 1)[-1])
        expected = pinned[name]["notices"] if pinned else None
        for file, record in fork_sources.notices(directory, expected).items():
            result[f"{name}/{file}"] = record
    return result


def reuse_prepared(work, source, record, receipt):
    """Reuse only the identical commit, archive, source path and file inventory."""
    if (receipt.get("fork") != FORK or receipt.get("origin") != record
            or receipt.get("source") != str(source)):
        raise ValueError("Prepared sources belong to a different fork commit or location")
    expected = fork_sources.read_inputs(work / "canopa-build.json", receipt)
    if fork_sources.inventory(source) != expected:
        raise ValueError("Prepared source inputs changed; refusing reuse")
    return expected


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--archives", type=Path, required=True, help="Directory holding the fork source archive")
    parser.add_argument("--download", action="store_true", help="Download a missing pinned archive")
    parser.add_argument("--ref", help="Build another fork branch, tag or commit instead of the pin")
    parser.add_argument("--work-dir", type=Path, required=True)
    build_root = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
    parser.add_argument("--target-dir", type=Path, default=build_root / "niri-fork")
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--reuse", action="store_true", help="Validate and reuse unchanged prepared sources")
    parser.add_argument("--fetch", action="store_true", help="Fetch missing locked dependencies")
    parser.add_argument("--release", action="store_true", help="Build the optimized, stripped distribution executable")
    parser.add_argument("--toolchain", help="Explicit Rustup toolchain for a controlled comparison")
    args = parser.parse_args()
    try:
        work = fork_sources.check_work_dir(args.work_dir)
    except ValueError as error:
        parser.error(str(error))
    target = args.target_dir.resolve()
    if not any(target.is_relative_to(root) for root in fork_sources.work_roots()):
        parser.error("--target-dir must be beneath the workspace/configured target")
    pin = fork_sources.pin(FORK)
    record, archive = fork_sources.origin(FORK, args.archives.resolve(), args.ref, args.download)
    source = work / record["archive_root"]
    path = work / "canopa-build.json"
    if args.reuse:
        receipt = json.loads(path.read_text())
        inputs = reuse_prepared(work, source, record, receipt)
    else:
        if source.exists():
            raise ValueError(f"{source} already exists; use --reuse or a new --work-dir")
        fork_sources.extract(archive, work, record["archive_root"])
        inputs = fork_sources.inventory(source)
        if inputs != fork_sources.archive_inventory(archive, record["archive_root"]):
            raise ValueError("Extracted sources differ from the verified archive")
        receipt = {"fork": FORK, "origin": record, "source": str(source),
                   "source_file_count": len(inputs), "source_inputs_sha256": fork_sources.write_inputs(work, inputs),
                   "jobs": 1, "installed": False}
    lock = (source / "Cargo.lock").read_text()
    pinned_git = pin["cargo_git"] if record["pinned"] else None
    receipt.update(lock_sha256=fork_sources.digest(lock.encode()),
                   cargo_git=cargo_git_sources(lock, pinned_git),
                   profile="release" if args.release else "dev", incremental=not args.release,
                   build_commit=record["commit"], status="prepared")
    for stale in ("exit_code", "binary", "binary_sha256", "notices", "command"):
        receipt.pop(stale, None)
    if record["pinned"]:
        fork_sources.notices(source, pin["notices"])
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    if args.prepare_only:
        print(json.dumps(receipt, indent=2))
        return
    env = os.environ.copy()
    env.update(CARGO_INCREMENTAL="0" if args.release else "1", NIRI_BUILD_COMMIT=receipt["build_commit"],
               GIT_CEILING_DIRECTORIES=str(work))
    if args.toolchain:
        env["RUSTUP_TOOLCHAIN"] = args.toolchain
    receipt["compiler"] = subprocess.check_output(["rustc", "--version"], cwd=source, env=env, text=True).strip()
    cargo = ["bash", str(ROOT / "scripts/cargo.sh")]
    if args.fetch:
        subprocess.run([*cargo, "fetch", "--locked"], cwd=source, env=env, check=True)
    command = [*cargo, "build", "--locked", "--offline", "-p", "niri", "--bin", "niri", "--jobs", "1",
               "--target-dir", str(target)]
    if args.release:
        env["CARGO_PROFILE_RELEASE_STRIP"] = "symbols"
        command.append("--release")
    receipt.update(status="building", command=command)
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    result = subprocess.run(command, cwd=source, env=env)
    receipt.update(status="failed", exit_code=result.returncode)
    if result.returncode == 0:
        if fork_sources.inventory(source) != inputs:
            raise RuntimeError("Prepared sources changed during the build")
        notices = {f"{FORK}/{name}": value for name, value in
                   fork_sources.notices(source, pin["notices"] if record["pinned"] else None).items()}
        notices.update(git_notices(source, env, receipt["cargo_git"], pinned_git))
        binary = target / ("release" if args.release else "debug") / "niri"
        receipt.update(status="built", notices=notices, binary=str(binary),
                       binary_sha256=fork_sources.file_digest(binary))
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2), flush=True)
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
