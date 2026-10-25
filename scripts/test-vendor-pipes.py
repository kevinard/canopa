#!/usr/bin/env python3
"""Test actual fork pipe workers without pulling Winit's excluded dev graph."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    locked = {(p["name"], p["version"], p.get("source")) for p in lock}
    version = next(p["version"] for p in lock if p["name"] == "rustix" and p["version"].startswith("0.38."))
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str((ROOT / "target").resolve())
    env["CARGO_INCREMENTAL"] = "0"
    with tempfile.TemporaryDirectory(prefix="canopa-pipe-tests-") as directory:
        snapshot = Path(directory)
        shutil.copyfile(ROOT / "vendor/winit/src/platform_impl/linux/wayland/drag_drop/pipes.rs", snapshot / "lib.rs")
        shutil.copyfile(ROOT / "Cargo.lock", snapshot / "Cargo.lock")
        (snapshot / "Cargo.toml").write_text('[package]\nname="canopa-native-pipe-tests"\nversion="0.0.0"\nedition="2021"\n'
            '[lib]\npath="lib.rs"\n[dependencies]\n'
            f'rustix={{version="={version}",features=["event","pipe","fs"]}}\n')
        metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--offline", "--format-version", "1",
            "--manifest-path", str(snapshot / "Cargo.toml")], cwd=ROOT, env=env, text=True))
        for p in metadata["packages"]:
            if p["source"] and (p["name"], p["version"], p["source"]) not in locked:
                raise RuntimeError(f"Pipe tests escaped root locked graph: {p['name']} {p['version']}")
        subprocess.run(["cargo", "test", "--locked", "--offline", "-j1", "--manifest-path", str(snapshot / "Cargo.toml")],
                       cwd=ROOT, env=env, check=True)


if __name__ == "__main__":
    main()
