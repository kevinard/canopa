# Notifications in Canopa

The default shell includes a notification indicator. Its count means **Requires
Attention**, including retained notifications whose banners have expired. Reading
the count or opening the Center does not acknowledge notifications.

- Click (or keyboard primary activation) opens the Notification Center.
- Right-click (or keyboard secondary activation) toggles Do Not Disturb in Notify.
- `DND` means banners and sound are silenced according to notification policy.
- `?` and an unavailable tooltip mean Notify cannot be reached; the shell retries.

Custom shell layouts can place `"notifications"` in a bar or panel zone. An optional
`module "notifications"` declaration accepts `format`, `expanded-format`,
`horizontal-format`, `tooltip`, and ordinary `actions`. Format values are `{icon}`,
`{value}` (the count, or `?`), `{state}` (`available`, `dnd`, `unavailable`), and
`{label}` (status and interaction help). Explicit actions override their matching
built-in trigger. `open-provider "notifications"` also works from other modules or
menus without a provider declaration. An explicit provider of that name overrides
the built-in Center; it does not change the notification summary's source.

## Start the desktop companions

Install the release archive's four `bin/` executables in `~/.local/bin`, its
`share/systemd/user/` units in `~/.config/systemd/user`, and optionally copy
`share/canopa/canopa.kdl` to `~/.config/canopa/config.kdl`. Shared and legacy files
remain supported. If an old custom button is named `notifications`, replace it
with `module "notifications"` (keeping desired action overrides), or rename that
button and its placements to `notification-launcher`: `notifications` is now a
reserved built-in module name. Explicit provider declarations remain supported.
The user units assume `~/.local/bin`; for another
prefix, adjust both services' `ExecStart` values.

Run these commands **inside the Niri session**. If the session launcher has not
already imported its environment, first publish the current compositor endpoints
to the D-Bus activation environment and user service manager:

```sh
dbus-update-activation-environment --systemd WAYLAND_DISPLAY NIRI_SOCKET XDG_CURRENT_DESKTOP
```

`WAYLAND_DISPLAY` selects Niri's display; `NIRI_SOCKET` lets the shell subscribe to
its windows and workspaces. Repeat the import for a new compositor session, before
starting the companions. The session launcher must activate
`graphical-session.target` for automatic startup and stop it when the session ends.
The units are ordered after `graphical-session-pre.target` and before
`graphical-session.target`. Then run:

```sh
systemctl --user daemon-reload
systemctl --user enable --now canopa-session.target
```

Use one notification daemon in that session; disable any previously configured
notification daemon before starting Notify. The target starts Niribar and Notify;
both remain independently restartable. No application writes or installs service
files automatically. Direct development launch also works without provider wiring:

```sh
target/debug/canopa-notify daemon --service
target/debug/niribar
```

The companions discover `$XDG_RUNTIME_DIR/canopa-notify/control.sock`. `--socket`
still selects an explicit endpoint for standalone Notify commands and fixtures.
The built-in shell module uses the standard session endpoint.

## Restart behavior

Restart Niribar to apply shell configuration changes. Notify keeps receiving and
presenting notifications; the new shell receives a complete summary immediately.
Restart Notify independently to see the indicator become unavailable, then recover
within a retry interval once the daemon accepts connections. DND and retained
history survive; runtime-only transient notifications do not. An already open
Center ends on daemon loss: close and reopen it after recovery. Old item actions
cannot target notification revisions from the restarted daemon.
