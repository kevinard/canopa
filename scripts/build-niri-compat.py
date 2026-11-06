#!/usr/bin/env python3
"""Build the pinned Niri compatibility companion without installing it.

Run from the Canopa workspace root. This deliberately builds Niri's independent
upstream workspace from disposable prepared sources. All Cargo outputs remain in
Canopa's target/. Dependencies must already be fetched for the upstream lockfile.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess

import vendor_sources
from niri_source_bindings import bind_sources

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archives", type=Path, required=True, help="Directory containing the pinned upstream tar.gz archives")
    parser.add_argument("--work-dir", type=Path, required=True, help="New disposable source directory beneath target/ or /tmp")
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target/niri-compat")
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    work = args.work_dir.resolve()
    if not (work.is_relative_to(ROOT / "target") or work.is_relative_to(Path("/tmp"))):
        parser.error("--work-dir must be beneath the workspace target/ or /tmp")
    if not args.target_dir.resolve().is_relative_to(ROOT / "target"):
        parser.error("--target-dir must be beneath the workspace target/")
    name = "niri-source-cancellation"
    record = vendor_sources.records(ROOT)[name]
    sources = vendor_sources.prepare(ROOT, args.archives.resolve(), work, name)
    binding = bind_sources(sources, record)
    receipt = {"overlay": name, "sources": {key: str(value) for key, value in sources.items()}, "binding": binding,
               "recipe_sha256": vendor_sources.digest(json.dumps(record, sort_keys=True).encode()), "installed": False}
    receipt_path = args.work_dir.resolve() / "canopa-build.json"
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2), flush=True)
    if args.prepare_only:
        return
    env = os.environ.copy()
    env.update(CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0", CARGO_TARGET_DIR=str(args.target_dir.resolve()),
               GIT_CEILING_DIRECTORIES=str(work))
    subprocess.run(["cargo", "build", "--manifest-path", str(sources["niri"] / "Cargo.toml"),
                    "-p", "niri", "--locked", "--offline", "-j1"], cwd=ROOT, env=env, check=True)
    binary = args.target_dir.resolve() / "debug/niri"
    receipt["binary"] = str(binary)
    receipt["binary_sha256"] = vendor_sources.digest(binary.read_bytes())
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"Built private companion: {binary}", flush=True)


if __name__ == "__main__":
    main()
