#!/usr/bin/env python3
"""Build the owned IPP test receiver against a verified matching CUPS client.

This builds one test program, publishes it once by checksum, and installs nothing.
Use a stable --work-dir and --reuse for subsequent checks.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

import vendor_sources

ROOT = Path(__file__).resolve().parents[1]
OVERLAY = "cups-ipp-receiver"
STORE = ROOT / "target/native-test-snapshots/print-fixtures-by-sha256"


def sha(path):
    return vendor_sources.digest(path.read_bytes())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archives", type=Path, required=True)
    parser.add_argument("--client-build", type=Path, required=True)
    parser.add_argument("--library", type=Path, help="Same client image at a retained checksum path")
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--reuse", action="store_true")
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    work, archives = args.work_dir.resolve(), args.archives.resolve()
    if not (work.is_relative_to(ROOT / "target") or work.is_relative_to(Path("/tmp"))):
        parser.error("--work-dir must be under root target/ or /tmp")
    if shutil.disk_usage(ROOT).free < 3_000_000_000:
        raise ValueError("The receiver build needs a 3 GB free-space reserve")
    records = vendor_sources.records(ROOT)
    record = records[OVERLAY]
    recipe = vendor_sources.digest(json.dumps(record, sort_keys=True).encode())
    spec = importlib.util.spec_from_file_location("cups_client_builder", ROOT / "scripts/build-cups-authentication.py")
    client_builder = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(client_builder)
    client_path = args.client_build.resolve()
    client = json.loads(client_path.read_text())
    client_recipe = vendor_sources.digest(json.dumps(records["cups-authentication"], sort_keys=True).encode())
    if client.get("status") != "built" or client.get("recipe_sha256") != client_recipe:
        raise ValueError("The CUPS client must have a successful current canonical build receipt")
    headers = Path(client["source"]).resolve()
    configuration = client.get("configuration_inputs", {})
    expected_configuration = {str(headers / name) for name in ["config.h", "Makedefs", "config.status"]}
    if set(configuration) != expected_configuration:
        raise ValueError("Rebuild the CUPS client to bind its exact configuration")
    library = args.library.resolve() if args.library else Path(client["binary"]).resolve()
    client_inputs = client_builder.verify_sources(headers, archives, records["cups-authentication"])
    if vendor_sources.digest(json.dumps(client_inputs, sort_keys=True).encode()) != client["source_inputs_sha256"]:
        raise ValueError("CUPS client input inventory differs from its build receipt")

    def check_client():
        if sha(client_path) != client_receipt_digest or sha(library) != client["binary_sha256"]:
            raise ValueError("CUPS client image or build receipt changed")
        if any(sha(Path(path)) != digest for path, digest in configuration.items()):
            raise ValueError("CUPS client configuration differs from its build receipt")

    client_receipt_digest = sha(client_path)
    check_client()
    # The compiled program includes private headers. Its default loader path
    # must keep the matching client even outside the validation launcher.
    frozen_library = STORE.parent / "print-client-by-sha256" / client["binary_sha256"] / "libcups.so.2"
    frozen_library.parent.mkdir(parents=True, exist_ok=True)
    if not frozen_library.exists():
        with tempfile.NamedTemporaryFile(dir=frozen_library.parent, prefix=".client-", delete=False) as temporary:
            temporary_path = Path(temporary.name)
            try:
                with library.open("rb") as source_image:
                    shutil.copyfileobj(source_image, temporary)
                temporary.flush()
                if sha(temporary_path) != client["binary_sha256"]:
                    raise ValueError("Client image changed while retaining its checksum copy")
                temporary_path.chmod(0o500)
                try:
                    os.link(temporary_path, frozen_library)
                except FileExistsError:
                    pass
            finally:
                temporary_path.unlink()
    if frozen_library.is_symlink() or sha(frozen_library) != client["binary_sha256"]:
        raise ValueError("Retained CUPS client checksum path is invalid")
    runpath = "$ORIGIN/../../print-client-by-sha256/" + client["binary_sha256"]
    receipt = work / "canopa-build.json"
    if args.reuse:
        previous = json.loads(receipt.read_text())
        if previous["recipe_sha256"] != recipe or previous["client_build_sha256"] != client_receipt_digest:
            raise ValueError("Existing receiver cache belongs to another source/client recipe")
        errors = vendor_sources.check(ROOT)
        if errors:
            raise ValueError("\n".join(errors))
    else:
        vendor_sources.prepare(ROOT, archives, work, OVERLAY)
    source = work / record["sources"]["cups"]["archive_root"]
    inputs = client_builder.verify_sources(source, archives, record)
    (work / "source-inputs.json").write_text(json.dumps(inputs, indent=2) + "\n")
    compiler = Path(shutil.which("gcc") or "gcc").resolve(strict=True)
    result = dict(status="prepared", overlay=OVERLAY, recipe_sha256=recipe,
                  source=str(source), source_file_count=len(inputs),
                  source_inputs_sha256=vendor_sources.digest(json.dumps(inputs, sort_keys=True).encode()),
                  client_build=str(client_path), client_build_sha256=client_receipt_digest,
                  client_recipe_sha256=client_recipe, library=str(frozen_library), library_sha256=client["binary_sha256"],
                  verified_client_image=str(library), runpath=runpath,
                  configuration_inputs=configuration, compiler=str(compiler), compiler_sha256=sha(compiler),
                  compiler_version=subprocess.check_output([compiler, "--version"], text=True).splitlines()[0],
                  installed=False, test_only=True, built_targets=["ippeveprinter"],
                  min_free_bytes=3_000_000_000, max_net_growth_bytes=256_000_000, steps=[])

    def save():
        receipt.write_text(json.dumps(result, indent=2) + "\n")

    pending = work / "ippeveprinter.pending"
    owns_pending = False
    baseline = shutil.disk_usage(ROOT).free
    try:
        save()
        if args.prepare_only:
            print(json.dumps(result, indent=2), flush=True)
            return
        if pending.exists():
            raise ValueError("An unfinished receiver build exists; inspect it before rebuilding")
        command = [str(compiler), "-I" + str(headers), "-I" + str(source / "tools"),
                   "-D_CUPS_SOURCE", "-D_GNU_SOURCE", "-D_THREAD_SAFE", "-D_REENTRANT",
                   "-D_LARGEFILE_SOURCE", "-D_LARGEFILE64_SOURCE", "-O0", "-g0", "-fstack-protector-strong",
                   "-o", str(pending), str(source / "tools/ippeveprinter.c"),
                   "-Wl,-rpath," + runpath,
                   "-lavahi-common", "-lavahi-client", "-lpam", "-ldl", str(frozen_library),
                   "-lpthread", "-lm", "-lcrypt", "-lz"]
        step = dict(command=command, cwd=str(ROOT))
        result.update(status="building", steps=[step]); save()
        env = dict(os.environ, LC_ALL="C", TZ="UTC")
        for key in ["LD_LIBRARY_PATH", "LD_PRELOAD", "LD_AUDIT", "CPATH", "C_INCLUDE_PATH",
                    "CPLUS_INCLUDE_PATH", "LIBRARY_PATH", "GCC_EXEC_PREFIX", "COMPILER_PATH"]:
            env.pop(key, None)
        before = time.monotonic()
        with (work / "receiver.out").open("w") as output, (work / "receiver.err").open("w") as errors:
            owns_pending = True
            process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=output, stderr=errors, start_new_session=True)
            step["pid"] = process.pid; save()
            try:
                while process.poll() is None:
                    free = shutil.disk_usage(ROOT).free
                    if free < 3_000_000_000 or baseline - free > 256_000_000 or time.monotonic() - before > 120:
                        raise RuntimeError("Receiver build exceeded its time/storage budget")
                    time.sleep(.1)
            finally:
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGTERM)
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL); process.wait()
            step.update(exit_code=process.wait(), seconds=round(time.monotonic() - before, 2)); save()
        if step["exit_code"]:
            raise RuntimeError("Receiver compilation failed; see receiver.err")
        check_client()
        if sha(frozen_library) != client["binary_sha256"]:
            raise ValueError("Retained CUPS client changed during compilation")
        if client_builder.verify_sources(source, archives, record) != inputs:
            raise ValueError("Receiver inputs changed during compilation")
        if client_builder.verify_sources(headers, archives, records["cups-authentication"]) != client_inputs:
            raise ValueError("CUPS client headers changed during compilation")
        checksum = sha(pending)
        destination = STORE / checksum / "ippeveprinter"
        destination.parent.mkdir(parents=True, exist_ok=True)
        if destination.exists():
            if destination.is_symlink() or sha(destination) != checksum:
                raise ValueError("Retained receiver checksum path is invalid")
            pending.unlink()
        else:
            pending.chmod(0o500)
            pending.rename(destination)
        result.update(status="built", binary=str(destination), binary_sha256=checksum,
                      bytes=destination.stat().st_size, inputs_unchanged=True)
    except BaseException as error:
        result.update(status="failed", error=str(error))
        raise
    finally:
        if owns_pending and pending.exists() and result["status"] == "failed":
            pending.unlink()
        result["free_bytes"] = shutil.disk_usage(ROOT).free
        save()
    print(json.dumps(result, indent=2), flush=True)


if __name__ == "__main__":
    main()
