#!/usr/bin/env python3
"""Build the pinned screencast companion without installing it.

Run from the Canopa root. Cargo builds Niri's independent prepared workspace,
using its pinned dependency graph and incremental dev outputs beneath root target/.
The combined desktop recipe replaces only the two verified Smithay source bindings.
Fetch that upstream lockfile's dependencies before an offline build.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

import vendor_sources
from niri_source_bindings import bind_sources

ROOT = Path(__file__).resolve().parents[1]
OVERLAY = "niri-screencast-resume"


def source_inputs(sources):
    inputs = {}
    for name, source in sources.items():
        if source.is_symlink() or not source.is_dir():
            raise ValueError("Prepared source roots must be real directories")
        files = {}
        for path in source.rglob("*"):
            if path.is_symlink():
                raise ValueError("Unexpected symlink in prepared source")
            if path.is_file():
                files[str(path.relative_to(source))] = vendor_sources.digest(path.read_bytes())
        inputs[name] = files
    return inputs


def reuse_prepared(work, sources, overlay, record):
    receipt = json.loads((work / "canopa-build.json").read_text())
    if (receipt.get("overlay") != overlay or
            receipt.get("recipe_sha256") != vendor_sources.digest(json.dumps(record, sort_keys=True).encode()) or
            receipt.get("sources") != {name: str(path) for name, path in sources.items()}):
        raise ValueError("Prepared sources belong to a different recipe or location")
    if source_inputs(sources) != receipt.get("inputs"):
        raise ValueError("Prepared source inputs changed; refusing reuse")
    return receipt


def summary(receipt):
    return {**{key: value for key, value in receipt.items() if key != "inputs"},
            "input_counts": {name: len(files) for name, files in receipt["inputs"].items()}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archives", type=Path, required=True)
    parser.add_argument("--overlay", choices=(OVERLAY, "niri-desktop-compat", "niri-remote-input", "niri-desktop-pointer", "niri-input-capture", "niri-global-shortcuts", "niri-input-capture-keyboard", "niri-remote-touch"), default=OVERLAY,
                        help="Explicit recording, desktop, injected-input, or experimental input-capture recipe")
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target/niri-screencast-native")
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--reuse", action="store_true", help="Validate and reuse unchanged prepared sources and their Cargo package paths")
    parser.add_argument("--toolchain", help="Explicit Rustup toolchain for a controlled baseline comparison")
    args = parser.parse_args()
    work, target = args.work_dir.resolve(), args.target_dir.resolve()
    if not (work.is_relative_to(ROOT / "target") or work.is_relative_to(Path("/tmp"))):
        parser.error("--work-dir must be beneath root target/ or /tmp")
    if not target.is_relative_to(ROOT / "target"):
        parser.error("--target-dir must be beneath root target/")
    record = vendor_sources.records(ROOT)[args.overlay]
    errors = vendor_sources.check(ROOT)
    if errors:
        raise ValueError("\n".join(errors))
    path = work / "canopa-build.json"
    if args.reuse:
        sources = {name: work / spec["archive_root"] for name, spec in record["sources"].items()}
        receipt = reuse_prepared(work, sources, args.overlay, record)
    else:
        sources = vendor_sources.prepare(ROOT, args.archives.resolve(), work, args.overlay)
    source = sources["niri"]
    if not args.reuse:
        original_lock = vendor_sources.digest((source / "Cargo.lock").read_bytes())
        binding = bind_sources(sources, record) if "smithay" in sources else None
    prepared_lock = vendor_sources.digest((source / "Cargo.lock").read_bytes())
    if not args.reuse:
        receipt = {"overlay": args.overlay, "source": str(source),
               "sources": {name: str(path) for name, path in sources.items()},
               "revision": record["sources"]["niri"]["revision"],
               "recipe_sha256": vendor_sources.digest(json.dumps(record, sort_keys=True).encode()),
               "lock_upstream_sha256": original_lock, "lock_sha256": prepared_lock, "binding": binding,
               "profile": "dev", "incremental": True, "jobs": 1,
               "status": "prepared", "installed": False,
               "inputs": source_inputs(sources)}
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    if args.prepare_only:
        print(json.dumps(summary(receipt), indent=2))
        return
    env = os.environ.copy()
    suffix = {OVERLAY: "+canopa-resume", "niri-desktop-compat": "+canopa-desktop",
              "niri-remote-input": "+canopa-input", "niri-desktop-pointer": "+canopa-pointer-lease",
              "niri-input-capture": "+canopa-input-capture",
              "niri-global-shortcuts": "+canopa-shortcut-lease",
              "niri-input-capture-keyboard": "+canopa-keyboard-capture",
              "niri-remote-touch": "+canopa-remote-touch"}[args.overlay]
    receipt["build_commit"] = receipt["revision"] + suffix
    env.update(CARGO_INCREMENTAL="1", NIRI_BUILD_COMMIT=receipt["revision"] + suffix,
               GIT_CEILING_DIRECTORIES=str(work))
    if args.toolchain:
        env["RUSTUP_TOOLCHAIN"] = args.toolchain
    receipt["compiler"] = subprocess.check_output(["rustc", "--version"], cwd=source, env=env, text=True).strip()
    command = ["cargo", "build", "--locked", "--offline", "-p", "niri", "--bin", "niri", "--jobs", "1",
               "--target-dir", str(target)]
    receipt.update(status="building", command=command)
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    result = subprocess.run(command, cwd=source, env=env)
    receipt.update(status="failed", exit_code=result.returncode)
    if result.returncode == 0:
        if vendor_sources.digest((source / "Cargo.lock").read_bytes()) != prepared_lock:
            raise RuntimeError("Prepared lockfile changed during build")
        binary = target / "debug/niri"
        with binary.open("rb") as stream:
            checksum = hashlib.file_digest(stream, "sha256").hexdigest()
        receipt.update(status="built", binary=str(binary), binary_sha256=checksum)
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(summary(receipt), indent=2), flush=True)
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
