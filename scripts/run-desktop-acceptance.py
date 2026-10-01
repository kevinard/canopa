#!/usr/bin/env python3
"""Run the maintained native desktop acceptance matrix on an isolated test runner."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
import time

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
            arg.format(shell="shell", notify="notify", artifacts="artifacts")
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
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    selected = [case for case in cases if not args.case or case["id"] in args.case]
    results = []
    try:
        for case in selected:
            paths = {"shell": str(args.shell.resolve()), "notify": str(args.notify.resolve()),
                     "artifacts": str(output / case["id"])}
            command = [sys.executable, str(ROOT / "scripts" / case["script"])]
            command.extend(arg.format(**paths) for arg in case["args"])
            result = {"id": case["id"], "command": command, "status": "interrupted"}
            results.append(result)
            started = time.monotonic()
            print(f'Starting {case["id"]}', flush=True)
            with (output / f'{case["id"]}.log').open("w") as log:
                process = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
            result.update(returncode=process.returncode, elapsed_seconds=time.monotonic() - started,
                          status="passed" if process.returncode == 0 else "failed")
            print(f'{case["id"]}: {result["status"]}', flush=True)
    finally:
        (output / "summary.json").write_text(json.dumps({"manifest": str(MANIFEST),
            "selected": [case["id"] for case in selected], "results": results}, indent=2) + "\n")
    return int(any(result["status"] != "passed" for result in results))


if __name__ == "__main__":
    sys.exit(main())
