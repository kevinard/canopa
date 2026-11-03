#!/usr/bin/env python3
"""Start a dedicated Canopa systemd user session; never enable host services."""
import argparse
import json
import os
from pathlib import Path
import shutil
import select
import signal
import subprocess
import sys
import threading

VARIABLES = ("WAYLAND_DISPLAY", "DISPLAY", "NIRI_SOCKET", "NIRI_CONFIG", "CANOPA_SESSION",
             "CANOPA_SESSION_PID", "XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP", "XDG_SESSION_TYPE")
COMPOSITOR = Path("/usr/lib/canopa/niri")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--print-command", action="store_true", help="show the session contract without changes")
    args = parser.parse_args()
    config_home = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    if not config_home.is_absolute():
        config_home = Path.home() / ".config"
    config = config_home / "canopa/niri.kdl"
    environment = dict(CANOPA_SESSION="1", NIRI_CONFIG=str(config), XDG_CURRENT_DESKTOP="canopa:niri",
                       XDG_SESSION_DESKTOP="canopa", XDG_SESSION_TYPE="wayland")
    command = ["systemctl", "--user", "--wait", "start", "canopa-compositor.service"]
    if args.print_command:
        print(json.dumps({"environment": environment, "command": command}, indent=2))
        return 0
    for unit in ("canopa-compositor.service", "niri.service", "graphical-session.target"):
        if subprocess.run(["systemctl", "--user", "--quiet", "is-active", unit], check=False).returncode == 0:
            raise RuntimeError("A compositor session is already running; log out before starting Canopa.")
    settings = shutil.which("canopa-settings")
    if settings is None or not COMPOSITOR.is_file():
        raise RuntimeError("Install the Canopa desktop and compatible compositor packages first.")
    env = os.environ.copy()
    for name in ("GREETD_SOCK", "NOTIFY_SOCKET", "WAYLAND_DISPLAY", "DISPLAY", "NIRI_SOCKET", "SYSTEMD_EXEC_PID"):
        env.pop(name, None)
    env.update(environment)
    env["CANOPA_SESSION_PID"] = str(os.getpid())
    subprocess.run([settings, "init-session"], env=env, check=True)
    started = False
    waiter = None
    observer = None
    completed = []
    interrupted = []
    wake_read, wake_write = os.pipe2(os.O_CLOEXEC | os.O_NONBLOCK)
    previous_wakeup = signal.set_wakeup_fd(wake_write)
    previous = {number: signal.signal(number, lambda number, frame: interrupted.append(number))
                for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)}
    def cleanup(command):
        try:
            subprocess.run(command, check=False)
        except OSError as error:
            print(f"canopa-session: cleanup: {error}", file=sys.stderr)
    try:
        subprocess.run(["systemctl", "--user", "unset-environment", *VARIABLES], check=True)
        subprocess.run(["systemctl", "--user", "import-environment", *sorted(env)], env=env, check=True)
        subprocess.run(["dbus-update-activation-environment", "--all"], env=env, check=True)
        units = ["canopa-compositor.service", "canopa-session.target", *[f"canopa-{name}.service" for name in ("shell", "notify", "wallpaper", "lock", "polkit", "idle", "display-profiles")]]
        subprocess.run(["systemctl", "--user", "reset-failed", *units], check=True)
        if interrupted:
            return 128 + interrupted[0]
        started = True
        waiter = subprocess.Popen(command, env=env)
        def observe_exit():
            try:
                completed.append(waiter.wait())
            except OSError as error:
                completed.append(error)
            try: os.write(wake_write, b'\0')
            except OSError: pass
        observer = threading.Thread(target=observe_exit, daemon=True)
        observer.start()
        while True:
            if interrupted:
                return 128 + interrupted[0]
            if completed:
                if isinstance(completed[0], OSError):
                    raise completed[0]
                return completed[0]
            select.select([wake_read], [], [])
            os.read(wake_read, 4096)
    finally:
        try:
            if started:
                cleanup(["systemctl", "--user", "start", "--job-mode=replace-irreversibly", "canopa-shutdown.target"])
            if waiter is not None and waiter.poll() is None:
                waiter.terminate()
                try: waiter.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    waiter.kill(); waiter.wait()
            if observer is not None and observer.ident is not None:
                observer.join()
            cleanup(["systemctl", "--user", "unset-environment", *VARIABLES])
            cleanup(["dbus-update-activation-environment", *[f"{name}=" for name in VARIABLES]])
        finally:
            signal.set_wakeup_fd(previous_wakeup)
            for number, handler in previous.items():
                signal.signal(number, handler)
            os.close(wake_read)
            os.close(wake_write)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"canopa-session: {error}", file=sys.stderr)
        sys.exit(1)
