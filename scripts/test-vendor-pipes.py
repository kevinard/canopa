#!/usr/bin/env python3
"""Test actual fork pipe workers without pulling Winit's excluded dev graph."""
import json
import os
from pathlib import Path
import subprocess
import tomllib

from vendor_test_workspace import workspace as test_workspace, write_if_changed

ROOT = Path(__file__).resolve().parents[1]


def main():
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    locked = {(p["name"], p["version"], p.get("source")) for p in lock}
    version = next(p["version"] for p in lock if p["name"] == "rustix" and p["version"].startswith("0.38."))
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str((ROOT / "target").resolve())
    env.setdefault("CARGO_INCREMENTAL", "1")
    profile = tomllib.loads((ROOT / "Cargo.toml").read_text())["profile"]["dev"]
    env.setdefault("CARGO_PROFILE_DEV_DEBUG", profile["debug"])
    with test_workspace("pipes") as snapshot:
        write_if_changed(snapshot / "lib.rs", (ROOT / "vendor/winit/src/platform_impl/linux/wayland/drag_drop/pipes.rs").read_bytes())
        write_if_changed(snapshot / "Cargo.lock", (ROOT / "Cargo.lock").read_bytes())
        write_if_changed(snapshot / "Cargo.toml", ('[workspace]\n[package]\nname="canopa-native-pipe-tests"\nversion="0.0.0"\nedition="2021"\n'
            '[lib]\npath="lib.rs"\n[dependencies]\n'
            f'rustix={{version="={version}",features=["event","pipe","fs"]}}\n').encode())
        metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--offline", "--format-version", "1",
            "--manifest-path", str(snapshot / "Cargo.toml")], cwd=ROOT, env=env, text=True))
        for p in metadata["packages"]:
            if p["source"] and (p["name"], p["version"], p["source"]) not in locked:
                raise RuntimeError(f"Pipe tests escaped root locked graph: {p['name']} {p['version']}")
        subprocess.run(["bash", str(ROOT / "scripts/cargo.sh"), "test", "--locked", "--offline", "-j1", "--manifest-path", str(snapshot / "Cargo.toml")],
                       cwd=ROOT, env=env, check=True)


if __name__ == "__main__":
    main()
