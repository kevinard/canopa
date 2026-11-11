# Freedesktop notification compliance

Protocol baseline: [Desktop Notifications Specification 1.3](https://specifications.freedesktop.org/notification/1.3/), published 2024-08-18.

Canopa Notify should advertise a freedesktop capability only when the sender-visible D-Bus behavior and Notify-owned presentation behavior are both implemented. Unsupported hints are ignored or decoded as request facts according to the spec's optional hint model.

The selected static profile includes all named actions, ordered body images, safe HTTP(S) links and retained attention. Native AT-SPI and real Orca evidence cover both banners and the compatible Canopa-v4 Niribar host. See [desktop acceptance](notification-desktop-acceptance.md) and the [execution tracker](../../.scratch/notify/freedesktop-completion-review/IMPLEMENTATION.md) for the final release checks. Native Notification portal v2, animated icons and authenticated application permissions are separate scope.

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

The protocol baseline is the versioned [Desktop Notifications 1.3 protocol](https://specifications.freedesktop.org/notification/1.3/protocol.html); image layout follows [icons and images](https://specifications.freedesktop.org/notification/1.3/icons-and-images.html). Automated executable tests run on a private bus through `bash scripts/test-notify.sh`.

| Requirement | Obligation | Implementation owner | Evidence / explicit limits |
| --- | --- | --- | --- |
| Four D-Bus methods, reply types and server information | Core protocol, §9.1 | D-Bus adapter | `tests/dbus_contract.rs`: exact method/signal introspection, all method reply types, four server-information strings independently decoded by `dbus-send`, exact headless capabilities, invalid-closure errors and malformed calls. Binary tests retain exact capability sets for optional runtime services. See the named contract cases below. |
| Nonzero live IDs, replacement and invalidated closures | Core protocol | D-Bus registry + Lifecycle | Adapter-local wire registry and shared nonzero internal type through core interfaces. Executable collision/concurrent-token regressions in `tests/dbus_receipt.rs`; retirement, exhaustion, sparse-range pressure, maximum wire values, failed-storage and disconnect tests in `notification_server/tests.rs`. `tests/dbus_restart.rs` covers service-owner change, stale history and the fresh live namespace; `decoding_an_unresolved_wire_id_cannot_replace_an_internal_thread` and a compile-fail example cover decoder/type isolation. Claimed application identities remain convenience policy under ADR 0029; authenticated app principals are separate scope. |
| Required closure/action signal delivery and ordering | Core protocol / advertised actions, §9.2 | Runtime + sender publisher | Binary `dbus_bulk_clear_publishes_all_130_closures`; `notification_server_emits_activation_token_before_action_invoked` checks token → action → closure on one ordered signal stream. `tests/dbus_contract.rs` checks actual action/closure envelopes and opaque keys through the executable; displayed expiry has binary/presentation evidence. `lost_bus_reports_publication_failure_without_undoing_closure` and `tests/service_lifecycle.rs` cover transport loss, signal drain and restart. |
| Expiration, resident and transient semantics | Protocol with explicit local timeout policy | Lifecycle + Runtime | `tests/lifecycle.rs`, `tests/presentation.rs`, `tests/control_api.rs`; display-based timing and runtime-only transient center content documented below. |
| Revision-safe banner interactions and display callbacks | Local concurrency policy supporting replacement/actions | Banner presenter + Runtime | `queued_banner_action_cannot_invoke_the_same_key_on_replacement_content`, `queued_banner_activation_and_dismissal_cannot_mutate_replaced_or_closed_content`, `banner_token_wait_cannot_commit_an_action_after_replacement`, `delayed_display_acknowledgement_cannot_start_a_cancelled_revisions_timeout`; renderer burst and acknowledgement lifetime tests in `presentation.rs`. Actual window/input evidence is linked in [desktop acceptance](notification-desktop-acceptance.md). |
| Renderer startup readiness and idle failure health | Local reliability policy, not a freedesktop capability | Banner presenter + Startup + Control | ADR 0043; renderer/Daemon fixtures cover early exit, delayed readiness, the ten-second timeout, cancellation, late redraw and last-owner shutdown. `idle_renderer_exit_degrades_status_without_disabling_notification_state` covers status and Center/receipt availability without a new presentation update. `tests/renderer_startup.rs` exercises the service executable against a disposable nonresponsive Wayland socket, preserving existing history and leaving legacy import unpublished. ADR 0047 probe retirement and surviving-output fallback have [two-output Sway evidence](../../.scratch/notify/freedesktop-completion-review/OUTPUT-LIFETIME-EVIDENCE.md). Shutdown drains bounded adapters and exits cleanly (`tests/shutdown.rs`, `tests/service_lifecycle.rs`). Renderer recovery uses explicit daemon restart; automatic reconnection is not claimed. |
| Revision/lifetime-safe Notification Center interactions | Local concurrency policy supporting replacement/actions | Notify Control + provider adapter + Runtime | ADR 0042 binds captured center controls to thread/revision/daemon lifetime while preserving stable item IDs; the current host uses protocol v4. `tests/transient_center.rs` covers actual provider values, reused action keys, changed Open/Dismiss meaning, terminal duplicates and daemon restart; `tests/control_api.rs` covers token-free stale rejection and gated token-wait replacement/closure. Ordinary CLI commands still resolve current state. Native accessibility and compositor tests cover current/replaced controls and focus. |
| Static raw images and legacy spellings | Advertised static profile | Request decoder + Intake | `tests/notification_receipt.rs`, decoder tests: signed channel validation, omitted final-row padding, and source precedence. ADR 0036 adds bounded banner pixel preparation and ADR 0037 adds indexed icon lookup; the selected local format scope is documented in the static-format assessment. |
| Indexed icon lookup and refresh | Selected Icon Theme 0.13 semantics with local PNG/JPEG and safety limits | Private renderer-side icon resolver | Nine renderer-consumer fixtures cover selected-theme/parent/root precedence, size/scale, refresh, unsafe metadata and exact/excessive budgets; configuration integration tests cover startup/reload and failed-install rollback. The [static-format assessment](static-icon-completion-research.md) records the retained bounded resolver and selected PNG/JPEG profile. Since [ADR 0138](../adr/0138-shared-icon-theme-and-mime-lookup.md), the desktop theme is followed without an explicit `icons theme`, SVG is rasterized, local files are recognized by content, and sender names try generic fallbacks; XPM remains excluded. |
| Actions, markup and body images on real surfaces | Optional capabilities, required when advertised | Banner presenter + Niribar host | Software-widget layout/input and Wayland-region tests cover rich-banner geometry, compositor-assigned dimensions, click-through and per-card acknowledgement. [Output-fit evidence](../../.scratch/notify/freedesktop-completion-review/OUTPUT-FIT-EVIDENCE.md) covers scales 1/1.5/2 and clipped-card expiry; [oversized-card evidence](../../.scratch/notify/freedesktop-completion-review/OVERSIZED-CARD-EVIDENCE.md) covers bounded internal scrolling, long-action clicks, separate rails, finite expiry, focus and replacement click-through at scale 1 and a narrow scale-2 output. [Two-output Sway evidence](../../.scratch/notify/freedesktop-completion-review/OUTPUT-LIFETIME-EVIDENCE.md) covers placement, reserved panels, surviving-output fallback, actions and expiry. [Earlier evidence](../../.scratch/notify/freedesktop-completion-review/BANNER-LAYOUT-EVIDENCE.md) covers basic host opening. Host contract tests cover v4 content and ADR 0041 transient actions. The [desktop acceptance](notification-desktop-acceptance.md) adds all-action, image/fallback, link and real Orca checks. Supported evidence covers Niri/Sway and scales 1/1.5/2; exhaustive compositor, font, locale and physical-device certification is not claimed. |
| Hyperlinks | Optional; runtime advertised | Intake + Runtime + portal opener + both renderers | Typed safe targets, revision-owned controls and bounded portal requests. Receipt/control/opener tests plus actual pointer, keyboard and Orca [desktop evidence](notification-desktop-acceptance.md). |
| Persistence | Optional; advertised | Lifecycle + SQLite + Center | [ADR 0049](adr/0049-retained-attention-and-sender-lifecycle.md); migration, pending expiry/restart, admission and acknowledgement in `tests/history.rs`, `tests/control_api.rs` and `tests/service_lifecycle.rs`. |
| Durable paths, installed configuration and reload | Local product/operational contract | Startup + Configuration + Control | `tests/control_spine.rs`, `installed_startup_loads_xdg_configuration_before_accepting_notifications`, failed-install sound test; socket partial-clear regression verifies committed counts. `tests/configuration_intake.rs` adds real installed-XDG/D-Bus policy evidence, exact/excessive file admission, FIFO rejection, strict annotation checks, private diagnostics, queued/cancelled preparation and concurrent coherent reloads. |
| Atomic storage and safe upgrades | Local reliability contract | Startup + Notification History adapter | Twelve SQLite adapter tests, future-schema rejection and partial-clear history tests. `tests/legacy_import.rs` verifies source ownership across sockets, committed WAL import, history/policy preservation, existing-destination precedence and failed-import cleanup/retry through executable/control interfaces. Version-2 migration preserves attention, and bounded legacy-body backfills have history coverage. Transactions and failed-import retries are tested; power-loss/filesystem durability certification is not claimed. |
| Text/action admission and markup work bounds | Local resource policy, not specification limits | Request decoder + Intake | `tests/input_limits.rs` covers atomic rejection, exact UTF-8/aggregate limits and markup degradation; `wire_input_limits_reject_before_normalization_without_consuming_ids_or_replacements` covers D-Bus errors and unchanged wire identity. Asset and provider-stream limits have separate policies below. |
| Bounded provider transport and subscription lifetime | Local resource policy; compatible Canopa-v4 frames | Private control transport adapter | `tests/provider_resources.rs` covers frame overflow/recovery, byte boundaries, malformed frames/input, connection admission, idle disconnect churn, stalled writes, bounded interaction replies, total deadlines and content-free diagnostics. The installed duplex `panel-session` uses bounded keyset pages and per-view revision-owned navigation; full-body pagination and independent subscriptions have executable coverage. |
| Bounded ordinary control replies and CLI exchanges | Local resource policy; no freedesktop capability change | Private control transport and command adapter | `tests/control_resources.rs` covers capped serialization with history preservation, exact/excessive reply bytes, request admission, malformed/incomplete/mismatched replies, success formats, partial-clear counts and a slow-drip total deadline without retries. The installed Center uses bounded keyset reads. Legacy full diagnostics remain bounded by admitted history and capped reply serialization. |
| Aggregate live count/text/raw-image admission | Local resource policy, not specification limits | Notification Runtime | `tests/active_resources.rs` covers exact boundaries, replacements, committed cleanup, failed writes, concurrent last-slot admission and stale history; `live_resource_exhaustion_returns_limits_exceeded_without_consuming_wire_ids` verifies the D-Bus boundary. |
| Reclaimable timeout bookkeeping | Local resource policy, preserving display-based expiration | Notification Runtime | `tests/timer_resources.rs` covers completed churn, late/duplicate frames, display/close interleavings, live admission bounds, failed writes, partial clear and delayed sender publication. Control status exposes `timeout_records`. |
| Bounded sound admission and playback | Local resource policy; best-effort sound presentation | Sound presenter | Offline `sound::playback::tests` exercise the public presenter and daemon with a real mixer: concurrency, cancellation/reopen, byte/PCM/duration limits, FIFO/invalid-file fallback, churn, backend availability and committed notification lifecycle under saturation. |
| Sound-theme lookup and disabled overrides | Sound Theme 0.2, with fixed local safety limits | Private sound-theme resolver | Public-presenter fixtures cover indexed profile directories, first-index/root precedence, inheritance cycles, locale/name fallback, disabled overrides, edits without restart, unsafe indexes and exact/over-limit budgets. Notify requests stereo, then profile-less sounds; configurable surround profiles are not claimed. |

The static profile excludes `icon-multi`; Wayland placement may ignore `x`/`y`. No native portal-v2 backend is claimed. See the existing [portal interoperability notes](xdg-notification-portal-v2.md) for negotiated bridge limitations.

## Executable D-Bus contract evidence

`crates/canopa-notify/tests/dbus_contract.rs` starts isolated headless executables on the private bus. These named cases cover the §9 wire contract without claiming graphical acceptance:

| Test | Observable contract |
| --- | --- |
| `introspection_matches_the_complete_notifications_1_3_wire_contract` | Exact ordered input/output argument types for all four methods and three signals at the standard object/interface path; no accidental interface properties. |
| `all_four_methods_return_the_documented_types_and_server_identity` | Actual method-return envelopes, nonzero/replaced IDs, empty successful close reply, server identity/version and exact headless capability set. |
| `independent_dbus_client_reads_four_top_level_server_information_strings` | The independent libdbus `dbus-send` decoder sees four top-level strings, not a single four-field struct. |
| `invalid_closures_return_empty_errors_and_do_not_consume_notification_ids` | Missing/terminal IDs return actual error replies; sender closure and no-op receipt emit reasons 3 and 4, with no spurious closure from rejection. |
| `malformed_calls_and_admission_errors_leave_live_state_and_id_allocation_unchanged` | Wrong argument types/counts, unknown methods and excessive content reject without replacing live state, consuming an ID or emitting lifecycle signals. |
| `action_signals_preserve_opaque_keys_and_precede_nonresident_closure` | Opaque action keys and sender IDs survive real control/D-Bus exchanges; resident invocation remains repeatable and nonresident action precedes reason-2 closure on one signal stream. |

The binary test `notification_server_emits_activation_token_before_action_invoked` checks token → action → closure, signatures, path/interface and service-owner identity on one stream through the public activation-provider seam. `notification_server_expires_notification_after_banner_display` covers reason 1 after display acknowledgement. A headless executable cannot obtain a real compositor token or prove display timing/rendering.

Error conventions are intentionally distinguished from standards requirements: missing closure IDs use `org.freedesktop.DBus.Error.Failed` with an empty string description; “empty D-Bus error” in §9.1.3 does not unambiguously prescribe a zero-byte body. Malformed-call rejection uses binding-generated errors without pinning their names. Both discovery methods enable generated empty-argument validation; previously the binding skipped validation for parameterless handlers. Sequential allocation and unchanged allocation on rejection, closed-token reuse and `LimitsExceeded` admission errors are Canopa policies, not specification-defined allocation algorithms or limits.

## Sender ID lifetime and compatibility

[ADR 0029](adr/0029-live-notification-id-boundary.md) defines a daemon-lifetime wire namespace, independent of retained history. Successful live replacements preserve their wire ID. A nonzero token that is not live, whether never issued or already closed, creates a new notification under that same token, as dunst does and as `notify-send -r` OSD scripts expect. Reuse never revives closed history, and automatic allocation still never returns an issued ID.

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
| Retained and transient notification threads, including silenced/DND threads; the oldest non-critical one expires when full | 1,024 |
| Accepted text, including duplicated identity/body representations, visuals, body-image references/alternatives and actions | 16 MiB |
| Retained raw-image payloads, including padding/trailing bytes | 64 MiB |

Admission follows the pure delivery-policy decision under the mutation coordinator, before lifecycle writes or receipt registration. Under [ADR 0054](adr/0054-bounded-retention-of-undisplayed-and-pending-notifications.md), a full live count first expires the oldest non-critical live thread (reason 1), checking the text and image budgets as if it were gone. Only an all-critical live set, or the text or image budget, returns `LimitsExceeded`, without consuming an ID, emitting sender events or altering a rejected replacement. A live replacement gets credit for its previous footprint. Policy/no-op drops need no live capacity; dropped replacements preserve the original footprint. DND/silenced threads consume count/text capacity, but their images are not retained.

Only committed lifecycle effects change accounting. Failed storage writes leave it untouched; committed closure, dismissal, expiry and staleness release capacity. Restart-loaded history is stale before the new runtime begins and consumes no live capacity. These are logical resource limits, not a total RSS guarantee: client projections, transport deserialization and provider streams remain separate work; banner decoding and sound have their own policies below. Aggregate image exhaustion rejects the request; it does not silently remove a valid image. No capability, D-Bus signature or schema changes.

## Provider stream resource policy

[ADR 0038](adr/0038-bounded-panel-provider-streams.md) bounds provider/control transport independently of notification lifecycle. Complete panel frames, including the newline, are limited to the shared protocol's 1 MiB host limit. Capped serialization emits a small visible error on overflow without deleting, closing or truncating notification state; subsequent fitting projections recover on the same subscription. The provider validates complete compatible frames before writing them to its host. It retains one bounded frame, not an application queue; an overflow-detection byte is permitted during reads.

At most eight subscriptions share the existing 64 control connections. EOF and write-half-close release subscription admission even without a state change. One-way subscriptions reject additional input; duplex sessions accept only validated page navigation. A healthy idle subscriber remains connected; each socket frame write has a five-second total deadline and a partially written timed-out frame is never reused. Changes coalesce through the existing state marker rather than accumulating projections behind a slow write.

Host interactions are limited to 65,518 bytes including the newline, allowing the 18-byte prefix within the 64 KiB daemon request limit. Oversized input is drained in bounded chunks; invalid UTF-8/JSON and unterminated final input are not forwarded. Exchanges are serial, consume at most 4 KiB of reply plus an overflow probe, and have one five-second connect/write/read deadline. Diagnostics never include host action values or daemon response content. Unknown outcomes after a deadline are not automatically retried. The installed provider uses duplex `panel-session`: at most 16 rows per keyset fetch, frames capped at 1 MiB, large action lists continued on subsequent pages, and at most 128 backward page positions per view. Navigation messages are capped at 512 bytes with a five-second partial-read deadline. State changes coalesce. Legacy one-way `panel-subscribe` retains overflow-error behavior. These are work/admission limits, not total-RSS or latency promises.

## Configuration candidate resource policy

[ADR 0040](adr/0040-safe-configuration-candidates.md) admits only readable regular UTF-8 configuration files up to 1 MiB, with one overflow probe and descriptor validation. Symlinks to regular files remain supported; FIFOs/devices/directories are rejected without treating them as streams. Only the documented rule/sound/icons entries are supported; child documents and type annotations are rejected. Candidate rejection preserves committed configuration and applies no partial rule or presentation slice.

Startup and reload preparation use background file work. Reload serialization remains held by both the awaiting caller and actual preparation job, preventing cancelled callers from admitting overlapping preparation or publishing a late candidate. Read/parse/installation errors do not retain arbitrary configuration names, values, parser source or adapter details. Syntax errors include a byte location and status separately reports the committed path/generation. These are local file/admission policies, not freedesktop requirements, parser-internal nesting/work limits, total-memory bounds or hard filesystem deadlines. The eleven daemon/executable regressions in `tests/configuration_intake.rs` use temporary files, a controlled runtime blocking pool, test presentation adapters and real private-bus clients.

## Ordinary control reply resource policy

[ADR 0039](adr/0039-bounded-control-replies.md) caps ordinary JSON reply serialization at 8 MiB including the newline, before socket output. An overflow produces a small content-free error, never partial JSON or a change to retained notification facts. Panel frames keep their separate 1 MiB limit. These bounds apply to encoded replies, not construction of the queried state, database work or total process memory.

The CLI rejects encoded requests above 64 KiB before connecting, buffers at most one 8 MiB reply plus one overflow-detection byte, and validates newline framing, UTF-8 and the expected response shape before stdout output. Connect, write and read share one 30-second deadline, including a five-second write allowance. Invalid, mismatched, incomplete, oversized and timed-out responses fail with sanitized diagnostics; a completed partial-clear response preserves its committed count. The CLI never retries an unknown command outcome. Stdout consumption after a valid reply remains caller-owned. `tests/control_resources.rs` exercises the real executable against local socket peers and real temporary history; pagination, aggregate projection memory and broader recovery acceptance remain separate work.

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

Installed `icons theme="hicolor" scale=1` settings choose the theme and integer source scale (1–8), not UI geometry. Reloads affect subsequent banner updates; invalid configuration or failed sound installation leaves the previous configuration active and the new candidate uncommitted. Without an explicit theme, the desktop theme (GTK `gtk-icon-theme-name`) is followed ([ADR 0138](../adr/0138-shared-icon-theme-and-mime-lookup.md)); output-scale tracking is not implemented. PNG and SVG are supported and JPEG remains a local extension; local files are admitted by content. Legacy XPM remains unsupported. Nine daemon/renderer-consumer fixtures and the extended worker-reopen test cover pixels, precedence, refresh, cycles, special files and budgets. Configuration and executable startup fixtures cover selection plumbing; graphical image/fallback checks are linked in desktop acceptance. The static-format research records the decision to keep this bounded resolver; automatic theme tracking and additional formats are excluded.

## Sound resource policy

[ADR 0034](adr/0034-bounded-notification-sound-playback.md) keeps sound limits behind the existing presenter interface. At most four jobs may resolve, decode, wait in the mixer or play, with no backlog. A reservation is released only after both its worker and audio source finish or are dropped. Further sound requests return a content-free capacity error to the runtime, which logs it and preserves the successful notification receipt, history, actions and closure behavior. Saturation does not change the `sound` capability.

Requested and fallback files must be regular files of at most 8 MiB, including trailing bytes. Symlinks to regular files remain supported; opened descriptors are validated with nonblocking open so FIFOs cannot wait for writers. Reads are bounded even if a file grows after the metadata check. Missing, invalid, oversized or unsupported requested audio uses the existing default fallback; if that also fails, playback is skipped. Audio supports at most eight channels and 192 kHz with a stable stream format. Worker-side decoding truncates to ten seconds or 4 MiB of PCM, whichever comes first, at complete-frame boundaries. The mixer receives PCM only, without file reads or decoding in its audio callback.

Stop, disable and changed sound settings cancel the old playback generation. Identical settings preserve playback; stop makes the backend unavailable until settings are applied again. Reopening never resets outstanding reservations. Cancellation is cooperative during lookup, decoding and source consumption; already buffered device samples cannot be recalled. These are local safety limits, not specification limits, hard filesystem deadlines or total-memory guarantees (including codec-internal allocations and buffer conversion overhead). Icon discovery, provider streams and shutdown have their own bounded adapters and executable coverage. No new capability, D-Bus method, configuration key or schema change is introduced.

[ADR 0035](adr/0035-bounded-sound-theme-resolution.md) defines sound-theme lookup. The first index across ordered XDG roots selects declared directories and parents. Notify searches stereo then profile-less directories, selected theme then recursive parents before freedesktop and unthemed fallback. Generic event names stay within the higher-priority theme; locale lookup supports modifier/territory/language fallback, literal and codeset-free spellings, `C`, and unlocalized files. Locale precedence is captured from `LC_ALL`, `LC_MESSAGES`, then `LANG` at startup. These choices follow the [Sound Theme lookup rules](https://specifications.freedesktop.org/sound-theme/0.2/sound_lookup.html), preferring their prose where the published pseudocode conflicts on parent/default ordering.

An encountered `.disabled` marker stops all lookup and application-default fallback, per the [directory-layout specification](https://specifications.freedesktop.org/sound-theme/0.2/directory_layout.html). Unsafe indexes or exhausted budgets also skip the sound; they are not treated as missing files. Lookup allows at most 32 roots, 32 visited themes, 64 KiB per index, 64 declared directories, 32 parents per index and 4,096 traversal/candidate checks. Lexical path traversal is rejected; opened indexes use bounded nonblocking regular-file reads. No arbitrary directory scan or permanent lookup cache remains. Unindexed installations retain only fixed stereo/mono/root probes; FLAC/MP3 remain extensions after standard formats. These local extensions and limits are not freedesktop requirements. The static-format research records the selected resolver policy and separately scoped alternatives.

## Expiration semantics

[ADR 0028](adr/0028-notification-timeouts-begin-with-banner-display.md) defines the implemented behavior.

- Positive and server-default expiration values begin counting only when the current **Notification Thread** revision first becomes visible in a **Notification Banner**. A zero value never expires.
- **Silenced Notifications** and requests accepted under **Do Not Disturb** begin their timeout at acceptance, and overflow notifications when they overflow, so they expire like displayed ones ([ADR 0054](adr/0054-bounded-retention-of-undisplayed-and-pending-notifications.md)). **Headless Mode** requests never display a banner and remain active without automatic expiration.
- Once a **Notification Banner** has displayed, its deadline continues if a critical notification displaces it. Notify does not later promote an overflow notification merely because a banner slot opens.
- A replacement that revises a **Notification Thread** cancels the prior revision's timeout and begins a fresh timeout when the replacement revision becomes visible, is silenced, or overflows. A dropped replacement preserves the existing thread and timeout.
- When a displayed notification reaches its deadline, Notify invalidates the active sender ID and emits `NotificationClosed` with reason `1` while retaining an **Expired Notification** in **Notification History**.

[ADR 0033](adr/0033-timeout-registration-without-tombstones.md) replaces permanent cancellation tombstones with one registration per pending or scheduled live revision. Only ordered committed lifecycle effects create registrations. Display callbacks can start a matching pending registration once; they cannot recreate one after cancellation or completion. Scheduling and cancellation share one lock. This preserves stale-revision safety while bounding retained timer records by the 1,024 live-thread admission limit. Completed churn returns the record count to zero, including DND, transient and never-expiring notifications.

`canopa-notify ctl status --json` exposes the additive `timeout_records` diagnostic field; plain-text availability output is unchanged. It counts registrations, not notifications in history, all process tasks or process memory. A firing timer releases its registration before expiration/effect publication, so delayed publication does not retain timer bookkeeping or abort its own publisher. Failed receipt/close/replacement writes preserve existing timers and partial clear cancels only committed transitions. Expiration write failures retain the existing no-retry behavior; shutdown cancels registrations; expiration write failures remain visible errors requiring an explicit later action or restart.

## Runtime-only transient center actions

The historical host [keyboard continuation](../../.scratch/notify/freedesktop-completion-review/CENTER-KEYBOARD-EVIDENCE.md) adds bounded action-first selection previews and pointer-gesture ownership: a press begun on old content cannot complete against its replacement. Real-widget regressions cover both ordinary controls and the selected preview; the complete 1,145-test host suite passes. Nested-Niri evidence covers paced keyboard activation/replacement/Escape and default/fourth actions on a hidden transient without starting its banner timeout. The later native AT-SPI bridge and [Orca acceptance](notification-desktop-acceptance.md) establish screen-reader semantics for the selected host.

The [window-lifetime follow-up](../../.scratch/notify/freedesktop-completion-review/CENTER-WINDOW-EVIDENCE.md) fixes rapid input loss caused by recreating the Canopa panel window on navigation and replacement. Forty actual rapid/paced trials invoke the correct action while retaining one window; replacement reset and Escape pass. All 1,151 host tests pass, and framed-v1 lifecycle behavior is unchanged. The later Canopa-v4 bridge adds content scrolling and native accessibility; current evidence and capability status supersede that historical continuation.

[ADR 0041](adr/0041-runtime-only-transient-center-actions.md) supersedes the initial transient-center exclusion without changing the [transient hint's persistence bypass](https://specifications.freedesktop.org/notification/1.3/hints.html). Live transient threads join retained Center items in one date-ordered reading list, with all named actions and default item activation. Retention metadata remains available through the diagnostic Control API; cards use the simplified presentation from [ADR 0052](adr/0052-center-reading-and-settings-views.md). Creation/ID ordering is deterministic and replacement preserves the item identity. Category controls include categories present in live transient items. This is a derived live view, not durable transient history or a claim of the `persistence` capability.

Transient items disappear on dismissal, nonresident action, sender closure, acknowledged-display expiry or restart; resident actions keep them live. Each active item advertises a provider-owned `Open notification` control when a default action exists, otherwise `Dismiss notification`; inactive history has no sender actions; pending retained items offer Mark as read and safe body links remain available. Center opening does not count as banner display; DND/silenced items time out from acceptance and overflow items from overflow (ADR 0054). History deletion leaves live items alone; clear-active includes transient threads. The retained-history snapshot and legacy active diagnostics remain retained-only; the typed Notification Center projection and provider stream carry live transient content.

Evidence: `notification_center_exposes_all_transient_actions_without_retaining_history`, `center_subscription_tracks_transient_replacement_policy_and_cleanup`, and `transient_center_order_survives_other_app_runtime_churn` in `tests/control_api.rs`; the two transient-center/overflow tests in `tests/presentation.rs`; the real provider/private-D-Bus round trip in `tests/transient_center.rs`; and the strengthened two-process `tests/dbus_restart.rs`. These checks do not exercise actual compositor hit regions, layout or assistive technology. Banners expose all accepted named controls in wrapping rows. Oversized cards scroll; the compatible Center provides deliberate keyboard access and bounded pagination without retaining transient content.

## Specification-defined capabilities

Compact banner cards use natural content layout with a body preview capped at 120 logical pixels. Under [ADR 0046](adr/0046-scrollable-oversized-compact-cards.md), oversized cards scroll internally within a bounded viewport: a single card can use the available height; multi-item stacks reserve 60 logical pixels of vertical navigation room, with a half-viewport minimum card budget on tiny outputs. Shorter cards retain natural height. A separate 12px stack-scrollbar gutter avoids overlapping internal card rails. [ADR 0045](adr/0045-compositor-sized-banner-surface.md) supplies compositor-assigned dimensions and 12 logical-pixel outer margins. Cards remain top-right, at most 360 logical pixels wide including the optional gutter. Natural measurement controls only pointer input: empty startup, cleared content and space outside the visible stack are click-through. Queued input-region commands resolve the latest bounds, so delayed delivery cannot restore an obsolete region. This uses larger transparent rendering buffers; no extra output-discovery or scale model is maintained by Notify.

Summaries and compact action labels wrap to their natural height rather than a fixed one-line box. Cards without named actions have no action row, so a short card is only as tall as its text. Text beside an action icon receives the remaining button width. Renderer/input fixtures measure natural card heights instead of fixing them, and cover explicit line breaks, ordinary words, unbroken labels, all compact action keys, clicks on final lines, and shrinking after replacement at 360/276 logical-pixel card widths. All accepted named actions are included; longer lists continue through the bounded card viewport. Oversized content remains intact behind internal scrolling; controls below the fold require scrolling, not a separate expanded surface.

Under [ADR 0044](adr/0044-per-card-banner-display-acknowledgement.md), extended by ADR 0046, each card independently starts its first-display timeout when its bounded compact viewport fits the drawing viewport, allowing half a logical pixel for rounded scroll translation. Display does not mean every line has been read: internal scrolling neither pauses nor restarts the sender's normal timeout. A clipped lower card waits without delaying a fitting upper card, and stack scrolling can later acknowledge it. Callbacks retain captured ID/revision ownership and ignore duplicate or unknown targets. Software-renderer, Wayland-region wire and daemon/presenter regressions cover this handshake, including ordinary one-line wheel traversal past an oversized middle card. Large wheel jumps can skip cards; available heights below 120 logical pixels, compositor presentation feedback, occlusion and the complete supported-scale/font/locale matrix remain outside this verification. No additional capability follows from these corrections.

[ADR 0047](adr/0047-compositor-selected-banner-stack-lifetimes.md) separates the renderer connection from individual stack surfaces. A transparent startup probe establishes readiness and retires; each new nonempty stack requests compositor-selected placement. A visible stack stays put across replacements/focus changes and is retired when cleared. Loss of a displayed surface retries the current stack on a fallback output without resetting notification deadlines. Failure before redraw, or the ten-second first-frame deadline, stops rendering under ADR 0043. Retired windows cannot acknowledge new queued content or overwrite current input geometry; each surface owns one cancellable deadline. This is not automatic renderer reconnection or guaranteed recovery after every output disappears.

| Capability | Status | Notes |
| --- | --- | --- |
| `actions` | Advertised | Named actions and `default` activation are supported. |
| `action-icons` | Advertised | `action-icons=true` maps action keys to static icon-name references; banners render resolved local raster icons when available. |
| `body` | Advertised | Body text is shown with a plain-text fallback. |
| `body-hyperlinks` | Runtime advertised | Validated HTTP(S) links are distinguishable, accessible controls on both surfaces. Advertised when the desktop opener is available; absent in ordinary headless mode. |
| `body-images` | Advertised | Ordered local PNG/JPEG body images render in banners and Canopa-v4 Center, with safe alternative-text fallback and native image semantics. |
| `body-markup` | Advertised | Bold, italic, underline, tolerated line breaks, and safe text fallback are supported. |
| `icon-multi` | Not implemented | Animated or multi-frame icons are not supported. |
| `icon-static` | Advertised | Raw image data, local static raster files, and static raster icon-theme names are supported. |
| `persistence` | Advertised | Retained content stays discoverable until acknowledgement, explicit removal or sender recall. Finite expiry and restart end sender actions without discarding pending attention. |
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
| `transient` | Implemented | New transient notifications retain runtime presentation, actions, timeouts, sound, and sender lifecycle without entering persisted Notification History. Live center items are marked runtime-only and disappear after termination/restart under ADR 0041. A thread's initial request fixes retention across replacements. |
| `urgency` | Implemented | Maps `0`, `1`, and `2` into low, normal, and critical behavior. The specified byte and any other D-Bus integer width (`notify-send -h int:urgency:2`) are accepted; other values mean normal. |
| `x`, `y` | Decoded, ignored | Wayland banner placement is daemon-owned; sender coordinates are not honored. |

The boolean hints `transient`, `resident`, `suppress-sound` and `action-icons` also accept any D-Bus integer, nonzero meaning true, because `notify-send -h int:transient:1` is the common spelling. `value` accepts any integer width.

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

## Retained attention, body links and accessibility

Pending retained content is independent of sender lifecycle. Admission caps it at
4,096 notifications and 64 MiB of logical serialized text. Exceeding either limit
archives the oldest inactive non-critical pending items as read (ADR 0054); only
active or critical pending content that fills the budget rejects the receipt
atomically. Age/count archive pruning never removes pending content.
Transient notifications bypass durable history. DND and silence preserve pending
attention; explicit drops do not create history. Resident sender actions acknowledge
without closing. Following a body link does not acknowledge, close or invoke a
sender action. Old sender callbacks are disabled after expiry or restart.

Body links accept only bounded absolute HTTP(S) targets, without credentials,
control/bidi characters, whitespace, backslashes or malformed percent escapes.
Unsafe/nested markup remains readable text. Portal OpenURI requests have bounded
admission, setup, response and cleanup; uncertain launches are never retried through
a shell fallback. Diagnostics omit raw targets. The host receives validated URL
metadata for assistive technology and sends an opaque revision-owned action back
to Notify for activation.

Each displayed banner exposes notification/body/link/button semantics through
native AT-SPI. The first displayed revision is announced politely once; geometry
updates do not repeat it, and banners do not request keyboard focus. The Center
exposes ordered labels/images, link destinations, named buttons and radio selection
states. Its keyboard surface provides Tab/arrows, Enter, PageUp/PageDown/Home/End,
and Escape. Retired/replaced controls cannot invoke replacement actions. The
vendored AccessKit cache-signal correction and real Orca pass are recorded in
[desktop acceptance](notification-desktop-acceptance.md).

## Release checks

See [release verification](notification-release-verification.md) for the final full/core/host suites, lint policy, isolated installation, source-export build, performance baseline and repeatable desktop job. Historical investigations above retain their narrower evidence boundaries.
