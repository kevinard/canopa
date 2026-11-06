#!/usr/bin/env python3
"""Stage or verify the optional, worker-only CUPS library and its original notices."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import struct
import subprocess

import vendor_sources

ROOT = Path(__file__).resolve().parents[1]
OVERLAY = "cups-authentication"
LIBRARY = Path("lib/canopa/print/libcups.so.2")
MANIFEST = Path("share/canopa/print-runtime.json")
NOTICES = Path("share/licenses/canopa-print-cups")
MAX_IMAGE_BYTES = 16 * 1024 * 1024


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def regular(path):
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError(f"Expected a regular file: {path}")


def check_parents(bundle, path):
    for parent in path.parents:
        if not parent.is_relative_to(bundle):
            break
        if parent.is_symlink():
            raise ValueError("Packaged Print runtime contains a directory symlink")


def inspect_library(path):
    regular(path)
    if not 64 <= path.stat().st_size <= MAX_IMAGE_BYTES:
        raise ValueError("Invalid CUPS image size")
    with path.open("rb") as stream:
        header = stream.read(20)
    if header[:7] != b"\x7fELF\x02\x01\x01" or struct.unpack_from("<HH", header, 16) != (3, 62):
        raise ValueError("The release requires an ELF64 little-endian x86-64 shared library")
    env = dict(os.environ, LC_ALL="C")
    dynamic = subprocess.check_output(["readelf", "--dynamic", "--wide", str(path)],
                                      env=env, text=True, timeout=15)
    if re.findall(r"\(SONAME\).*\[([^]]+)\]", dynamic) != ["libcups.so.2"]:
        raise ValueError("Unexpected CUPS library soname")
    if re.search(r"\((?:RPATH|RUNPATH)\)", dynamic):
        raise ValueError("A packaged CUPS library must not embed a runtime search path")
    needed = sorted(re.findall(r"\(NEEDED\).*\[([^]]+)\]", dynamic))
    if "libgnutls.so.30" not in needed or any("/" in name for name in needed):
        raise ValueError("Expected the pinned GnuTLS client with soname-only dependencies")
    symbols = subprocess.check_output(["readelf", "--dyn-syms", "--wide", str(path)],
                                      env=env, text=True, timeout=15)
    if not re.search(r"\bFUNC\s+GLOBAL\s+DEFAULT\s+\d+\s+canopaCupsAuthenticationVersion\s*$",
                     symbols, re.M):
        raise ValueError("Missing defined CUPS authentication capability symbol")
    return dict(format="ELF64", architecture="x86_64", soname="libcups.so.2", needed=needed)


def record():
    value = vendor_sources.records(ROOT)[OVERLAY]
    for source in value["sources"].values():
        for asset in vendor_sources.license_assets(source):
            path = ROOT / asset["path"]
            regular(path)
            if digest(path) != asset["sha256"]:
                raise ValueError("CUPS notice differs from canonical provenance")
    for patch in value["patches"]:
        path = ROOT / patch["path"]
        regular(path)
        if digest(path) != patch["sha256"]:
            raise ValueError("CUPS patch differs from canonical provenance")
    return value


def verified_build(receipt_path, recipe):
    receipt_path = receipt_path.resolve(strict=True)
    receipt = json.loads(receipt_path.read_text())
    expected_recipe = vendor_sources.digest(json.dumps(recipe, sort_keys=True).encode())
    if (receipt.get("overlay") != OVERLAY or receipt.get("status") != "built"
            or receipt.get("inputs_unchanged") is not True
            or receipt.get("recipe_sha256") != expected_recipe
            or receipt.get("built_targets") != ["libcups.so.2"]):
        raise ValueError("CUPS receipt is incomplete or belongs to another recipe")
    source = Path(receipt["source"])
    library = source / "cups/libcups.so.2"
    if not source.is_absolute() or receipt.get("binary") != str(library):
        raise ValueError("CUPS receipt has an unexpected binary location")
    regular(library)
    if digest(library) != receipt.get("binary_sha256"):
        raise ValueError("CUPS library differs from its build receipt")
    inputs = json.loads((receipt_path.parent / "source-inputs.json").read_text())
    if (len(inputs) != receipt.get("source_file_count")
            or vendor_sources.digest(json.dumps(inputs, sort_keys=True).encode())
            != receipt.get("source_inputs_sha256")):
        raise ValueError("CUPS source inventory differs from its build receipt")
    for name, expected in inputs.items():
        relative = Path(name)
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError("Invalid CUPS source inventory path")
        path = source / relative
        regular(path)
        if digest(path) != expected:
            raise ValueError(f"CUPS source differs from its build receipt: {name}")
    for patch in recipe["patches"]:
        if any(inputs.get(name) != hashes["patched"] for name, hashes in patch["files"].items()):
            raise ValueError("CUPS build does not contain the canonical patch")
    return library, receipt


def stage(bundle, receipt_path):
    recipe = record()
    library, receipt = verified_build(receipt_path, recipe)
    elf = inspect_library(library)
    if any((bundle / path).exists() or (bundle / path).is_symlink()
           for path in [LIBRARY.parent, MANIFEST, NOTICES]):
        raise ValueError("Print runtime destination already exists")
    for path in [LIBRARY, MANIFEST, NOTICES / "LICENSE"]:
        check_parents(bundle, bundle / path)
    files = {LIBRARY: library}
    spec = recipe["sources"]["cups"]
    for asset in vendor_sources.license_assets(spec):
        files[NOTICES / Path(asset["path"]).name] = ROOT / asset["path"]
    for patch in recipe["patches"]:
        files[NOTICES / Path(patch["path"]).name] = ROOT / patch["path"]
    for relative, source in files.items():
        target = bundle / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        target.chmod(0o644)
    if digest(bundle / LIBRARY) != receipt["binary_sha256"]:
        raise ValueError("CUPS library changed while staging")
    changes = bundle / NOTICES / "CHANGES.md"
    changed = sorted({name for patch in recipe["patches"] for name in patch["files"]})
    changes.write_text("# Canopa modifications to CUPS " + spec["revision"] + "\n\n"
                       + recipe["description"] + ".\n\n"
                       + "This is a modified CUPS client, loaded only in Canopa's isolated Print workers. "
                       "The system scheduler and tools retain their own client library.\n\n"
                       + "Modified upstream files:\n\n" + "".join(f"- `{name}`\n" for name in changed)
                       + "\nComplete source patches:\n\n"
                       + "".join(f"- `{Path(patch['path']).name}`\n" for patch in recipe["patches"])
                       + "\nOriginal LICENSE and NOTICE are retained beside these patches.\n")
    changes.chmod(0o644)
    manifest = dict(schema_version=1, overlay=OVERLAY, recipe_sha256=receipt["recipe_sha256"],
                    upstream=dict(version=spec["revision"], url=spec["url"],
                                  archive_sha256=spec["archive_sha256"]),
                    library=dict(path=str(LIBRARY), sha256=digest(bundle / LIBRARY),
                                 bytes=(bundle / LIBRARY).stat().st_size, **elf),
                    build=dict(profile=receipt["profile"], cflags=receipt["cflags"]),
                    files={str(path): digest(bundle / path) for path in [*files, NOTICES / "CHANGES.md"]})
    (bundle / MANIFEST).parent.mkdir(parents=True, exist_ok=True)
    (bundle / MANIFEST).write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    (bundle / MANIFEST).chmod(0o644)
    check(bundle, required=True)


def check(bundle, required=False):
    paths = [bundle / path for path in [LIBRARY.parent, MANIFEST, NOTICES]]
    if not any(path.exists() or path.is_symlink() for path in paths):
        if required:
            raise ValueError("Missing packaged Print runtime")
        print("Release Print runtime: absent; system CUPS fallback.")
        return
    recipe = record()
    regular(bundle / MANIFEST)
    manifest = json.loads((bundle / MANIFEST).read_text())
    spec = recipe["sources"]["cups"]
    expected_recipe = vendor_sources.digest(json.dumps(recipe, sort_keys=True).encode())
    if (manifest.get("schema_version") != 1 or manifest.get("overlay") != OVERLAY
            or manifest.get("recipe_sha256") != expected_recipe
            or manifest.get("upstream") != dict(version=spec["revision"], url=spec["url"],
                                                 archive_sha256=spec["archive_sha256"])):
        raise ValueError("Packaged CUPS manifest differs from canonical provenance")
    expected_files = {str(LIBRARY), str(NOTICES / "CHANGES.md")}
    originals = {str(NOTICES / Path(asset["path"]).name): asset["sha256"]
                 for asset in vendor_sources.license_assets(spec)}
    originals.update({str(NOTICES / Path(patch["path"]).name): patch["sha256"]
                      for patch in recipe["patches"]})
    expected_files.update(originals)
    if set(manifest.get("files", {})) != expected_files:
        raise ValueError("Incomplete packaged CUPS library, patch or notice inventory")
    for name, expected in manifest["files"].items():
        path = bundle / name
        check_parents(bundle, path)
        regular(path)
        if digest(path) != expected or (name in originals and expected != originals[name]):
            raise ValueError(f"Packaged CUPS file differs from its inventory: {name}")
    library = bundle / LIBRARY
    expected = dict(path=str(LIBRARY), sha256=digest(library), bytes=library.stat().st_size,
                    **inspect_library(library))
    if manifest.get("library") != expected:
        raise ValueError("Packaged CUPS library metadata differs from the actual image")
    print(f"Release Print runtime: CUPS {spec['revision']}; library, source patch and original notices verified.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--build-receipt", type=Path)
    mode.add_argument("--check", action="store_true")
    parser.add_argument("--required", action="store_true", help="Refuse an archive without the optional runtime")
    args = parser.parse_args()
    if args.required and not args.check:
        parser.error("--required is only valid with --check")
    try:
        if args.check:
            check(args.bundle.resolve(), required=args.required)
        else:
            stage(args.bundle.resolve(), args.build_receipt)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Print runtime packaging: {error}\n")


if __name__ == "__main__":
    main()
