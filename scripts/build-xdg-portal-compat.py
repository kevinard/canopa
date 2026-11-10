#!/usr/bin/env python3
"""Build the portal frontend from the pinned kevinard/xdg-desktop-portal fork without installing it.

Run from the Canopa root. --archives must hold the fork's GitHub archive and
the official release tarball of the fork base (both checksummed in
packaging/forks.json); --download fetches missing ones. A git archive lacks the
gvdb and libglnx Meson subprojects, so only those directories are taken from
the release tarball and Meson never downloads. Build tools must already be
available. Use --reuse to continue the same verified source tree and
Meson/Ninja cache.
"""
import argparse
import json
from pathlib import Path
import subprocess

import fork_sources

ROOT = Path(__file__).resolve().parents[1]
FORK = "xdg-desktop-portal"
# Meson creates an empty lock beside the bundled subprojects.
IGNORED = frozenset({"subprojects/.wraplock"})


def expected_inputs(archive, record, subprojects, subproject_archive):
    expected = fork_sources.archive_inventory(archive, record["archive_root"])
    bundled = fork_sources.archive_inventory(subproject_archive, subprojects["archive_root"], subprojects["paths"])
    if not bundled or set(bundled) & set(expected):
        raise ValueError("Release subprojects are missing or overlap the fork sources")
    return {**expected, **bundled}


def verify_source(source, expected):
    wraplock = source / "subprojects/.wraplock"
    if wraplock.exists() and (wraplock.is_symlink() or wraplock.stat().st_size):
        raise ValueError("Unexpected Meson wrap lock contents")
    if fork_sources.inventory(source, IGNORED) != expected:
        raise ValueError("Prepared frontend source tree differs from the pinned fork and subprojects")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--archives", type=Path, required=True)
    parser.add_argument("--download", action="store_true", help="Download missing pinned archives")
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--native-file", type=Path, help="Explicit Meson build-tool overrides")
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--reuse", action="store_true", help="Reuse this commit's existing sources and build cache")
    parser.add_argument("--release", action="store_true", help="Build the optimized distribution executable")
    args = parser.parse_args()
    try:
        work = fork_sources.check_work_dir(args.work_dir)
    except ValueError as error:
        parser.error(str(error))
    archives = args.archives.resolve()
    pin = fork_sources.pin(FORK)
    record, archive = fork_sources.origin(FORK, archives, fetch=args.download)
    subprojects = dict(pin["subprojects"])
    subproject_archive, _ = fork_sources.archive_file(archives, subprojects["archive"], subprojects["url"],
                                                      subprojects["archive_sha256"], args.download)
    subprojects.pop("description", None)
    expected = expected_inputs(archive, record, subprojects, subproject_archive)
    path = work / "canopa-build.json"
    source = work / record["archive_root"]
    previous = {}
    if args.reuse:
        previous = json.loads(path.read_text())
        if (previous.get("fork") != FORK or previous.get("origin") != record
                or previous.get("subprojects") != subprojects or previous.get("source") != str(source)):
            raise ValueError("Existing build belongs to a different frontend commit or location")
    else:
        if source.exists():
            raise ValueError(f"{source} already exists; use --reuse or a new --work-dir")
        fork_sources.extract(archive, work, record["archive_root"])
        for name in subprojects["paths"]:
            if (source / name).exists():
                raise ValueError(f"The fork already contains {name}")
        bundled = fork_sources.extract(subproject_archive, work / ".release-subprojects",
                                       subprojects["archive_root"], subprojects["paths"])
        for name in subprojects["paths"]:
            (bundled / name).rename(source / name)
        for directory in sorted((work / ".release-subprojects").rglob("*"), reverse=True):
            directory.rmdir()
        (work / ".release-subprojects").rmdir()
    fork_sources.notices(source, pin["notices"])
    verify_source(source, expected)
    build = work / "build"
    native = args.native_file.resolve() if args.native_file else None
    native_spec = {"path": str(native), "sha256": fork_sources.file_digest(native)} if native else None
    if args.reuse and (build / "meson-private/coredata.dat").exists() and previous.get("native_file") != native_spec:
        raise ValueError("Configured build-tool overrides differ; retain the existing native file")
    profile = "release" if args.release else "debug"
    receipt = {"fork": FORK, "origin": record, "subprojects": subprojects, "source": str(source),
               "source_file_count": len(expected), "source_inputs_sha256": fork_sources.write_inputs(work, expected),
               "build": str(build), "profile": profile, "jobs": 1, "installed": False,
               "native_file": native_spec, "status": "prepared"}
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    if args.prepare_only:
        print(json.dumps(receipt, indent=2))
        return
    configure = ["meson", "setup"]
    if (build / "meson-private/coredata.dat").exists():
        configure.append("--reconfigure")
    configure += [str(build), str(source), "--prefix=/usr", "--libexecdir=/usr/lib",
                  "--buildtype=" + profile, "--wrap-mode=nodownload", "-Db_sanitize=none", "-Dtests=disabled",
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
        verify_source(source, expected)
    except ValueError as error:
        receipt.update(status="failed", error=str(error))
        path.write_text(json.dumps(receipt, indent=2) + "\n")
        raise
    binary = build / "src/xdg-desktop-portal"
    receipt.update(status="built", exit_code=0, binary=str(binary),
                   binary_sha256=fork_sources.file_digest(binary), inputs_unchanged=True,
                   notices={f"{FORK}/{name}": value for name, value in fork_sources.notices(source, pin["notices"]).items()})
    path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2), flush=True)


if __name__ == "__main__":
    main()
