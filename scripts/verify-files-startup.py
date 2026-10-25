#!/usr/bin/env python3
"""Repeat first-window acceptance with fresh private Files desktop sessions."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scheme", choices=("dark", "light"), default="dark")
    parser.add_argument("--scale", type=float, default=1.0)
    parser.add_argument("--attempts", type=int, choices=range(1, 17), default=4)
    parser.add_argument("--fixture", choices=("operations", "interrupted"), default="operations")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    results = []
    try:
        for attempt in range(1, args.attempts + 1):
            artifact = output / f"attempt-{attempt:02d}"
            started = time.monotonic()
            with (output / f"attempt-{attempt:02d}.log").open("w") as log:
                process = subprocess.run([
                    sys.executable, str(ROOT / "scripts/verify-files.py"),
                    "--binary", str(args.binary.resolve()), "--output", str(artifact),
                    "--iced-operations-only", "--iced-operations-case",
                    "startup-interrupted" if args.fixture == "interrupted" else "startup",
                    "--scheme", args.scheme, "--scale", str(args.scale),
                ], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
            summary = json.loads((artifact / "summary.json").read_text()) if (artifact / "summary.json").exists() else {}
            results.append({"attempt": attempt, "returncode": process.returncode,
                            "status": "passed" if process.returncode == 0 else "failed",
                            "elapsed_seconds": time.monotonic() - started,
                            "first_browser_capture_seconds": summary.get("first_browser_capture_seconds"),
                            "error": summary.get("error")})
            print(f"Startup attempt {attempt}: {results[-1]['status']}", flush=True)
    finally:
        passed = len(results) == args.attempts and all(r["status"] == "passed" for r in results)
        (output / "summary.json").write_text(json.dumps({
            "status": "passed" if passed else "failed", "scheme": args.scheme,
            "scale": args.scale, "fixture": args.fixture, "attempts": results,
            "checks": [f"fresh private process {r['attempt']} maps its first Browser" for r in results if r["status"] == "passed"],
            "scope": "fresh-process first-window readiness; timing includes settling/capture; not resident activation or a system cold-cache benchmark",
        }, indent=2) + "\n")
    return int(not passed)


if __name__ == "__main__":
    sys.exit(main())
