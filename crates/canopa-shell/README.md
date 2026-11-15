# Canopa Shell

Canopa's desktop shell for the Niri compositor, written in Rust with Iced and
layer-shell surfaces. It draws the bar, Quick settings and other panels, the
Applications launcher, the taskbar, the tray, the on-screen display for device
keys, and hosts the Notification Center.

This crate builds four programs:

| Program | Role |
| --- | --- |
| `canopa-shell` | The shell, its command line (`reload`, `--toggle-menu`, device keys, `--dmenu`) and the built-in Bluetooth and Power panels |
| `canopa-wifi-provider` | Wi-Fi panel (NetworkManager or iwd) |
| `canopa-audio-provider` | Sound panel (PulseAudio or PipeWire's Pulse service) |
| `canopa-media-provider` | Media panel (MPRIS players) |

Install the providers beside `canopa-shell` or on `PATH`, together with
`canopa-notify` for the Notification Center.

## Documentation

- [Canopa Shell user guide](../../docs/shell.md): the bar, modules, Quick settings,
  panels, device keys and the command line.
- [Launcher](../../docs/launcher.md): search, calculator, extensions and clipboard
  history.
- [Configuration reference](../../docs/configuration.md#shell): every `shell` key.
  The embedded defaults are in [config/default.kdl](config/default.kdl).
- [Theme customization](../../docs/theme-customization.md).
- [Installation](../../docs/installation.md): packages, the session and updates.
- [External panel providers](../../docs/shell/external-panel-providers.md): the
  provider protocol.

## Development

This crate is part of the Canopa workspace; run commands from the repository root.
The `canopa-shell` binary needs the default `desktop` feature.

```sh
bash scripts/cargo.sh build --locked -p canopa-shell --bins
target/debug/canopa-shell --defaults
```

Run it inside a Niri session with the session D-Bus environment (`WAYLAND_DISPLAY`,
`NIRI_SOCKET`). Stop the session's `canopa-shell.service` first: only one Shell
runs per session. See [building](../../docs/building.md) and
[testing](../../docs/testing.md) for checks, and
[docs/shell/CONTEXT.md](../../docs/shell/CONTEXT.md) for the domain vocabulary.

Canopa Shell is [MIT licensed](LICENSE).
