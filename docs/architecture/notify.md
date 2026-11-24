# Canopa Notify architecture

Canopa Notify (`crates/canopa-notify`) is the session's notification daemon. It
serves `org.freedesktop.Notifications` and the Notification portal backend, decides
delivery, keeps history in SQLite, draws banners, plays sounds and supplies the
Notification Center that Canopa Shell hosts. The user guide is
[notifications.md](../notifications.md); maintenance procedures are in
[notify/maintaining.md](../notify/maintaining.md); the freedesktop support matrix
is [notify/freedesktop-notification-compliance.md](../notify/freedesktop-notification-compliance.md).

Domain vocabulary (Notification Thread, Delivery Outcome, Requires Attention,
Notification Center Projection, ...) is defined in
[docs/notify/CONTEXT.md](../notify/CONTEXT.md). Notify decision records live in
[docs/notify/adr/](../notify/adr/). Earlier shared decision records were
consolidated into this document; the originals are in the git history at commit
cb300936 under docs/adr/.

## Process model

One binary, `canopa-notify`, has four subcommands (clap, `src/main.rs`;
[Notify ADR 0006](../notify/adr/0006-single-binary-with-subcommands.md)):

| Subcommand | Role |
| --- | --- |
| `daemon --service` | The session daemon: owns `org.freedesktop.Notifications` and `org.freedesktop.impl.portal.desktop.canopa.notify`, draws banners. Requires Wayland and the `gui` feature. |
| `daemon --headless [--dev-bus-name NAME]` | Same core without renderer, sound or portal; optional private bus name. Used by tests. |
| `ctl [--socket P] COMMAND` | Control client (status, DND, preferences, clear, history). Never starts the daemon. |
| `panel-provider [--protocol canopa-v4\|canopa-v5] [--socket P]` | Notification Center provider spawned by the Shell. The CLI default is `canopa-v4`; the Shell passes `canopa-v5`. |
| `check-config` | Validates the `notifications` namespace and prints one JSON line. |

The daemon is started by `packaging/systemd/canopa-notify.service`
(`daemon --service --socket %t/canopa-notify/control.sock`, part of
`canopa-session.target`). There is no D-Bus activation file for
`org.freedesktop.Notifications`: the systemd unit is the only starter
([Notify ADR 0008](../notify/adr/0008-systemd-user-service-before-dbus-activation.md)).
The portal backend name is D-Bus activatable through
`packaging/portal/org.freedesktop.impl.portal.desktop.canopa.notify.service`,
which names the same systemd unit.

The daemon runs on Tokio with zbus ([ADR 0017](../notify/adr/0017-tokio-daemon-runtime.md),
[ADR 0023](../notify/adr/0023-zbus-for-dbus.md)). Banners are drawn in the same
process ([ADR 0018](../notify/adr/0018-same-process-ui-for-version-one.md)).

Features: `default = ["gui", "audio"]`. `gui` adds Iced, `iced_layershell`,
Wayland, image preparation, AccessKit and Niri IPC; `audio` adds Rodio. A
`--no-default-features` build runs `daemon --headless`, `ctl` and
`panel-provider` and rejects `--service`.

### Startup order

`run_daemon` in `src/main.rs`:

1. Resolve the socket (default `$XDG_RUNTIME_DIR/canopa-notify/control.sock`; an
   absolute `XDG_RUNTIME_DIR` is required) and the database
   (default `$XDG_STATE_HOME/canopa-notify/history.db`).
2. Take the socket and database ownership locks. Another owner is an error; a
   running daemon is never replaced.
3. Connect to the session bus and request `org.freedesktop.Notifications`.
4. Start the renderer and wait up to 10 s for its first processed redraw
   ([ADR 0043](../notify/adr/0043-renderer-readiness-and-degraded-service.md)).
5. Import a legacy socket-adjacent `<socket>.db` if no destination exists, then
   open history (restart recovery marks retained active rows stale).
6. Load configuration, then start the control socket, banner input forwarder and
   compositor suppression tracker, attach the D-Bus server and finally request the
   portal bus name.

SIGTERM stops new mutations, cancels timers and sound, retires banners, closes
pending portal requests and drains committed sender events within bounded
deadlines.

## Source map

| Owner | Responsibility |
| --- | --- |
| `lib.rs`, `runtime.rs` | `Daemon` entry point, the mutation coordinator, revision guards, committed effects |
| `lifecycle.rs`, `domain.rs` | Notification Threads, sender lifecycle, retained attention, transience, actions |
| `notification_server.rs`, `notification_server/` | freedesktop wire interface and live sender-ID registry; `portal.rs` is the Notification portal adapter |
| `notification_server_intake.rs`, `intake.rs`, `input_limits.rs`, `notification_link.rs` | Request decoding, bounded markup and body segments, safe link targets |
| `delivery_policy.rs`, `preferences.rs` | Rules, DND, break-through, suppression, per-app preferences |
| `sqlite_persisted_state.rs`, `migrations/` | SQLite store and forward migrations |
| `startup.rs` | Paths, ownership locks, legacy import |
| `control.rs`, `control_adapters.rs`, `control_adapters/` | Typed control commands, the Unix socket, CLI formatting, Center pages |
| `center.rs` | Keyset reads for the Center |
| `presentation.rs`, `presentation/renderer.rs` and children | Banner projection, Iced renderer, surfaces, images, accessibility |
| `sound.rs`, `sound/` | Sound playback and sound-theme lookup |
| `link_opening/` | OpenURI portal opener |
| `activation/` | Wayland activation tokens for banner input |
| `compositor_suppression.rs` | Fullscreen and screencast state from Niri |
| `configuration.rs` | `notifications` namespace parser and reload |

## Request flow

1. **Receipt.** `Notify` (D-Bus) or `AddNotification` (portal) is decoded into
   `NotificationInput`. Fixed admission limits apply before anything changes:
   body 64 KiB, summary 8 KiB, other fields 4 KiB, 128 KiB total text, 64 actions,
   64 hints ([ADR 0031](../notify/adr/0031-bounded-notification-intake.md)). Over-limit
   requests get `org.freedesktop.DBus.Error.LimitsExceeded` and consume no ID.
2. **Identity.** App identity prefers the `desktop-entry` hint, then the app name
   ([ADR 0002](../notify/adr/0002-app-identity-uses-desktop-entry-first.md)).
   Claimed identity is grouping and policy metadata, not authentication.
3. **Delivery decision** (`DeliveryPolicy::decide_at`): an explicit drop rule wins
   first; then DND and automatic suppression silence the notification unless it is
   critical or a break-through category; then user rules, configuration rules and
   the default. Outcomes are shown, silenced (saved quietly) or dropped
   ([ADR 0003](../notify/adr/0003-first-decisive-notification-policy-rules.md)).
4. **Admission.** Live resources are capped at 1,024 threads, 16 MiB of text and
   64 MiB of raw images; a full count expires the oldest non-critical thread
   ([ADR 0032](../notify/adr/0032-live-notification-resource-admission.md),
   [ADR 0054](../notify/adr/0054-bounded-retention-of-undisplayed-and-pending-notifications.md)).
5. **Commit.** The mutation coordinator writes lifecycle changes to SQLite, then
   publishes effects in order: banner updates, sound, timers and sender events
   (`NotificationClosed`, `ActionInvoked`, `ActivationToken`, portal signals)
   ([ADR 0030](../notify/adr/0030-mutation-commit-and-effects.md)). Slow file,
   device and portal work runs outside the coordinator.

Timeouts start when a revision is first displayed in a banner, not at receipt;
silenced and DND notifications start at acceptance
([ADR 0028](../notify/adr/0028-notification-timeouts-begin-with-banner-display.md)).
The default timeout is 5 s for low and normal urgency; critical never expires.
Retained content keeps an attention marker until acknowledged, removed or recalled
by its sender; expiry and restart do not clear it
([ADR 0049](../notify/adr/0049-retained-attention-and-sender-lifecycle.md)).
Transient notifications live only in memory
([ADR 0041](../notify/adr/0041-runtime-only-transient-center-actions.md)). Sender IDs
are a daemon-lifetime namespace independent of history
([ADR 0029](../notify/adr/0029-live-notification-id-boundary.md)).

## Storage

History and policy state are one SQLite database accessed through `sqlx` with a
single connection and a 5 s busy timeout
([ADR 0004](../notify/adr/0004-persistent-local-notification-history.md),
[ADR 0012](../notify/adr/0012-sqlite-for-persisted-state.md)).

- Tables: `notification_threads`, `user_rules` and `daemon_metadata` (DND state,
  timers and schedule, per-app preferences).
- Migrations are forward-only SQL files in `crates/canopa-notify/migrations/`,
  applied in one transaction by comparing `PRAGMA user_version`. A database newer
  than the binary is refused.
- Retention: inactive history is pruned to 30 days and 1,000 threads; pending
  (unread) content is never pruned by age or count, but is capped at 4,096
  notifications and 64 MiB of text, archiving the oldest inactive non-critical
  pending items as read when full.
- Legacy import: when the default database is absent, a socket-adjacent
  `<socket>.db` is copied with SQLite `VACUUM INTO`, verified and published without
  overwriting. The source is kept.

Configuration-level preferences come from the config file; UI-managed preferences
(Settings, `ctl app`, Center) are stored in the database
([ADR 0011](../notify/adr/0011-ui-managed-rules-use-user-rule-store.md)).

## Presentation

Banners are Iced layer-shell surfaces that never take keyboard focus
([ADR 0005](../notify/adr/0005-daemon-owned-notification-banners.md),
[ADR 0045](../notify/adr/0045-compositor-sized-banner-surface.md),
[ADR 0047](../notify/adr/0047-compositor-selected-banner-stack-lifetimes.md)).

- Position is configured with `banners { position ... }` (default `bottom-start`;
  `top-start`, `top-end`, `bottom-end`), mirrored for right-to-left locales. Cards
  are at most 360 logical pixels wide; the body preview is capped at 120 logical
  pixels and oversized cards scroll inside a bounded viewport
  ([ADR 0046](../notify/adr/0046-scrollable-oversized-compact-cards.md)).
- Each card acknowledges its own first display, which starts its timeout
  ([ADR 0044](../notify/adr/0044-per-card-banner-display-acknowledgement.md)).
- All named actions are shown ([ADR 0048](../notify/adr/0048-all-named-actions-in-banner-cards.md)).
  With the default button style, a single action sizes to its label, while
  multiple actions use the two-column layout. The inline reply composer
  accepts up to 4,096 bytes.
- Images are read and decoded on a worker with fixed limits (16 MiB per file,
  4,096 pixels per side) ([ADR 0036](../notify/adr/0036-bounded-banner-image-preparation.md)).
  Icons use an indexed, bounded icon-theme lookup that follows the desktop theme
  unless `icons theme="..."` is set ([ADR 0037](../notify/adr/0037-bounded-indexed-icon-discovery.md)).
- Banner input asks for an `xdg_activation_v1` token from the click's serial and
  surface (100 ms budget) before publishing `ActivationToken` and `ActionInvoked`.
- Banners expose AT-SPI semantics through AccessKit; the first displayed revision
  is announced politely once.
- If the renderer stops after startup, status reports it and notifications are
  still received and listed in the Center; restart the daemon to restore banners.

The shared `appearance` block styles banners and repaints existing ones on reload.

## Sound

With the `audio` feature, sound plays in-process through Rodio
([ADR 0025](../notify/adr/0025-in-process-notification-sound-playback.md)). Sound
names resolve through the freedesktop sound theme (default theme `freedesktop`,
default sound `message-new-instant`) with bounded index reads
([ADR 0035](../notify/adr/0035-bounded-sound-theme-resolution.md)). At most four
playback jobs run at once, files are limited to 8 MiB and decoded audio to 10 s
([ADR 0034](../notify/adr/0034-bounded-notification-sound-playback.md)). The
`sound` capability is advertised only while playback is available; the audio
device is released when idle ([ADR 0051](../notify/adr/0051-idle-audio-output-lifetime.md)).

## Compositor suppression

In service mode, `compositor_suppression.rs` follows Niri's IPC event stream
(windows, workspaces, outputs and casts). A focused-output active window that
covers the whole logical output counts as fullscreen; any Niri cast (including
the Canopa ScreenCast portal) counts as screen sharing. Both silence banners
unless disabled under `do-not-disturb { suppress ... }`; screen sharing also
hides content. The tracker reconnects with backoff (2 s to 30 s).

## Control socket and clients

The private Unix socket ([ADR 0007](../notify/adr/0007-private-control-api-over-unix-socket.md))
accepts newline-terminated requests: `control <json>` (typed `ControlCommand`),
`status`, `active`, `active-json`, the Center streams `panel-session` and
`panel-subscribe`, `panel-interaction <json>`, and the two feeds below.

- Requests are at most 64 KiB; ordinary replies at most 8 MiB
  ([ADR 0039](../notify/adr/0039-bounded-control-replies.md)). At most 64
  connections are served; panel and summary streams have eight subscriptions each.
- `canopa-notify ctl` gives each exchange one 30 s deadline and never retries; a
  failed exchange can leave the outcome unknown.

`crates/canopa-notify-client` is the small client library (Tokio and Serde only; no
storage, policy or GUI dependencies) shared by other processes:

| Request | Consumer | Content |
| --- | --- | --- |
| `summary-subscribe-v1` | Canopa Shell `notifications` module | Complete snapshots of daemon instance, attention count, DND state, reason and end time, and suppression; at most 512 bytes per frame |
| `lock-screen-subscribe-v1` | Lock screen (`canopa-session-ui`) | At most four "App — text" entries plus a "more" count, already filtered by each app's lock-screen preference; no actions, IDs or bodies |
| `control <json>` via `canopa_notify_client::preferences` | Canopa Settings | DND, schedule, break-through, suppression and per-app preferences; see [settings-control-api.md](../notify/settings-control-api.md) |

Clients connect with a 900 ms deadline for the first snapshot and a 5 s
partial-frame deadline. A disconnect invalidates the snapshot; clients reconnect
and replace it rather than merge.

## Notification Center provider

The Center is daemon-owned content hosted by the Shell
([ADR 0001](../notify/adr/0001-daemon-owned-notification-center.md),
[ADR 0014](../notify/adr/0014-shared-panel-protocol-crate.md),
[ADR 0020](../notify/adr/0020-streaming-control-api-for-panel-state.md)).
`canopa-notify panel-provider` connects to the control socket with
`panel-session` and translates the daemon's Notification Center Projection into
generic rich panel content (`src/control_adapters.rs`, `control_adapters/pages.rs`).

- The reading view is one list ordered newest first, with a Group by app toggle,
  labeled unread indicators, per-item actions (Open, Dismiss, Mark as read, Remove,
  Reply), Mark
  all read, Clear all with a 30 s Undo, and DND quick choices. A Settings view holds
  per-app and per-category delivery choices and Delete history. The close
  control dismisses live notifications and removes completed history entries.
  Open stays a labeled action on the card
  ([ADR 0052](../notify/adr/0052-center-reading-and-settings-views.md),
  [ADR 0055](../notify/adr/0055-everyday-controls-lock-screen-and-portal-v2-presentation.md)).
- Pages are built from keyset reads of at most 16 rows and capped at 1 MiB per
  frame; each view keeps up to 128 previous positions
  ([ADR 0038](../notify/adr/0038-bounded-panel-provider-streams.md)).
- Every action value carries a Notification Center Target bound to the displayed
  revision and daemon instance, so a queued action cannot act on replaced content
  or a restarted daemon ([ADR 0042](../notify/adr/0042-revision-bound-notification-center-actions.md)).
- Body links are opaque provider actions; the validated URL travels separately as
  accessibility metadata, and Notify opens it through the OpenURI portal
  ([ADR 0050](../notify/adr/0050-revision-scoped-notification-links.md)).
- Under `canopa-v5`, actions that need an activation token opt in and receive the
  token the Shell obtained from its own input (see Design decisions).

Shell-side hosting and protocol details: [shell.md](shell.md),
[panel-protocol.md](panel-protocol.md) and
[external-panel-providers.md](../shell/external-panel-providers.md).

## Notification portal adapter

`src/notification_server/portal.rs` implements
`org.freedesktop.impl.portal.Notification` version 2 at
`/org/freedesktop/portal/desktop` on the bus name
`org.freedesktop.impl.portal.desktop.canopa.notify`. The descriptor
`packaging/portal/canopa-notify.portal` (`UseIn=canopa`) is routed by
`packaging/system/canopa-portals.conf`
(`org.freedesktop.impl.portal.Notification=canopa-notify`).

- `SupportedOptions` advertises categories `im.received`, `alarm.ringing`,
  `call.incoming`, button purposes `im.reply-with-text` and the call controls, and
  the Canopa option `org.canopa.request-timeout-ms`.
- Priority `low`, `normal`/`high` and `urgent` map to low, normal and critical
  urgency. All v2 `display-hint` values are honoured.
- Ordinary portal notifications use the server-default timeout to hide the banner
  only; the notification and its actions stay live until the user acts or the
  application withdraws it. Application exit does not withdraw it.
- The transport app ID owns the notification. Its localized desktop-entry name
  supplies the sender label when available.
- Icon and sound descriptors must be sealed; their contents are copied into
  bounded owned buffers before receipt.
- Exported `app.*` actions and default activation call
  `org.freedesktop.Application` directly with the activation token; other actions
  are signalled to the portal frontend.

See [portals.md](portals.md) for routing across all portals.

## Configuration

The `notifications` namespace of `$XDG_CONFIG_HOME/canopa/config.kdl` has the root
nodes `rule`, `app`, `do-not-disturb`, `sound`, `icons` and `banners`; anything
else, and type annotations, are errors ([ADR 0010](../notify/adr/0010-kdl-configuration.md)).
The legacy `$XDG_CONFIG_HOME/canopa-notify/config.kdl` is read only when the shared
file is absent. Files must be regular UTF-8 files of at most 1 MiB
([ADR 0040](../notify/adr/0040-safe-configuration-candidates.md)).

There is no file watching: `ctl reload` (repeat the current selection),
`ctl reload PATH` (legacy file) and `ctl reload-canopa PATH` (shared file) install a
validated candidate atomically. Canopa Settings calls `canopa-notify ctl reload`
after saving. A failed reload keeps the last good configuration; policy changes
apply to future notifications
([ADR 0027](../notify/adr/0027-policy-changes-apply-to-future-notification-requests.md)).

## Design decisions

**The bar shows a summary stream, not history.** The Shell's `notifications`
module gets attention count and DND from a bounded summary subscription in
`canopa-notify-client`, which has no storage, policy or GUI dependencies. Notify
counts retained Requires Attention rows with an index and reads DND under its
mutation coordinator. One subscription is shared by every module instance.
Consequence: Shell and Notify restart independently; banners keep working while
the Shell is stopped, and an open Center ends on daemon loss and must be reopened
(queued actions are never replayed against a new daemon).

**The Notification portal adapter lives inside Notify.** Notify already owns
policy, lifecycle, history, banners, Center, sound and input-bound activation. A
separate bridge would see action signals only after commitment and look up action
targets in its own mutable table, so a replacement could change a target before an
older committed action arrived. Inside Notify, the portal binding (including action
targets) is frozen under the same mutation coordinator as the receipt, and the
queued publication keeps the binding it observed. Consequence: the portal backend
shares Notify's service lifetime and restart contract, and needs its own descriptor
(`canopa-notify.portal`) separate from the main Canopa portal backend.

**Panel actions carry input-bound activation context on request.** Notify cannot
obtain an activation token for a Center click because the Shell owns that surface
and input serial. Generic v5 rich actions can opt in with
`request_activation_token`; the Shell then requests a token through its own
layer-shell connection with the recent input serial and adds an optional
`activation_token` to the v5 event. The token is bounded, tied to the captured
revision and control, and never reused across clicks; a missing or failed token
sends the same action without one. Consequence: the v4 wire is unchanged, action
values stay opaque to the Shell, and activation works only through `canopa-v5`.

## Known limitations

- Animated or multi-frame icons (`icon-multi`) are not supported.
- XPM icons are not supported.
- Configuration is not watched; reload with `canopa-notify ctl reload`.
- A renderer failure after startup needs a daemon restart to restore banners.
- Restarting the daemon invalidates live sender IDs and drops transient notifications.
- Sender `x`/`y` hints are ignored; banner placement is daemon-owned.
- Fullscreen and screen-sharing suppression need Niri's IPC.
