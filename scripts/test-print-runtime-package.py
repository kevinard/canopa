#!/usr/bin/env python3
"""Exercise optional runtime staging and refusal using a verified CUPS build."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TOOL = ROOT / "scripts/package-print-runtime.py"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-receipt", type=Path, required=True)
    parser.add_argument("--binary", type=Path, help="Also check the shipped library with actual Rust workers")
    args = parser.parse_args()
    receipt_path = args.build_receipt.resolve()
    checks = []
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")

    def invoke(bundle, *options, refusal=None):
        result = subprocess.run(["python3", str(TOOL), str(bundle), *map(str, options)],
                                cwd=ROOT, env=env, capture_output=True, text=True, timeout=30)
        if refusal:
            assert result.returncode == 1 and refusal in result.stderr, result.stderr
        else:
            assert result.returncode == 0, result.stderr
        return result

    with tempfile.TemporaryDirectory(prefix="canopa-print-runtime-package-") as temporary:
        base = Path(temporary)
        empty = base / "empty"
        empty.mkdir()
        invoke(empty, "--check")
        invoke(empty, "--check", "--required", refusal="Missing packaged Print runtime")
        checks.append("absent-optional-and-required")

        bundle = base / "relocated-prefix"
        invoke(bundle, "--build-receipt", receipt_path)
        invoke(bundle, "--check", "--required")
        manifest = json.loads((bundle / "share/canopa/print-runtime.json").read_text())
        assert str(base) not in json.dumps(manifest)
        assert str(ROOT) not in json.dumps(manifest)
        pin = json.loads((ROOT / "packaging/forks.json").read_text())["cups"]
        assert manifest["upstream"]["commit"] == pin["commit"]
        assert manifest["upstream"]["archive_sha256"] == pin["archive_sha256"]
        changes = (bundle / "share/licenses/canopa-print-cups/CHANGES.md").read_text()
        assert pin["repository"] in changes and pin["commit"] in changes and pin["base"] in changes
        checks.append("staged-and-relocatable-inventory")
        invoke(bundle, "--build-receipt", receipt_path, refusal="destination already exists")
        checks.append("existing-runtime-refused")

        if args.binary:
            subprocess.run(["python3", str(ROOT / "scripts/test-cups-authentication.py"),
                            "--binary", str(args.binary.resolve()),
                            "--library", str(bundle / manifest["library"]["path"]),
                            "--expected-library-sha256", manifest["library"]["sha256"],
                            "--case", "plain", "--case", "realm-combined",
                            "--case", "maximum-utf8", "--artifacts", str(base / "native")],
                           cwd=ROOT, env=env, check=True, timeout=90)
            checks.append("actual-worker-packaged-image")

        for name in [manifest["library"]["path"],
                     "share/licenses/canopa-print-cups/LICENSE",
                     "share/licenses/canopa-print-cups/NOTICE",
                     "share/licenses/canopa-print-cups/CHANGES.md"]:
            path = bundle / name
            original = path.read_bytes()
            path.write_bytes(original + b"changed")
            invoke(bundle, "--check", "--required", refusal="differs from its inventory")
            path.write_bytes(original)
        checks.append("library-license-notice-changes-tampering-refused")

        notice = bundle / "share/licenses/canopa-print-cups/NOTICE"
        original = notice.read_bytes()
        notice.unlink()
        invoke(bundle, "--check", "--required", refusal="No such file")
        external = base / "external-notice"
        external.write_bytes(original)
        notice.symlink_to(external)
        invoke(bundle, "--check", "--required", refusal="Expected a regular file")
        notice.unlink()
        notice.write_bytes(original)
        checks.append("missing-and-symlink-notice-refused")

        partial = base / "partial"
        (partial / "lib/canopa/print").mkdir(parents=True)
        invoke(partial, "--check", refusal="No such file")
        checks.append("partial-runtime-not-silently-absent")

        linked = base / "linked-prefix"
        linked.mkdir()
        external_directory = base / "external-directory"
        external_directory.mkdir()
        (linked / "lib").symlink_to(external_directory, target_is_directory=True)
        invoke(linked, "--build-receipt", receipt_path, refusal="directory symlink")
        assert not list(external_directory.iterdir())
        assert not (linked / "share").exists()
        checks.append("directory-symlink-refused-before-writing")

        original = json.loads(receipt_path.read_text())
        inputs = (receipt_path.parent / "source-inputs.json").read_bytes()
        for key, value, message in [
            ("status", "prepared", "incomplete or belongs to another build"),
            ("origin", {**original["origin"], "commit": "0" * 40}, "not the pinned fork commit"),
            ("binary_sha256", "0" * 64, "differs from its build receipt"),
            ("source_inputs_sha256", "0" * 64, "Source inventory differs"),
        ]:
            (base / key).mkdir()
            bad = base / key / "canopa-build.json"
            bad.write_text(json.dumps({**original, key: value}))
            (base / key / "source-inputs.json").write_bytes(inputs)
            refused = base / (key + "-refused")
            invoke(refused, "--build-receipt", bad, refusal=message)
            assert not refused.exists()
        checks.append("incomplete-stale-and-mismatched-builds-refused-before-staging")

        invoke(bundle, "--check", "--required")
    print(json.dumps(dict(status="passed", checks=checks, private_temporary_data_removed=True)))


if __name__ == "__main__":
    main()
