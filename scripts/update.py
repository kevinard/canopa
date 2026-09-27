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
PROGRAMS = ("niribar", "niribar-wifi-provider", "canopa-notify", "canopa")
SERVICES = ("canopa-notify.service", "niribar.service")


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


def build(jobs):
    # Full metadata downloads registry sources for other platforms too. Only
    # workspace manifests and our managed forks are needed for local cleanup.
    metadata = json.loads(run("cargo", "metadata", "--locked", "--no-deps", "--offline",
                              "--format-version=1").stdout)
    forks = json.loads((ROOT / "vendor/provenance.json").read_text())["crates"]
    # Invalidate all local packages: Cargo can reuse another checkout's
    # fingerprints in a shared target directory. Keep registry dependencies.
    packages = sorted({package["name"] for package in metadata["packages"]} | forks.keys())
    clean = ["cargo", "clean", "--locked", "--release", "--target-dir", "target"]
    for package in packages:
        clean.extend(["--package", package])
    subprocess.run(clean, cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "--locked", "--release", "--bins",
                    "--package", "niribar", "--package", "canopa-notify",
                    "--package", "canopa-cli", "--jobs", str(jobs),
                    "--target-dir", "target"], cwd=ROOT, check=True)


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


def update(jobs):
    os.umask(0o077)
    states = {}
    for service in SERVICES:
        if property_value(service, "LoadState") != "loaded":
            raise RuntimeError(f"{service} is not loaded; install the companion user services first")
        state = property_value(service, "ActiveState")
        if state not in ("active", "inactive", "failed"):
            raise RuntimeError(f"{service} is {state}; retry when it has settled")
        states[service] = state

    build(jobs)
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
        for service in SERVICES:
            state = property_value(service, "ActiveState")
            if state not in ("active", "inactive", "failed"):
                raise RuntimeError(f"{service} is {state}; retry when it has settled")
            states[service] = state
        restarted = [service for service in SERVICES if states[service] != "inactive"]
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
            systemctl("stop", *SERVICES)
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
                systemctl("stop", *SERVICES)
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
        for service in SERVICES:
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
            update(args.jobs)
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
