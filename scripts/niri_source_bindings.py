"""Bind only the two pinned Smithay packages in a disposable Niri workspace."""
import json
import re
import tomllib

from vendor_sources import digest


def bind_sources(sources, record, inputs=None, *, write=True):
    manifest = sources["niri"] / "Cargo.toml"
    text = manifest.read_text() if inputs is None else inputs[0]
    original = digest(text.encode())
    revision = record["sources"]["smithay"]["revision"]
    packages = {"smithay": sources["smithay"], "smithay-drm-extras": sources["smithay"] / "smithay-drm-extras"}
    for package, path in packages.items():
        heading = f"[workspace.dependencies.{package}]\n"
        start = text.index(heading)
        end = text.find("\n[", start + len(heading))
        end = len(text) if end < 0 else end
        section = text[start:end]
        dependency = tomllib.loads(section)["workspace"]["dependencies"][package]
        if dependency.get("rev") != revision or dependency.get("git") != "https://github.com/Smithay/smithay.git":
            raise ValueError(f"Unexpected upstream binding for {package}")
        lines = [line for line in section.splitlines(keepends=True) if not line.startswith(("git = ", "rev = "))]
        lines.insert(1, "path = " + json.dumps(str(path)) + "\n")
        text = text[:start] + "".join(lines) + text[end:]
    lock = sources["niri"] / "Cargo.lock"
    before = lock.read_text() if inputs is None else inputs[1]
    blocks = before.split("[[package]]")
    changed = []
    for index in range(1, len(blocks)):
        package = tomllib.loads("[[package]]" + blocks[index])["package"][0]
        if package["name"] not in packages:
            continue
        expected_source = f"git+https://github.com/Smithay/smithay.git?rev={revision}#{revision}"
        if package.get("source") != expected_source:
            raise ValueError("Unexpected locked Smithay source")
        blocks[index] = re.sub(r'^source = "[^"\n]+"\n', "", blocks[index], flags=re.MULTILINE)
        changed.append(package["name"])
    if sorted(changed) != sorted(packages):
        raise ValueError("Unexpected locked package scope")
    after = "[[package]]".join(blocks)
    old_packages = tomllib.loads(before)["package"]
    new_packages = tomllib.loads(after)["package"]
    for old, new in zip(old_packages, new_packages, strict=True):
        expected = dict(old)
        if old["name"] in packages:
            expected.pop("source")
        if new != expected:
            raise ValueError("The prepared lockfile changed unrelated dependencies")
    # Validate both transformations before modifying either prepared file.
    if write:
        manifest.write_text(text)
        lock.write_text(after)
    return {"manifest_upstream_sha256": original, "manifest_prepared_sha256": digest(text.encode()),
            "lock_upstream_sha256": digest(before.encode()), "lock_prepared_sha256": digest(after.encode()),
            "local_source_packages": sorted(changed), "other_locked_dependencies_unchanged": True}
