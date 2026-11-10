#!/usr/bin/env python3
"""Record the locked Linux release graph and collect original dependency notices."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib


ROOT = Path(__file__).resolve().parents[1]


def release_packages(manifest=None, roots=None):
    command = [
        "cargo", "metadata", "--locked", "--format-version", "1",
        "--filter-platform", "x86_64-unknown-linux-gnu",
    ]
    if manifest:
        command.extend(("--manifest-path", str(manifest)))
    metadata = json.loads(subprocess.check_output(command, cwd=ROOT, text=True))
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    roots = set(roots or {"canopa-shell", "canopa-notify", "canopa-files", "canopa-cli", "canopa-portal", "canopa-session", "canopa-greeter", "canopa-polkit", "canopa-pinentry", "canopa-settings"})
    pending = [p["id"] for p in packages.values() if p["name"] in roots]
    if roots - {p["name"] for p in packages.values()}:
        raise ValueError("Requested attribution roots are absent from the selected workspace")
    selected = set()
    while pending:
        package = pending.pop()
        if package in selected:
            continue
        selected.add(package)
        pending.extend(dep["pkg"] for dep in nodes[package]["deps"]
                       if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]))
    return sorted((packages[p] for p in selected), key=lambda p: (p["name"], p["version"]))


def collect_notices(package, destination):
    base = Path(package["manifest_path"]).parent
    files = set()
    for directory, subdirs, names in os.walk(base):
        subdirs[:] = [name for name in subdirs if name not in {"target", ".git"}]
        for filename in names:
            path = Path(directory) / filename
            if not path.is_file() or not path.resolve().is_relative_to(base.resolve()):
                continue
            relative = path.relative_to(base)
            name = filename.upper()
            if (name.startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE"))
                    or name == "UNLICENSE" or "LICENSES" in (part.upper() for part in relative.parts)):
                files.add(path)
    if package["license_file"]:
        path = base / package["license_file"]
        if path.resolve().is_relative_to(base.resolve()) and path.is_file():
            files.add(path)
    if not files and not package["source"]:
        # Some upstream members inherit their workspace's license declaration
        # and text. Require the same declared license before collecting it.
        for parent in base.parents:
            manifest = parent / "Cargo.toml"
            if not manifest.is_file():
                continue
            workspace = tomllib.loads(manifest.read_text()).get("workspace")
            if workspace is None:
                continue
            if workspace.get("package", {}).get("license") == package["license"]:
                files.update(path for path in parent.iterdir() if path.is_file()
                             and path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE")))
            break
    if not files and (package["source"] or "").startswith("git+"):
        # A crate inside a pinned multi-crate fork (Iced, winit's dpi) keeps its
        # texts at the repository root. Use the nearest notices inside the same
        # Cargo git checkout, which Cargo marks with `.cargo-ok`.
        for parent in base.parents:
            found = {path for path in parent.iterdir() if path.is_file()
                     and path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE"))}
            if found or (parent / ".cargo-ok").exists():
                files.update(found)
                break
    target = destination / f"{package['name']}-{package['version']}"
    target.mkdir(parents=True, exist_ok=True)
    for path in sorted(files):
        relative = path.relative_to(base) if path.is_relative_to(base) else Path(path.name)
        copy = target / relative
        copy.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, copy)
    if not files:
        # Preserve notices embedded in source when the publisher omitted separate texts.
        archive = base.parents[2] / "cache" / base.parent.name / f"{base.name}.crate"
        if not package["source"] or not archive.is_file():
            raise RuntimeError(f"No notices or published source archive for {package['id']}")
        shutil.copyfile(archive, target / archive.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--notices", type=Path)
    parser.add_argument("--check", action="store_true", help="check that the output inventory is current")
    parser.add_argument("--manifest-path", type=Path, help="independently locked upstream workspace")
    parser.add_argument("--root", action="append", help="executable package to attribute; repeatable")
    args = parser.parse_args()
    if args.manifest_path and not args.root:
        parser.error("--manifest-path requires explicit --root selection")
    packages = release_packages(args.manifest_path, args.root)
    lock_hash = hashlib.sha256(((args.manifest_path.resolve().parent if args.manifest_path else ROOT) / "Cargo.lock").read_bytes()).hexdigest()
    text = [
        "# Dependency attributions\n",
        "Generated by `python3 scripts/dependency-attributions.py --output docs/dependency-attributions.md`.\n",
        "Combined Linux x86-64 default-feature normal/build graph for the Canopa executables. "
        "Development-only edges are excluded; build tools are included. Source declarations "
        "are preserved without changing license choices. System libraries retain their "
        "distribution notices.\n",
        f"Cargo.lock SHA-256: `{lock_hash}`.\n",
        "Release packaging collects original license/copyright/notice files. For a crate "
        "without separate texts, its published source archive preserves embedded notices.\n",
        "| Package | Version | Declared license | Source |",
        "| --- | --- | --- | --- |",
    ]
    if args.manifest_path:
        text[1] = "Generated from the pinned upstream workspace by `scripts/dependency-attributions.py`.\n"
        text[2] = ("Linux x86-64 default-feature normal/build graph for " + ", ".join(args.root) +
                   ". Development-only edges are excluded; source license declarations and original notices are preserved.\n")
    for package in packages:
        name, version = package["name"], package["version"]
        origin = package["source"] or ""
        if origin.startswith("git+"):
            # A pinned Canopa fork: name the exact fork commit, not a release.
            url, _, commit = origin.removeprefix("git+").partition("#")
            source = f"{url.split('?')[0]} @ {commit[:12]}"
        elif origin:
            source = f"https://crates.io/crates/{name}/{version}"
        else:
            source = package["repository"] or "Workspace source"
        license = package["license"] or package["license_file"] or "See source"
        text.append(f"| {name} | {version} | {license} | {source} |")
        if args.notices:
            collect_notices(package, args.notices)
    document = "\n".join(text) + "\n"
    if args.check:
        if not args.output.is_file() or args.output.read_text() != document:
            parser.exit(1, f"Regenerate stale dependency inventory: {args.output}\n")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(document)
    print(f"Recorded {len(packages)} release dependencies.")


if __name__ == "__main__":
    main()
