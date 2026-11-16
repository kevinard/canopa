#!/usr/bin/env python3
"""Stage a system installation under DESTDIR without changing the running system."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SHARING_SPEC = importlib.util.spec_from_file_location("desktop_sharing", ROOT / "scripts/package-desktop-sharing.py")
desktop_sharing = importlib.util.module_from_spec(SHARING_SPEC)
SHARING_SPEC.loader.exec_module(desktop_sharing)
PROGRAMS = ("canopa", "canopa-shell", "canopa-notify", "canopa-files", "canopa-settings",
            "canopa-wifi-provider", "canopa-audio-provider", "canopa-media-provider",
            "canopa-wallpaper", "canopa-lock", "canopa-idle", "canopa-greeter",
            "canopa-polkit", "canopa-pinentry", "xdg-desktop-portal-canopa")


def stage(destination, binaries, compositor=None, component="desktop", frontend=None):
    destination = destination.resolve()
    if destination == Path("/"):
        raise ValueError("use an unprivileged DESTDIR; installation belongs to the package manager")
    destination.mkdir(parents=True, exist_ok=True)

    def install(source, target, executable=False):
        target = destination / target
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        target.chmod(0o755 if executable else 0o644)
        return target

    def install_companion(receipt, kind, package, executable, provenance):
        if receipt is None:
            raise ValueError(f"the {kind} companion requires a build receipt")
        binary, metadata, notices = desktop_sharing.verified_build(receipt, kind)
        install(binary, executable, True)
        # Original license texts from the fork sources (and Niri's Smithay fork).
        for relative, source in notices.items():
            install(source, f"usr/share/licenses/{package}/{relative}")
        target = destination / "usr/share/canopa" / provenance
        target.parent.mkdir(parents=True, exist_ok=True)
        record = {"source": desktop_sharing.source_record(kind, metadata), "binary_sha256": metadata["sha256"]}
        target.write_text(json.dumps(record, indent=2) + "\n")

    if component == "compositor":
        install_companion(compositor, "niri", "canopa-compositor", "usr/lib/canopa/niri",
                          "compositor-provenance.json")
        return
    if component == "login":
        install(ROOT / "packaging/system/canopa-login.sysusers", "usr/lib/sysusers.d/canopa-login.conf")
        install(ROOT / "packaging/system/canopa-login.tmpfiles", "usr/lib/tmpfiles.d/canopa-login.conf")
        install(ROOT / "packaging/greetd/greeter.kdl", "etc/canopa/greeter.kdl")
        helper = binaries / "canopa-greeter-displays"
        if not helper.is_file():
            raise ValueError(f"missing build executable: {helper}")
        install(helper, "usr/lib/canopa/greeter-displays", True)
        install(ROOT / "packaging/polkit/org.canopa.greeter.policy",
                "usr/share/polkit-1/actions/org.canopa.greeter.policy")
        # The release archive ships the same samples; both use the packaged layout.
        for sample, name in (("config.toml", "canopa.toml"), ("config-cage.toml", "canopa-cage.toml"),
                             ("config-weston.toml", "canopa-weston.toml")):
            install(ROOT / "packaging/greetd" / sample, f"usr/share/canopa/greetd/{name}")
        return
    if component == "oo7":
        prompter = binaries / "canopa-keyring-prompter"
        if not prompter.is_file():
            raise ValueError(f"missing build executable: {prompter}")
        install(prompter, "usr/lib/canopa/canopa-keyring-prompter", True)
        service = "org.gnome.keyring.SystemPrompter.service"
        install(ROOT / "packaging/keyring" / service, f"usr/share/canopa/dbus-1/services/{service}")
        install(ROOT / "packaging/pam/canopa-oo7.arch", "etc/pam.d/canopa-oo7")
        install(ROOT / "packaging/system/canopa-oo7-portals.conf",
                "etc/xdg/xdg-desktop-portal/canopa-portals.conf")
        install(ROOT / "packaging/system/oo7-early-login.conf",
                "usr/lib/systemd/user/oo7-daemon.service.d/50-canopa-login.conf")
        install(ROOT / "packaging/system/oo7-wait-for-pam.py", "usr/lib/canopa/oo7-wait-for-pam", True)
        return
    for name in PROGRAMS:
        if not (binaries / name).is_file():
            raise ValueError(f"missing build executable: {name}")
    install_companion(frontend, "frontend", "canopa", "usr/lib/canopa/xdg-desktop-portal",
                      "portal-frontend-provenance.json")
    install(ROOT / "packaging/system/canopa-portal-frontend", "usr/lib/canopa/portal-frontend", True)
    install(ROOT / "packaging/system/xdg-desktop-portal.service.conf",
            "usr/lib/systemd/user/xdg-desktop-portal.service.d/50-canopa.conf")
    sharing = ROOT / "packaging/portal/desktop-sharing"
    install(sharing / "canopa-screencast.lua", "usr/share/wireplumber/scripts/canopa-screencast.lua")
    install(sharing / "90-canopa-screencast.conf",
            "usr/share/wireplumber/wireplumber.conf.d/90-canopa-screencast.conf")
    for name in PROGRAMS:
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
    for name in ("canopa-compositor.service", "canopa-display-profiles.service", "canopa-session.target", "canopa-shutdown.target"):
        install(ROOT / "packaging/system" / name, f"usr/lib/systemd/user/{name}")
    install(ROOT / "packaging/system/canopa.desktop", "usr/share/wayland-sessions/canopa.desktop")
    install(ROOT / "packaging/niri/defaults.kdl", "usr/share/canopa/niri-defaults.kdl")
    install(ROOT / "packaging/system/canopa-portals.conf", "usr/share/xdg-desktop-portal/canopa-portals.conf")
    install(ROOT / "packaging/pam/canopa-lock.arch", "etc/pam.d/canopa-lock")
    for source in (ROOT / "packaging/applications").glob("*.desktop"):
        install(source, f"usr/share/applications/{source.name}")
    portals = ROOT / "packaging/portal"
    for source in portals.glob("*.portal"):
        target = install(source, f"usr/share/xdg-desktop-portal/portals/{source.name}")
        if source.name == "canopa.portal":
            target.write_bytes(desktop_sharing.descriptor())
    for source in portals.glob("*.service"):
        target = install(source, f"usr/share/dbus-1/services/{source.name}")
        target.write_text(target.read_text().replace(
            "Exec=/usr/bin/env xdg-desktop-portal-canopa", "Exec=/usr/bin/xdg-desktop-portal-canopa").replace(
            "Exec=/usr/bin/env canopa-notify", "Exec=/usr/bin/canopa-notify"))
    service = "org.freedesktop.FileManager1.service"
    target = install(ROOT / "packaging/file-manager" / service, f"usr/share/canopa/dbus-1/services/{service}")
    target.write_text(target.read_text().replace(
        "Exec=/usr/bin/env canopa-files", "Exec=/usr/bin/canopa-files"))
    # Settings' single-instance name is Canopa's own, so it is activatable everywhere.
    service = "org.canopa.Settings.service"
    target = install(ROOT / "packaging/settings" / service, f"usr/share/dbus-1/services/{service}")
    target.write_text(target.read_text().replace(
        "Exec=/usr/bin/env canopa-settings", "Exec=/usr/bin/canopa-settings"))
    install(ROOT / "packaging/canopa-notify.1", "usr/share/man/man1/canopa-notify.1")
    install(ROOT / "LICENSES.md", "usr/share/licenses/canopa/LICENSES.md")
    subprocess.run(["python3", str(ROOT / "scripts/package-documentation.py"), str(destination / "usr")], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destdir", type=Path, required=True)
    parser.add_argument("--build-dir", type=Path, default=ROOT / "target/release")
    parser.add_argument("--compositor-build", type=Path)
    parser.add_argument("--portal-frontend-build", type=Path)
    parser.add_argument("--component", choices=("desktop", "compositor", "login", "oo7"), default="desktop")
    args = parser.parse_args()
    stage(args.destdir, args.build_dir.resolve(), args.compositor_build, args.component,
          frontend=args.portal_frontend_build)


if __name__ == "__main__":
    main()
