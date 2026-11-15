# Canopa Notify

A Wayland notification daemon implementing the freedesktop Desktop Notifications
interface and the Notification portal (version 2). It shows banners that never take
keyboard focus, applies Do Not Disturb, per-application rules and sounds, keeps a
local SQLite history, and serves the Notification Center hosted by Canopa Shell.

The `canopa-notify` program has four modes: `daemon`, `ctl`, `panel-provider` and
`check-config`. See `man canopa-notify` ([source](../../packaging/canopa-notify.1)).

## Documentation

- [Notifications user guide](../../docs/notifications.md): banners, the Center, Do
  Not Disturb, preferences and `canopa-notify ctl`.
- [Configuration reference](../../docs/configuration.md#notifications): the
  `notifications` block. [examples/config.kdl](examples/config.kdl) is a commented
  sample (installed as `/usr/share/canopa-notify/config.kdl`).
- [Installation](../../docs/installation.md): the packaged session and local
  service setup.
- [Maintaining Notify](../../docs/notify/maintaining.md), the
  [freedesktop compliance matrix](../../docs/notify/freedesktop-notification-compliance.md)
  and the [Settings control API](../../docs/notify/settings-control-api.md).

## Development

This crate is part of the Canopa workspace; run commands from the repository root.

```sh
bash scripts/cargo.sh build --locked -p canopa-notify
bash scripts/test-notify.sh
```

Default features are `gui` and `audio`; `--no-default-features` builds the headless
core. Tests run on a private session bus and never replace the desktop's
notification service. For a development daemon that does not take the real service
name, use `canopa-notify daemon --headless --socket PATH --dev-bus-name NAME`.
See [testing](../../docs/testing.md) and
[docs/notify/CONTEXT.md](../../docs/notify/CONTEXT.md).

The project license is MIT OR Apache-2.0.
