#!/usr/bin/env python3
"""Keep a user installation on successful builds of a local Git branch (Linux)."""

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time


BINARIES = ("niribar", "niribar-wifi-provider")


def run(*args, capture=False, **kwargs):
    return subprocess.run(
        [str(a) for a in args], check=True, text=True,
        stdout=subprocess.PIPE if capture else None, **kwargs,
    ).stdout


def atomic_json(path, value):
    temporary = path.with_suffix(".new")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    os.replace(temporary, path)


def point(link, target):
    temporary = link.with_name(link.name + ".new")
    temporary.unlink(missing_ok=True)
    temporary.symlink_to(target)
    os.replace(temporary, link)


def checksum(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def revision(config):
    return run(config["git"], "-C", config["repository"], "rev-parse", "--verify",
               "--end-of-options", config["ref"] + "^{commit}", capture=True).strip()


@contextlib.contextmanager
def update_lock(config):
    state = Path(config["state"])
    state.mkdir(parents=True, exist_ok=True)
    with (state / "update.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield


def service(config, *args, capture=False):
    return run(config["systemctl"], "--user", *args, capture=capture)


def active(config):
    return subprocess.run(
        [config["systemctl"], "--user", "is-active", "--quiet", "niribar.service"],
        check=False,
    ).returncode == 0


def healthy_restart(config, binary):
    service(config, "restart", "niribar.service")
    pid = None
    for _ in range(10):
        time.sleep(0.5)
        current = service(config, "show", "niribar.service", "--property=MainPID",
                          "--value", capture=True).strip()
        if not active(config) or not current.isdigit() or int(current) == 0:
            raise RuntimeError("Niribar did not remain active after restart")
        if pid is not None and current != pid:
            raise RuntimeError("Niribar restarted unexpectedly during its startup check")
        if Path(f"/proc/{current}/exe").resolve() != binary.resolve():
            raise RuntimeError("The service is running a different executable")
        pid = current


def prune_releases(data):
    keep = {link.resolve() for name in ("current", "previous")
            if (link := data / name).exists()}
    for path in (data / "releases").iterdir():
        if path.is_dir() and not path.is_symlink() and path.resolve() not in keep:
            shutil.rmtree(path)


def prepare_checkout(config, wanted):
    source = Path(config["cache"]) / "checkout"
    if source.is_symlink() or source.resolve() == Path(config["repository"]).resolve():
        raise RuntimeError("Refusing to use the author's working checkout as the build checkout")
    if not source.exists():
        run(config["git"], "-c", "core.hooksPath=/dev/null", "-C", config["repository"],
            "worktree", "add", "--detach", source, wanted)
    marker = source / ".git"
    if marker.is_symlink() or not marker.is_file():
        raise RuntimeError("The build checkout must be a separate linked Git worktree")
    common = run(config["git"], "-C", source, "rev-parse", "--path-format=absolute",
                 "--git-common-dir", capture=True).strip()
    expected = run(config["git"], "-C", config["repository"], "rev-parse",
                   "--path-format=absolute", "--git-common-dir", capture=True).strip()
    git_dir = Path(run(config["git"], "-C", source, "rev-parse", "--absolute-git-dir",
                       capture=True).strip())
    if (Path(common).resolve() != Path(expected).resolve()
            or Path((git_dir / "gitdir").read_text().strip()).resolve() != marker.resolve()):
        raise RuntimeError("Build checkout is not registered at its managed location")
    # Only this exclusively owned, validated worktree may be reset and cleaned.
    run(config["git"], "-c", "core.hooksPath=/dev/null", "-C", source,
        "checkout", "--detach", "--force", wanted)
    run(config["git"], "-C", source, "clean", "-ffdx")
    return source


def build_release(config, wanted):
    data, cache = (Path(config[k]) for k in ("data", "cache"))
    destination = data / "releases" / wanted
    if destination.exists() and destination.resolve() == (data / "previous").resolve():
        build = json.loads((destination / "build.json").read_text())
        if build["revision"] != wanted or any(
                checksum(destination / name) != build["sha256"][name] for name in BINARIES):
            raise RuntimeError("The retained release failed its integrity check")
        print(f"Reusing previous successful release {wanted[:12]}", flush=True)
        return destination
    source = prepare_checkout(config, wanted)
    target = cache / "target"
    print(f"Building committed revision {wanted[:12]}", flush=True)
    run(config["cargo"], "build", "--locked", "--release", "--bins", "--jobs", "2",
        "--target-dir", target, cwd=source)
    with tempfile.TemporaryDirectory(prefix=".stage-", dir=data) as temporary:
        stage = Path(temporary)
        for name in BINARIES:
            shutil.copy2(target / "release" / name, stage / name)
        # Exercise the real version command and provider handshake/Close.
        run("bash", source / "scripts/package-release.sh", stage, stage / "bundle", cwd=source)
        run("bash", source / "scripts/smoke-release.sh",
            stage / "bundle/niribar-x86_64-unknown-linux-gnu.tar.gz", cwd=source)
        shutil.rmtree(stage / "bundle")
        atomic_json(stage / "build.json", {
            "revision": wanted, "built_at": time.time(),
            "sha256": {name: checksum(stage / name) for name in BINARIES},
        })
        if destination.exists():
            # Failed activation may have left a complete, unused build here.
            if destination.resolve() == (data / "current").resolve():
                raise RuntimeError("Refusing to replace the current release")
            shutil.rmtree(destination)
        os.replace(stage, destination)
    return destination


@contextlib.contextmanager
def defer_termination():
    handlers = {sig: signal.signal(sig, signal.SIG_IGN)
                for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        yield
    finally:
        for sig, handler in handlers.items():
            signal.signal(sig, handler)


def rollback_activation(config, error):
    data, state = (Path(config[k]) for k in ("data", "state"))
    journal = state / "activation.json"
    pending = json.loads(journal.read_text())
    previous = Path(pending["previous"])
    with defer_termination():
        point(data / "current", previous)
        # A stopped service must stay stopped, including during immediate rollback.
        restart = pending["was_active"] and service(config, "show", "niribar.service",
            "--property=ActiveState", "--value", capture=True).strip() not in ("inactive", "deactivating")
        if restart:
            healthy_restart(config, previous / "niribar")
        atomic_json(state / "status.json", {"phase": "failed", "revision": pending["revision"],
                                            "time": time.time(), "error": str(error)})
        journal.unlink()


def activate(config, destination, wanted):
    data, state = (Path(config[k]) for k in ("data", "state"))
    current = data / "current"
    previous, was_active = current.resolve(), active(config)
    journal = state / "activation.json"
    atomic_json(journal, {"revision": wanted, "previous": str(previous), "was_active": was_active})
    try:
        point(current, destination)
        if was_active:
            healthy_restart(config, destination / "niribar")
        point(data / "previous", previous)
        atomic_json(state / "status.json", {"phase": "installed", "revision": wanted, "time": time.time()})
        journal.unlink()
    except BaseException as error:
        if journal.exists():
            rollback_activation(config, error)
        raise
    print(f"Installed {wanted[:12]}" + (" and restarted Niribar." if was_active else "."), flush=True)


def update(config, retry=False):
    data, state = (Path(config[k]) for k in ("data", "state"))
    if (state / "activation.json").exists():
        rollback_activation(config, "Recovered interrupted activation")
    wanted = revision(config)
    installed = json.loads((data / "current/build.json").read_text())
    if installed.get("revision") == wanted:
        print(f"Already installed: {wanted[:12]}", flush=True)
        return
    status_file = state / "status.json"
    last = json.loads(status_file.read_text()) if status_file.exists() else {}
    if (not retry and last.get("phase") == "failed" and last.get("revision") == wanted
            and time.time() - last["time"] < 900):
        print("Previous attempt failed; retrying after the 15-minute cooldown.", flush=True)
        return
    atomic_json(status_file, {"phase": "building", "revision": wanted, "time": time.time()})
    try:
        destination = build_release(config, wanted)
        if revision(config) != wanted:
            atomic_json(status_file, {"phase": "superseded", "revision": wanted,
                                      "time": time.time()})
            print("A newer commit arrived; leaving the installed version unchanged.", flush=True)
            return
        activate(config, destination, wanted)
    except Exception as error:
        atomic_json(status_file, {"phase": "failed", "revision": wanted,
                                  "time": time.time(), "error": str(error)})
        raise
    try:
        prune_releases(data)
    except OSError as error:
        print(f"Installed successfully, but could not remove old builds: {error}", file=sys.stderr)


def systemd_quote(value):
    return '"' + str(value).replace("\\", "\\\\").replace('"', '\\"').replace(
        "%", "%%").replace("$", "$$") + '"'


def install(args):
    home = Path.home()
    config_root = Path(os.environ.get("XDG_CONFIG_HOME", home / ".config"))
    config_file = args.config
    data = Path(os.environ.get("XDG_DATA_HOME", home / ".local/share")) / "niribar-dev"
    cache = Path(os.environ.get("XDG_CACHE_HOME", home / ".cache")) / "niribar-dev"
    state = Path(os.environ.get("XDG_STATE_HOME", home / ".local/state")) / "niribar-dev"
    config = {"repository": str(args.repository.resolve()), "ref": args.ref,
              "data": str(data), "cache": str(cache), "state": str(state)}
    for tool in ("git", "cargo", "systemctl"):
        config[tool] = shutil.which(tool)
        if config[tool] is None:
            raise RuntimeError(f"Required program is missing: {tool}")
    revision(config)
    with update_lock(config):
        for directory in (data / "releases", cache, config_file.parent):
            directory.mkdir(parents=True, exist_ok=True)
        bin_dir = home / ".local/bin"
        original = data / "original"
        if not (data / "current").exists():
            if not (bin_dir / "niribar").is_file():
                raise RuntimeError("This setup expects an existing ~/.local/bin/niribar installation")
            original.mkdir(exist_ok=False)
            for name in BINARIES:
                if (bin_dir / name).is_file():
                    shutil.copy2(bin_dir / name, original / name)
            atomic_json(original / "build.json", {"revision": None, "origin": "pre-development installation"})
            point(data / "current", original)
        if args.seed_cache and not (cache / "target").exists():
            (cache / "target").mkdir()
            run("cp", "-a", "--reflink=auto", args.repository / "target/release",
                cache / "target/release")
        atomic_json(config_file, config)
        updater = data / "updater.py"
        shutil.copy2(Path(__file__), updater.with_suffix(".new"))
        updater.with_suffix(".new").chmod(0o755)
        os.replace(updater.with_suffix(".new"), updater)
        for name in BINARIES:
            point(bin_dir / name, data / "current" / name)
        point(bin_dir / "niribar-dev", updater)
        units = config_root / "systemd/user"
        override = units / "niribar.service.d/50-development.conf"
        override.parent.mkdir(parents=True, exist_ok=True)
        override.write_text("[Service]\nExecStart=\nExecStart=" + systemd_quote(data / "current/niribar") + "\nRestartSec=3\n")
        (units / "niribar-dev-update.service").write_text(
            "[Unit]\nDescription=Build and install committed Niribar updates\n"
            "\n[Service]\nType=oneshot\nNice=10\nIOSchedulingClass=idle\nTimeoutStartSec=45min\n"
            f"ExecStart={systemd_quote(sys.executable)} {systemd_quote(updater)} update --config {systemd_quote(config_file)}\n")
        (units / "niribar-dev-update.timer").write_text(
            "[Unit]\nDescription=Follow local Niribar commits\nPartOf=graphical-session.target\n"
            "\n[Timer]\nOnStartupSec=30s\nOnUnitInactiveSec=60s\nAccuracySec=5s\n"
            "\n[Install]\nWantedBy=graphical-session.target\n")
    service(config, "daemon-reload")
    service(config, "enable", "--now", "niribar-dev-update.timer")
    service(config, "start", "--no-block", "niribar-dev-update.service")
    print(f"Following {config['ref']} in {config['repository']}. Use niribar-dev status.")


def status(config):
    data = Path(config["data"])
    result = {"repository": config["repository"], "ref": config["ref"],
              "desired_revision": revision(config),
              "installed": json.loads((data / "current/build.json").read_text())}
    state = Path(config["state"]) / "status.json"
    result["update"] = json.loads(state.read_text()) if state.exists() else {"phase": "pending"}
    result["service_active"] = active(config)
    if result["service_active"]:
        pid = service(config, "show", "niribar.service", "--property=MainPID", "--value", capture=True).strip()
        result["running_pid"] = pid
        result["running_installed_binary"] = Path(f"/proc/{pid}/exe").resolve() == (data / "current/niribar").resolve()
    print(json.dumps(result, indent=2))


def main():
    def interrupted(signum, _frame):
        raise InterruptedError(f"Update interrupted by signal {signum}")

    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", nargs="?", choices=("install", "update", "status"), default="status")
    parser.add_argument("--config", type=Path, default=Path(os.environ.get(
        "XDG_CONFIG_HOME", Path.home() / ".config")) / "niribar-dev/config.json")
    parser.add_argument("--repository", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--ref", default="refs/heads/main")
    parser.add_argument("--seed-cache", action="store_true", help="copy the existing release build cache once")
    parser.add_argument("--retry", action="store_true", help="bypass the failed-build cooldown")
    args = parser.parse_args()
    try:
        if args.command == "install":
            install(args)
        else:
            config = json.loads(args.config.read_text())
            if args.command == "status":
                status(config)
            else:
                with update_lock(config):
                    update(config, args.retry)
    except BlockingIOError:
        parser.exit(1, "Another Niribar update is already running.\n")
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Niribar development update failed: {error}\n")


if __name__ == "__main__":
    main()
