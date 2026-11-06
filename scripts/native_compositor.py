"""Select and preserve the Niri executable used by private desktop acceptance."""
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

SNAPSHOT_STORE = Path(__file__).resolve().parents[1] / "target/native-test-snapshots/compositor-by-sha256"


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


def freeze_niri(source, checksum):
    """Keep one immutable copy per checksum, independent of mutable build outputs."""
    if not re.fullmatch(r"[0-9a-f]{64}", checksum):
        raise ValueError("Invalid Niri snapshot checksum")
    if sha256(Path(source)) != checksum:
        raise ValueError("Niri changed while preparing its acceptance snapshot")
    directory = SNAPSHOT_STORE / checksum
    directory.mkdir(parents=True, exist_ok=True)
    canonical = directory / "niri"
    if not canonical.exists() and not canonical.is_symlink():
        # Publish without overwriting another run's snapshot. Copy rather than
        # hardlink: Cargo may replace or write the original executable later.
        with tempfile.NamedTemporaryFile(dir=directory, prefix=".niri-", delete=False) as temporary:
            pending = Path(temporary.name)
            try:
                with Path(source).open("rb") as stream:
                    shutil.copyfileobj(stream, temporary)
                temporary.flush()
                if sha256(pending) != checksum:
                    raise ValueError("Niri changed while preparing its acceptance snapshot")
                pending.chmod(0o500)
                try:
                    os.link(pending, canonical)
                except FileExistsError:
                    pass
            finally:
                pending.unlink()
    if canonical.is_symlink() or not canonical.is_file() or sha256(canonical) != checksum:
        raise ValueError("Corrupt content-addressed Niri snapshot")
    if canonical.stat().st_mode & 0o222:
        raise ValueError("Content-addressed Niri snapshot must be read-only")
    return canonical


def stage_niri(selection, output):
    """Reference frozen bytes so repeated runs do not each retain a full binary."""
    canonical = freeze_niri(selection["source"], selection["sha256"])
    directory = output / "compositor" / "bin"
    directory.mkdir(parents=True)
    snapshot = directory / "niri"
    snapshot.symlink_to(canonical)
    return {**selection, "snapshot": str(snapshot)}
