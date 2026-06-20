# Canopa Notify Service Mode

Canopa Notify ships as one `canopa-notify` binary with `daemon`, `ctl`, and `panel-provider` modes.

## Startup

Install `packaging/systemd/canopa-notify.service` as a systemd user unit and enable it for the graphical session:

```bash
systemctl --user enable --now canopa-notify.service
```

The unit starts service mode:

```bash
canopa-notify daemon --service --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock"
```

Service mode owns the real `org.freedesktop.Notifications` D-Bus service name. Development and tests should use an isolated name instead:

```bash
canopa-notify daemon --headless --socket /tmp/canopa-notify.sock --dev-bus-name org.canopa.NotifyTest
```

## Diagnostics

Control commands never start the daemon implicitly:

```bash
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" status
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" active --json
```

If another process already owns the runtime socket or the freedesktop notification service name, startup fails clearly instead of replacing the running daemon.

## Graphical Smoke Check

Headless Mode covers policy, storage, D-Bus receipt, control, and panel-provider behavior in automated tests. A graphical smoke check for banner rendering should run inside a Wayland Session:

```bash
canopa-notify daemon --service --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock"
notify-send "Canopa Notify" "Minimal banner smoke check"
```

Expected result: a non-focus-stealing Notification Banner appears in the Wayland Session, and `ctl active --json` reports the active notification.
