#!/usr/bin/env python3
"""Check manual idle inhibition on a private compositor, never the live desktop.

Requires swayidle, a C compiler and either Sway or Weston/Niri.
Checks real idle events, pointer toggles and exit; Sway also checks output loss.
"""
import argparse
import importlib.util
from pathlib import Path
import re
import shlex
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def start_compositor(session, compositor):
    runtime = session.runtime
    if compositor == "niri":
        config = runtime / "niri.kdl"
        config.write_text("prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n"
                          "hotkey-overlay { skip-at-startup; }\n")
        session.start(["weston", "--backend=headless", "--renderer=pixman", "--fake-seat",
                       "--width=1600", "--height=1000", "--socket=wayland-parent",
                       "--no-config", "--idle-time=0"], "weston.log")
        session.wait_for(lambda: (runtime / "wayland-parent").is_socket(), "Weston", timeout=60)
        session.env["WAYLAND_DISPLAY"] = "wayland-parent"
        session.start(["niri", "-c", str(config)], "niri.log")
        session.env["NIRI_SOCKET"] = str(session.wait_for(
            lambda: list(runtime.glob("niri.*.sock")), "Niri IPC", timeout=60)[0])
        session.env["WAYLAND_DISPLAY"] = str(session.wait_for(
            lambda: [p for p in runtime.glob("wayland-*")
                     if p.is_socket() and p.name != "wayland-parent"], "Niri display")[0])
        return
    config = runtime / "sway.conf"
    config.write_text("output HEADLESS-1 mode 1600x1000\nseat seat0 fallback true\nxwayland disable\n")
    session.env.update(WLR_BACKENDS="headless", WLR_HEADLESS_OUTPUTS="1", WLR_RENDERER="pixman")
    session.start(["sway", "-c", str(config)], "sway.log")
    session.env["SWAYSOCK"] = str(session.wait_for(lambda: list(runtime.glob("sway-ipc.*.sock")), "Sway")[0])
    session.env["WAYLAND_DISPLAY"] = str(session.wait_for(
        lambda: [p for p in runtime.glob("wayland-*") if p.is_socket()], "Wayland")[0])


def verify(binary, session, compositor):
    runtime, output = session.runtime, session.output
    # Explicitly isolate the session bus, including activation services.
    bus = runtime / "bus.conf"
    bus.write_text(f'<busconfig><type>session</type><listen>unix:tmpdir={runtime}</listen>'
                   '<policy context="default"><allow send_destination="*"/>'
                   '<allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
    session.start(["dbus-daemon", f"--config-file={bus}", "--nofork", "--print-address=1"], "dbus.log")
    session.env["DBUS_SESSION_BUS_ADDRESS"] = session.wait_for(
        lambda: (output / "dbus.log").read_text().strip(), "private bus").splitlines()[0]
    start_compositor(session, compositor)
    def screenshot(name):
        if compositor == "niri":
            session.screenshot(name)
        else:
            session.run(["grim", str(output / name)])
    config = runtime / "config/canopa/config.kdl"
    config.parent.mkdir(parents=True)
    config.write_text('schema-version 1\nshell {\n'
                      'bar "top" { output "all"; edge "top"; collapsed-width 44; '
                      'modules { start "idle-inhibitor"; }; }\n'
                      'bar "bottom" { output "all"; edge "bottom"; collapsed-width 44; '
                      'modules { start "awake"; }; }\n'
                      'modules { module "awake" { type "idle-inhibitor"; }; }\n}\n')
    source = ROOT / "scripts/desktop-fixture"
    protocol = str(source / "wlr-virtual-pointer-unstable-v1.xml")
    session.run(["wayland-scanner", "client-header", protocol, str(runtime / "virtual-pointer.h")])
    session.run(["wayland-scanner", "private-code", protocol, str(runtime / "virtual-pointer.c")])
    flags = shlex.split(session.run(["pkg-config", "--cflags", "--libs", "wayland-client"]))
    pointer = str(runtime / "click")
    session.run(["cc", "-Wall", "-Wextra", "-Werror", "-I"+str(runtime), str(source / "click.c"),
                 str(runtime / "virtual-pointer.c"), "-o", pointer, *flags])
    session.env.update(WAYLAND_DEBUG="client", RUST_LOG="canopa_shell::idle_inhibitor=debug")
    shell = session.start([str(binary)], "shell.log")
    session.env.pop("WAYLAND_DEBUG")
    trace = lambda: (output / "shell.log").read_text()
    session.wait_for(lambda: "state=Inactive" in trace(), "initial off state", timeout=60)
    assert "create_inhibitor(" not in trace(), "startup must not inhibit idle"
    idle = runtime / "idled"
    daemon = session.start(["swayidle", "-w", "timeout", "2", f"touch {shlex.quote(str(idle))}"], "idle.log")
    session.wait_for(idle.exists, "idle timeout while off")
    session.run([pointer, "18", "20"])
    session.wait_for(lambda: "state=Active" in trace(), "enabled state")
    session.wait_for(lambda: trace().count("create_inhibitor(") == 2, "one inhibitor per bar")
    screenshot("active.png")
    # A fresh daemon has no previous idle transition to mask a regression.
    def restart_idle():
        nonlocal daemon
        desktop.stop(daemon)
        session.processes.remove(daemon)
        idle.unlink(missing_ok=True)
        daemon = session.start(["swayidle", "-w", "timeout", "2", f"touch {shlex.quote(str(idle))}"], "idle.log")
    restart_idle()
    time.sleep(3.5)
    assert not idle.exists(), "enabled inhibitor must suppress idle timeout"
    before = len(trace())
    session.run([pointer, "18", "980"])
    session.wait_for(lambda: "state=Inactive" in trace()[before:], "disabled state")
    assert len(re.findall(r'zwp_idle_inhibitor_v1[@#]\d+\.destroy\(', trace()[before:])) == 2
    session.wait_for(idle.exists, "idle resumes after disabling")
    screenshot("inactive.png")
    before = len(trace())
    session.run([pointer, "18", "20"])
    session.wait_for(lambda: "state=Active" in trace()[before:], "re-enabled state")
    if compositor == "sway":
        before = len(trace())
        session.run(["swaymsg", "output", "HEADLESS-1", "disable"])
        session.wait_for(lambda: len(re.findall(r'zwp_idle_inhibitor_v1[@#]\d+\.destroy\(', trace()[before:])) == 2, "inhibitors destroyed with output")
        session.run(["swaymsg", "output", "HEADLESS-1", "enable"])
        session.wait_for(lambda: trace()[before:].count("create_inhibitor(") == 2 and "state=Active" in trace()[before:], "inhibition restored with output")
    restart_idle()
    time.sleep(3.5)
    assert not idle.exists(), "inhibition must survive another idle interval"
    desktop.stop(shell)
    session.processes.remove(shell)
    session.wait_for(idle.exists, "shell exit releases inhibition")
    checks = [
        "starts off", "shared toggle across two named modules and bars", "real idle suppression and resumption",
        "release on shell exit"]
    if compositor == "sway":
        checks.append("output retirement and restoration")
    desktop.write_json(output / "result.json", {"passed": True, "compositor": compositor, "checks": checks})
    print(f"Idle inhibitor desktop checks passed: {output}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/canopa-shell")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--compositor", choices=["sway", "niri"], default="sway")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="canopa-idle-desktop-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(args.binary.resolve(strict=True), session, args.compositor)
        finally:
            session.close()


if __name__ == "__main__":
    main()
