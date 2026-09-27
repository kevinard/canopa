#!/usr/bin/env python3
"""Measure Niribar in a private software-rendered Weston/Niri session (Linux)."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import tempfile
import time


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def stop(process):
    # Each launched service owns a process group, including any helper children.
    try:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=5)
    except ProcessLookupError:
        process.wait()
    except subprocess.TimeoutExpired:
        pass
    finally:
        # Helpers may outlive a leader that exits promptly on SIGTERM.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()


class Session:
    def __init__(self, runtime, output):
        self.runtime = runtime
        self.output = output
        self.processes = []
        self.env = os.environ.copy()
        for key in (
            "DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "NIRI_SOCKET",
            "DBUS_SESSION_BUS_ADDRESS", "AT_SPI_BUS_ADDRESS", "SESSION_MANAGER",
            "NOTIFY_SOCKET", "RUST_LOG", "NIRIBAR_PERF",
        ):
            self.env.pop(key, None)
        self.env.update(
            XDG_RUNTIME_DIR=str(runtime),
            XDG_CONFIG_HOME=str(runtime / "config"),
            XDG_DATA_HOME=str(runtime / "data"),
            XDG_STATE_HOME=str(runtime / "state"),
            XDG_CACHE_HOME=str(runtime / "cache"),
            DBUS_SYSTEM_BUS_ADDRESS=f"unix:path={runtime}/no-system-bus",
            PULSE_SERVER=f"unix:{runtime}/no-pulse",
            LIBGL_ALWAYS_SOFTWARE="1",
            WGPU_BACKEND="gl",
        )

    def start(self, command, log):
        with (self.output / log).open("w") as stream:
            process = subprocess.Popen(
                command, env=self.env, stdout=stream, stderr=stream,
                start_new_session=True,
            )
        self.processes.append(process)
        return process

    def run(self, command):
        return subprocess.check_output(
            command, env=self.env, text=True, stderr=subprocess.PIPE, timeout=10,
        )

    def wait_for(self, condition, description, timeout=15):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            for process in self.processes:
                if process.poll() is not None:
                    raise RuntimeError(f"Process {process.args} exited: {process.returncode}")
            result = condition()
            if result:
                return result
            time.sleep(0.05)
        raise RuntimeError(f"Timed out waiting for {description}")

    def layers(self):
        return [
            layer for layer in json.loads(self.run(["niri", "msg", "--json", "layers"]))
            if layer["namespace"] == "niribar"
        ]

    def wait_for_layers(self, count):
        return self.wait_for(lambda: len(self.layers()) == count, f"{count} Niribar layers")

    def screenshot(self, name):
        path = self.output / name
        self.run([
            "niri", "msg", "action", "screenshot-screen", "--show-pointer", "false",
            "--path", str(path),
        ])
        self.wait_for(lambda: path.exists() and path.stat().st_size > 0, "screenshot")

    def retire(self, process):
        stop(process)
        self.processes.remove(process)

    def close(self):
        errors = []
        for process in reversed(self.processes):
            try:
                stop(process)
            except (OSError, subprocess.SubprocessError) as error:
                errors.append(f"{process.args}: {error}")
        if errors:
            message = "Failed to retire services: " + "; ".join(errors)
            write_json(self.output / "summary.json", {"status": "failed", "error": message})
            raise RuntimeError(message)


def sample_process(pid):
    fields = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()
    if fields[0] in ("Z", "X"):
        raise RuntimeError("Niribar exited while being measured")
    return {
        "monotonic_seconds": time.monotonic(),
        "cpu_ticks": int(fields[11]) + int(fields[12]),
        "rss_kib": int(fields[21]) * os.sysconf("SC_PAGE_SIZE") / 1024,
        "start_ticks": int(fields[19]),
    }


def measure(pid, seconds):
    samples = [sample_process(pid)]
    for _ in range(seconds):
        time.sleep(1)
        samples.append(sample_process(pid))
    first, last = samples[0], samples[-1]
    if any(sample["start_ticks"] != first["start_ticks"] for sample in samples):
        raise RuntimeError("Process identity changed during measurement")
    elapsed = last["monotonic_seconds"] - first["monotonic_seconds"]
    return {
        "elapsed_seconds": elapsed,
        "cpu_percent_one_core": 100 * (last["cpu_ticks"] - first["cpu_ticks"])
        / os.sysconf("SC_CLK_TCK") / elapsed,
        "rss_median_kib": statistics.median(s["rss_kib"] for s in samples),
        "rss_range_kib": [min(s["rss_kib"] for s in samples), max(s["rss_kib"] for s in samples)],
        "samples": samples,
    }


def validate(args, session):
    config = session.runtime / "config/niribar/config.kdl"
    config.parent.mkdir(parents=True)
    config.write_bytes(args.config.read_bytes())
    niri_config = session.runtime / "niri.kdl"
    niri_config.write_text(
        "prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n"
        "hotkey-overlay { skip-at-startup; }\n"
    )
    session.start(["dbus-daemon", "--session", "--nofork", "--print-address=1"], "dbus.log")
    address = session.wait_for(
        lambda: (session.output / "dbus.log").read_text().strip(), "private session bus",
    ).splitlines()[0]
    if not address.startswith("unix:"):
        raise RuntimeError(f"Invalid private bus address: {address}")
    session.env["DBUS_SESSION_BUS_ADDRESS"] = address
    session.start([
        "weston", "--backend=headless", "--renderer=pixman", "--fake-seat",
        "--width=1280", "--height=800", "--socket=wayland-parent", "--no-config",
        "--idle-time=0",
    ], "weston.log")
    session.wait_for(lambda: (session.runtime / "wayland-parent").is_socket(), "Weston")
    session.env["WAYLAND_DISPLAY"] = "wayland-parent"
    session.start(["niri", "-c", str(niri_config)], "niri.log")
    sockets = session.wait_for(lambda: list(session.runtime.glob("niri.*.sock")), "Niri IPC")
    session.env["NIRI_SOCKET"] = str(sockets[0])
    displays = session.wait_for(
        lambda: [p for p in session.runtime.glob("wayland-*")
                 if p.is_socket() and p.name != "wayland-parent"], "Niri Wayland socket",
    )
    session.env["WAYLAND_DISPLAY"] = displays[0].name
    metadata = {
        "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "label": args.label,
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "config_sha256": hashlib.sha256(config.read_bytes()).hexdigest(),
        "kernel": platform.platform(),
        "niri": session.run(["niri", "--version"]).strip(),
        "weston": session.run(["weston", "--version"]).strip(),
        "outputs": json.loads(session.run(["niri", "msg", "--json", "outputs"])),
        "renderer": "software, WGPU_BACKEND=gl, LIBGL_ALWAYS_SOFTWARE=1",
        "weston_idle_timeout_seconds": 0,
        "system_bus": "unavailable", "audio": "unavailable",
        "settle_seconds": args.settle_seconds, "sample_seconds": args.sample_seconds,
        "planned_runs": args.runs,
        "panel_settle_seconds_before_input": 1,
        "clock_ticks_per_second": os.sysconf("SC_CLK_TCK"),
        "startup_observation": "Niri layer registration, not first visible populated frame",
    }
    write_json(session.output / "metadata.json", metadata)
    results = []
    for run in range(1, args.runs + 1):
        started = time.monotonic()
        bar = session.start([str(args.binary)], f"niribar-{run}.log")
        session.wait_for_layers(1)
        layer_ms = (time.monotonic() - started) * 1000
        session.wait_for(lambda: (session.runtime / "niribar.sock").is_socket(), "bar IPC")
        print(f"Run {run}/{args.runs}: bar registered, settling {args.settle_seconds}s", flush=True)
        time.sleep(args.settle_seconds)
        result = measure(bar.pid, args.sample_seconds)
        result.update(run=run, layer_registered_ms=layer_ms, layers=session.layers())
        write_json(session.output / f"run-{run}.json", result)
        if run == 1:
            session.screenshot("bar.png")
        for cycle in range(2):
            session.run([str(args.binary), "--toggle-panel"])
            session.wait_for_layers(2)
            # Registration precedes the first buffer and keyboard focus. Give
            # every cycle the same settling time before sending input.
            time.sleep(1)
            if cycle == 0:
                session.run(["wtype", "-k", "Tab"])
            if run == 1 and cycle == 0:
                session.screenshot("panel.png")
            if cycle == 0:
                session.run(["wtype", "-k", "Escape"])
            else:
                session.run([str(args.binary), "--toggle-panel"])
            session.wait_for_layers(1)
        result["panel_open_close_cycles"] = 2
        result["keyboard_escape_closed_panel"] = True
        write_json(session.output / f"run-{run}.json", result)
        results.append(result)
        session.retire(bar)
        session.wait_for_layers(0)
        print(f"Run {run}: {result['cpu_percent_one_core']:.3f}% CPU, "
              f"{result['rss_median_kib'] / 1024:.1f} MiB RSS; panel cycles passed", flush=True)
    summary = {"runs": len(results), "status": "passed"}
    for key in ("cpu_percent_one_core", "rss_median_kib", "layer_registered_ms"):
        values = [r[key] for r in results]
        summary[key] = {"median": statistics.median(values), "range": [min(values), max(values)]}
    write_json(session.output / "summary.json", summary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="new directory for logs and measurements")
    parser.add_argument("--label", required=True, help="revision and build profile of the supplied binary")
    parser.add_argument("--config", type=Path, default=Path(__file__).resolve().parents[1] / "crates/niribar/config/default.kdl")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--settle-seconds", type=int, default=60)
    parser.add_argument("--sample-seconds", type=int, default=60)
    args = parser.parse_args()
    if args.runs < 1 or args.settle_seconds < 0 or args.sample_seconds < 1:
        parser.error("runs and sample-seconds must be positive; settle-seconds cannot be negative")
    args.binary = args.binary.resolve(strict=True)
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)

    def terminated(signum, frame):
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        raise InterruptedError("Desktop validation received SIGTERM")

    signal.signal(signal.SIGTERM, terminated)
    with tempfile.TemporaryDirectory(prefix="niribar-validation-") as directory:
        session = Session(Path(directory), args.output)
        try:
            validate(args, session)
        except (Exception, KeyboardInterrupt) as error:
            write_json(args.output / "summary.json", {"status": "failed", "error": str(error)})
            raise
        finally:
            session.close()


if __name__ == "__main__":
    main()
