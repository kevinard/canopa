# Freedesktop notification compliance

Protocol baseline: [Desktop Notifications Specification 1.3](https://specifications.freedesktop.org/notification/1.3/).
Image handling follows its [icons and images](https://specifications.freedesktop.org/notification/1.3/icons-and-images.html)
section.

Canopa Notify advertises a capability only when both the sender-visible D-Bus
behaviour and Notify's own presentation of it are implemented. Unsupported hints
are ignored or decoded as request facts, as the specification's optional hint model
allows. Runtime design is described in [architecture/notify.md](../architecture/notify.md).

The Notification portal (`org.freedesktop.impl.portal.Notification` version 2) is
served by the same daemon; see [Notification portal adapter](../architecture/notify.md#notification-portal-adapter).

## D-Bus interface

`org.freedesktop.Notifications` at `/org/freedesktop/Notifications`, served only in
service mode.

| Spec surface | Status | Notes |
| --- | --- | --- |
| `Notify` | Implemented | Request values are decoded and admitted through Notification Intake. |
| `CloseNotification` | Implemented | Closes an active notification and emits `NotificationClosed` with reason `3`. A missing or terminal ID returns `org.freedesktop.DBus.Error.Failed`. |
| `GetCapabilities` | Implemented | Returns exactly the capabilities listed below for the current runtime. |
| `GetServerInformation` | Implemented | `Canopa Notify`, `Canopa`, the package version and spec version `1.3`. |
| `NotificationClosed` | Implemented | Reasons for expired, dismissed, sender-closed and undefined terminal states. A no-op or policy-dropped new request returns a nonzero ID and then emits reason `4`; a dropped replacement leaves the existing notification as it was. |
| `ActionInvoked` | Implemented | Action keys are opaque and case-sensitive. |
| `ActivationToken` | Implemented | Emitted before `ActionInvoked` when a user interaction obtains a Wayland activation token. |

## Requirements and implementation

Automated tests run on a private bus through `bash scripts/test-notify.sh`; test
modules are in `crates/canopa-notify/tests/` (modules of the `integration` target)
unless noted.

| Requirement | Obligation | Implementation owner | Tests |
| --- | --- | --- | --- |
| Four methods, reply types and server information | Core protocol | D-Bus adapter | `dbus_contract` (introspection, reply envelopes, `dbus-send` decoding, malformed calls) |
| Nonzero live IDs, replacement and closure | Core protocol | D-Bus registry and lifecycle | `dbus_receipt`, `dbus_restart`, `src/notification_server/tests.rs` |
| Closure and action signal delivery and order | Core protocol, advertised actions | Runtime and sender publisher | `dbus_contract`, `service_lifecycle`, `src/notification_server/tests.rs` |
| Expiration, resident and transient semantics | Protocol with local timeout policy | Lifecycle and runtime | `lifecycle`, `presentation`, `control_api`, `transient_center` |
| Revision-safe banner and Center interactions | Local policy supporting replacement and actions | Banner presenter, Center provider, runtime | `presentation`, `transient_center`, `control_api` |
| Static raw images and legacy hint spellings | Advertised static profile | Request decoder and intake | `notification_receipt`, decoder unit tests |
| Icon lookup | Icon Theme lookup with local limits | Renderer-side icon resolver | Renderer and configuration tests |
| Actions, markup and body images on real surfaces | Optional capabilities, required when advertised | Banner renderer and Center provider | Renderer widget tests; `scripts/test-desktop.py` |
| Hyperlinks | Optional, advertised at runtime | Intake, runtime, OpenURI opener, both surfaces | Receipt, control and opener tests |
| Persistence | Optional, advertised | Lifecycle, SQLite, Center | `history`, `control_api`, `service_lifecycle` |
| Durable paths, configuration and reload | Local operational contract | Startup, configuration, control | `control_spine`, `configuration_intake` |
| Atomic storage and upgrades | Local reliability contract | Startup and history adapter | `legacy_import`, `storage_failures`, SQLite adapter tests |
| Input, live-resource, reply and stream bounds | Local resource policy, not spec limits | Intake, runtime, control transport | `input_limits`, `active_resources`, `control_resources`, `provider_resources`, `timer_resources` |
| Sound admission and theme lookup | Sound Theme 0.2 with local limits | Sound presenter | `sound::playback::tests` (unit) |
| Renderer readiness and shutdown | Local reliability policy | Renderer, startup, control | `renderer_startup`, `shutdown`, `service_lifecycle` |

The static profile excludes `icon-multi`. Wayland placement ignores `x` and `y`.

## Capabilities

| Capability | Status | Notes |
| --- | --- | --- |
| `actions` | Advertised | Named actions and `default` activation. All named actions are shown in banners and the Center. |
| `action-icons` | Advertised | With `action-icons=true`, action keys are also icon names; banners render resolved local icons. |
| `body` | Advertised | Body text with a plain-text fallback. |
| `body-hyperlinks` | Advertised at runtime | Validated HTTP(S) links are distinct, accessible controls in banners and the Center. Advertised while the OpenURI portal opener is available; absent in headless mode. |
| `body-images` | Advertised | Ordered local PNG/JPEG body images in banners and the Center, with alternative-text fallback and image accessibility semantics. |
| `body-markup` | Advertised | Bold, italic, underline, line breaks and safe text fallback. |
| `icon-multi` | Not implemented | Animated or multi-frame icons are not supported. |
| `icon-static` | Advertised | Raw image data, local files and icon-theme names. |
| `persistence` | Advertised | Retained content stays available until acknowledged, removed or recalled by the sender. Expiry and restart end sender actions without discarding pending attention. |
| `sound` | Advertised at runtime | Advertised when the `audio` feature is built, sound is enabled and an output is available. Not advertised in headless mode. |

This is the complete capability vocabulary of the 1.3 specification. Runtime
`GetCapabilities` results are tested as exact sets.

## Hints

| Hint | Status | Notes |
| --- | --- | --- |
| `action-icons` | Implemented | When true, each action key is also a static icon name. |
| `category` | Implemented | Standard and vendor-prefixed values become Notification Category metadata and can drive category rules and break-through; malformed values are ignored. |
| `desktop-entry` | Implemented | Preferred source of App Identity. |
| `image-data` | Implemented | Raw static images are validated and rendered; payload bytes are not persisted. |
| `image_data`, `icon_data` | Compatibility aliases | Deprecated spellings are decoded. |
| `image-path` | Implemented | Local static images are rendered when resolvable; files are recognized by content. |
| `image_path` | Compatibility alias | Deprecated spelling is decoded. |
| `resident` | Implemented | Invoking an action keeps a resident notification active. |
| `sound-file`, `sound-name` | Implemented | Shown notifications play the file or themed sound; `sound-file` wins. Missing or undecodable sounds may use the default; a theme's disable marker and unsafe or exhausted lookups play nothing. |
| `suppress-sound` | Implemented | Suppresses default, named and file sounds for the request. |
| `transient` | Implemented | Presentation, actions, timeouts, sound and sender lifecycle without entering persisted history. Live transient items appear in the Center until they end or the daemon restarts. A thread's first request fixes its retention. |
| `urgency` | Implemented | `0`, `1`, `2` map to low, normal and critical. Any D-Bus integer width is accepted (`notify-send -h int:urgency:2`); other values mean normal. |
| `x`, `y` | Decoded, ignored | Banner placement is daemon-owned. |

The boolean hints `transient`, `resident`, `suppress-sound` and `action-icons` also
accept any D-Bus integer (nonzero is true), because `notify-send -h int:transient:1`
is the common spelling.

## Compatibility extensions

These are widely implemented but not part of the 1.3 specification.

| Extension | Kind | Status | Reason |
| --- | --- | --- | --- |
| `activation-token` | Capability | Advertised at runtime | Wayland clients probe this unnamespaced name. Advertised only while the daemon can request tokens. The standard `ActivationToken` signal works independently of it. |
| `inline-reply` | Capability | Not advertised | Inline reply is offered through the Notification portal's `im.reply-with-text` button purpose, not through this interface. |
| `value` | Hint | Implemented | De-facto progress hint; any integer width is accepted and shown as Notification Progress. |

## Canopa extensions

Canopa Notify advertises no Canopa-specific capabilities and decodes no
Canopa-specific hints on `org.freedesktop.Notifications`. New Canopa-only names use
the `x-canopa-` prefix.

## Client portability guidance

- Portable clients should depend only on the interface, capabilities and hints in
  the specification sections above.
- Clients may subscribe to `ActivationToken` without treating `activation-token` as
  part of the specification.
- Clients using `activation-token` or `value` must tolerate servers that omit or
  ignore them.
- Inline reply is portal behaviour, not an `org.freedesktop.Notifications`
  capability.

## Rules for changes

- `GetCapabilities` tests assert exact sets for headless, graphical, sound-enabled
  and activation-token-capable runtimes.
- A new advertised capability needs a D-Bus-level test and at least one
  presentation or lifecycle test.
- Activation-token behaviour is tested through a replaceable activation-token
  provider; compositor-specific code stays out of lifecycle tests.
- Banner interactions first request an `xdg_activation_v1` token with the click's
  input serial and banner surface (100 ms budget). An empty or failed result means
  no token; `ActionInvoked` still proceeds without `ActivationToken`.
- `activation-token` advertisement is fixed for the daemon lifetime; `sound`
  advertisement follows runtime sound support and can change on reload for future
  requests.
- Sound playback is best effort: a failure never changes the `Notify` result or the
  stored delivery outcome.
- A hint that is decoded without effect is documented here as `Decoded, ignored`.

## Sender IDs

IDs form a daemon-lifetime namespace independent of retained history
([Notify ADR 0029](adr/0029-live-notification-id-boundary.md)).

- A successful live replacement keeps its ID. A nonzero `replaces_id` that is not
  live (never issued or already closed) creates a new notification under that ID, as
  `notify-send -r` scripts expect; it never revives closed history.
- Generated IDs never wrap or reuse an issued ID, including immediately dropped
  notifications. Exhaustion returns `LimitsExceeded`. Issued-ID bookkeeping holds a
  contiguous prefix plus at most 4,096 sparse ranges; a caller token needing another
  range is rejected.
- A receipt that fails before commitment consumes no ID.
- Client disconnection does not close a notification.
- A daemon restart starts a new namespace: retained active history becomes stale,
  transient notifications disappear and old IDs are not live. Clients must discard
  old IDs when the service owner changes.

## Request limits and markup

Fixed admission limits, in UTF-8 bytes ([Notify ADR 0031](adr/0031-bounded-notification-intake.md)):

| Input | Limit |
| --- | --- |
| Raw body | 64 KiB |
| Summary | 8 KiB |
| Each other text field (action keys and labels, icons, hint keys and string values) | 4 KiB |
| Aggregate text | 128 KiB |
| Actions | 64 (128 wire strings before normalization) |
| Hints | 64 entries |

An over-limit request returns `org.freedesktop.DBus.Error.LimitsExceeded` with a
fixed message and changes nothing: no ID, policy decision, stored change or
signal. A rejected replacement leaves the original active and unchanged.

Markup may nest at most 64 emphasis tags and produce at most 256 rich segments.
Beyond that, the whole body becomes plain text, keeping decoded text and image
alternative text.

## Live resources and retention

| Resource | Limit |
| --- | --- |
| Live notification threads (retained and transient, including silenced) | 1,024; when full, the oldest non-critical live thread expires |
| Accepted live text | 16 MiB |
| Retained raw-image payloads | 64 MiB |
| Pending retained notifications | 4,096 and 64 MiB of text; when full, the oldest inactive non-critical pending items are archived as read |

Only an all-critical live set, or the text or image budget, rejects a request
([Notify ADR 0032](adr/0032-live-notification-resource-admission.md),
[ADR 0054](adr/0054-bounded-retention-of-undisplayed-and-pending-notifications.md)).
Restart-loaded history is stale and uses no live capacity. Age and count pruning of
inactive history never removes pending content.

## Expiration

[Notify ADR 0028](adr/0028-notification-timeouts-begin-with-banner-display.md):

- Positive and server-default timeouts start when the thread's current revision is
  first visible in a banner. A timeout of zero never expires. The server default is
  5 s for low and normal urgency; critical notifications do not expire.
- Silenced and DND notifications start their timeout at acceptance, and overflow
  notifications when they overflow. Headless mode never displays banners, so its
  notifications do not expire on their own.
- A displayed banner's deadline continues if a critical notification displaces it.
- A replacement cancels the previous revision's timeout and starts a new one when
  the replacement is displayed, silenced or overflows. A dropped replacement keeps
  the existing timeout.
- At the deadline Notify retires the sender ID, emits `NotificationClosed` with
  reason `1` and keeps an Expired Notification in history.

Timer registrations exist only for pending or scheduled live revisions
([ADR 0033](adr/0033-timeout-registration-without-tombstones.md));
`ctl status --json` reports them as `timeout_records`.

## Transient notifications in the Center

[Notify ADR 0041](adr/0041-runtime-only-transient-center-actions.md): live transient
threads appear with retained items in one date-ordered list, with all named actions
and default activation, without entering durable history. They disappear on
dismissal, nonresident action, sender closure, expiry after display or restart;
resident actions keep them. Opening the Center does not count as display. Deleting
history leaves live items alone; clearing active notifications includes them.

## Banner layout

- Cards are at most 360 logical pixels wide, in the configured corner (default
  `bottom-start`). The body preview is capped at 120 logical pixels; oversized cards
  scroll inside a bounded viewport ([ADR 0046](adr/0046-scrollable-oversized-compact-cards.md)).
- The surface is sized by the compositor; space outside the visible stack is
  click-through ([ADR 0045](adr/0045-compositor-sized-banner-surface.md)).
- Each card starts its first-display timeout when its compact viewport fits on
  screen; scrolling does not pause or restart it
  ([ADR 0044](adr/0044-per-card-banner-display-acknowledgement.md)).
- A visible stack stays on its output; if its surface is lost, the stack is retried
  on another output without resetting deadlines
  ([ADR 0047](adr/0047-compositor-selected-banner-stack-lifetimes.md)).

## Images, icons and sound

| Resource | Limit |
| --- | --- |
| Concurrent file-backed image jobs | 1, no backlog |
| File attempts per banner update | 32 |
| Encoded bytes per file / per update | 16 MiB / 32 MiB |
| Image width or height | 4,096 pixels |
| Pixels per image | 4,194,304 |
| Concurrent sound jobs | 4, no backlog |
| Sound file size | 8 MiB |
| Decoded sound | 10 s or 4 MiB of PCM |

- Body images are local PNG/JPEG files recognized by content; failed images show
  their alternative text ([ADR 0036](adr/0036-bounded-banner-image-preparation.md)).
- Icon names resolve through the icon theme (following the desktop theme unless
  `icons theme="..."` is set, then hicolor and unthemed fallback). Sender icon names
  also fall back to shorter dash-separated forms, keeping or dropping a
  `-symbolic` suffix. Applications without a
  sender icon use their desktop entry's `Icon=`. PNG and SVG icons render; XPM does
  not ([ADR 0037](adr/0037-bounded-indexed-icon-discovery.md)).
- Sound names resolve through the freedesktop sound theme with locale fallback;
  stereo is preferred, then profile-less directories. A `.disabled` marker stops
  lookup ([ADR 0034](adr/0034-bounded-notification-sound-playback.md),
  [ADR 0035](adr/0035-bounded-sound-theme-resolution.md)).

## Retained attention, links and accessibility

- Pending retained content is independent of sender lifecycle
  ([ADR 0049](adr/0049-retained-attention-and-sender-lifecycle.md)). DND and silence
  keep pending attention; drops create no history. Following a body link does not
  acknowledge, close or invoke a sender action.
- Body links accept only bounded absolute HTTP(S) targets without credentials,
  control or bidirectional characters, whitespace, backslashes or malformed percent
  escapes. Anything else stays readable text. Links open through the OpenURI portal
  with bounded requests and no shell fallback; diagnostics omit targets
  ([ADR 0050](adr/0050-revision-scoped-notification-links.md)).
- Banners expose notification, body, link and button semantics through AT-SPI. The
  first displayed revision is announced politely once; banners never request
  keyboard focus.
- The Center exposes ordered labels and images, link destinations, named buttons and
  radio selection states. Its keyboard surface supports Tab and arrow keys, Enter,
  Page Up, Page Down, Home, End and Escape. Retired controls cannot invoke
  replacement actions.

## Known limitations

- `icon-multi` (animated icons) is not supported.
- XPM icons are not rendered.
- Sender `x` and `y` hints are ignored.
- Icon scale is configured (`icons scale=`), not derived from the output scale.
- Surround sound-theme profiles are not used.
