# Niribar

This application is part of the [Canopa workspace](../../README.md). Run the
commands below from the repository root. Cargo uses the root lockfile and writes
normal build outputs to root `target/`.

A configurable Wayland bar for Niri, built with Rust and Iced. It includes
workspaces, a launcher, tray menus, battery, network, volume, clock/calendar,
script modules, dmenu selection, and external panels.

## Build and run

Use Rust 1.95.0 (the CI baseline) or newer. On Debian/Ubuntu, install the native
build dependencies:

```sh
sudo apt-get install build-essential pkg-config libpulse-dev libwayland-dev libxkbcommon-dev libudev-dev dbus at-spi2-core
cargo +1.95.0 build --locked --release -p niribar --bins
target/release/niribar
```

Run inside a Niri/Wayland desktop session with its session D-Bus environment.
Keep `niribar-wifi-provider` alongside `niribar` or on PATH. PulseAudio or
PipeWire's PulseAudio compatibility service supplies audio state and control.
Ordinary desktop-entry launching requires `gio` (`libglib2.0-bin` on Debian/Ubuntu).
D-Bus-enabled entries activate through the session bus. Executable launches use niri's IPC; without `NIRI_SOCKET`, they require
`systemd-run` and a systemd user manager.

Configuration comes from the `shell` namespace of `$XDG_CONFIG_HOME/canopa/config.kdl`
(default `~/.config/canopa/config.kdl`). When that file is absent, the legacy
`niribar/config.kdl` remains supported; otherwise missing namespaces use embedded
defaults. See the [shared configuration guide](../../docs/configuration.md) for validation,
explicit sources, and migration. Shell changes require a restart. Existing option
examples in [config/default.kdl](config/default.kdl), [docs](../../docs/niribar/) and
[CONTEXT.md](../../docs/niribar/CONTEXT.md) go inside the shared `shell` block.
The adjacent shared `appearance` block controls the dark/light palette and accent
for the shell, Notification Center, and Notify banners.

### Panel appearance and motion

Drawers and automatic popovers share headings, close controls, menu rows, spacing,
and focus treatment. Their sizing and anchoring continue to follow the selected
presentation. Panels slide 8 logical pixels from their originating edge and fade
in or out, without changing the desktop's reserved space on each frame. Opening
decelerates into place; closing eases gently to transparent before removal.

The existing per-bar animation settings now control these transitions:

```kdl
panel {
    animation {
        open-ms 180
        close-ms 160
    }
}
```

Set both durations to `0` for reduced motion. Either duration can be disabled
independently. Motion is configured per bar; the shared `appearance` block does not
currently read a desktop motion preference. Restart Niribar after changing settings.
Closing ends interaction immediately; the remaining exit image cannot receive input.

### Vertical side drawer

The shipped left bar opens panels in a full-height, 360-logical-pixel side drawer.
Clicking another module replaces its contents in the same position; clicking the
same module again, pressing Escape, or using Close dismisses it. The drawer overlays
the desktop without resizing tiled windows. Each panel shows its own content;
the bar icons remain the navigation.

To enable this in an existing bar, add `presentation "drawer"` to its `panel` block.
Remove any existing `height` or `max-height` to let the drawer fill the screen's
usable height; for example, an old `height 720` keeps the drawer at 720 logical pixels.

```kdl
panel {
    presentation "drawer"
    // drawer-mode "push-bar" // drawer at the edge, bar on its inner side
    // width 400       // optional consistent drawer width
    // max-height 800  // optional cap; the drawer stays at the top
    // Keep your existing start / center / end module lists here.
}
```

Explicit `width`/`height` and `max-width`/`max-height` still apply. On top/bottom
bars, the drawer preference uses the existing automatic layout. Omitted
`presentation`, or `presentation "auto"`, keeps the previous content-aware panel
sizing. Tooltips and tray context menus remain small popups. Restart Niribar after
editing configuration.

`drawer-mode "overlay"` (the default) keeps the bar at the screen edge and opens
the drawer beside it. `drawer-mode "push-bar"` opens the drawer at the bar's edge
and moves the bar inward: on the left, the drawer is on the left and the bar on
its right; on the right, this is mirrored. Closing the drawer returns the bar to
the edge. Both modes overlay application windows without resizing them.
`drawer-mode` applies only to left/right bars with `presentation "drawer"`.

For an existing module with no click action, add `actions { click { open-panel; } }`
inside that module to open its details. The shipped clock, sound, battery and
focused-window modules include this binding. Network and notifications retain
their provider actions; workspace switching and custom action bindings still work
as configured.

Use `launch "kitty"` or `launch "firefox" "https://example.org"` in menu items
and module actions to open applications. Arguments are literal strings. Launches
belong to the desktop session and survive bar restarts. Existing `command`
actions remain bounded automation (three seconds by default); migrate any
application-opening `command` actions in your own config to `launch`.

```sh
niribar --version
niribar --toggle-panel
niribar --open-menu apps
niribar --open-menu main
printf 'First\nSecond\n' | niribar --dmenu --prompt 'Choose' --no-custom
```

The IPC commands require a running bar. Selection input is bounded to 1 MiB.

`--open-menu NAME` opens any named menu in the running bar's configuration,
including menus with `source "installed-apps"`. Names are exact and case-sensitive;
quote names containing spaces and use `--open-menu=NAME` for names starting with
`-`. The request uses a bar on Niri's focused output,
falling back to one deterministic bar surface if that output has no bar or focus
is unavailable. Opening a menu replaces the current interactive overlay; repeating
the command opens the menu again instead of toggling it closed.

The command exits as soon as the running bar accepts the request, without waiting
for a selection. Exit status is `0` for acceptance, `1` for an unknown menu, no
available bar surface, or an IPC failure, and `2` for invalid command-line usage.
The running bar must also support this command; restart it after upgrading.

For example, put this inside Niri's existing `binds` block to open an `apps` menu:

```kdl
Mod+D repeat=false hotkey-overlay-title="Run an Application: niribar" {
    spawn "niribar" "--open-menu" "apps";
}
```

## Install a release

The Canopa Linux x86-64 archive contains all four companion executables, shared
configuration, documentation and user service units. From the extracted directory:

```sh
mkdir -p ~/.local/bin ~/.config/systemd/user
install -m755 bin/niribar bin/niribar-wifi-provider bin/canopa-notify bin/canopa ~/.local/bin/
cp share/systemd/user/*.service share/systemd/user/canopa-session.target ~/.config/systemd/user/
systemctl --user daemon-reload
```

The units use `~/.local/bin`. Once the Niri session exports `WAYLAND_DISPLAY` and
`NIRI_SOCKET` to the user service manager, enable `canopa-session.target` to start
both companions. Use one notification daemon in the session. See the
[notification and session guide](../../docs/notifications.md) for controls and restart
behavior. Niribar can still be launched directly from Niri's startup configuration;
use one startup mechanism for each process.

## Wi-Fi and external panels

The built-in Wi-Fi provider prefers NetworkManager; when it is absent, it uses
iwd. It supports scanning, saved/open networks, personal credentials, and
hidden personal networks. The system daemon controls authorization and saved
credentials. New enterprise/EAP provisioning is outside the panel's scope;
provision such profiles with the system's network tools first. iwd network
configuration also depends on the system's existing DHCP/DNS setup.

Built-in providers use `canopa-v5`; configured providers also support explicit
`niribar-v1` and `canopa-v4`. All contracts
are available directly from [canopa-panel-protocol](../canopa-panel-protocol/README.md)
without depending on the shell. See
[the provider guide](../../docs/niribar/external-panel-providers.md).

## Development and validation

To update your local desktop installation whenever you choose, run:

```sh
./scripts/update.sh
```

This updates Niribar, its Wi-Fi helper, Notify, and the `canopa` configuration tool
together from the current checkout, including uncommitted changes. It validates
configuration before stopping services, backs up binaries and notification state,
and verifies startup. Intentionally stopped services stay stopped. Configuration
and service definitions are preserved. See the [workspace update guide](../../README.md)
for prerequisites, backups, recovery, and build-job options. The script does not
fetch Git updates.

```sh
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --locked -p niribar --all-targets -- -D warnings
dbus-run-session -- cargo +1.95.0 test --locked -p niribar --all-targets
python3 scripts/check-vendor.py
python3 scripts/check-contrast.py
python3 scripts/test-update.py
```

For the combined release archive, build all workspace binaries and run the root
packager as described in the [Canopa guide](../../README.md).

The transport suite also requires `libglib2.0-bin`, `pipewire`, `pipewire-bin`, `pipewire-pulse`
and `pulseaudio-utils`. Tests start private D-Bus/audio daemons and Unix sockets;
environments that forbid local socket binding cannot run the transport suite.
Audio fixtures use synthetic sinks without hardware discovery. Tests do not
change your desktop's audio or Wi-Fi services. See [the audit](../../docs/niribar/audits/2026-09-23-project-state.md)
and [follow-up evidence](../../docs/niribar/audits/2026-09-23-remediation.md).
The [desktop validation harness](../../docs/niribar/manual-validation.md) runs a private
compositor session for measurements and panel checks. The
[accessibility assessment](../../docs/niribar/ACCESSIBILITY.md) records default contrast ratios,
native AT-SPI coverage, and remaining operator checks. Nix packaging is deferred;
the optional CI job runs only if a flake is added.

Niribar is [MIT licensed](LICENSE). Vendored dependencies retain their own
licenses; see [vendor maintenance](../../docs/niribar/vendor-maintenance.md).
