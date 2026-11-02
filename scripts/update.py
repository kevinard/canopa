#!/usr/bin/env python3
"""Update the standard local Canopa companions from this checkout."""

import argparse
import fcntl
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
PROGRAMS = ("canopa-shell", "canopa-wifi-provider", "canopa-audio-provider", "canopa-media-provider", "canopa-notify", "canopa-files", "canopa", "canopa-wallpaper", "canopa-lock", "canopa-idle", "canopa-greeter", "canopa-polkit", "canopa-pinentry")
SERVICES = ("canopa-notify.service", "canopa-shell.service", "canopa-wallpaper.service", "canopa-lock.service", "canopa-polkit.service", "canopa-idle.service")


def run(*args, check=True, timeout=60):
    return subprocess.run([str(arg) for arg in args], cwd=ROOT, check=check,
                          capture_output=True, text=True, timeout=timeout)


def systemctl(*args, **kwargs):
    return run("systemctl", "--user", *args, **kwargs)


def property_value(service, name):
    return systemctl("show", service, f"--property={name}", "--value").stdout.strip()


def xdg_directory(variable, fallback):
    value = os.environ.get(variable, "")
    return Path(value) if value and Path(value).is_absolute() else Path.home() / fallback


def positive_jobs(value):
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("jobs must be at least 1")
    return number


def build_cache_state(packages):
    # Observe Cargo's records without interpreting their contents. An outside
    # build can replace local artifacts with another checkout's code, even when
    # Cargo considers the sources here fresh. Such writes invalidate our record.
    fingerprints = {}
    directory = ROOT / "target/release/.fingerprint"
    for package in directory.glob("*"):
        if package.name.rpartition("-")[0] in packages and package.is_dir():
            for path in package.iterdir():
                if path.is_file():
                    stat = path.stat()
                    fingerprints[str(path.relative_to(directory))] = [
                        stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns]
    return {"version": 1, "checkout": str(ROOT), "packages": packages,
            "fingerprints": fingerprints}


def build(jobs, rebuild=False):
    # Full metadata downloads registry sources for other platforms too. Only
    # workspace manifests and our managed forks are needed for local cleanup.
    metadata = json.loads(run("cargo", "metadata", "--locked", "--no-deps", "--offline",
                              "--format-version=1").stdout)
    forks = json.loads((ROOT / "vendor/provenance.json").read_text())["crates"]
    packages = sorted({package["name"] for package in metadata["packages"]} | forks.keys())
    cache = ROOT / "target/.canopa-update-cache.json"
    try:
        previous = json.loads(cache.read_text())
    except (FileNotFoundError, ValueError):
        previous = None
    reuse = previous == build_cache_state(packages)
    # Failed/interrupted builds must not leave a trusted record behind.
    cache.unlink(missing_ok=True)
    if rebuild or not reuse:
        print("Refreshing local release build cache; keeping registry dependencies.", flush=True)
        clean = ["cargo", "clean", "--locked", "--release", "--target-dir", "target"]
        for package in packages:
            clean.extend(["--package", package])
        subprocess.run(clean, cwd=ROOT, check=True)
    else:
        print("Reusing local release build cache; Cargo will rebuild changed sources.", flush=True)
    subprocess.run(["bash", str(ROOT / "scripts/cargo.sh"), "build", "--locked", "--release", "--bins",
                    "--package", "canopa-shell", "--package", "canopa-notify",
                    "--package", "canopa-cli", "--package", "canopa-files", "--package", "canopa-session", "--package", "canopa-greeter", "--package", "canopa-polkit", "--package", "canopa-pinentry", "--jobs", str(jobs),
                    "--target-dir", "target"], cwd=ROOT, check=True)
    cache.write_text(json.dumps(build_cache_state(packages)) + "\n")


def copy_binary(source, destination):
    fd, name = tempfile.mkstemp(prefix=f".{destination.name}-", dir=destination.parent)
    pending = Path(name)
    try:
        with os.fdopen(fd, "wb") as output, source.open("rb") as input_file:
            shutil.copyfileobj(input_file, output)
            os.fchmod(output.fileno(), 0o755)
            output.flush()
            os.fsync(output.fileno())
        pending.replace(destination)
    finally:
        pending.unlink(missing_ok=True)


def verify_services(bin_dir, restarted):
    pids = {}
    for service in restarted:
        pid = int(property_value(service, "MainPID"))
        if pid <= 0:
            raise RuntimeError(f"{service} did not start; check journalctl --user -u {service}")
        pids[service] = pid
    if "canopa-notify.service" in restarted:
        # Service mode can take ten seconds to establish graphical readiness.
        deadline = time.monotonic() + 20
        while True:
            result = run(bin_dir / "canopa-notify", "ctl", "status", "--json", check=False,
                         timeout=max(1, deadline - time.monotonic()))
            if result.returncode == 0:
                status = json.loads(result.stdout)
                if (not status["available"] or status["presentation_error"]
                        or status["sender_publication_error"] or status["configuration"]["error"]):
                    raise RuntimeError("Notify reports a startup or rendering error; check its journal")
                break
            if time.monotonic() >= deadline:
                raise RuntimeError("Notify's control socket did not become ready")
            time.sleep(0.2)
    if restarted:
        time.sleep(2)
    for service, pid in pids.items():
        active = systemctl("is-active", "--quiet", service, check=False).returncode == 0
        if not active or property_value(service, "MainPID") != str(pid):
            raise RuntimeError(f"{service} failed during startup or restarted")
        binary = bin_dir / service.removesuffix(".service")
        if not Path(f"/proc/{pid}/exe").samefile(binary):
            raise RuntimeError(f"{service} is running a different executable; set ExecStart to {binary}")
        print(f"{service} is running the installed executable (PID {pid}).", flush=True)


def update(jobs, rebuild=False):
    os.umask(0o077)
    states = {}
    for service in SERVICES:
        if property_value(service, "LoadState") != "loaded":
            if service in ("canopa-wallpaper.service", "canopa-lock.service", "canopa-polkit.service", "canopa-idle.service"):
                continue  # New optional companion; existing installations can add its unit later.
            raise RuntimeError(
                f"{service} is not loaded; install the companion user services first "
                "(existing Niribar installations: see docs/shell-rename.md)")
        state = property_value(service, "ActiveState")
        if state not in ("active", "inactive", "failed"):
            raise RuntimeError(f"{service} is {state}; retry when it has settled")
        states[service] = state

    services = tuple(states)
    build(jobs, rebuild)
    bin_dir = Path.home() / ".local/bin"
    bin_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".canopa-update-", dir=bin_dir) as temporary:
        stage = Path(temporary)
        for name in PROGRAMS:
            shutil.copy2(ROOT / "target/release" / name, stage / name)
        # The sibling validators must come from the same release. Validate before
        # touching live binaries, notification state or running services.
        validated = run(stage / "canopa", "config", "check", "--show-source")
        print(validated.stdout, end="", flush=True)

        # Respect service changes made while the build ran.
        for service in services:
            state = property_value(service, "ActiveState")
            if state not in ("active", "inactive", "failed"):
                raise RuntimeError(f"{service} is {state}; retry when it has settled")
            states[service] = state
        restarted = [service for service in services if states[service] != "inactive"]
        state_root = xdg_directory("XDG_STATE_HOME", ".local/state")
        state_dir = state_root / "canopa-notify"
        backup_root = state_root / "canopa-backups"
        backup_root.mkdir(parents=True, exist_ok=True)
        backup = Path(tempfile.mkdtemp(prefix=time.strftime("%Y%m%d-%H%M%S-"), dir=backup_root))
        print(f"Backup: {backup}", flush=True)
        (backup / "bin").mkdir()
        for name in PROGRAMS:
            if (bin_dir / name).exists():
                shutil.copy2(bin_dir / name, backup / "bin" / name)
        config = xdg_directory("XDG_CONFIG_HOME", ".config") / "canopa/config.kdl"
        if config.exists():
            shutil.copy2(config, backup / "config.kdl")
        (backup / "services.json").write_text(json.dumps(states, indent=2) + "\n")
        (backup / "config-check.jsonl").write_text(validated.stdout)

        replaced = False
        state_saved = False
        try:
            systemctl("stop", *services)
            if state_dir.exists():
                shutil.copytree(state_dir, backup / "notify-state", symlinks=True)
                state_saved = True
            replaced = True
            for name in PROGRAMS:
                (stage / name).replace(bin_dir / name)
            for service in restarted:
                if states[service] == "failed":
                    systemctl("reset-failed", service)
                systemctl("start", service)
            verify_services(bin_dir, restarted)
        except BaseException:
            if replaced:
                print("Update failed; restoring the previous binaries and notification state.",
                      file=sys.stderr, flush=True)
                # Never touch a database unless both services have stopped.
                systemctl("stop", *services)
                if state_dir.exists():
                    state_dir.rename(backup / "failed-update-notify-state")
                if state_saved:
                    shutil.copytree(backup / "notify-state", state_dir, symlinks=True)
                for name in PROGRAMS:
                    previous = backup / "bin" / name
                    if previous.exists():
                        copy_binary(previous, bin_dir / name)
                    else:
                        (bin_dir / name).unlink(missing_ok=True)
            for service in restarted:
                systemctl("reset-failed", service, check=False)
                systemctl("start", service, check=False)
            raise
        for service in services:
            if service not in restarted:
                print(f"{service} is inactive; leaving it stopped.")
        print(f"Canopa updated in {bin_dir}. Configuration and user service definitions preserved.")


def main():
    parser = argparse.ArgumentParser(prog="scripts/update.sh", description=__doc__, epilog=
        "Builds the current checkout, including uncommitted changes; does not fetch Git updates. "
        "Requires the standard ~/.local/bin companion services and default config/state discovery.")
    parser.add_argument("--jobs", type=positive_jobs,
                        default=os.environ.get("CARGO_BUILD_JOBS", "1"),
                        help="parallel Cargo jobs (default: CARGO_BUILD_JOBS or 1)")
    parser.add_argument("--rebuild", action="store_true",
                        help="force rebuilding local packages, keeping registry dependencies cached")
    args = parser.parse_args()
    try:
        os.umask(0o077)
        bin_dir = Path.home() / ".local/bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        with (bin_dir / ".canopa-update.lock").open("a") as lock:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise RuntimeError("another Canopa update is already running") from None
            update(args.jobs, args.rebuild)
    except subprocess.TimeoutExpired as error:
        command = shlex.join(str(arg) for arg in error.cmd)
        print(f"Update failed: command timed out after {error.timeout:g} seconds: {command}",
              file=sys.stderr)
        detail = error.stderr
        if isinstance(detail, bytes):
            detail = detail.decode("utf-8", errors="replace")
        if detail:
            print(detail.rstrip(), file=sys.stderr)
        return 1
    except (OSError, RuntimeError, ValueError, KeyError, subprocess.SubprocessError) as error:
        detail = getattr(error, "stderr", None) or str(error)
        print(f"Update failed: {detail}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("Update interrupted.", file=sys.stderr)
        return 130
    return 0


if __name__ == "__main__":
    sys.exit(main())
