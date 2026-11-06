"""Stable source paths for fork tests that Cargo excludes from the workspace."""
from contextlib import contextmanager
import fcntl
from pathlib import Path
import shutil
import tempfile

ROOT = Path(__file__).resolve().parents[1]


@contextmanager
def workspace(name):
    directory = ROOT / "target/vendor-test-sources"
    directory.mkdir(parents=True, exist_ok=True)
    # Keep source package identities and mtimes between runs. Serialize users of
    # each fixture so one check cannot change sources while another compiles.
    with (directory / f"{name}.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        snapshot = directory / name
        snapshot.mkdir(exist_ok=True)
        yield snapshot


def write_if_changed(path, data):
    if path.is_symlink():
        raise ValueError(f"Unexpected symlink in vendor test workspace: {path}")
    if path.exists() and path.read_bytes() == data:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".sync-", delete=False) as temporary:
        pending = Path(temporary.name)
        try:
            temporary.write(data)
            temporary.flush()
            pending.replace(path)
        finally:
            pending.unlink(missing_ok=True)


def sync_tree(source, destination, *, overrides=None):
    if source.is_symlink() or destination.is_symlink():
        raise ValueError("Vendor test source roots must not be symlinks")
    overrides = overrides or {}
    entries = {path.relative_to(source): path for path in source.rglob("*")}
    destination.mkdir(parents=True, exist_ok=True)
    for path in sorted(destination.rglob("*"), key=lambda path: len(path.parts), reverse=True):
        original = entries.get(path.relative_to(destination))
        if original is None or path.is_dir() != original.is_dir():
            if path.is_dir() and not path.is_symlink():
                shutil.rmtree(path)
            else:
                path.unlink()
    for relative, path in entries.items():
        target = destination / relative
        if path.is_symlink() or target.is_symlink():
            raise ValueError(f"Unexpected symlink in vendor test sources: {path}")
        if path.is_dir():
            target.mkdir(exist_ok=True)
        else:
            write_if_changed(target, overrides.get(str(relative), path.read_bytes()))
            mode = path.stat().st_mode & 0o777
            if target.stat().st_mode & 0o777 != mode:
                target.chmod(mode)
