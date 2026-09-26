# Canopa Notify

This application is part of the [Canopa workspace](../README.md). Cargo uses the
root lockfile and writes normal build outputs to `../target/` when invoked here.
The commands below describe the application; prefer the root validation entry points.

A Wayland notification daemon implementing the freedesktop Desktop Notifications
interface, with non-focus-stealing banners, application rules, persistent local
history, and a Notification Center hosted by Niribar.

The target is the static Desktop Notifications 1.3 profile. The
[compliance matrix](docs/freedesktop-notification-compliance.md) records supported
capabilities, runtime requirements and explicit limits. The selected profile is
implemented and [verified](docs/notification-release-verification.md); a native
Notification portal v2 backend is separate scope.

## Build and test

Use Rust 1.95 or newer with the 2024 edition and Cargo.lock. On Ubuntu 24.04, install the
same desktop dependencies used by CI:

```sh
sudo apt-get install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev dbus at-spi2-core
cargo build --release --locked -p canopa-notify
cargo fmt --all -- --check
cargo clippy -p canopa-notify --all-targets --locked -- -D warnings
bash scripts/test.sh
bash scripts/check-core-dependencies.sh
```

Tests run on a private session bus. Native accessibility tests also create a
private runtime directory and AT-SPI bus; they require the launcher and registry
from `at-spi2-core`. They do not connect to the desktop notification service.
Graphical acceptance is tracked separately in the
[Center verification guide](docs/notification-center-verification.md).
Core-only builds use `--no-default-features`; `gui` and `audio` can be enabled
independently. See the [maintainer guide](docs/maintaining.md) for module ownership,
profiles and vendor updates.

## Install in your graphical session

Stop another notification daemon before starting Canopa Notify. It will fail
startup if the freedesktop name is already owned.

```sh
install -Dm755 ../target/release/canopa-notify "$HOME/.local/bin/canopa-notify"
install -Dm644 packaging/systemd/canopa-notify.service "$HOME/.config/systemd/user/canopa-notify.service"
install -Dm644 packaging/canopa-notify.1 "$HOME/.local/share/man/man1/canopa-notify.1"
systemctl --user daemon-reload
systemctl --user import-environment WAYLAND_DISPLAY XDG_CURRENT_DESKTOP
systemctl --user enable --now canopa-notify.service
```

The supplied user unit uses `%h/.local/bin/canopa-notify`. System packages should
replace that path with their installed binary path. The compositor must support
the wlr layer-shell protocol and start `graphical-session.target` for automatic
session startup. The Center requires the compatible host configuration in
[Niribar integration](docs/niribar-panel-provider.md).

Copy [the example configuration](examples/config.kdl) to
`$XDG_CONFIG_HOME/canopa-notify/config.kdl` (default
`~/.config/canopa-notify/config.kdl`). Configuration changes take effect on future
notification requests after a successful reload.

```sh
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" status --json
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" reload
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" dnd toggle
journalctl --user -u canopa-notify.service -b
```

Ensure `~/.local/bin` is in your interactive shell's PATH when using those commands.
See [service operation and recovery](docs/service-mode.md) for configuration,
control limits, import behavior, renderer failures, and smoke checks.

## Upgrade and remove

Stop the service before replacing its binary. Back up
`$XDG_STATE_HOME/canopa-notify` (default `~/.local/state/canopa-notify`) while it is
stopped, install the new build, then restart. Database migrations are forward only;
restore the backup before using an older binary that cannot read the new schema.
Restart invalidates old sender IDs and callbacks.

To remove the installation:

```sh
systemctl --user disable --now canopa-notify.service
rm "$HOME/.config/systemd/user/canopa-notify.service" "$HOME/.local/bin/canopa-notify" "$HOME/.local/share/man/man1/canopa-notify.1"
systemctl --user daemon-reload
```

Configuration and history remain available for backup or reinstallation. Remove
their directories separately if you intend to erase local notification content.

## Contributing

Start with [CONTEXT.md](CONTEXT.md), the [architecture decisions](docs/adr), and
the [implementation tracker](.scratch/freedesktop-completion-review/IMPLEMENTATION.md).
The daemon's public entry points live in `lib.rs`; runtime coordination, lifecycle,
SQLite storage, D-Bus, control transports, presentation, and sound each have an
owning module. Tests use those public boundaries and actual renderer widgets.
The generic panel protocol has one source in root `crates/canopa-panel-protocol`;
run `bash scripts/check-panel-protocol.sh` to check both workspace dependencies.

The project license is MIT OR Apache-2.0; vendored dependencies keep their upstream
licenses.
