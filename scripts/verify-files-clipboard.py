#!/usr/bin/env python3
"""Exercise the file clipboard worker against independent peers in private Niri."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def test_binary(output):
    command = ["bash", str(ROOT / "scripts/cargo.sh"), "test", "--offline", "-j", "1", "-p", "canopa-files",
               "--no-default-features", "--features", "wayland-clipboard",
               "--test", "file_clipboard_wayland", "--no-run", "--message-format=json"]
    # Finish compilation before creating any native fixture or compositor.
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
    (output / "build.jsonl").write_text(result.stdout)
    (output / "build.log").write_text(result.stderr)
    result.check_returncode()
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    return Path(next(item["executable"] for item in reversed(artifacts)
                     if item.get("reason") == "compiler-artifact"
                     and item.get("target", {}).get("name") == "file_clipboard_wayland"
                     and item.get("executable")))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = (args.test_binary or test_binary(output)).resolve()
    digest = hashlib.sha256()
    with binary.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    result = {"status": "failed", "binary": str(binary), "sha256": digest.hexdigest()}
    with tempfile.TemporaryDirectory(prefix="canopa-clipboard-native-") as directory:
        runtime = Path(directory)
        runtime.chmod(0o700)
        session = desktop.Session(runtime, output)
        home = runtime / "home"
        home.mkdir()
        session.env.update(HOME=str(home), CANOPA_PRIVATE_CLIPBOARD_RUNTIME=str(runtime))
        try:
            session.start_niri('animations { off; }\nxwayland-satellite { off; }\n')
            inventory = session.run(["wayland-info"])
            (output / "wayland-info.txt").write_text(inventory)
            assert "ext_data_control_manager_v1" in inventory
            names = [line.removesuffix(": test") for line in session.run([str(binary), "--list"]).splitlines()
                     if line.endswith(": test") and not line.removesuffix(": test").endswith("_helper")]
            assert names, "No native clipboard tests discovered"
            process = session.start([str(binary), "--ignored", "--test-threads=1"], "clipboard.log")
            status = process.wait(timeout=90)
            log = (output / "clipboard.log").read_text()
            assert status == 0, log
            for name in names:
                assert re.search(rf"(?m)^test {re.escape(name)} \.\.\. ok$", log), name
            result.update(status="passed", checks=names)
        except Exception as error:
            result["error"] = str(error)
            raise
        finally:
            session.close()
            (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
