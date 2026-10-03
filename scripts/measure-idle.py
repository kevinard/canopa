#!/usr/bin/env python3
"""Measure idle Shell and graphical Notify in a private desktop with synthetic audio.

Requires the desktop fixture dependencies and PipeWire/Pulse tools. No live
desktop, audio device, configuration or notification history is used.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import platform
from pathlib import Path
import signal
import statistics
import sys
import tempfile
import time
from datetime import datetime, timezone

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]


def fixture(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / f"scripts/{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


desktop = fixture("validate-desktop")
appearance = fixture("verify-shell-appearance")


def sample(pid):
    result = desktop.sample_process(pid)
    status = Path(f"/proc/{pid}/status").read_text().splitlines()
    result["threads"] = int(next(line.split()[1] for line in status if line.startswith("Threads:")))
    for line in Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines():
        key, *values = line.split()
        if key in ("Pss:", "Private_Clean:", "Private_Dirty:"):
            result[key.rstrip(":").lower() + "_kib"] = int(values[0])
    result["private_kib"] = result.pop("private_clean_kib") + result.pop("private_dirty_kib")
    # These are scheduler sleeps, including I/O waits; they are not CPU time.
    switches = {}
    threads = {}
    for task in Path(f"/proc/{pid}/task").iterdir():
        try:
            fields = (task / "stat").read_text().rsplit(") ", 1)[1].split()
            identity = f"{task.name}:{fields[19]}"
            threads[identity] = {
                "name": (task / "comm").read_text().strip(),
                "cpu_ticks": int(fields[11]) + int(fields[12]),
            }
            for line in (task / "status").read_text().splitlines():
                if line.startswith("voluntary_ctxt_switches:"):
                    switches[identity] = int(line.split()[1])
        except FileNotFoundError:
            pass  # A completed transient worker can disappear while sampling.
    result["thread_voluntary_context_switches"] = switches
    result["thread_cpu"] = threads
    return result


def summarize(samples):
    first, last = samples[0], samples[-1]
    if any(s["start_ticks"] != first["start_ticks"] for s in samples):
        raise RuntimeError("Process identity changed during measurement")
    elapsed = last["monotonic_seconds"] - first["monotonic_seconds"]
    result = {
        "elapsed_seconds": elapsed,
        "cpu_percent_one_core": 100 * (last["cpu_ticks"] - first["cpu_ticks"])
        / os.sysconf("SC_CLK_TCK") / elapsed,
        # Retired workers must not subtract their lifetime counts. Sleeps after
        # the last observation of a retiring thread cannot be counted here.
        "observed_voluntary_context_switches_per_second": sum(
            max(0, count - previous["thread_voluntary_context_switches"].get(thread, 0))
            for previous, current in zip(samples, samples[1:])
            for thread, count in current["thread_voluntary_context_switches"].items()
        ) / elapsed,
        "samples": samples,
    }
    for key in ("rss_kib", "pss_kib", "private_kib", "threads"):
        values = [s[key] for s in samples]
        result[key] = {"median": statistics.median(values), "range": [min(values), max(values)]}
    return result


def churn(session, count):
    """Render and retire real banners before sampling the resulting idle state."""
    import gi
    gi.require_version("Gio", "2.0")
    from gi.repository import Gio, GLib
    bus = Gio.DBusConnection.new_for_address_sync(
        session.env["DBUS_SESSION_BUS_ADDRESS"],
        Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
        None, None,
    )

    def call(method, parameters, reply, timeout=5000):
        return bus.call_sync(
            "org.freedesktop.Notifications", "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications", method, parameters,
            GLib.VariantType.new(reply), Gio.DBusCallFlags.NONE, timeout, None,
        ).unpack()

    def server_ready():
        try:
            return bool(call("GetServerInformation", None, "(ssss)", timeout=1000))
        except GLib.Error:
            return False

    def banner_visible():
        layers = json.loads(session.run(["niri", "msg", "--json", "layers"]))
        return any(layer["namespace"] == "canopa-notify-banners" for layer in layers)

    try:
        # Socket ownership is established before the daemon finishes opening its
        # graphical adapter and exporting the D-Bus notification interface.
        # Probe an idempotent method; never retry a timed-out Notify mutation.
        session.wait_for(server_ready, "exported notification API")
        for index in range(count):
            notification, = call("Notify", GLib.Variant("(susssasa{sv}i)", (
                "Idle measurement", 0, "", f"Banner cycle {index + 1}",
                "Repeated rendered notification workload", [],
                {"transient": GLib.Variant("b", True)}, 0,
            )), "(u)")
            session.wait_for(banner_visible, "rendered Notify stack")
            # Permit actual frames and layout before retiring each stack.
            time.sleep(0.5)
            call("CloseNotification", GLib.Variant("(u)", (notification,)), "()")
            session.wait_for(lambda: not banner_visible(), "retired Notify stack")
        print(f"Rendered and retired {count} banner stacks", flush=True)
    finally:
        bus.close_sync(None)


def measure(args, session):
    # Explicit legacy path keeps the fixture independent of discovery aliases.
    config = session.runtime / "shell.kdl"
    config.write_bytes(args.config.read_bytes())
    session.start_niri("prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n"
                       "hotkey-overlay { skip-at-startup; }\n")
    session.env.pop("ICED_BACKEND", None)
    appearance.start_controls_backends(args.binary, session)
    notify = next(p for p in session.processes if Path(p.args[0]).name == "canopa-notify")
    shell = session.start([str(args.binary), "--config", str(config)], "shell.log")
    session.wait_for_layers(1)
    metadata = {
        "label": args.label,
        "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
        "sampler_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "shell_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "notify_sha256": hashlib.sha256(args.binary.with_name("canopa-notify").read_bytes()).hexdigest(),
        "config_sha256": hashlib.sha256(config.read_bytes()).hexdigest(),
        "platform": platform.platform(),
        "cpu_count": os.cpu_count(),
        "cpu_model": next((line.split(":", 1)[1].strip() for line in
                           Path("/proc/cpuinfo").read_text().splitlines()
                           if line.startswith("model name")), "unknown"),
        "renderer_environment": "WGPU_BACKEND=gl, LIBGL_ALWAYS_SOFTWARE=1",
        "audio": "private PipeWire/Pulse with two null sinks and a remapped source",
        "settle_seconds": args.settle_seconds,
        "sample_seconds": args.sample_seconds,
        "rendered_banner_cycles": args.churn_count,
        "clock_ticks_per_second": os.sysconf("SC_CLK_TCK"),
        "niri": session.run(["niri", "--version"]).strip(),
    }
    desktop.write_json(session.output / "metadata.json", metadata)
    if args.churn_count:
        churn(session, args.churn_count)
    print(f"Ready; settling {args.settle_seconds}s", flush=True)
    time.sleep(args.settle_seconds)
    processes = {"shell": shell, "notify": notify}
    samples = {name: [sample(p.pid)] for name, p in processes.items()}
    for _ in range(args.sample_seconds):
        time.sleep(1)
        for name, process in processes.items():
            samples[name].append(sample(process.pid))
    result = {name: summarize(values) for name, values in samples.items()}
    desktop.write_json(session.output / "result.json", result)
    for name, values in result.items():
        print(f"{name}: {values['cpu_percent_one_core']:.3f}% CPU, "
              f"{values['rss_kib']['median'] / 1024:.1f} MiB RSS, "
              f"{values['pss_kib']['median'] / 1024:.1f} MiB PSS, "
              f"{values['observed_voluntary_context_switches_per_second']:.1f} observed voluntary switches/s", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True,
                        help="Shell binary; canopa-notify must be in the same directory")
    parser.add_argument("--config", type=Path, default=ROOT / "crates/canopa-shell/config/default.kdl")
    parser.add_argument("--output", type=Path, required=True, help="new directory for evidence")
    parser.add_argument("--label", required=True)
    parser.add_argument("--settle-seconds", type=int, default=60)
    parser.add_argument("--sample-seconds", type=int, default=60)
    parser.add_argument("--churn-count", type=int, default=0,
                        help="render/close this many real banner stacks before the idle grace period")
    args = parser.parse_args()
    if args.settle_seconds < 0 or args.sample_seconds < 1:
        parser.error("settle-seconds must be nonnegative and sample-seconds positive")
    if not 0 <= args.churn_count <= 100:
        parser.error("churn-count must be between 0 and 100")
    args.binary = args.binary.resolve(strict=True)
    args.binary.with_name("canopa-notify").resolve(strict=True)
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)

    def terminated(signum, frame):
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        raise InterruptedError("Idle measurement received SIGTERM")

    signal.signal(signal.SIGTERM, terminated)
    with tempfile.TemporaryDirectory(prefix="canopa-idle-") as directory:
        session = desktop.Session(Path(directory), args.output)
        try:
            measure(args, session)
        except (Exception, KeyboardInterrupt) as error:
            desktop.write_json(args.output / "error.json", {"error": str(error)})
            raise
        finally:
            session.close()


if __name__ == "__main__":
    main()
