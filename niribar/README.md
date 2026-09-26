# Niribar

This application is part of the [Canopa workspace](../README.md). Cargo uses the
root lockfile and writes normal build outputs to `../target/` when invoked here.
The commands below describe the application; prefer the root validation entry points.

A configurable Wayland bar for Niri, built with Rust and Iced. It includes
workspaces, a launcher, tray menus, battery, network, volume, clock/calendar,
script modules, dmenu selection, and external panels.

## Build and run

Use Rust 1.95.0 (the CI baseline) or newer. On Debian/Ubuntu, install the native
build dependencies:

```sh
sudo apt-get install build-essential pkg-config libpulse-dev libwayland-dev libxkbcommon-dev libudev-dev dbus at-spi2-core
cargo +1.95.0 build --locked --release --bins
../target/release/niribar
```

Run inside a Niri/Wayland desktop session with its session D-Bus environment.
Keep `niribar-wifi-provider` alongside `niribar` or on PATH. PulseAudio or
PipeWire's PulseAudio compatibility service supplies audio state and control.
Ordinary desktop-entry launching requires `gio` (`libglib2.0-bin` on Debian/Ubuntu).
D-Bus-enabled entries activate through the session bus. Executable launches use niri's IPC; without `NIRI_SOCKET`, they require
`systemd-run` and a systemd user manager.

Configuration is read from `$XDG_CONFIG_HOME/niribar/config.kdl`, falling back
to `~/.config/niribar/config.kdl`. The embedded default works without a file;
copy [config/default.kdl](config/default.kdl) there to customize it. See
[docs](docs/) and [CONTEXT.md](CONTEXT.md) for configuration concepts.

Use `launch "kitty"` or `launch "firefox" "https://example.org"` in menu items
and module actions to open applications. Arguments are literal strings. Launches
belong to the desktop session and survive bar restarts. Existing `command`
actions remain bounded automation (three seconds by default); migrate any
application-opening `command` actions in your own config to `launch`.

```sh
niribar --version
niribar --toggle-panel
printf 'First\nSecond\n' | niribar --dmenu --prompt 'Choose' --no-custom
```

The IPC commands require a running bar. Selection input is bounded to 1 MiB.

## Install a release

Extract the Linux x86-64 release archive. It contains both executables, the
default configuration, this guide, the MIT license, and a user systemd unit.
From the extracted directory:

```sh
sudo install -m755 bin/niribar bin/niribar-wifi-provider /usr/bin/
mkdir -p ~/.config/systemd/user
cp share/systemd/user/niribar.service ~/.config/systemd/user/
systemctl --user daemon-reload
```

Start Niribar from Niri's startup configuration, or enable the supplied user
unit once your graphical session exports `WAYLAND_DISPLAY` and `NIRI_SOCKET`
to the systemd user manager. Do not start it through both mechanisms.

## Wi-Fi and external panels

The built-in Wi-Fi provider prefers NetworkManager; when it is absent, it uses
iwd. It supports scanning, saved/open networks, personal credentials, and
hidden personal networks. The system daemon controls authorization and saved
credentials. New enterprise/EAP provisioning is outside the panel's scope;
provision such profiles with the system's network tools first. iwd network
configuration also depends on the system's existing DHCP/DNS setup.

External providers use explicit `niribar-v1` or `canopa-v4` protocols. See
[the provider guide](docs/external-panel-providers.md).

## Development and validation

To update your local desktop installation whenever you choose, run:

```sh
./scripts/update.sh
```

This rebuilds the current checkout (including uncommitted changes) and its local
dependencies, installs both executables into `~/.local/bin`, and restarts
`niribar.service` if it is running or failed. An intentionally stopped service
stays stopped. Registry dependencies remain cached; rebuilding local packages
prevents an older checkout's cached release from being installed.

The service must already be configured to run `~/.local/bin/niribar`; this command
preserves your service configuration. After a restart, it checks that the process
survives startup and is running the installed executable, then prints its SHA-256.
If compilation fails, the installed binaries stay unchanged.

```sh
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --locked --all-targets -- -D warnings
cargo +1.95.0 test --locked --all-targets
python3 scripts/check-vendor.py
python3 scripts/check-contrast.py
python3 scripts/test-update.py
```

For the combined release archive, build all workspace binaries and run the root
packager as described in the [Canopa guide](../README.md).

The transport suite also requires `libglib2.0-bin`, `pipewire`, `pipewire-bin`, `pipewire-pulse`
and `pulseaudio-utils`. Tests start private D-Bus/audio daemons and Unix sockets;
environments that forbid local socket binding cannot run the transport suite.
Audio fixtures use synthetic sinks without hardware discovery. Tests do not
change your desktop's audio or Wi-Fi services. See [the audit](docs/audits/2026-09-23-project-state.md)
and [follow-up evidence](docs/audits/2026-09-23-remediation.md).
The [desktop validation harness](docs/manual-validation.md) runs a private
compositor session for measurements and panel checks. The
[accessibility assessment](ACCESSIBILITY.md) records default contrast ratios,
native AT-SPI coverage, and remaining operator checks. Nix packaging is deferred;
the optional CI job runs only if a flake is added.

Niribar is [MIT licensed](LICENSE). Vendored dependencies retain their own
licenses; see [vendor maintenance](docs/vendor-maintenance.md).
