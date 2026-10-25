#!/usr/bin/env python3
"""Run the maintained native desktop acceptance matrix on an isolated test runner."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

from native_compositor import select_niri, sha256, stage_niri

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "scripts/desktop-fixture/acceptance.json"


def scenarios():
    cases = json.loads(MANIFEST.read_text())["scenarios"]
    ids = set()
    covered = set()
    for case in cases:
        name, script, args = case["id"], case["script"], case["args"]
        if not re.fullmatch(r"[a-z0-9-]+", name) or name in ids:
            raise ValueError(f"Invalid or duplicate scenario: {name}")
        if Path(script).name != script or not (ROOT / "scripts" / script).is_file():
            raise ValueError(f"Missing scenario script: {script}")
        if not isinstance(args, list) or not all(isinstance(arg, str) for arg in args):
            raise ValueError(f"Invalid scenario arguments: {name}")
        if "{artifacts}" not in args:
            raise ValueError(f"Scenario must retain artifacts: {name}")
        for arg in args:
            arg.format(shell="shell", notify="notify", files="files", artifacts="artifacts")
        ids.add(name)
        covered.add(script)
    required = {p.name for p in (ROOT / "scripts").glob("verify-*.py")}
    required.update(("test-desktop.py", "test-fork-desktop.py"))
    if missing := required - covered:
        raise ValueError(f"Native scenarios missing from manifest: {sorted(missing)}")
    return cases


def main():
    cases = scenarios()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shell", type=Path, default=ROOT / "target/debug/canopa-shell")
    parser.add_argument("--notify", type=Path, default=ROOT / "target/debug/canopa-notify")
    parser.add_argument("--files", type=Path, default=ROOT / "target/debug/canopa-files")
    parser.add_argument("--niri", type=Path, help="Niri executable for private sessions; default: resolve PATH")
    parser.add_argument("--niri-sha256", help="Require this exact Niri executable SHA-256")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--case", action="append", choices=[case["id"] for case in cases],
                        help="Select a scenario; repeat to select several. Default: entire matrix.")
    parser.add_argument("--list", action="store_true", help="Validate and print the matrix; no desktop.")
    args = parser.parse_args()
    if args.list:
        for case in cases:
            print(f'{case["id"]}: {case["script"]}')
        return 0
    if args.output is None:
        parser.error("--output is required when running scenarios")
    try:
        niri = select_niri(args.niri, args.niri_sha256)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.error(str(error))
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    selected = [case for case in cases if not args.case or case["id"] in args.case]
    results = []
    summary = {"manifest": str(MANIFEST), "selected": [case["id"] for case in selected],
               "niri": niri, "results": results, "status": "interrupted"}
    try:
        env = os.environ.copy()
        if niri:
            niri = stage_niri(niri, output)
            summary["niri"] = niri
            # Only the preserved Niri is added to PATH. IPC clients and fixtures
            # that start Niri directly consequently use the same frozen binary.
            env["PATH"] = str(Path(niri["snapshot"]).parent) + os.pathsep + env.get("PATH", os.defpath)
        for case in selected:
            paths = {"shell": str(args.shell.resolve()), "notify": str(args.notify.resolve()),
                     "files": str(args.files.resolve()),
                     "artifacts": str(output / case["id"])}
            command = [sys.executable, str(ROOT / "scripts" / case["script"])]
            command.extend(arg.format(**paths) for arg in case["args"])
            result = {"id": case["id"], "command": command, "status": "interrupted"}
            results.append(result)
            started = time.monotonic()
            print(f'Starting {case["id"]}', flush=True)
            with (output / f'{case["id"]}.log').open("w") as log:
                process = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
            result.update(returncode=process.returncode, elapsed_seconds=time.monotonic() - started,
                          status="passed" if process.returncode == 0 else "failed")
            print(f'{case["id"]}: {result["status"]}', flush=True)
        if niri:
            if sha256(Path(niri["snapshot"])) != niri["sha256"]:
                raise ValueError("The preserved Niri executable changed during acceptance")
            niri["snapshot_unchanged"] = True
        summary["status"] = "failed" if any(result["status"] != "passed" for result in results) else "passed"
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        summary.update(status="failed", error=str(error))
        print(f"Desktop acceptance failed: {error}", file=sys.stderr)
    finally:
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return int(summary["status"] != "passed")


if __name__ == "__main__":
    sys.exit(main())
