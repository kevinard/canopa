#!/usr/bin/env python3
"""Build the pinned process-private CUPS client library without installing it.

Use the same --work-dir and --reuse for subsequent checks. This builds libcups
only; its private HTTP layout must not be mixed with a system CUPS daemon.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tarfile
import time

import vendor_sources

ROOT = Path(__file__).resolve().parents[1]
OVERLAY = "cups-authentication"


def file_digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_sources(source, archives, record):
    spec = record["sources"]["cups"]
    archive = archives / spec["archive"]
    if file_digest(archive) != spec["archive_sha256"]:
        raise ValueError("CUPS archive checksum mismatch")
    prefix = spec["archive_root"] + "/"
    with tarfile.open(archive) as contents:
        expected = {member.name.removeprefix(prefix): vendor_sources.digest(contents.extractfile(member).read())
                    for member in contents if member.isfile() and member.name.startswith(prefix)}
    for patch in record["patches"]:
        expected.update({name: hashes["patched"] for name, hashes in patch["files"].items()})
    for name, digest in expected.items():
        path = source / name
        if path.is_symlink() or file_digest(path) != digest:
            raise ValueError(f"Prepared CUPS source differs from its recipe: {name}")
    for path in source.rglob("*"):
        name = str(path.relative_to(source))
        if path.suffix in {".c", ".h", ".in", ".m4", ".rs"} and name not in expected and name != "config.h":
            raise ValueError(f"Undeclared source in CUPS build: {name}")
    return expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archives", type=Path, required=True)
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
    record = vendor_sources.records(ROOT)[OVERLAY]
    recipe = vendor_sources.digest(json.dumps(record, sort_keys=True).encode())
    receipt = work / "canopa-build.json"
    source = work / record["sources"]["cups"]["archive_root"]
    previous = {}
    if args.reuse:
        previous = json.loads(receipt.read_text())
        if previous["recipe_sha256"] != recipe:
            raise ValueError("Existing CUPS build belongs to another recipe")
        errors = vendor_sources.check(ROOT)
        if errors:
            raise ValueError("\n".join(errors))
    else:
        vendor_sources.prepare(ROOT, archives, work, OVERLAY)
    inputs = verify_sources(source, archives, record)
    (work / "source-inputs.json").write_text(json.dumps(inputs, indent=2) + "\n")
    flags = "-O0 -g0"
    env = dict(os.environ, CFLAGS=flags, CXXFLAGS=flags)
    # The candidate's private ABI is confined to its own future worker process.
    env.pop("LD_LIBRARY_PATH", None)
    baseline = previous.get("baseline_free_bytes", shutil.disk_usage(ROOT).free) if args.reuse else shutil.disk_usage(ROOT).free
    result = {"overlay": OVERLAY, "recipe_sha256": recipe, "source": str(source),
              "source_file_count": len(inputs), "source_inputs_sha256": vendor_sources.digest(json.dumps(inputs, sort_keys=True).encode()),
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
        verify_sources(source, archives, record)
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
