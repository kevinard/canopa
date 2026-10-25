"""Select and preserve the Niri executable used by private desktop acceptance."""
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def select_niri(path=None, expected_sha256=None):
    """Resolve and identify a binary; absence on PATH remains available to Sway-only runs."""
    if expected_sha256 is not None:
        expected_sha256 = expected_sha256.lower()
        if not re.fullmatch(r"[0-9a-f]{64}", expected_sha256):
            raise ValueError("--niri-sha256 must contain exactly 64 hexadecimal digits")
    selected = path if path is not None else shutil.which("niri")
    if selected is None:
        if expected_sha256:
            raise ValueError("Cannot check --niri-sha256: Niri was not found on PATH")
        return None
    source = Path(selected).resolve(strict=True)
    if not source.is_file() or not os.access(source, os.X_OK):
        raise ValueError(f"Niri must be an executable file: {source}")
    digest = sha256(source)
    if expected_sha256 and digest != expected_sha256:
        raise ValueError(f"Niri checksum mismatch: expected {expected_sha256}, got {digest}")
    version = subprocess.check_output([str(source), "--version"], text=True,
                                      stderr=subprocess.STDOUT, timeout=10).strip()
    if not version.startswith("niri "):
        raise ValueError(f"Selected executable did not identify itself as Niri: {source}")
    return {"source": str(source), "sha256": digest, "version": version,
            "expected_sha256": expected_sha256}


def stage_niri(selection, output):
    """Freeze the selected bytes so a later rebuild cannot change a running batch."""
    directory = output / "compositor" / "bin"
    directory.mkdir(parents=True)
    snapshot = directory / "niri"
    with Path(selection["source"]).open("rb") as source, snapshot.open("xb") as destination:
        shutil.copyfileobj(source, destination)
    if sha256(snapshot) != selection["sha256"]:
        raise ValueError("Niri changed while preparing its acceptance snapshot")
    snapshot.chmod(0o500)
    return {**selection, "snapshot": str(snapshot)}
