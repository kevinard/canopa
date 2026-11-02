#!/usr/bin/env python3
"""Build the bounded uutils interface proof and use private files only.

The proof deliberately links uu_cp too, to reproduce the rejected path API.
It is not a production backend. Outputs are JSON on stdout; --keep retains files.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="canopa-uutils-build-") as build:
        build = Path(build)
        (build / "src").mkdir()
        shutil.copyfile(ROOT / "scripts/fixtures/uutils-copy-proof.rs", build / "src/main.rs")
        manifest = f'''[package]
name = "canopa-uutils-proof"
version = "0.0.0"
edition = "2024"
[dependencies]
uucore = {{ path = "{ROOT / 'vendor/uucore'}", default-features = false, features = ["buf-copy", "fs", "signals"] }}
uu_cp = "=0.12.0"
'''
        (build / "Cargo.toml").write_text(manifest)
        shutil.copyfile(ROOT / "scripts/fixtures/uutils-proof.lock", build / "Cargo.lock")
        env = dict(os.environ, CARGO_INCREMENTAL="0", TMPDIR="/tmp",
                   CARGO_TARGET_DIR=str(ROOT / "target/uutils-proof"))
        subprocess.run(["bash", str(ROOT / "scripts/cargo.sh"), "build", "--manifest-path", str(build / "Cargo.toml"),
                        "--locked", "--offline", "-j1"], env=env, check=True)
    binary = ROOT / "target/uutils-proof/debug/canopa-uutils-proof"
    root = Path(tempfile.mkdtemp(prefix="canopa-uutils-data-", dir=ROOT / "target"))
    try:
        measured = subprocess.check_output([binary, root], text=True)
        evidence = json.loads(measured.strip().splitlines()[-1])
        child = subprocess.Popen([binary, root, "hold"])
        try:
            deadline = time.monotonic() + 10
            while not (root / "written").exists():
                if child.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("copy process never reached its write boundary")
                time.sleep(0.005)
            child.kill()
            assert child.wait(timeout=10) == -9
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=10)
        size = (root / "killed-output").stat().st_size
        time.sleep(0.1)
        assert (root / "killed-output").stat().st_size == size
        evidence.update(killed_process_reaped=True, killed_bytes=size,
                        writer_after_owner_death=False, data=str(root))
        print(json.dumps(evidence, sort_keys=True))
    finally:
        if not args.keep:
            shutil.rmtree(root)


if __name__ == "__main__":
    main()
