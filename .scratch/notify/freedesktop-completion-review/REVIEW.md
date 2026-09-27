# Canopa Notify: implementation and architecture review

Review date: 2026-09-10. Baseline: clean checkout at `d3f12df1888acbeb1cf05fd81baa00fb500e6a89`. Scope: the whole project, including the two workspace crates, tests, service packaging, manual fixtures, domain documentation, ADRs, and local backlog. No implementation changes were made during this review.

Read the [implementation plan](PLAN.md) for the proposed work sequence and the [standards research](standards-research.md) for authoritative sources, optionality, and specification ambiguities.

## Assessment

The project has a useful foundation worth preserving: pure Delivery Policy, an explicit Notification Lifecycle, a Notification History trait with SQLite and test adapters, runtime-owned presentation state, conservative content normalization, and a generic panel protocol. Most ordinary notification flows work and have coverage. Completing this project should be an incremental effort around those modules.

It is not ready to claim complete support or reliable daily use. The most significant defects concern ID translation, lifecycle ordering, event delivery, startup ownership, and durable storage paths. Several user-facing features exist only as library methods. More tests of ordinary sequential success paths will not address these gaps; the next tests need to exercise the installed binary, concurrent transitions, slow consumers, and failures after persistence.

The review axes below remain separate. P1 means high-priority correctness or product-blocking work; P2 means significant completeness, robustness, or maintainability work; P3 means a localized lower-impact issue. Findings marked **reproduced** were exercised during this review. **Code-established** findings follow directly from current control flow. **Verification gaps** need the specified experiment before being treated as confirmed visible failures.

## Verification performed

| Check | Result and limits |
| --- | --- |
| `cargo test --workspace --locked` | Passed: 216 tests, no failed or ignored tests; no doctests. Includes 14 D-Bus integration tests. |
| `cargo fmt --all -- --check` | Passed. |
| `cargo clippy --workspace --all-targets --locked` | Passed with warnings: four library lint findings, a test comparison lint, and unused shared test helpers. This was not a warnings-as-errors run. |
| `scripts/manual/verify-xdg-notification-portal.sh` | Passed against real installed portals on a private session bus. Public Notification version 1, GTK backend version 0, empty SupportedOptions. v2-only assertions were explicitly unavailable. Desktop notification ownership was not replaced. |
| Replacement ID probe | Reproduced overwriting an unrelated active thread using a previously closed wire ID. |
| Sender-event capacity probe | Produced 130 closure events; the same broadcast stream used by D-Bus reported `Lagged(2)`. |
| Duplicate-start probe | Did not complete because its private-bus launch was blocked by sandbox permissions. A01 is based on the verified startup control flow, not an observed corruption result. |
| Graphical rendering and accessibility | Reviewed source and presentation-model tests. No screenshot, compositor interaction, screen-reader, or Niribar-host rendering certification was performed. |

The portal fixture emitted environment warnings from GTK/GVfs/other portal modules but completed its notification assertions successfully. That result establishes the tested bridge path, not portal v2 completeness.

## Standards and architecture findings

### A01 — P1: singleton ownership is checked after mutating live history

**Code-established.** [main.rs](../../../crates/canopa-notify/src/main.rs), lines 102–151, opens the daemon before checking its socket or acquiring its D-Bus name. Opening the runtime calls `mark_active_stale` at [runtime.rs](../../../crates/canopa-notify/src/runtime.rs):175.

A second process using the first process's database can mark its active rows stale and then exit with an already-running error. The surviving daemon still owns its runtime banners, timers, and sender registrations, so those no longer agree with storage. The same ordering risks modifications when startup ultimately fails to own D-Bus.

This violates the documented single-instance and fail-fast rules in [CONTEXT.md](../../../docs/notify/CONTEXT.md). Acquire session ownership before migrations or stale recovery, and retain that ownership for the daemon lifetime. Regression coverage must assert that a failed competing startup leaves database content and the first process's live operations unchanged.

### A02 — P1: installed history and policy are stored in the runtime directory

**Code-established.** [main.rs](../../../crates/canopa-notify/src/main.rs):104 defaults the database to the socket path with a `.db` extension. The [service unit](../../../packaging/systemd/canopa-notify.service):9 supplies `%t/canopa-notify/control.sock` without a database override. Consequently history, DND, and the User Rule Store are stored under the session runtime directory.

This conflicts with [ADR 0004](../../../docs/notify/adr/0004-persistent-local-notification-history.md) and persisted-policy expectations. XDG runtime files must not survive a full logout or reboot; durable state belongs under an appropriate persistent directory. Use `$XDG_STATE_HOME/canopa-notify/`, with the documented XDG fallback, independently from the socket. Plan a safe import of an existing database, including SQLite sidecars, while the old owner is stopped. [XDG Base Directory specification](https://specifications.freedesktop.org/basedir/latest/).

### A03 — P1: revision guards do not cover the complete transition

**Code-established concurrency defect.** [lifecycle.rs](../../../crates/canopa-notify/src/lifecycle.rs):443 checks the timer's revision, then calls `transition_active`, which rereads the thread at line 526. A replacement between those reads allows an old timeout to expire the replacement. `invoke_action` similarly validates one thread at line 470, then calls `dismiss`, which rereads it at line 491.

Persistence preconditions protect the second read's revision, not necessarily the revision whose timeout/action was validated. Runtime methods also apply returned effects after asynchronous persistence without a common ordering mechanism. CAS checks alone do not order mutations, image updates, banner state, timer acknowledgements, and sender publication.

This violates the documented revision-safe lifecycle intent. Carry the validated thread/revision into the guarded write and order runtime mutations with effect enqueueing. Use a barrier-controlled history adapter to force the read/replace/write interleaving, plus tests that delay publication of an earlier revision. Avoid holding an ordinary mutex through an await or serializing slow image/audio/token work inside the mutation path.

### A04 — P1: storage operations can fail after committing their mutation

**Code-established failure mode.** [sqlite_persisted_state.rs](../../../crates/canopa-notify/src/sqlite_persisted_state.rs):720–774 executes an insert/update, rereads it through the pool, and separately calls `finish_history_write`. That helper can return an error from pruning at line 706 after the original write has committed. The reread can also observe a later concurrent revision.

The caller can receive failure while the new state is already durable, suppressing the expected lifecycle effects. A client retry may then create a duplicate. `clear_active_notifications` also accumulates effects across multiple writes and returns no accumulated result if a later write fails ([lifecycle.rs](../../../crates/canopa-notify/src/lifecycle.rs):457).

Define the commit point. Return the exact written revision and either include mandatory maintenance in the transaction or treat post-commit pruning as separately reported maintenance. Make bulk-operation partial success explicit, or provide transactional semantics where feasible. Inject pruning, busy database, readback, and later-item failures; assert both persisted state and emitted effects.

### A05 — P2: runtime state grows for the daemon's entire lifetime

**Code-established.** [runtime.rs](../../../crates/canopa-notify/src/runtime.rs):44–46 retains all sender events and delivery decision logs in vectors. `cancelled_through` at line 104 retains a tombstone for each completed notification. Active notifications are exempt from storage retention, while undisplayed notifications intentionally never expire. The overflow map stores complete threads, and runtime image storage has no aggregate byte budget.

The 1,000-thread history limit therefore does not bound runtime memory or active database growth. This is particularly relevant to DND and transient/overflow requests. Sound playback additionally spawns a thread per request ([sound.rs](../../../crates/canopa-notify/src/sound.rs):112).

Separate production observability from test recording, bound diagnostic capture, reclaim completed timer state safely, and define active-count/image/worker budgets. Admission or eviction policy must preserve a documented sender lifecycle and the user's DND/drop choices. A long-running create/replace/close workload should return close to steady-state memory after completion.

### A06 — P2: configuration activation has two commit points

**Code-established interface weakness; production failure reproduction still required.** [lib.rs](../../../crates/canopa-notify/src/lib.rs):309 replaces `LoadedNotificationConfiguration.active` before awaiting runtime installation. [runtime.rs](../../../crates/canopa-notify/src/runtime.rs):443 can fail while applying sound settings before installing the rule slice. Concurrent reloads can also release the configuration lock and acquire the policy lock in a different order.

The parse-time invalid-config behavior is tested, but the complete loaded configuration and runtime policy need one observable version. Parse a candidate without mutating active state, serialize installation, define which adapter failures are fatal versus degraded capability, then expose the committed version. Test a failing sound adapter and deliberately reordered reloads. This is an implementation of the atomic configuration rules already in CONTEXT.md, not a proposal to re-evaluate existing notifications.

### A07 — P2: architectural intent and dependency boundaries have drifted

**Maintainability judgment.** The lifecycle/history distinction is valuable, but several concrete seams remain leaky:

- Wire IDs, thread IDs, and replacement aliases share numeric representations across intake, the binary, lifecycle, and storage; F01 demonstrates the consequence.
- `presentation.rs` combines UI lifecycle, Wayland activation, image/theme resolution, banner selection, layout, and widget tests. Image lookup occurs while runtime presentation is being built.
- The library unconditionally depends on the GUI/audio stack, so its pure policy/lifecycle test build still compiles those dependencies. [ADR 0015](../../../docs/notify/adr/0015-cargo-workspace-from-start.md) intended dependency separation, while CONTEXT.md explicitly defers a separate domain crate.
- `control_adapters.rs` combines CLI I/O, socket framing, panel projection, and panel interaction decoding.

These are possible divergent-change and primitive-obsession smells, not violations established by line count. Strengthen ownership and typed IDs, move cohesive implementations behind small interfaces, and feature-gate the graphical adapters before considering another crate. Preserve the real Notification History, BannerPresenter, SoundPresenter, and ActivationTokenProvider seams; avoid adding generic repositories or buses over existing interfaces.

### A08 — P2: the documented implementation baseline is contradictory

**Documented inconsistency.** The original [PRD](../canopa-notify/PRD.md) still requires active-policy re-evaluation and excludes sounds. [ADR 0027](../../../docs/notify/adr/0027-policy-changes-apply-to-future-notification-requests.md) explicitly supersedes re-evaluation, and sound is implemented under [ADR 0025](../../../docs/notify/adr/0025-in-process-notification-sound-playback.md). CONTEXT.md also contains initial-scope body-image and category descriptions alongside later implemented rules.

Retain the ADR decisions and mark superseded PRD requirements explicitly. Audit ticket completion at the installed product surface, since implemented library methods do not establish CLI or service behavior. Add versioned migration and release conventions; the current schema uses startup inspection/backfills rather than a numbered migration history.

Standards axis: eight findings. The most immediate risks within this axis are startup mutation, temporary durable state, and lifecycle/storage commit ordering.

## Freedesktop specification findings

### F01 — P1: wire IDs can alias unrelated history primary keys

**Reproduced with the built binary on an isolated D-Bus session.** [main.rs](../../../crates/canopa-notify/src/main.rs):366 translates a replacement ID only if the sender registry resolves it. Otherwise the original wire value survives in `NotificationInput.replaces_id`. [lifecycle.rs](../../../crates/canopa-notify/src/lifecycle.rs):413 first interprets that number as a thread ID, without requiring an active thread or matching identity. Registry updates then replace existing mappings.

Observed sequence:

| Operation | Wire ID | Stored result |
| --- | --- | --- |
| Empty new notification | 1, immediately closed | No history row. |
| AppA sends a meaningful notification | 2 | Thread 1, AppA, revision 1. |
| AppB sends `replaces_id=1` | 1 | Thread 1 overwritten with AppB content, revision 2. |
| Close AppA's ID 2 | Error | Its registration has been displaced. |

The final observed row was `1|AppB|Foreign replacement|2|active`. This can also revive inactive history. The issue is a collision of ID domains; it does not depend on inventing a cross-client ownership rule. Closed IDs must be invalidated. Unknown replacement handling needs an explicit compatibility decision because the specification's broad nonzero-return wording leaves edge cases unclear. [Notifications 1.3 protocol](https://specifications.freedesktop.org/notification/1.3/protocol.html).

Resolve wire IDs exclusively in a protocol-owned live registry. Pass a typed resolved target into lifecycle. Caller-token compatibility, if retained, needs its own explicit mapping and cannot use database keys as a fallback. Test this exact sequence, restarts, stale IDs, dropped/transient requests, wraparound, and separate senders.

### F02 — P1: the D-Bus event stream drops required lifecycle events

**Reproduced at the exact runtime stream consumed by D-Bus.** [runtime.rs](../../../crates/canopa-notify/src/runtime.rs):189 creates a 128-entry broadcast channel. A synchronous bulk event publication can exceed it. [main.rs](../../../crates/canopa-notify/src/main.rs):472 logs lag and continues without the missed events. Closure-driven ID invalidation also happens in that relay, so a lost closure leaves a registration behind.

A probe used a current-thread Tokio runtime, created 130 transient notifications, subscribed, and cleared active notifications. It printed:

```text
Sender events produced: 130
First receiver read: Err(Lagged(2))
```

The production relay may drain concurrently, so this does not establish loss on every burst. It proves that the transport permits loss under an ordinary scheduling condition. Use ordered delivery with backpressure and capacity reservation before commit; keep replaceable current-state snapshots on their existing coalescing path. Increasing channel capacity is not a correctness fix. Assert all closure/action events under a slow sink, including token/action/closure ordering and ID invalidation when transport fails. [Notifications 1.3 protocol](https://specifications.freedesktop.org/notification/1.3/protocol.html).

### F03 — P2: transient notifications can hide advertised actions permanently

**Code-established.** [presentation.rs](../../../crates/canopa-notify/src/presentation.rs):1739 takes only two named actions. Retained notifications expose the rest in Notification Center; transient notifications are omitted because that projection uses retained state. A transient notification's third action therefore has no user-accessible surface. Overflow transient notifications can be inaccessible altogether while remaining active indefinitely.

Advertising `actions` promises to make the supplied actions available. Provide an expandable action area or an active-only transient surface without persisting transient content. The existing banner limit and transient exclusion are deliberate product choices, so update the corresponding domain rules as part of resolving their interaction. Acceptance: four distinct named actions are reachable and preserve opaque keys, residency, and non-persistence. [Capability definition](https://specifications.freedesktop.org/notification/1.3/protocol.html).

### F04 — P3: raw channel counts are truncated before validation

**Code-established.** [domain.rs](../../../crates/canopa-notify/src/domain.rs):473 casts `i32 channels` to `u8` before checking for three or four. Values such as 259 and 260 can wrap into accepted counts. Subsequent indexing uses the normalized count, so this review does not claim memory corruption.

Validate the signed source value before conversion. Include negative and out-of-range values in the image fixture table. The required RGB/RGBA channel counts and byte layout are explicit in the [image format](https://specifications.freedesktop.org/notification/1.3/icons-and-images.html).

Spec axis: four findings. F01 and F02 are the highest-priority defects within this axis.

## Product completion and presentation findings

### C01 — P1: users cannot load config, reload it, or control DND through the shipped commands

**Code-established.** `DaemonArgs` has no config argument; startup never calls `reload_configuration_file`. `CtlCommand` has only `Status` and `Active` ([main.rs](../../../crates/canopa-notify/src/main.rs):50–79). `ControlCommand` likewise omits reload and DND ([control.rs](../../../crates/canopa-notify/src/control.rs):98). KDL accepts rules and sound settings, but not the documented retention/presentation defaults.

Consequently normal installed startup cannot apply the user's KDL rules or sound preferences, and several completed tickets describe behavior available only to Rust callers. Expose config discovery/override, startup loading, reload, DND reads/writes, useful status/errors, and selected supported settings through one Notify Control API. Test the executable against temporary XDG directories; do not establish completion only by calling Daemon methods.

### C02 — P1: a full or failed renderer queue turns a committed Notify into an error

**Code-established.** The graphical presenter uses `try_send` on a queue of 32 ([presentation.rs](../../../crates/canopa-notify/src/presentation.rs):145,199). [runtime.rs](../../../crates/canopa-notify/src/runtime.rs):209–248 persists, registers the sender receipt, and updates runtime state before presentation failure propagates to `Notify`. State subscriptions are notified only after successful effect application.

A slow renderer can therefore leave a stored notification whose client saw an error and whose Notification Center subscription missed a change. A dead UI thread is logged but not supervised ([presentation.rs](../../../crates/canopa-notify/src/presentation.rs):313), leaving service health misleading.

Use coalesced complete presentation state with generation/revision acknowledgements, separate rendering health from request acceptance, and define recovery after renderer failure. Test a blocked/closed presenter and a burst beyond queue capacity; each accepted request must have consistent receipt/state publication, and timers must start only for acknowledged revisions.

### C03 — P2: sender-controlled resource handling is only partly bounded

**Code-established limitations, not a demonstrated exploit.** Raw images have useful byte/dimension checks. File visuals check extension and encoded file length only ([presentation.rs](../../../crates/canopa-notify/src/presentation.rs):1831), then pass the path to iced for later decoding. The current path does not establish an application-owned decoded-pixel budget or restore alt text after decode failure. Corrupt-but-existing files can therefore select an image segment instead of its fallback.

Markup limits the final segment count only after constructing per-character parse nodes ([intake.rs](../../../crates/canopa-notify/src/intake.rs):86,245); it does not bound parsing allocation up front. Repeated malformed `<` characters cause repeated searches of the remaining input. Socket frames are unbounded `read_line` calls, accepted connections spawn unbounded tasks, and the entire control request is logged ([control_adapters.rs](../../../crates/canopa-notify/src/control_adapters.rs):123).

Introduce explicit intake/frame/action/text/decoded-image budgets, a linear bounded parser, bounded connection and sound workers, and redacted structured diagnostics. Decode assets through a reusable resolver that returns a validated renderable result or fallback. Measure limits under malformed inputs and corrupt/oversized assets, including changed files and symbolic-link cases.

### C04 — P2: rich-content layout and external-host behavior lack rendered evidence

**Layout risk established in source; visible clipping requires a graphical fixture.** A banner has fixed height 140, while a rich body alone gets 70, title 22, actions 28, card padding 20, and additional metadata/spacing ([presentation.rs](../../../crates/canopa-notify/src/presentation.rs):941–1099). Multiple image/text blocks exceed the body's clipped height. Model tests confirm segments are present, not that actions and fallback text remain visible or operable.

The [body-images completion ticket](../body-images/issues/05-advertise-body-images-after-notification-center-rendering.md) correctly keeps the capability disabled pending Niribar host verification. This checkout contains protocol v3 and provider output but no external host implementation to certify. Hyperlink targets are discarded; no `body-hyperlinks` claim is made. Accessibility is still an open ticket.

Use measured content layout with an explicit overflow policy, then rendered tests of body/image/action combinations at several scales. Verify the real Notification Center host before advertising body images or links. Keyboard and assistive-technology support require host/stack evidence; adding labels to a projection alone is insufficient.

### C05 — P2: theme lookup and release tooling remain incomplete

**Code-established.** The icon resolver recursively indexes raster filenames and chooses the first name match, without selected-theme inheritance, size/scale selection, or invalidation ([presentation.rs](../../../crates/canopa-notify/src/presentation.rs):1875–2035). The sound resolver has useful inheritance/fallback, but lacks `.disabled` stop semantics and the complete profile/locale/name fallback rules; its directory walk follows directories without a cycle budget ([sound.rs](../../../crates/canopa-notify/src/sound.rs):247–352).

These are supporting-standard completion tasks, distinct from mandatory Notification method support. Evaluate existing resolver libraries against small conformance fixtures before deciding whether to replace the custom implementations. Sources: [Icon Theme](https://specifications.freedesktop.org/icon-theme/0.13/), [Sound Theme lookup](https://specifications.freedesktop.org/sound-theme/0.2/sound_lookup.html).

The checkout also lacks a root README, tracked CI configuration, top-level license texts, a man page referenced by the service unit, and a complete install workflow. Workspace repository metadata is a placeholder. Vendored graphics code needs an upstream revision/patch/update record. Add these to a release milestone with clean-checkout and installed-service tests.

## Correct behavior to preserve

- Notification image precedence is correct for the current separate icon/image layout. Raw RGB/RGBA validation correctly permits missing padding after the final row; do not replace it with a `height * stride` requirement.
- Actions retain case-sensitive opaque keys, handle malformed pairs, and support the portal's empty-label default action. Resident and nonresident semantics have focused tests.
- Display acknowledgements and revision-aware timeout intents are useful architecture. Preserve [ADR 0028](../../../docs/notify/adr/0028-notification-timeouts-begin-with-banner-display.md), while correcting concurrent transition handling.
- Delivery Policy is pure; loaded policy snapshots and user-rule precedence are explicit. Preserve [ADR 0027](../../../docs/notify/adr/0027-policy-changes-apply-to-future-notification-requests.md): changes affect subsequent requests, including replacements.
- New dropped/no-op notifications receive nonzero IDs and closure; dropped replacements preserve the existing notification. Transient contents bypass persisted history.
- Capability sets distinguish standard names from extensions. Sound advertisement follows backend availability. `body-images`, `body-hyperlinks`, and `persistence` are not falsely advertised today.
- The private portal fixture tests the real frontend/backend/daemon path, including FD icon translation and default/named action round trips.

## Decisions that must be explicit in the completion plan

These are not automatically bugs: critical positive-timeout precedence; ignored Wayland `x`/`y` hints; sticky transient retention across replacements; sender identity/ownership policy; unknown replacement IDs; and the meaning of persistence alongside finite expiry. Relevant ADR changes must be called out before implementing a different behavior.

Use a complete **static Notifications 1.3 profile** as the main target, adding usable hyperlinks, body images, reliable sound/theme support, and persistence only after its retention contract is established. `icon-static` and `icon-multi` cannot be advertised together. Animation has no complete current wire contract and should remain an explicit optional exclusion. A native portal v2 backend is a separate interface and separately scoped milestone, not something proven by the passing v1 bridge fixture.
