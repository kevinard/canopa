#!/usr/bin/env python3
"""Inspect the bundled Canopa spine in a private Weston/Niri desktop.

Requires the existing desktop fixture dependencies. Never uses the live session.
Checks native palette, reservation, 44px launcher target, controls, and Escape.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def start_controls_backends(binary, session):
    """Hardware-free audio and Notify services, isolated from the user's session."""
    # Notify needs no sound output here. Start before the synthetic PipeWire
    # client configuration is installed; it intentionally has no playback factory.
    session.start([str(binary.with_name("canopa-notify")), "daemon", "--service"], "notify.log")
    session.wait_for(lambda: (session.runtime / "canopa-notify/control.sock").exists(), "private Notify")
    audio = session.runtime / "audio"
    audio.mkdir()
    session.env.update(
        PIPEWIRE_RUNTIME_DIR=str(audio), PIPEWIRE_CONFIG_DIR=str(audio),
        PIPEWIRE_CONFIG_PREFIX="", PIPEWIRE_REMOTE="pipewire-0",
        PULSE_RUNTIME_PATH=str(audio / "pulse"), PULSE_SERVER=f"unix:{audio}/pulse/native",
    )
    (audio / "client.conf").write_text('''context.modules = [
        { name = libpipewire-module-protocol-native }
        { name = libpipewire-module-metadata }
    ]''')
    core = '''context.properties = { core.daemon = true core.name = pipewire-0 }
        context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
        context.modules = [
            { name = libpipewire-module-protocol-native }
            { name = libpipewire-module-access }
            { name = libpipewire-module-metadata }
            { name = libpipewire-module-spa-node-factory }
            { name = libpipewire-module-client-node }
            { name = libpipewire-module-adapter }
            { name = libpipewire-module-link-factory }
        ]
        context.objects = [ { factory = metadata args = { metadata.name = default } } ]'''
    pulse = '''context.properties = { }
        context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
        context.modules = [
            { name = libpipewire-module-protocol-native }
            { name = libpipewire-module-client-node }
            { name = libpipewire-module-adapter }
            { name = libpipewire-module-metadata }
            { name = libpipewire-module-protocol-pulse args = { server.address = [ "unix:native" ] } }
        ]'''
    for program, config, socket in [
        ("pipewire", core, "pipewire-0"),
        ("pipewire-pulse", pulse, "pulse/native"),
    ]:
        (audio / f"{program}.conf").write_text(config)
        session.start([program, "-c", f"{program}.conf"], f"{program}.log")
        session.wait_for(lambda: (audio / socket).exists(), f"private {program}")
    session.run(["pactl", "load-module", "module-null-sink", "sink_name=speakers",
                 "sink_properties=device.description=Speakers"])
    session.run(["pactl", "load-module", "module-null-sink", "sink_name=headphones",
                 "sink_properties=device.description=Headphones"])
    session.run(["pactl", "load-module", "module-remap-source", "master=speakers.monitor",
                 "source_name=microphone", "source_properties=device.description=Microphone"])
    session.run(["pactl", "set-default-sink", "speakers"])
    session.run(["pactl", "set-default-source", "microphone"])
    session.run(["pw-metadata", "-n", "default", "0", "default.audio.sink",
                 '{"name":"speakers"}', "Spa:String:JSON"])
    session.run(["pw-metadata", "-n", "default", "0", "default.audio.source",
                 '{"name":"microphone"}', "Spa:String:JSON"])
    session.run(["pactl", "set-sink-volume", "speakers", "50%"])


def verify(binary, session, scheme, scale, controls=False, session_actions=False):
    runtime = session.runtime
    session.env["PATH"] = f'{binary.parent}:{session.env.get("PATH", "")}'
    output = session.start_niri(
        'prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n'
        'hotkey-overlay { skip-at-startup; }\nlayout { gaps 0; border { off; }; '
        'focus-ring { off; }; default-column-width { proportion 1.0; }; }\n',
        width=round(1280 * scale), height=round(800 * scale), scale=scale,
    )
    logical = json.loads(session.run(["niri", "msg", "--json", "outputs"]))[output]["logical"]
    width, height = logical["width"], logical["height"]
    session.start(["wtype", "-s", "600000"], "keyboard.log")
    session.env["GDK_BACKEND"] = "wayland"
    applications = runtime / "data/applications"
    applications.mkdir(parents=True)
    for name in ["Files", "Notes", "Terminal"]:
        (applications / f"canopa-fixture-{name.lower()}.desktop").write_text(
            f"[Desktop Entry]\nType=Application\nName=Canopa {name}\nExec=true\n"
            "Comment=Native appearance validation fixture\n"
        )
    client = runtime / "client.py"
    client.write_text('''import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, GLib
GLib.set_prgname("org.canopa.AppearanceFixture")
window = Gtk.Window(title="Native appearance fixture")
window.add(Gtk.Label(label="Native client: its layout stays fixed while shell panels open."))
window.show_all()
Gtk.main()
''')
    session.start(["python3", str(client)], "client.log")

    def client_layout():
        windows = json.loads(session.run(["niri", "msg", "--json", "windows"]))
        return windows[0]["layout"] if windows else None

    initial = session.wait_for(client_layout, "native client")
    config = runtime / "config/canopa/config.kdl"
    config.parent.mkdir(parents=True)
    # Use the real bundled layout, including its hooks, rather than a lookalike fixture.
    shell = (ROOT / "crates/canopa-shell/config/default.kdl").read_text()
    if session_actions:
        # Exercise the actual configured routes without touching the user's
        # locker, compositor lifetime or machine power. Each stub writes one marker.
        stub = runtime / "session-action.py"
        stub.write_text('import pathlib, sys\npathlib.Path(__file__).with_name(sys.argv[1] + ".requested").touch()\n')
        for original, replacement in [
            ('launch "canopa-lock";', f'launch "python3" "{stub}" "lock";'),
            ('argv "niri" "msg" "action" "quit" "--skip-confirmation";', f'argv "python3" "{stub}" "logout";'),
            ('argv "systemctl" "reboot";', f'argv "python3" "{stub}" "reboot";'),
            ('argv "systemctl" "poweroff";', f'argv "python3" "{stub}" "poweroff";'),
        ]:
            assert shell.count(original) == 1, f"missing session fixture binding: {original}"
            shell = shell.replace(original, replacement)
    config.write_text(f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\nnotifications {{ sound enabled=#false; }}\nshell {{\n{shell}\n}}\n')
    if controls:
        start_controls_backends(binary, session)
    session.env["RUST_LOG"] = "canopa_shell::panel=debug,canopa_shell::modules=trace,canopa_shell::idle_inhibitor=debug,canopa_shell::external_panel=debug"
    shell_process = session.start([str(binary)], "shell.log")
    session.wait_for(lambda: len(session.layers()) == 1, "native spine startup", timeout=45)
    expected = (27, 39, 43) if scheme == "dark" else (247, 249, 244)

    def pixel(frame, x, y):
        return frame.getpixel((round(x * scale), round(y * scale)))

    session.wait_for(
        lambda: pixel(session.capture("spine.png"), 44, 200) == expected,
        "native spine palette", timeout=30,
    )
    reserved = session.wait_for(
        lambda: (value if (value := client_layout()) != initial else None),
        "stable 88px reservation",
    )
    assert initial["tile_size"][0] - reserved["tile_size"][0] == 88, (initial, reserved)
    pointer = session.build_pointer()

    def click(x, y):
        session.run([str(pointer), str(round(x * 1600 / width)), str(round(y * 1000 / height))])

    panel_color = (32, 45, 49) if scheme == "dark" else (251, 252, 248)

    def panel_top(frame):
        return next((y for y in range(height) if pixel(frame, 110, y) == panel_color), None)

    def panel_visible(name):
        return session.wait_for(
            lambda: panel_top(session.capture(name)),
            "native panel palette", timeout=30,
        )

    def panel_click(x, y):
        click(x, panel_visible("current-panel.png") + y)

    def escape(layers=1):
        # OnDemand overlays receive keyboard input after pointer focus. Session
        # launch tests can leave focus on the underlying native client.
        panel_click(260, 28)
        session.run(["wtype", "-k", "Escape"])
        session.wait_for_layers(layers)

    # The lower edge of the 44px target activates, even below the visible 20px logo.
    click(44, 66)
    session.wait_for_layers(2)
    panel_visible("launcher.png")
    assert client_layout() == reserved, "launcher resized the native client"
    session.run(["wtype", "Canopa"])
    session.capture("launcher-search.png")
    escape(2)  # Search clears first.
    escape()
    assert client_layout() == reserved
    click(44, height - 98)
    session.wait_for_layers(2)
    panel_visible("controls.png")
    session.wait_for(
        lambda: 'content="bar-panel"' in (session.output / "shell.log").read_text(),
        "controls open the current bar's panel",
    )
    assert client_layout() == reserved, "controls resized the native client"
    if controls:
        def provider_running(name):
            children = subprocess.run(["ps", "--ppid", str(shell_process.pid), "-o", "comm="],
                                      env=session.env, capture_output=True, text=True, timeout=5)
            return f"canopa-{name}-provider"[:15] in children.stdout

        # The real default drawer uses the existing provider names and volume adapter.
        session.wait_for(lambda: "Volume::StateChanged" in (session.output / "shell.log").read_text(), "audio summary")
        panel_click(448, 231)
        session.wait_for(lambda: "55%" in session.run(["pactl", "get-sink-volume", "speakers"]), "quick volume increase")
        panel_click(338, 231)
        session.wait_for(lambda: "50%" in session.run(["pactl", "get-sink-volume", "speakers"]), "quick volume decrease")
        # At fractional scale the fixed footer leaves a shorter scroll viewport.
        # Keyboard routing activates the same compact card without assuming its Y.
        # The volume button click above has already selected Sound.
        session.run(["wtype", "-k", "Tab", "-k", "Return"])
        notify = str(binary.with_name("canopa-notify"))
        session.wait_for(lambda: json.loads(session.run([notify, "ctl", "status", "--json"]))["do_not_disturb"], "quick DND toggle")
        session.run(["wtype", "-k", "Tab", "-k", "Return"])
        session.wait_for(lambda: "Active" in (session.output / "shell.log").read_text(), "quick keep-awake toggle")
        session.capture("quick-settings.png")
        panel_click(250, 98)
        session.wait_for(lambda: provider_running("wifi") and (session.output / "shell.log").read_text().count('content="external-provider"') == 1, "built-in Wi-Fi provider")
        panel_visible("wifi.png")
        panel_click(148, 34)  # Host Back returns to the same Bar Panel.
        session.wait_for(lambda: (session.output / "shell.log").read_text().count('content="bar-panel"') == 2, "back from Wi-Fi")
        panel_visible("back-from-wifi.png")
        # Keyboard entry selection opens Sound without needing a volume Module on the spine.
        session.run(["wtype", "-k", "Tab", "-k", "Tab", "-k", "Return"])
        session.wait_for(lambda: provider_running("audio") and (session.output / "shell.log").read_text().count('content="external-provider"') == 2, "built-in Sound provider")
        panel_visible("sound.png")
        raised = (43, 59, 63) if scheme == "dark" else (231, 237, 231)
        def sound_rendered():
            frame = session.capture("sound.png")
            top = panel_top(frame)
            return top is not None and top + 145 < height and pixel(frame, 405, top + 145) == raised

        session.wait_for(sound_rendered, "rendered Sound device controls", timeout=30)
        assert client_layout() == reserved, "provider resized native client"
        panel_click(148, 34)
        session.wait_for(lambda: (session.output / "shell.log").read_text().count('content="bar-panel"') == 3, "back from Sound")
        panel_visible("back-from-sound.png")
        # The fixture has no hardware system bus. These built-in panels should
        # remain reachable and return to the same Bar Panel.
        for steps, name in [(5, "bluetooth"), (6, "power")]:
            panel_click(260, 28)
            session.run(["wtype", *["-k", "Tab"] * steps, "-k", "Return"])
            def hardware_provider_running():
                children = subprocess.run(["ps", "--ppid", str(shell_process.pid), "-o", "args="],
                                          env=session.env, capture_output=True, text=True, timeout=5)
                return f"--hardware-panel {name}" in children.stdout
            session.wait_for(hardware_provider_running, f"built-in {name} provider")
            panel_visible(f"{name}.png")
            bar_count = (session.output / "shell.log").read_text().count('content="bar-panel"')
            panel_click(148, 34)
            session.wait_for(lambda: (session.output / "shell.log").read_text().count('content="bar-panel"') == bar_count + 1,
                             f"back from {name}")
            panel_visible(f"back-from-{name}.png")
    if session_actions:
        def press_keys(*keys):
            session.run(["wtype", *[arg for key in keys for arg in ("-k", key)]])

        def count_panels(kind):
            return (session.output / "shell.log").read_text().count(f'content="{kind}"')

        def quick_settings_visible(name):
            # The button uses the panel fill. Look for its left edge, away
            # from the icon and label, to distinguish it from a menu.
            def footer_rendered():
                frame = session.capture(name)
                y = height - 62
                return any(pixel(frame, x, y) != pixel(frame, 109, y) for x in range(112, 116))

            session.wait_for(footer_rendered, "session footer rendered", timeout=30)
            panel_click(260, 28)  # Give the replacement OnDemand surface keyboard focus.

        # Source controls above the footer are available in this isolated fixture.
        # Lock is the seventh keyboard target and uses a session-owned launch.
        quick_settings_visible("session-controls.png")
        press_keys(*(["Tab"] * 7), "Return")
        session.wait_for(lambda: (runtime / "lock.requested").exists(), "stub locker launch")
        session.capture("session-controls.png")
        for target, steps in [("logout", 8), ("reboot", 9), ("poweroff", 10)]:
            for confirm in [False, True]:
                menu_count = count_panels("menu")
                if confirm:
                    press_keys(*(["Tab"] * steps), "Return")
                else:
                    # Pointer activation covers every session button alongside
                    # explicit keyboard confirmation from a fresh drawer.
                    x, y = {"logout": (372, height - 112),
                            "reboot": (196, height - 62),
                            "poweroff": (372, height - 62)}[target]
                    click(x, y)
                session.wait_for(lambda: count_panels("menu") == menu_count + 1,
                                 f"{target} confirmation menu")
                def compact_confirmation():
                    top = panel_top(session.capture(f"confirm-{target}.png"))
                    return top if top is not None and top > height - 250 else None

                top = session.wait_for(compact_confirmation, "compact confirmation rendered")
                click(260, top + 28)  # Focus the replacement surface without activating either action.
                assert not (runtime / f"{target}.requested").exists(), "action ran before confirmation"
                if confirm:
                    press_keys("Down", "Return")
                    session.wait_for(lambda: (runtime / f"{target}.requested").exists(),
                                     f"explicit {target} confirmation")
                    session.wait_for_layers(1)
                    click(44, height - 98)
                    session.wait_for_layers(2)
                    quick_settings_visible("session-controls.png")
                else:
                    bar_count = count_panels("bar-panel")
                    press_keys("Return")  # Cancel is selected first.
                    session.wait_for(lambda: count_panels("bar-panel") == bar_count + 1,
                                     f"cancel {target} returns to quick settings")
                    quick_settings_visible("cancel-session-action.png")
                    assert not (runtime / f"{target}.requested").exists(), "Cancel ran a session action"
                assert client_layout() == reserved, "session controls resized native client"
    click(44, height - 98)
    session.wait_for_layers(1)
    click(44, height - 98)
    session.wait_for_layers(2)
    escape()
    # Keep the text-only clock's target at least 44px high, too. This point is
    # above its visible text and must open Calendar rather than the bar background.
    click(44, height - 66)
    session.wait_for_layers(2)
    session.wait_for(
        lambda: 'content="module-panel"' in (session.output / "shell.log").read_text(),
        "clock target upper edge opens its module details",
    )
    assert client_layout() == reserved, "calendar resized the native client"
    escape()
    session.capture("closed.png")
    desktop.write_json(session.output / "result.json", {
        "passed": True, "scheme": scheme, "scale": scale,
        "reservation": 88, "launcher_target_lower_edge": True,
        "search_escape_then_close": True, "controls_pointer_activation": True,
        "controls_bar_panel": True, "controls_repeat_click_dismissal": True,
        "clock_target_upper_edge": True,
        "unchanged_native_client": True,
        "quick_controls": controls,
        "session_actions_stubbed": session_actions,
        "controls_validation": {
            "volume_50_to_55_and_back": True, "dnd_toggle": True,
            "keep_awake_toggle": True, "builtin_wifi_provider": True,
            "builtin_audio_provider_by_keyboard": True,
            "sound_device_controls_rendered": True, "provider_back_navigation": True,
            "hardware_panels_with_unavailable_bus": True,
        } if controls else None,
    })


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/canopa-shell")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scheme", choices=["dark", "light"], default="dark")
    parser.add_argument("--scale", type=float, default=1.0)
    parser.add_argument("--controls", action="store_true", help="Exercise private audio, DND, keep-awake and provider navigation")
    parser.add_argument("--session-actions", action="store_true", help="Exercise session buttons with harmless command stubs (requires --controls)")
    args = parser.parse_args()
    if args.session_actions and not args.controls:
        parser.error("--session-actions requires --controls")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="canopa-appearance-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(args.binary.resolve(strict=True), session, args.scheme, args.scale, args.controls, args.session_actions)
        finally:
            session.close()
    print(f"Native {args.scheme} appearance checks passed: {output}")


if __name__ == "__main__":
    main()
