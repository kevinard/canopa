#!/usr/bin/env python3
"""Build the pinned frontend input-order companion without installing it.

Run from the Canopa root. Archives and build tools must already be available.
Use --reuse to continue the same verified source tree and Meson/Ninja cache.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile

import vendor_sources

ROOT = Path(__file__).resolve().parents[1]
OVERLAY = "xdg-desktop-portal-input-order"


def file_digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_source(source, archives, record):
    if source.is_symlink() or not source.is_dir():
        raise ValueError("Prepared frontend source root must be a real directory")
    spec = record["sources"]["xdg-desktop-portal"]
    archive = archives / spec["archive"]
    if file_digest(archive) != spec["archive_sha256"]:
        raise ValueError("Frontend archive checksum mismatch")
    prefix = spec["archive_root"] + "/"
    with tarfile.open(archive) as contents:
        expected = {member.name.removeprefix(prefix): vendor_sources.digest(contents.extractfile(member).read())
                    for member in contents if member.isfile() and member.name.startswith(prefix)}
    for patch in record["patches"]:
        for name, hashes in patch["files"].items():
            expected[name] = hashes["patched"]
    actual = {}
    for path in source.rglob("*"):
        if path.is_symlink():
            raise ValueError("Unexpected symlink in prepared frontend sources")
        if path.is_file():
            name = str(path.relative_to(source))
            # Meson creates an empty lock beside the bundled subprojects.
            if name == "subprojects/.wraplock" and path.stat().st_size == 0:
                continue
            actual[name] = file_digest(path)
    if actual != expected:
        raise ValueError("Prepared frontend source tree differs from its pinned recipe")
    return expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archives", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--native-file", type=Path, help="Explicit Meson build-tool overrides")
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--reuse", action="store_true", help="Reuse this recipe's existing sources and build cache")
    args = parser.parse_args()
    work, archives = args.work_dir.resolve(), args.archives.resolve()
    if not (work.is_relative_to(ROOT / "target") or work.is_relative_to(Path("/tmp"))):
        parser.error("--work-dir must be beneath root target/ or /tmp")
    record = vendor_sources.records(ROOT)[OVERLAY]
    recipe = vendor_sources.digest(json.dumps(record, sort_keys=True).encode())
    path = work / "canopa-build.json"
    source = work / record["sources"]["xdg-desktop-portal"]["archive_root"]
    if args.reuse:
        previous = json.loads(path.read_text())
        if previous["recipe_sha256"] != recipe:
            raise ValueError("Existing build belongs to a different frontend recipe")
        errors = vendor_sources.check(ROOT)
        if errors:
            raise ValueError("\n".join(errors))
    else:
        vendor_sources.prepare(ROOT, archives, work, OVERLAY)
    inputs = verify_source(source, archives, record)
    build = work / "build"
    native = args.native_file.resolve() if args.native_file else None
    native_spec = {"path": str(native), "sha256": file_digest(native)} if native else None
    if args.reuse and (build / "meson-private/coredata.dat").exists() and previous.get("native_file") != native_spec:
        raise ValueError("Configured build-tool overrides differ; retain the existing native file")
    receipt = {"overlay": OVERLAY, "recipe_sha256": recipe, "source": str(source),
               "source_file_count": len(inputs), "source_inputs_sha256": vendor_sources.digest(json.dumps(inputs, sort_keys=True).encode()),
               "build": str(build), "profile": "debug", "jobs": 1, "installed": False,
               "native_file": native_spec,
               "status": "prepared"}
    (work / "source-inputs.json").write_text(json.dumps(inputs, indent=2) + "\n")
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    if args.prepare_only:
        print(json.dumps(receipt, indent=2))
        return
    configure = ["meson", "setup"]
    if (build / "meson-private/coredata.dat").exists():
        configure.append("--reconfigure")
    configure += [str(build), str(source), "--prefix=/usr", "--libexecdir=/usr/lib",
                  "--buildtype=debug", "--wrap-mode=nodownload", "-Db_sanitize=none", "-Dtests=disabled",
                  "-Ddocumentation=disabled", "-Dman-pages=disabled"]
    if native:
        configure.append("--native-file=" + str(native))
    commands = [configure, ["ninja", "-C", str(build), "-j", "1", "src/xdg-desktop-portal"]]
    receipt.update(status="building", commands=commands,
                   upstream_tests="disabled; native acceptance must be run separately")
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    for command in commands:
        result = subprocess.run(command, cwd=ROOT)
        if result.returncode:
            receipt.update(status="failed", exit_code=result.returncode)
            path.write_text(json.dumps(receipt, indent=2) + "\n")
            raise SystemExit(result.returncode)
        if command == configure:
            receipt["compilers"] = json.loads(subprocess.check_output(["meson", "introspect", "--compilers", str(build)], text=True))
            path.write_text(json.dumps(receipt, indent=2) + "\n")
    try:
        verify_source(source, archives, record)
    except ValueError as error:
        receipt.update(status="failed", error=str(error))
        path.write_text(json.dumps(receipt, indent=2) + "\n")
        raise
    binary = build / "src/xdg-desktop-portal"
    receipt.update(status="built", exit_code=0, binary=str(binary),
                   binary_sha256=file_digest(binary), inputs_unchanged=True)
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2), flush=True)


if __name__ == "__main__":
    main()
