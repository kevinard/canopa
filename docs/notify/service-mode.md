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

Graphical startup waits at most ten seconds for the renderer's first processed window redraw, after window/graphics setup, before importing or recovering history. Exit before this first readiness signal, or a readiness timeout, aborts startup without marking existing notifications stale or publishing a legacy import. Readiness is checked again before opening history; a later startup failure does not roll back an import already published after first readiness. This establishes event-loop readiness, not proof of visible compositor output. Headless mode does not start or wait for a renderer.

## Diagnostics

History and persisted user policy default to `$XDG_STATE_HOME/canopa-notify/history.db` (or `$HOME/.local/state/canopa-notify/history.db`); they no longer depend on the session socket directory. `--database` remains an explicit override. Startup takes socket/database ownership locks before recovery, rejects non-socket endpoint collisions and keeps private state permissions. When no destination exists, it imports an existing legacy socket-adjacent `.db` using a verified SQLite snapshot, preserving the source.

Legacy import also acquires the source database's normal ownership lock until the verified copy is published. Stop any daemon using that source, including one listening on another socket, before importing. An existing destination is always reused without inspecting or locking the legacy source. SQLite-aware import includes committed WAL data, preserves history and user policy, and applies normal restart recovery only to the destination; imported history does not regain live sender actions. This is a cooperative path-based ownership check, not a guarantee against unrelated SQLite writers or filesystem aliases that bypass the daemon's lock convention.

A failure before publication leaves the source and any existing destination intact, removes its unpublished copy and exits without serving notifications. Correct the source or access problem and retry; do not delete the source to silence an import error. Import uses [SQLite's consistent `VACUUM INTO` snapshot](https://www.sqlite.org/lang_vacuum.html), followed by verification and non-overwriting publication. After abrupt process interruption, the next default-path startup removes reserved `.history-import-*` snapshots and SQLite sidecars while holding the destination ownership lock, then retries or reuses the published destination. Cleanup accepts only owned regular files and refuses symlinks; unrelated files and older generic temporary filenames are left alone. SIGKILL migration rollback, interrupted snapshot writes and SQLite busy errors are covered by executable/public-daemon regressions. Power-loss and filesystem failure certification remain outside this guarantee. The executable regression suite is `bash scripts/test-notify.sh -p canopa-notify --test legacy_import`.

The installed configuration is the `notifications` namespace of `$XDG_CONFIG_HOME/canopa/config.kdl` (or `$HOME/.config/canopa/config.kdl`). When the common file is absent, `canopa-notify/config.kdl` remains supported. `daemon --canopa-config PATH` selects an explicit common source; `daemon --config PATH` keeps the legacy flat syntax. See [shared configuration](../configuration.md) for validation and migration. Initial configuration is loaded before accepting notifications. Invalid reloads retain the last valid configuration and expose the failure in status.

Configuration must be a readable regular UTF-8 file of at most 1 MiB; symlinks to regular files work, but directories, FIFOs and devices are not configuration streams. Inside the notification namespace (or a legacy flat file), supported KDL nodes are `rule`, `sound` and `icons`, with their documented entries only. Child documents and type annotations are rejected. Read, parse or installation failure leaves the committed path, generation and settings unchanged. Diagnostics identify the setting category or syntax byte offset without echoing arbitrary names, values or source text. File reading and parsing run off the async executor, and reloads remain serialized even if a waiting caller is cancelled. These are source/admission bounds, not a hard filesystem deadline or a limit on parser-internal nesting and work.

Select a static icon theme and source-asset scale with:

```kdl
icons theme="Adwaita" scale=2
```

Defaults are `theme="hicolor"` and `scale=1`. Scale is an integer from 1 to 8 used for icon lookup, not banner geometry; it is not automatically derived from the compositor. Theme names are case-sensitive local directory names. A reload applies selection to subsequent banner updates without changing existing notification delivery decisions. Missing icons fall back through inherited themes, hicolor and unthemed files. Unsafe metadata or exhausted lookup budgets omit icons. PNG is supported; JPEG is a local extension; SVG and XPM remain unsupported. Theme metadata and files are refreshed on the next banner update without restarting; simply editing a theme does not generate a notification update.

Control commands never start the daemon implicitly:

```bash
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" status
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" active --json
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" status --json
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" dnd on
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" dnd toggle
canopa-notify ctl --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock" reload
```

If another process already owns the runtime socket or the freedesktop notification service name, startup fails clearly instead of replacing the running daemon.

`dnd` without an argument reports the current state; changes survive restart. `reload` repeats the last committed selection (discovery or explicit source); `reload PATH` selects a legacy file and `reload-canopa PATH` selects a shared file. Each installs a validated candidate atomically. `clear` dismisses active notifications; `delete-history` removes inactive local history. JSON status includes configuration generation/error and source path/format/fingerprint, presentation error and sender-publication error. Retained attention follows the advertised `persistence` contract: expiry and restart preserve pending content; explicit deletion can remove inactive pending content.

If the renderer exits after startup, JSON status reports `banner renderer stopped; restart the daemon to restore banners` even without another notification. Notification receipt, sender lifecycles and Center controls remain available. The error is terminal for that renderer lifetime; restart the daemon explicitly to restore banners. The process stays running, so the unit's `Restart=on-failure` does not automatically recover this degraded state. Restart also invalidates live sender IDs and removes runtime-only transient notifications; retained history becomes stale as usual. See [ADR 0043](adr/0043-renderer-readiness-and-degraded-service.md).

Control requests must fit 64 KiB and ordinary replies must fit 8 MiB, including their terminating newline. Oversized replies return an error without truncating or deleting notification history. The CLI validates a complete reply before printing it and gives connect/write/read one 30-second deadline, with at most five seconds for writing. Malformed, incomplete, mismatched or oversized replies fail without printing their contents. A failed exchange can leave the command outcome unknown: the CLI never retries automatically. Inspect state before repeating a state-changing command, especially `dnd toggle` or `clear`. A completed partial-clear reply reports the number of committed dismissals.

Run automated checks on a private session bus with `bash scripts/test-notify.sh`. This avoids replacing the desktop notification service.

The D-Bus contract tests require `dbus-run-session` and `dbus-send` from the `dbus` package (also installed by CI). Run the focused suite with `bash scripts/test-notify.sh -p canopa-notify --test dbus_contract`. It checks introspection, method replies, malformed requests and ordered lifecycle signals through isolated headless executables; `dbus-send` independently verifies the server-information output shape. It does not replace the graphical checks below.

## Graphical Smoke Check

Headless Mode covers policy, storage, D-Bus receipt, control, and panel-provider behavior in automated tests. A graphical smoke check for banner rendering should run inside a Wayland Session:

```bash
canopa-notify daemon --service --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock"
notify-send "Canopa Notify" "Minimal banner smoke check"
```

Expected result: a non-focus-stealing Notification Banner appears in the Wayland Session, and `ctl active --json` reports the active notification.

For repeatable manual checks, use the scripts under `scripts/manual/`:

```bash
scripts/manual/run-service-daemon.sh
scripts/manual/notify-send-scenarios.sh smoke
scripts/manual/notify-send-scenarios.sh actions
scripts/manual/watch-notification-signals.sh
```

Run the signal watcher in a second terminal while using the `actions` scenario to confirm `ActivationToken` is emitted before `ActionInvoked` when the compositor provides a token.

## Shutdown and bounded background work

SIGTERM and session shutdown stop accepting mutations, cancel display timers and
sound, retire banner transport, close pending portal requests, and drain committed
sender events before closing the bus. Component waits are bounded and the service
has a fifteen-second daemon-drain deadline; remaining native blocking workers have
a five-second Tokio shutdown budget. Failed publication is reported as failure;
committed lifecycle state is not undone. Shutdown does not fabricate notification
closures, so restart can recover pending attention and invalidate old sender actions.

`status --json` also reports `link_opening.available`, `pending`, and a sanitized
`last_error`. No raw notification link target is logged. Body hyperlinks are
advertised only while an opener is available; lack of a portal leaves readable
text and does not prevent notification receipt. GUI/audio can be built separately;
see [build profiles](maintaining.md).

The installed Canopa-v4 provider uses bounded independent pages. Every accepted
retained or live transient item remains reachable, including unnamed senders;
unnamed groups are displayed as “Unknown application” without a persistent app-rule
control. Opaque section identifiers are distinct from claimed application identities.
Category controls cover categories on the current page and are deduplicated.
