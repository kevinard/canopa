#!/usr/bin/env python3
"""Check source-overlay recipes and patches against their recorded provenance.

Rust crate forks are pinned git dependencies in the root Cargo.toml, and
Cargo.lock records their exact commits, so only the overlay recipes for
separately built upstream programs are checked here.
"""

import argparse
from pathlib import Path
import tarfile

import vendor_sources


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--archives", type=Path,
        help="also verify the pinned source archives in this directory (no downloads)",
    )
    args = parser.parse_args()
    try:
        errors = vendor_sources.check(Path(__file__).resolve().parents[1], args.archives)
    except (OSError, ValueError, tarfile.TarError) as error:
        parser.exit(1, f"Vendor verification failed: {error}\n")
    if errors:
        parser.exit(1, "\n".join(errors) + "\n")
    print("Source overlays verified" + (
        " against pinned archives." if args.archives else " against provenance manifest."
    ))


if __name__ == "__main__":
    main()
