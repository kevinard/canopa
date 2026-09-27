# Canopa Notify

This application is part of the [Canopa workspace](../../README.md). Run the
commands below from the repository root. Cargo uses the root lockfile and writes
normal build outputs to root `target/`.

A Wayland notification daemon implementing the freedesktop Desktop Notifications
interface, with non-focus-stealing banners, application rules, persistent local
history, and a Notification Center hosted by Niribar.

The target is the static Desktop Notifications 1.3 profile. The
[compliance matrix](../../docs/notify/freedesktop-notification-compliance.md) records supported
capabilities, runtime requirements and explicit limits. The selected profile is
implemented and [verified](../../docs/notify/notification-release-verification.md); a native
Notification portal v2 backend is separate scope.

## Build and test

Use Rust 1.95 or newer with the 2024 edition and Cargo.lock. On Ubuntu 24.04, install the
same desktop dependencies used by CI:

```sh
sudo apt-get install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev dbus at-spi2-core
cargo build --release --locked -p canopa-notify
cargo fmt --all -- --check
cargo clippy -p canopa-notify --all-targets --locked -- -D warnings
bash scripts/test-notify.sh
bash scripts/check-core-dependencies.sh
```

Tests run on a private session bus. Native accessibility tests also create a
private runtime directory and AT-SPI bus; they require the launcher and registry
from `at-spi2-core`. They do not connect to the desktop notification service.
Graphical acceptance is tracked separately in the
[Center verification guide](../../docs/notify/notification-center-verification.md).
Core-only builds use `--no-default-features`; `gui` and `audio` can be enabled
independently. See the [maintainer guide](../../docs/notify/maintaining.md) for module ownership,
profiles and vendor updates.

## Install in your graphical session

Stop another notification daemon before starting Canopa Notify. It will fail
startup if the freedesktop name is already owned.

```sh
install -Dm755 target/release/canopa-notify "$HOME/.local/bin/canopa-notify"
install -Dm644 packaging/systemd/canopa-notify.service "$HOME/.config/systemd/user/canopa-notify.service"
install -Dm644 packaging/canopa-notify.1 "$HOME/.local/share/man/man1/canopa-notify.1"
systemctl --user daemon-reload
systemctl --user enable --now canopa-notify.service
```

Before starting services, follow the shared [graphical session environment setup](../../docs/notifications.md#start-the-desktop-companions). For the full desktop, that guide installs and starts both companions together.

The supplied user unit uses `%h/.local/bin/canopa-notify`. System packages should
replace that path with their installed binary path. The compositor must support
the wlr layer-shell protocol and start `graphical-session.target` for automatic
session startup. The Center requires the compatible host configuration in
[Niribar integration](../../docs/notify/niribar-panel-provider.md).

Configuration comes from the `notifications` namespace of
`$XDG_CONFIG_HOME/canopa/config.kdl` (default `~/.config/canopa/config.kdl`). When the
common file is absent, the legacy `canopa-notify/config.kdl` is still supported.
Put [the example settings](examples/config.kdl) inside that namespace, or use the
[shared configuration guide](../../docs/configuration.md) to migrate and validate both
applications. Policy changes affect future requests after a successful Notify reload.
The shared `appearance` block selects the same palette as Niribar and repaints
existing banners on reload; the shell applies it after a restart.

```sh
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" status --json
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" reload
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" dnd toggle
journalctl --user -u canopa-notify.service -b
```

Ensure `~/.local/bin` is in your interactive shell's PATH when using those commands.
See [service operation and recovery](../../docs/notify/service-mode.md) for configuration,
control limits, import behavior, renderer failures, and smoke checks.

## Upgrade and remove

For the standard local companion installation, run `./scripts/update.sh` from the
workspace root to build and update Notify, Niribar, its Wi-Fi helper, and `canopa`
together. It validates configuration, backs up stopped notification state, and
checks startup with automatic rollback on failure. See the
[workspace update guide](../../README.md) for prerequisites and options.

For a manual upgrade:

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

Start with [CONTEXT.md](../../docs/notify/CONTEXT.md), the [architecture decisions](../../docs/notify/adr), and
the [implementation tracker](../../.scratch/notify/freedesktop-completion-review/IMPLEMENTATION.md).
The daemon's public entry points live in `lib.rs`; runtime coordination, lifecycle,
SQLite storage, D-Bus, control transports, presentation, and sound each have an
owning module. Tests use those public boundaries and actual renderer widgets.
The generic panel protocol has one source in root `crates/canopa-panel-protocol`;
run `bash scripts/check-panel-protocol.sh` to check both workspace dependencies.

The project license is MIT OR Apache-2.0; vendored dependencies keep their upstream
licenses.
