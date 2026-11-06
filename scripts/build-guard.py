#!/usr/bin/env python3
"""Run a command while preserving free space on its build filesystems."""

import argparse
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time


GIB = 1024 ** 3
SPACE_FAILURE = 75


def positive_number(value):
    number = float(value)
    if not 0 < number < float("inf"):
        raise argparse.ArgumentTypeError("must be a finite number greater than zero")
    return number


def existing_parent(path):
    path = Path(path).resolve()
    while not path.exists():
        if path == path.parent:
            raise OSError(f"no existing parent for {path}")
        path = path.parent
    return path


def build_paths(command, extra_paths=()):
    # Include the checkout as well as output directories on other mounts.
    paths = [Path.cwd(), Path.cwd() / "target", *map(Path, extra_paths)]
    for variable in ("CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
        if os.environ.get(variable):
            paths.append(Path(os.environ[variable]))
    paths.append(Path(os.environ.get("CARGO_HOME") or Path.home() / ".cargo"))
    for index, argument in enumerate(command):
        if argument == "--":
            break  # Cargo run/test arguments are not Cargo output options.
        if argument.startswith("--target-dir="):
            paths.append(Path(argument.split("=", 1)[1]))
        elif argument == "--target-dir" and index + 1 < len(command):
            paths.append(Path(command[index + 1]))
    return paths


def check_space(paths, minimum, temporary, temporary_minimum):
    # Deduplicate mounts, applying the larger reserve when paths share one.
    filesystems = {}
    for path, reserve in [*((path, minimum) for path in paths), (temporary, temporary_minimum)]:
        parent = existing_parent(path)
        device = parent.stat().st_dev
        if device not in filesystems or reserve > filesystems[device][1]:
            filesystems[device] = (parent, reserve)
    for path, reserve in filesystems.values():
        available = shutil.disk_usage(path).free
        if available < reserve * GIB:
            raise OSError(f"{path}: {available / GIB:.2f} GiB available; "
                          f"{reserve:g} GiB free-space reserve required")


def send_group(process, signum):
    try:
        os.killpg(process.pid, signum)
    except ProcessLookupError:
        pass


def stop_group(process, signum, grace):
    send_group(process, signum)
    deadline = time.monotonic() + grace
    while time.monotonic() < deadline:
        process.poll()  # Reap the leader, but still stop surviving children.
        try:
            os.killpg(process.pid, 0)
        except ProcessLookupError:
            break
        time.sleep(min(0.05, max(0, deadline - time.monotonic())))
    send_group(process, signal.SIGKILL)
    process.wait()


def guarded_run(command, paths, minimum=10, temporary_minimum=1, interval=1, grace=5):
    temporary = Path(tempfile.gettempdir())

    def check():
        check_space(paths, minimum, temporary, temporary_minimum)

    try:
        check()
    except OSError as error:
        print(f"build-guard: refusing to start: {error}", file=sys.stderr, flush=True)
        return SPACE_FAILURE

    received = []

    def interrupted(signum, _frame):
        received.append(signum)

    previous = {signum: signal.signal(signum, interrupted)
                for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)}
    process = None
    try:
        process = subprocess.Popen(command, start_new_session=True)
        while True:
            if received:
                stop_group(process, received[0], grace)
                return 128 + received[0]
            code = process.poll()
            if code is not None:
                return code if code >= 0 else 128 - code
            try:
                check()
            except OSError as error:
                print(f"build-guard: stopping command: {error}", file=sys.stderr, flush=True)
                stop_group(process, signal.SIGTERM, grace)
                return SPACE_FAILURE
            try:
                process.wait(timeout=interval)
            except subprocess.TimeoutExpired:
                pass
    finally:
        if process is not None and process.poll() is None:
            stop_group(process, signal.SIGTERM, grace)
        for signum, handler in previous.items():
            signal.signal(signum, handler)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--min-free-gib", type=positive_number,
                        default=os.environ.get("CANOPA_BUILD_MIN_FREE_GIB", "10"),
                        help="build filesystem reserve (default: 10, or CANOPA_BUILD_MIN_FREE_GIB)")
    parser.add_argument("--min-tmp-free-gib", type=positive_number,
                        default=os.environ.get("CANOPA_BUILD_MIN_TMP_FREE_GIB", "1"),
                        help="temporary filesystem reserve (default: 1, or CANOPA_BUILD_MIN_TMP_FREE_GIB)")
    parser.add_argument("--path", type=Path, action="append", default=[],
                        help="additional output/cache directory to monitor (repeatable)")
    parser.add_argument("command", nargs=argparse.REMAINDER, help="-- COMMAND [ARGS...]")
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a command is required after --")
    try:
        return guarded_run(command, build_paths(command, args.path),
                           args.min_free_gib, args.min_tmp_free_gib)
    except OSError as error:
        print(f"build-guard: {error}", file=sys.stderr)
        return 127


if __name__ == "__main__":
    sys.exit(main())
