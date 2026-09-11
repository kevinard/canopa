# Freedesktop notification compliance

Source: Desktop Notifications Specification 1.3, latest upstream publication dated 2024-08-18.

Canopa Notify should advertise a freedesktop capability only when the sender-visible D-Bus behavior and Notify-owned presentation behavior are both implemented. Unsupported hints are ignored or decoded as request facts according to the spec's optional hint model.

Completion is still in progress. The following status tables describe implemented behavior, not a claim that every selected presentation requirement has passed. The [execution tracker](../.scratch/freedesktop-completion-review/IMPLEMENTATION.md) records remaining work; the [graphical checklist](notification-center-verification.md) has not yet been executed for the Niribar port.

## D-Bus interface

| Spec surface | Status | Notes |
| --- | --- | --- |
| `org.freedesktop.Notifications.Notify` | Implemented | Maps request values through Notification Server Request decoding into Notification Intake. |
| `CloseNotification` | Implemented | Closes active notifications and emits `NotificationClosed` with reason `3`. |
| `GetCapabilities` | Implemented | Advertises only the capabilities listed below. |
| `GetServerInformation` | Implemented | Reports spec version `1.3`. |
| `NotificationClosed` | Implemented | Emits reasons for expired, dismissed, sender-closed, and undefined terminal states. New no-op and policy-dropped requests return a nonzero ID and then emit reason `4`; a dropped replacement preserves the existing notification lifecycle. |
| `ActionInvoked` | Implemented | Preserves opaque, case-sensitive sender action keys. |
| `ActivationToken` | Implemented | Emits before `ActionInvoked` when a user-originated notification interaction can obtain a Wayland activation token. |

## Requirement-to-evidence matrix

The protocol baseline is the versioned [Desktop Notifications 1.3 protocol](https://specifications.freedesktop.org/notification/1.3/protocol.html); image layout follows [icons and images](https://specifications.freedesktop.org/notification/1.3/icons-and-images.html). Automated executable tests run on a private bus through `bash scripts/test.sh`.

| Requirement | Obligation | Implementation owner | Evidence / remaining verification |
| --- | --- | --- | --- |
| Four D-Bus methods, reply types and server information | Core protocol | D-Bus adapter | `tests/dbus_receipt.rs` exercises Notify/Close; binary tests check exact GetCapabilities sets. Dedicated introspection/GetServerInformation assertions remain open in W01. |
| Nonzero live IDs, replacement and invalidated closures | Core protocol | D-Bus registry + Lifecycle | Adapter-local wire registry and shared nonzero internal type through core interfaces. Executable collision/concurrent-token regressions in `tests/dbus_receipt.rs`; retirement, exhaustion, sparse-range pressure, maximum wire values, failed-storage and disconnect tests in `notification_server/tests.rs`. `tests/dbus_restart.rs` covers service-owner change, stale history and the fresh live namespace; `decoding_an_unresolved_wire_id_cannot_replace_an_internal_thread` and a compile-fail example cover decoder/type isolation. Trustworthy identity remains separate W03 work. |
| Required closure/action signal delivery and ordering | Core protocol / advertised actions | Runtime + sender publisher | Binary `dbus_bulk_clear_publishes_all_130_closures`, `notification_server_emits_activation_token_before_action_invoked`; D-Bus action-before-close test. Transport failure and renderer recovery remain W05. |
| Expiration, resident and transient semantics | Protocol with explicit local timeout policy | Lifecycle + Runtime | `tests/lifecycle.rs`, `tests/presentation.rs`, `tests/control_api.rs`; display-based timing and transient-center exclusion documented below. |
| Revision-safe banner interactions and display callbacks | Local concurrency policy supporting replacement/actions | Banner presenter + Runtime | `queued_banner_action_cannot_invoke_the_same_key_on_replacement_content`, `queued_banner_activation_and_dismissal_cannot_mutate_replaced_or_closed_content`, `banner_token_wait_cannot_commit_an_action_after_replacement`, `delayed_display_acknowledgement_cannot_start_a_cancelled_revisions_timeout`; renderer burst and acknowledgement lifetime tests in `presentation.rs`. Graphical verification remains open. |
| Static raw images and legacy spellings | Advertised static profile | Request decoder + Intake | `tests/notification_receipt.rs`, decoder tests: signed channel validation, omitted final-row padding, and source precedence. ADR 0036 adds bounded banner pixel preparation and ADR 0037 adds indexed icon lookup; wider asset acceptance remains W09. |
| Indexed icon lookup and refresh | Selected Icon Theme 0.13 semantics with local PNG/JPEG and safety limits | Private renderer-side icon resolver | Nine renderer-consumer fixtures cover selected-theme/parent/root precedence, size/scale, refresh, unsafe metadata and exact/excessive budgets; configuration integration tests cover startup/reload and failed-install rollback. Automatic theme/output-scale tracking, XPM/SVG, alternative-resolver fixtures and graphical acceptance remain W09. |
| Actions, markup and body images on real surfaces | Optional capabilities, required when advertised | Banner presenter + Niribar host | Banner presentation tests and Niribar `canopa_provider`/`provider_protocol` tests; host v3 port now renders sections, controls, styled bodies and bounded images. All-transient action reachability and actual graphical evidence remain W10/W11. |
| Hyperlinks and persistence | Optional; not advertised | Intake, presentation, retention | Explicitly unfinished W12/W13; retaining closed history alone does not satisfy persistence. |
| Durable paths, installed configuration and reload | Local product/operational contract | Startup + Configuration + Control | `tests/control_spine.rs`, `installed_startup_loads_xdg_configuration_before_accepting_notifications`, failed-install sound test; socket partial-clear regression verifies committed counts. |
| Atomic storage and safe upgrades | Local reliability contract | Notification History adapter | Twelve SQLite adapter tests, future-schema rejection and partial-clear history tests. Interrupted upgrades, large backfills and additional startup failure fixtures remain W02/W06. |
| Text/action admission and markup work bounds | Local resource policy, not specification limits | Request decoder + Intake | `tests/input_limits.rs` covers atomic rejection, exact UTF-8/aggregate limits and markup degradation; `wire_input_limits_reject_before_normalization_without_consuming_ids_or_replacements` covers D-Bus errors and unchanged wire identity. Banner decoding and icon lookup have their own policies below; provider-stream limits remain W07. |
| Aggregate live count/text/raw-image admission | Local resource policy, not specification limits | Notification Runtime | `tests/active_resources.rs` covers exact boundaries, replacements, committed cleanup, failed writes, concurrent last-slot admission and stale history; `live_resource_exhaustion_returns_limits_exceeded_without_consuming_wire_ids` verifies the D-Bus boundary. |
| Reclaimable timeout bookkeeping | Local resource policy, preserving display-based expiration | Notification Runtime | `tests/timer_resources.rs` covers completed churn, late/duplicate frames, display/close interleavings, live admission bounds, failed writes, partial clear and delayed sender publication. Control status exposes `timeout_records`. |
| Bounded sound admission and playback | Local resource policy; best-effort sound presentation | Sound presenter | Offline `sound::tests` exercise the public presenter and daemon with a real mixer: concurrency, cancellation/reopen, byte/PCM/duration limits, FIFO/invalid-file fallback, churn, backend availability and committed notification lifecycle under saturation. |
| Sound-theme lookup and disabled overrides | Sound Theme 0.2, with fixed local safety limits | Private sound-theme resolver | Public-presenter fixtures cover indexed profile directories, first-index/root precedence, inheritance cycles, locale/name fallback, disabled overrides, edits without restart, unsafe indexes and exact/over-limit budgets. Notify requests stereo, then profile-less sounds; configurable surround profiles are not claimed. |

The static profile excludes `icon-multi`; Wayland placement may ignore `x`/`y`. No native portal-v2 backend is claimed. See the existing [portal interoperability notes](xdg-notification-portal-v2.md) for negotiated bridge limitations.

## Sender ID lifetime and compatibility

[ADR 0029](adr/0029-live-notification-id-boundary.md) defines a daemon-lifetime wire namespace, independent of retained history. Successful live replacements preserve their wire ID. A never-issued nonzero token still creates a new notification for compatibility; an issued but closed token now returns `InvalidArgs`. Clients that reused closed fixed tokens must submit `replaces_id = 0` to start a new notification.

Generated IDs never wrap or reuse committed IDs, including immediately dropped notifications. Exhaustion returns `LimitsExceeded` without a notification mutation. Issued-ID memory uses a contiguous prefix plus at most 4,096 sparse ranges: a caller token requiring another range is rejected, but automatic IDs can still extend the prefix. Receipts that fail before commitment consume no ID. This budget bounds retirement bookkeeping, independently of the live-resource admission limits below.

Client disconnection does not close a live notification or make its ID private to that connection. Claimed app identity remains grouping/policy metadata, not authentication. Daemon restart creates a new namespace; clients must discard old IDs when the service owner changes.

The restart fixture observes [D-Bus service-owner changes](https://dbus.freedesktop.org/doc/dbus-specification.html#bus-messages-name-owner-changed) across two daemon processes. Retained active history becomes stale, transient state disappears, and old IDs are not registered as live in the successor. Numbers may recur across owners. Binding ID-bearing requests to the unique owner associated with their receipt prevents a request from being rerouted to a successor between an owner check and send; a bare old number sent to the new owner cannot carry its former namespace.

Rust library callers now use `InternalNotificationId` for receipts, replacement targets, lifecycle commands and presentation/control targets. `NotificationServerRequest::into_notification_input` decodes content only; the transport supplies its resolved target under the receipt guard. Numeric control JSON and SQLite encodings are unchanged. Zero is rejected as a control target, while legacy stored replacement metadata of zero is read as no replacement. `control_ids_keep_signed_numeric_json_and_reject_zero` and `legacy_zero_replacement_metadata_remains_readable_after_restart` cover that compatibility boundary.

## Request limits and markup degradation

[ADR 0031](adr/0031-bounded-notification-intake.md) defines fixed admission limits, measured in UTF-8 bytes rather than characters. These are Canopa resource policies, not requirements of the [Notifications 1.3 protocol](https://specifications.freedesktop.org/notification/1.3/protocol.html).

| Input | Limit |
| --- | --- |
| Raw body | 64 KiB |
| Summary | 8 KiB |
| Each other request text field, including action keys/labels/icons and hint keys/string values | 4 KiB |
| Aggregate text | 128 KiB, checked on both raw wire fields and normalized library input |
| Actions | 64 library actions; 128 wire strings before malformed-pair/duplicate normalization |
| Hints | 64 wire entries |

Body-markup attributes count within the raw-body budget rather than as separate 4 KiB fields. Normalized aggregate accounting includes derived action-icon references as well as their action keys.

An over-limit request returns `org.freedesktop.DBus.Error.LimitsExceeded` without a receipt, ID consumption, policy decision, stored thread change or sender event. A rejected replacement leaves the original notification active and unchanged. Error descriptions contain only fixed categories, never sender text. This also applies to DND, transient and otherwise no-op requests. The Rust request decoder now returns `Result<NotificationInput, NotificationLimitExceeded>`; direct daemon intake enforces the same normalized limits. No D-Bus signature, numeric encoding or database schema changes.

Within the byte budget, markup may use at most 64 nested emphasis tags and 256 rich segments during parsing, before whitespace normalization. Exceeding either limit switches the whole body to plain text, preserving decoded text and image alternative text but dropping styling and image references. Empty fallback still follows the ordinary no-op policy; no placeholder text is invented. Malformed tag/entity scans move forward, and leading whitespace is trimmed without repeatedly shifting the remaining body. New receipts retain the complete admitted raw body. Legacy backfill interprets only a UTF-8-safe 64 KiB prefix of oversized raw bodies, without rewriting the original raw or plain body fields.

These checks run after D-Bus deserialization. Nested unknown hint values, provider streams and sound workers are not bounded by this text policy; live-resource admission and existing per-image limits are separate. No additional capability is advertised.

## Live-resource admission

[ADR 0032](adr/0032-live-notification-resource-admission.md) defines fixed daemon-wide safety defaults:

| Live resource | Limit |
| --- | --- |
| Retained and transient notification threads, including silenced/DND threads | 1,024 |
| Accepted text, including duplicated identity/body representations, visuals, body-image references/alternatives and actions | 16 MiB |
| Retained raw-image payloads, including padding/trailing bytes | 64 MiB |

Admission follows the pure delivery-policy decision under the mutation coordinator, before lifecycle writes or receipt registration. Exhaustion returns `LimitsExceeded` without consuming an ID, emitting sender events, evicting existing notifications or altering a rejected replacement. A live replacement gets credit for its previous footprint. Policy/no-op drops need no live capacity; dropped replacements preserve the original footprint. DND/silenced threads consume count/text capacity, but their images are not retained.

Only committed lifecycle effects change accounting. Failed storage writes leave it untouched; committed closure, dismissal, expiry and staleness release capacity. Restart-loaded history is stale before the new runtime begins and consumes no live capacity. These are logical resource limits, not a total RSS guarantee: client projections, transport deserialization and provider streams remain separate work; banner decoding and sound have their own policies below. Aggregate image exhaustion rejects the request; it does not silently remove a valid image. No capability, D-Bus signature or schema changes.

## Banner image resource policy

[ADR 0036](adr/0036-bounded-banner-image-preparation.md) prepares stable RGBA pixels behind the Iced presenter/renderer-consumer seam. File reads and decoding run on the captured daemon executor outside notification mutation and widget drawing. Widgets never reopen sender paths. Public notification/control projections still carry visual references; no pixel payloads are added to history or the panel protocol.

| Resource | Fixed limit |
| --- | --- |
| File-backed decoder jobs across renderer lifetimes | 1, no decode backlog |
| File attempts per presentation update | 32 |
| Encoded bytes per file / per update | 16 MiB / 32 MiB, plus one overflow-detection probe byte |
| Image width or height | 4,096 |
| Pixels per image | 4,194,304 |
| Published RGBA bytes across all banners and visual slots per update | 32 MiB |

Files are opened nonblocking, validated as regular files through the opened descriptor, and read with a hard byte bound even if they grow. Only PNG/JPEG content at supported local raster paths is decoded. The decoder receives dimension and best-effort allocation limits; Notify separately checks pixel totals before decoding/conversion and applies EXIF rotations/mirroring from the same decoder metadata. Corrupt, missing, unsupported, oversized or over-budget body images show their supplied alternative text, preserving segment order/style; images without alternatives are omitted. Header/action visuals are omitted on failure. Previously prepared pixels remain stable after source edits/deletion; later updates reread files without a path-only decode cache. Image, sound and theme owners share the descriptor-safe bounded-file reader while retaining their own budget and fallback policies.

Supersession cancels the wait for old preparation; discarded preparation is never acknowledged as displayed, while already-delivered frames retain their existing acknowledgement semantics. Admission remains owned by the actual decoder job even after consumer cancellation. While it is occupied, new frames preserve text, alternatives and valid raw pixels without file-backed visuals; a later presentation update may retry. This prevents blocked decoder work from holding text or closure updates. These are local logical budgets, not total process/GPU memory or filesystem-deadline guarantees. Icon discovery now runs in this same worker under ADR 0037 below. Provider streams, alternative-resolver comparison, automatic desktop-theme/output-scale tracking and graphical/host acceptance remain open. No advertised capability, D-Bus signature or schema change.

## Icon lookup resource policy

[ADR 0037](adr/0037-bounded-indexed-icon-discovery.md) places discovery in the existing banner asset worker. Banner projections carry unresolved paths and icon names without filesystem access. The resolver uses the first theme index across ordered roots, only declared directories, exact nominal size/scale before nearest physical size, selected-theme inheritance, hicolor and unthemed fallback. Ties preserve declaration, root and supported-extension order. This follows the [Icon Theme 0.13 lookup and directory-key definitions](https://specifications.freedesktop.org/icon-theme/latest/#icon-lookup), not the inconsistent `Scaled`/threshold pseudocode spellings.

| Resource | Local limit |
| --- | --- |
| Captured icon roots | 32, with one retained overflow sentinel |
| Distinct theme metadata records per frame | 32 |
| Bytes per index / total per frame | 256 KiB / 2 MiB, plus at most one overflow probe byte |
| Declared directories / parents per index | 1,024 / 32 |
| Traversal and candidate checks per frame | 65,536 |

Absolute XDG data-home icons, legacy user `.icons`, data-directory icons and finally pixmap roots are searched in that order; empty XDG values use defaults, relative values are ignored and duplicates are removed. Excess roots disable icon lookup. Metadata is reused only within one frame and reread on the next update; existing pixels remain stable. Unsafe indexes or exhausted budgets stop named-icon lookup without dropping notifications, bypassing overrides or preventing separately budgeted local body images. All index reads validate the opened regular-file descriptor and use bounded nonblocking reads. These are local work limits, not hard filesystem or total-memory guarantees.

Installed `icons theme="hicolor" scale=1` settings choose the theme and integer source scale (1–8), not UI geometry. Reloads affect subsequent banner updates; invalid configuration or failed sound installation leaves the previous configuration active and the new candidate uncommitted. Automatic desktop-theme/output-scale tracking is not implemented. PNG is the supported standard icon format; JPEG remains a local extension, while SVG and legacy XPM remain unsupported. Nine daemon/renderer-consumer fixtures and the extended worker-reopen test cover pixels, precedence, refresh, cycles, special files and budgets. Configuration and executable startup fixtures cover selection plumbing; actual graphical and comparative alternate-resolver evidence remain open.

## Sound resource policy

[ADR 0034](adr/0034-bounded-notification-sound-playback.md) keeps sound limits behind the existing presenter interface. At most four jobs may resolve, decode, wait in the mixer or play, with no backlog. A reservation is released only after both its worker and audio source finish or are dropped. Further sound requests return a content-free capacity error to the runtime, which logs it and preserves the successful notification receipt, history, actions and closure behavior. Saturation does not change the `sound` capability.

Requested and fallback files must be regular files of at most 8 MiB, including trailing bytes. Symlinks to regular files remain supported; opened descriptors are validated with nonblocking open so FIFOs cannot wait for writers. Reads are bounded even if a file grows after the metadata check. Missing, invalid, oversized or unsupported requested audio uses the existing default fallback; if that also fails, playback is skipped. Audio supports at most eight channels and 192 kHz with a stable stream format. Worker-side decoding truncates to ten seconds or 4 MiB of PCM, whichever comes first, at complete-frame boundaries. The mixer receives PCM only, without file reads or decoding in its audio callback.

Stop, disable and changed sound settings cancel the old playback generation. Identical settings preserve playback; stop makes the backend unavailable until settings are applied again. Reopening never resets outstanding reservations. Cancellation is cooperative during lookup, decoding and source consumption; already buffered device samples cannot be recalled. These are local safety limits, not specification limits, hard filesystem deadlines or total-memory guarantees (including codec-internal allocations and buffer conversion overhead). Icon discovery, provider streams and broader shutdown/recovery remain unfinished. No new capability, D-Bus method, configuration key or schema change is introduced.

[ADR 0035](adr/0035-bounded-sound-theme-resolution.md) defines sound-theme lookup. The first index across ordered XDG roots selects declared directories and parents. Notify searches stereo then profile-less directories, selected theme then recursive parents before freedesktop and unthemed fallback. Generic event names stay within the higher-priority theme; locale lookup supports modifier/territory/language fallback, literal and codeset-free spellings, `C`, and unlocalized files. Locale precedence is captured from `LC_ALL`, `LC_MESSAGES`, then `LANG` at startup. These choices follow the [Sound Theme lookup rules](https://specifications.freedesktop.org/sound-theme/0.2/sound_lookup.html), preferring their prose where the published pseudocode conflicts on parent/default ordering.

An encountered `.disabled` marker stops all lookup and application-default fallback, per the [directory-layout specification](https://specifications.freedesktop.org/sound-theme/0.2/directory_layout.html). Unsafe indexes or exhausted budgets also skip the sound; they are not treated as missing files. Lookup allows at most 32 roots, 32 visited themes, 64 KiB per index, 64 declared directories, 32 parents per index and 4,096 traversal/candidate checks. Lexical path traversal is rejected; opened indexes use bounded nonblocking regular-file reads. No arbitrary directory scan or permanent lookup cache remains. Unindexed installations retain only fixed stereo/mono/root probes; FLAC/MP3 remain extensions after standard formats. These local extensions and limits are not freedesktop requirements. W09's broader alternative-resolver/asset evaluation remains open.

## Expiration semantics

[ADR 0028](adr/0028-notification-timeouts-begin-with-banner-display.md) defines the implemented behavior.

- Positive and server-default expiration values begin counting only when the current **Notification Thread** revision first becomes visible in a **Notification Banner**. A zero value never expires.
- Overflow notifications, **Silenced Notifications**, requests accepted under **Do Not Disturb**, and **Headless Mode** requests that never display a banner remain active without automatic expiration. This deliberately permits a finite **Sender Notification Lifecycle** to remain open until user or sender action.
- Once a **Notification Banner** has displayed, its deadline continues if a critical notification displaces it. Notify does not later promote an overflow notification merely because a banner slot opens.
- A replacement that revises a **Notification Thread** cancels the prior revision's timeout and begins a fresh timeout only if the replacement revision becomes visible. A dropped replacement preserves the existing thread and timeout.
- When a displayed notification reaches its deadline, Notify invalidates the active sender ID and emits `NotificationClosed` with reason `1` while retaining an **Expired Notification** in **Notification History**.

[ADR 0033](adr/0033-timeout-registration-without-tombstones.md) replaces permanent cancellation tombstones with one registration per pending or scheduled live revision. Only ordered committed lifecycle effects create registrations. Display callbacks can start a matching pending registration once; they cannot recreate one after cancellation or completion. Scheduling and cancellation share one lock. This preserves stale-revision safety while bounding retained timer records by the 1,024 live-thread admission limit. Completed churn returns the record count to zero, including DND, transient and never-expiring notifications.

`canopa-notify ctl status --json` exposes the additive `timeout_records` diagnostic field; plain-text availability output is unchanged. It counts registrations, not notifications in history, all process tasks or process memory. A firing timer releases its registration before expiration/effect publication, so delayed publication does not retain timer bookkeeping or abort its own publisher. Failed receipt/close/replacement writes preserve existing timers and partial clear cancels only committed transitions. Expiration write failures retain the existing no-retry behavior; broader failure recovery and shutdown remain separate work.

## Specification-defined capabilities

| Capability | Status | Notes |
| --- | --- | --- |
| `actions` | Advertised | Named actions and `default` activation are supported. |
| `action-icons` | Advertised | `action-icons=true` maps action keys to static icon-name references; banners render resolved local raster icons when available. |
| `body` | Advertised | Body text is shown with a plain-text fallback. |
| `body-hyperlinks` | Not implemented | Link text is preserved, but targets are not retained or actionable. |
| `body-images` | Not advertised | Body-embedded image markup may be decoded and carried before all Notify-owned body-display surfaces render it. |
| `body-markup` | Advertised | Bold, italic, underline, tolerated line breaks, and safe text fallback are supported. |
| `icon-multi` | Not implemented | Animated or multi-frame icons are not supported. |
| `icon-static` | Advertised | Raw image data, local static raster files, and static raster icon-theme names are supported. |
| `persistence` | Not advertised | Notify persists local history, but does not yet promise freedesktop persistence semantics to senders. |
| `sound` | Runtime advertised | Advertised when graphical notification sound support is enabled and the audio backend initialized successfully. Not advertised in headless mode or when sound is disabled. |

This table is the complete capability vocabulary defined by Desktop Notifications Specification 1.3. Runtime `GetCapabilities` results are tested as exact sets so compatibility or Canopa extensions cannot silently become specification claims.

## Specification-defined hints

| Hint | Status | Notes |
| --- | --- | --- |
| `action-icons` | Implemented | When true, each action key is also treated as a static icon name for presentation. |
| `category` | Implemented | Valid standard and vendor-prefixed values are normalized into Notification Category metadata and may drive category-specific policy; malformed values are ignored. |
| `desktop-entry` | Implemented | Preferred source for App Identity. |
| `image-data` | Implemented | Raw static image payloads are validated and rendered at runtime without persisting payload bytes. |
| `image_data`, `icon_data` | Compatibility aliases | Deprecated spellings remain decoded for older senders. |
| `image-path` | Implemented | Local static raster paths are rendered when resolvable. |
| `image_path` | Compatibility alias | The deprecated spelling remains decoded for older senders. |
| `resident` | Implemented | Action invocation keeps resident notifications active. |
| `sound-file`, `sound-name` | Implemented | Shown notifications play requested local files or themed sound names, with `sound-file` taking precedence. Missing/undecodable sounds may use the default; explicit theme disable markers and unsafe/exhausted lookup stop without fallback. |
| `suppress-sound` | Implemented | Suppresses default, named, and file-backed notification sounds for the request. |
| `transient` | Implemented | New transient notifications retain runtime presentation, actions, timeouts, sound, and sender lifecycle while remaining absent from persisted Notification History and the first Notification Center. A thread's initial request fixes retention across replacements. |
| `urgency` | Implemented | Maps `0`, `1`, and `2` into low, normal, and critical behavior. |
| `x`, `y` | Decoded, ignored | Wayland banner placement is daemon-owned; sender coordinates are not honored. |

## Compatibility extensions

These names are widely implemented interoperability surfaces, but they are not capabilities or hints defined by Desktop Notifications Specification 1.3.

| Extension | Kind | Status | Compatibility reason |
| --- | --- | --- | --- |
| `activation-token` | Capability | Runtime advertised | Existing Wayland clients probe this unnamespaced name. It is advertised only when the daemon can request tokens. The standard `ActivationToken` signal remains independently usable and is not made standard by this capability name. |
| `inline-reply` | Capability | Not advertised | Portal reply purposes require a keyboard-interactive response UI and are not a freedesktop 1.3 capability. |
| `value` | Hint | Implemented | The de-facto progress hint is retained because common clients send it; Notify stores and renders it as Notification Progress. |

## Canopa extensions

Canopa Notify currently advertises no Canopa-specific capabilities and decodes no Canopa-specific hints. New Canopa-only protocol names must use the `x-canopa-` namespace rather than adding another unnamespaced extension.

## Client portability guidance

- Portable freedesktop 1.3 clients should depend only on the D-Bus interface, capabilities, and hints in the specification-defined sections above.
- Clients may subscribe to the standard `ActivationToken` signal without treating the `activation-token` compatibility capability as part of freedesktop 1.3.
- Clients using `activation-token` or `value` must tolerate servers that omit or ignore them.
- `inline-reply` is portal-specific behavior, not a portable `org.freedesktop.Notifications` capability.
- Canopa-specific behavior, when introduced, will be identifiable by its `x-canopa-` name.

## Compatibility rules

- `GetCapabilities` tests must assert the exact specification-defined and extension sets for headless, graphical, sound-enabled, and activation-token-capable runtimes.
- Adding a new advertised capability requires a D-Bus-level test and at least one presentation or lifecycle test for the behavior.
- Activation-token behavior should be tested through a replaceable daemon activation-token provider, with compositor-specific integration kept outside lifecycle tests.
- Banner interactions first try a Wayland `xdg_activation_v1` token using the click's input serial and banner surface; if unavailable or empty, sender-visible publication falls back to the daemon activation-token provider.
- `activation-token` capability advertisement follows activation-token provider availability; normal `--headless` startup does not install a provider.
- `activation-token` capability advertisement is stable for the daemon lifetime; individual token requests remain best-effort.
- `sound` capability advertisement follows runtime sound support; normal `--headless` startup does not enable sound, and valid configuration reloads may change sound capability for future requests.
- Sound playback is a best-effort presentation side effect. Playback failure must not change the sender-visible `Notify` result or the persisted delivery outcome.
- The initial activation-token generation budget is a fixed 100 ms daemon-side timeout.
- Empty activation-token provider results are treated as unavailable tokens; `ActionInvoked` still proceeds without an `ActivationToken` signal.
- Decoding a hint without acting on it must be documented here as `Decoded, neutral` or `Decoded, ignored`.
