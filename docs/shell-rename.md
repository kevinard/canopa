# Migrating Niribar to Canopa Shell

Niribar is now **Canopa Shell**. Build and install its companions from the same
Canopa release.

| Previous name | Current name |
| --- | --- |
| `niribar` executable / Cargo package | `canopa-shell` |
| `niribar-wifi-provider` | `canopa-wifi-provider` |
| `niribar.service` | `canopa-shell.service` |
| `$XDG_RUNTIME_DIR/niribar.sock` | `$XDG_RUNTIME_DIR/canopa-shell.sock` |
| Layer namespace `niribar` / `niribar-dismissal` | `canopa-shell` / `canopa-shell-dismissal` |
| `RUST_LOG=niribar=debug` | `RUST_LOG=canopa_shell=debug` |
| `NIRIBAR_PERF` | `CANOPA_SHELL_PERF` |
| `NIRIBAR_WIFI_PROVIDER_BACKEND` | `CANOPA_WIFI_PROVIDER_BACKEND` |
| `crates/niribar/` / `docs/niribar/` | `crates/canopa-shell/` / `docs/shell/` |

Configuration continues to use `~/.config/canopa/config.kdl` and its `shell` block.
An existing `~/.config/niribar/config.kdl` is still discovered if the shared file
is absent. Do not move it to a `canopa-shell` directory. See the
[configuration guide](configuration.md) to migrate it into the shared file.
Notification history, DND, appearance, and notification sockets are unchanged.

Update literal `niribar` commands in Niri startup, key bindings and scripts to
`canopa-shell`, and explicit `niribar-wifi-provider` commands to
`canopa-wifi-provider`. Built-in `open-provider "wifi"` needs no change. Update
compositor rules matching the layer namespaces as well. Keep protocol selectors
such as `niribar-v1`: they describe an unchanged wire contract. Old performance
and provider-backend environment variables remain fallbacks; when both are set,
the new variable takes precedence. `RUST_LOG` filters need the new target names.

## Existing local service installation

The normal updater expects `canopa-shell.service`. Complete this one-time switch
before running `scripts/update.sh` again. From the repository root, build and
validate the new companion binaries before stopping anything:

```sh
cargo build --locked --release --bins -p canopa-shell -p canopa-notify -p canopa-cli
target/release/canopa config check --show-source
```

Back up your installed binaries and user service definitions. Port any custom
`niribar.service` overrides into `canopa-shell.service.d/`, updating `ExecStart`
and logging targets. For the standard `~/.local/bin` companion installation,
inside your graphical session:

```sh
systemctl --user disable --now niribar.service
systemctl --user stop canopa-notify.service
install -m755 target/release/canopa-shell target/release/canopa-wifi-provider \
    target/release/canopa-notify target/release/canopa ~/.local/bin/
install -m644 packaging/systemd/canopa-shell.service \
    packaging/systemd/canopa-notify.service packaging/systemd/canopa-session.target \
    ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable canopa-session.target
systemctl --user start canopa-shell.service canopa-notify.service
canopa config check
systemctl --user status canopa-shell.service canopa-notify.service
```

Starting the companions explicitly also works when `canopa-session.target` was
already active. If Notify was intentionally stopped, leave it stopped. Retire the
old executable and disabled service definition after verifying the new installation.
Do not leave a separate Niri startup command launching the old shell.

For a direct compositor launch, stop the old shell, install the new binaries,
update its startup command and bindings, and launch `canopa-shell` once. Use one
startup mechanism per process. Release archive users follow the same service
switch using `bin/` and `share/systemd/user/` from the extracted bundle.
