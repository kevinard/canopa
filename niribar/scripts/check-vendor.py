#!/usr/bin/env python3
"""Compatibility entry point for the workspace vendor checker."""
from pathlib import Path
import runpy

runpy.run_path(str(Path(__file__).resolve().parents[2] / "scripts/check-vendor.py"), run_name="__main__")
