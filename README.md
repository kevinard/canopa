# Canopa

Canopa is a desktop environment for Linux built around the
[Niri](https://github.com/niri-wm/niri) scrollable-tiling Wayland compositor. It
is written in Rust with [Iced](https://iced.rs) and provides the pieces a desktop
needs on top of the compositor:

| Component | Binary | What it does |
| --- | --- | --- |
| Canopa Shell | `canopa-shell` | Bar, launcher, taskbar, Quick settings, panels for Wi-Fi, Bluetooth, power, sound and media, on-screen display |
| Shell providers | `canopa-wifi-provider`, `canopa-audio-provider`, `canopa-media-provider` | Wi-Fi (NetworkManager), sound (PulseAudio/PipeWire) and MPRIS media panels |
| Canopa Notify | `canopa-notify` | Notification daemon: banners, Notification Center, Do Not Disturb, history, sounds, Notification portal |
| Canopa Files | `canopa-files` | File manager: tabs, search, Recent, archives, network locations, previews, Undo and crash-safe transfers |
| Canopa Settings | `canopa-settings` | Displays, network, Bluetooth, sound, power, appearance, accessibility, users, region and more |
| Wallpaper and lock | `canopa-wallpaper`, `canopa-lock`, `canopa-idle` | Per-monitor wallpaper, secure session lock, Stasis idle integration |
| Login screen | `canopa-greeter` | greetd greeter |
| Authentication dialogs | `canopa-polkit`, `canopa-pinentry` | polkit agent and GnuPG pinentry |
| Portal backend | `xdg-desktop-portal-canopa` | Desktop portals: file chooser, screenshots, screen sharing, settings and more |
| Command-line tool | `canopa` | One entry point: `canopa config check`/`migrate`, `canopa settings`, and the `wallpaper`, `lock`, `greeter`, `polkit`, `pinentry` and `idle` tools |

All components share one configuration file, `~/.config/canopa/config.kdl`, one
appearance (dark or light theme with an accent color) and one set of translations
(English and French).

## Install

The supported installation is the set of Arch Linux packages built from
`packaging/arch/`, which provide a dedicated **Canopa** login session with the
pinned compositor. A portable release archive with user services is also
available. See [installation](docs/installation.md).

## Documentation

Start with the [documentation index](docs/README.md). The main user guides:

- [Installation](docs/installation.md) and [configuration](docs/configuration.md)
- [Shell](docs/shell.md), [launcher](docs/launcher.md), [notifications](docs/notifications.md),
  [Files](docs/files.md), [Settings](docs/settings.md)
- [Wallpaper, lock and idle](docs/session-tools.md), [login screen](docs/greeter.md),
  [polkit](docs/polkit.md), [pinentry](docs/pinentry.md)
- [Desktop portals](docs/portals.md), [desktop sharing](docs/desktop-sharing.md),
  [printing](docs/printing.md), [themes](docs/theme-customization.md)

## Develop

```sh
bash scripts/cargo.sh build --workspace --bins --locked   # binaries in target/debug/
bash scripts/check.sh                                     # focused default checks
bash scripts/check.sh --workspace                         # the full gate
```

Use Rust 1.95.0 (selected by `rust-toolchain.toml`) and build through
`scripts/cargo.sh`, which guards free disk space. Read the
[contributing guide](docs/contributing.md) for system dependencies, workflow and
documentation rules, and the [architecture overview](docs/architecture/overview.md)
for how the pieces fit together. Issues are tracked on
[GitHub](https://github.com/kevinard/canopa/issues).

## License

Canopa's own code is MIT or MIT OR Apache-2.0 depending on the crate; distributed
Shell and Settings executables are GPL-3.0-or-later because they link `niri-ipc`.
Forked dependencies keep their upstream licenses. See [LICENSES.md](LICENSES.md)
and the generated [dependency attributions](docs/dependency-attributions.md).
