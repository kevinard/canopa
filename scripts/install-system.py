#!/usr/bin/env python3
"""Stage a system installation under DESTDIR without changing the running system."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PROGRAMS = ("canopa", "canopa-shell", "canopa-notify", "canopa-files", "canopa-settings",
            "canopa-wifi-provider", "canopa-audio-provider", "canopa-media-provider",
            "canopa-wallpaper", "canopa-lock", "canopa-idle", "canopa-greeter",
            "canopa-polkit", "canopa-pinentry", "xdg-desktop-portal-canopa")


def stage(destination, binaries, compositor=None, component="desktop"):
    destination = destination.resolve()
    if destination == Path("/"):
        raise ValueError("use an unprivileged DESTDIR; installation belongs to the package manager")
    destination.mkdir(parents=True, exist_ok=True)

    def install(source, target, executable=False):
        target = destination / target
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        target.chmod(0o755 if executable else 0o644)

    if component == "compositor":
        if compositor is None or not compositor.is_file():
            raise ValueError("--compositor is required for the compatible compositor package")
        install(compositor, "usr/lib/canopa/niri", True)
        base = ROOT / "vendor/source-overlays/niri-source-cancellation"
        for name in ("niri-LICENSE", "smithay-LICENSE", "smithay.patch"):
            install(base / name, f"usr/share/licenses/canopa-compositor/{name}")
        record = json.loads((ROOT / "vendor/provenance.json").read_text())["source_overlays"]["niri-source-cancellation"]
        target = destination / "usr/share/canopa/compositor-provenance.json"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps({"source": record, "binary_sha256": hashlib.sha256(compositor.read_bytes()).hexdigest()}, indent=2) + "\n")
        return
    if component == "login":
        install(ROOT / "packaging/system/canopa-login.sysusers", "usr/lib/sysusers.d/canopa-login.conf")
        install(ROOT / "packaging/system/canopa-login.tmpfiles", "usr/lib/tmpfiles.d/canopa-login.conf")
        install(ROOT / "packaging/greetd/greeter.kdl", "etc/canopa/greeter.kdl")
        target = destination / "usr/share/canopa/greetd/canopa.toml"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text('[terminal]\nvt = 1\n\n[default_session]\ncommand = "dbus-run-session /usr/bin/canopa-greeter session --compositor cage"\nuser = "canopa-greeter"\n')
        return
    for name in PROGRAMS:
        if not (binaries / name).is_file():
            raise ValueError(f"missing build executable: {name}")
        install(binaries / name, f"usr/bin/{name}", True)
    install(ROOT / "scripts/canopa-session.py", "usr/bin/canopa-session", True)
    for source in (ROOT / "packaging/systemd").glob("*.service"):
        target = destination / "usr/lib/systemd/user" / source.name
        target.parent.mkdir(parents=True, exist_ok=True)
        text = source.read_text().split("[Install]", 1)[0]
        text = text.replace("%h/.local/bin/", "/usr/bin/").replace("niri.service", "canopa-compositor.service")
        if source.name == "canopa-idle.service":
            text = text.replace("[Service]", "ConditionFileIsExecutable=/usr/bin/stasis\n\n[Service]")
        target.write_text(text)
    for name in ("canopa-compositor.service", "canopa-session.target", "canopa-shutdown.target"):
        install(ROOT / "packaging/system" / name, f"usr/lib/systemd/user/{name}")
    install(ROOT / "packaging/system/canopa.desktop", "usr/share/wayland-sessions/canopa.desktop")
    install(ROOT / "packaging/niri/defaults.kdl", "usr/share/canopa/niri-defaults.kdl")
    install(ROOT / "packaging/system/canopa-portals.conf", "usr/share/xdg-desktop-portal/canopa-portals.conf")
    install(ROOT / "packaging/pam/canopa-lock.arch", "etc/pam.d/canopa-lock")
    for source in (ROOT / "packaging/applications").glob("*.desktop"):
        install(source, f"usr/share/applications/{source.name}")
    for source in (ROOT / "packaging/portal").glob("*"):
        directory = "usr/share/xdg-desktop-portal/portals" if source.suffix == ".portal" else "usr/share/dbus-1/services"
        install(source, f"{directory}/{source.name}")
    install(ROOT / "packaging/file-manager/org.freedesktop.FileManager1.service", "usr/share/dbus-1/services/org.freedesktop.FileManager1.service")
    for target in (destination / "usr/share/dbus-1/services").glob("*.service"):
        text = target.read_text().replace("Exec=/usr/bin/env canopa-files", "Exec=/usr/bin/canopa-files").replace("Exec=/usr/bin/env xdg-desktop-portal-canopa", "Exec=/usr/bin/xdg-desktop-portal-canopa")
        target.write_text(text)
    install(ROOT / "packaging/canopa-notify.1", "usr/share/man/man1/canopa-notify.1")
    install(ROOT / "LICENSES.md", "usr/share/licenses/canopa/LICENSES.md")
    subprocess.run(["python3", str(ROOT / "scripts/package-documentation.py"), str(destination / "usr")], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destdir", type=Path, required=True)
    parser.add_argument("--build-dir", type=Path, default=ROOT / "target/release")
    parser.add_argument("--compositor", type=Path)
    parser.add_argument("--component", choices=("desktop", "compositor", "login"), default="desktop")
    args = parser.parse_args()
    stage(args.destdir, args.build_dir.resolve(), args.compositor, args.component)


if __name__ == "__main__":
    main()
