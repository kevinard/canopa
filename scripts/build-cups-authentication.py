#!/usr/bin/env python3
"""Build the process-private CUPS client library from the pinned kevinard/cups fork.

--archives holds the fork's GitHub archive (checksummed in packaging/forks.json);
--download fetches it. The fork tree includes the generated configure script.
Use the same --work-dir and --reuse for subsequent checks. This builds libcups
only and installs nothing; its private HTTP layout must not be mixed with a
system CUPS daemon. The matching IPP backend and test receiver are compiled
from this same verified source tree.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time

import fork_sources

ROOT = Path(__file__).resolve().parents[1]
FORK = "cups"
file_digest = fork_sources.file_digest


def verify_sources(source, expected):
    """Every fork file is unchanged and no undeclared source joined the build."""
    for name, digest in expected.items():
        path = source / name
        if path.is_symlink() or file_digest(path) != digest:
            raise ValueError(f"Prepared CUPS source differs from the pinned fork: {name}")
    for path in source.rglob("*"):
        name = str(path.relative_to(source))
        if path.suffix in {".c", ".h", ".in", ".m4", ".rs"} and name not in expected and name != "config.h":
            raise ValueError(f"Undeclared source in CUPS build: {name}")
    return expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archives", type=Path, required=True)
    parser.add_argument("--download", action="store_true", help="Download a missing pinned archive")
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--reuse", action="store_true")
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--min-free-bytes", type=int, default=6_000_000_000,
                        help="Reserved free space during this native build (default: 6 GB)")
    args = parser.parse_args()
    if args.min_free_bytes <= 0:
        parser.error("--min-free-bytes must be positive")
    work, archives = args.work_dir.resolve(), args.archives.resolve()
    if not (work.is_relative_to(ROOT / "target") or work.is_relative_to(Path("/tmp"))):
        parser.error("--work-dir must be under root target/ or /tmp")
    pin = fork_sources.pin(FORK)
    record, archive = fork_sources.origin(FORK, archives, fetch=args.download)
    receipt = work / "canopa-build.json"
    source = work / record["archive_root"]
    previous = {}
    if args.reuse:
        previous = json.loads(receipt.read_text())
        if (previous.get("fork") != FORK or previous.get("origin") != record
                or previous.get("source") != str(source)):
            raise ValueError("Existing CUPS build belongs to another fork commit or location")
    else:
        if source.exists():
            raise ValueError(f"{source} already exists; use --reuse or a new --work-dir")
        fork_sources.extract(archive, work, record["archive_root"])
    inputs = fork_sources.archive_inventory(archive, record["archive_root"])
    verify_sources(source, inputs)
    notices = {name: value for name, value in fork_sources.notices(source, pin["notices"]).items()}
    inputs_sha256 = fork_sources.write_inputs(work, inputs)
    flags = "-O0 -g0"
    env = dict(os.environ, CFLAGS=flags, CXXFLAGS=flags)
    # The candidate's private ABI is confined to its own future worker process.
    env.pop("LD_LIBRARY_PATH", None)
    baseline = previous.get("baseline_free_bytes", shutil.disk_usage(ROOT).free) if args.reuse else shutil.disk_usage(ROOT).free
    result = {"fork": FORK, "origin": record, "source": str(source), "notices": notices,
              "source_file_count": len(inputs), "source_inputs_sha256": inputs_sha256,
              "profile": "development", "cflags": flags, "jobs": 1, "installed": False,
              "built_targets": ["libcups.so.2"], "baseline_free_bytes": baseline,
              "min_free_bytes": args.min_free_bytes,
              "status": "prepared", "steps": []}

    def save():
        receipt.write_text(json.dumps(result, indent=2) + "\n")

    def run(name, command, cwd):
        step = {"name": name, "command": command, "cwd": str(cwd)}
        result["steps"].append(step)
        start = time.monotonic()
        with (work / (name + ".out")).open("w") as output, (work / (name + ".err")).open("w") as errors:
            process = subprocess.Popen(command, cwd=cwd, env=env, stdout=output, stderr=errors, start_new_session=True)
            step["pid"] = process.pid
            save()
            while process.poll() is None:
                free = shutil.disk_usage(ROOT).free
                if free < args.min_free_bytes or baseline - free > 3_000_000_000:
                    step["storage_stop"] = {"free_bytes": free, "net_growth_bytes": baseline - free}
                    os.killpg(process.pid, signal.SIGTERM)
                    break
                time.sleep(1)
            step.update(exit_code=process.wait(), seconds=round(time.monotonic() - start, 2))
        save()
        print(json.dumps(step), flush=True)
        if step["exit_code"]:
            raise RuntimeError(f"CUPS {name} failed")

    try:
        save()
        if args.prepare_only:
            print(json.dumps(result, indent=2))
            return
        result["status"] = "building"
        configuration = [source / name for name in ["config.h", "Makedefs", "config.status"]]
        if args.reuse and previous.get("configuration_inputs"):
            if set(previous["configuration_inputs"]) != {str(path) for path in configuration}:
                raise ValueError("Existing CUPS configuration paths differ")
            if any(file_digest(Path(path)) != digest for path, digest in previous["configuration_inputs"].items()):
                raise ValueError("Existing CUPS configuration differs from its build receipt")
        if not (source / "config.status").exists() or not previous.get("configuration_inputs"):
            run("configure", ["./configure", "--prefix=/usr", "--sysconfdir=/etc", "--localstatedir=/var",
                              "--disable-static", "--disable-debug", "--disable-debug-guards", "--disable-debug-printfs",
                              "--with-tls=gnutls"], source)
        else:
            result["configuration_reused"] = True
            if previous.get("cflags") != flags:
                raise ValueError("Existing CUPS compiler flags differ")
        result["configuration_inputs"] = {str(path): file_digest(path) for path in configuration}
        save()
        run("library", ["make", "-j1", "libcups.so.2"], source / "cups")
        verify_sources(source, inputs)
        if any(file_digest(Path(path)) != digest for path, digest in result["configuration_inputs"].items()):
            raise ValueError("CUPS configuration changed during the build")
        binary = source / "cups/libcups.so.2"
        result.update(status="built", binary=str(binary), binary_sha256=file_digest(binary), inputs_unchanged=True)
    except BaseException as error:
        result.update(status="failed", error=str(error))
        raise
    finally:
        result["free_bytes"] = shutil.disk_usage(ROOT).free
        save()
    print(json.dumps(result, indent=2), flush=True)


if __name__ == "__main__":
    main()
