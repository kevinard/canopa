# Address the 2026-10-01 code-quality audit

Status: complete

User objective: **address all the audit findings**. Baseline: `2978fb3`.
The audit found 20 items; none may be silently dropped or treated as complete
merely because related tests pass. Preserve the supported v1/v4/v5 protocols,
independent Shell/Notify lifetimes, domain ownership, and shared vendor provenance.

## Required work and evidence

| ID | Required outcome | Verification seam | State |
| --- | --- | --- | --- |
| F01 | One Foreground replacement lifetime; stale pending menus cannot overwrite Panels; remove duplicate Panel ownership/forwarding data | Foreground transitions including pending menu → Panel/dmenu, stale closure, origin removal | Implemented; 546 App checks pass |
| F02 | Wi-Fi operations bind and revalidate selected backend owner/network identity | Private backend fixtures with owner/AP-path reuse and credential prompt | Implemented; private owner/path/roaming/agent/prompt checks pass |
| F03 | Failed firing expiry retains recoverable revision-scoped work and reports terminal failure | Daemon timeout/storage failure, recovery, replacement and shutdown | Implemented; all 10 timer-resource checks pass |
| F04 | Configured and dynamic Module identities cannot collide | Real runtime reconciliation, both placement orders, actions/focus/removal | Implemented; 548 App checks and strict Clippy pass |
| F05 | Essential window events always observed; effective configured interactions determine navigation | Subscription/event → App, clock/network-only and inert modules | Implemented; 546 App, 9 input and 24 module checks plus strict Clippy pass |
| F06 | Invalid clock format directives rejected before runtime; one format interpretation | Config diagnostics and clock cadence/rendering | Implemented; 37 focused checks pass |
| F07 | Media observation survives host-input cancellation after consuming Changed | Delayed Snapshot plus interleaved host event, no second Changed | Implemented; 3 media-provider checks pass |
| F08 | Bar/panel share channel-volume policy including amplified audio | Shared policy cases and private PulseAudio control fixture | Implemented; 7 private audio fixtures pass |
| F09 | Ordinary module rendering centralizes mode/template/rotation policy; remove dead fallbacks | Format-capable Modules across four edges and rotation modes | Implemented; 704 layout combinations and 36 native screenshot checks pass |
| F10 | Notify owns completion of admitted committed operations despite caller cancellation | Paused sender publication, cancelled waiter, ordered effects/presenter/state updates | Implemented; cancellation/shutdown plus 49 private receipt/configuration checks pass |
| F11 | Replacement uses point lookup; active diagnostics do not decode history | Counting history/query tests and unrelated corrupt-row isolation | Implemented; 70 lifecycle/control/replacement/adapter checks and strict Clippy pass |
| F12 | Center batches reads and avoids repeated full-page serialization/policy reconstruction | Instrumented many-small-item page assembly, exact byte/navigation limits | Implemented; 100 Center/control/history/worker checks pass |
| F13 | Media refresh coalesces dirty owners and stays responsive to controls | Slow/unrelated players, bursts, deadlines and transient metadata failure | Implemented; private controls/burst/read-failure checks and strict Clippy pass |
| F14 | Every advertised fixed v5 interaction fits event framing; editable input has explicit bounded behavior | Form/rich action round trips at escaped byte boundaries | Implemented; 27 protocol and 25 external-panel checks pass |
| F15 | Resolve and implement coherent network status/control backend ownership, update conflicting docs | Private backend appearance/disappearance with open and new panels | Implemented; 44 headless/provider and 12 status checks pass |
| F16 | Banner interaction work has bounded admission and owned shutdown | Blocked publication plus burst, overload, drain/shutdown | Implemented; worker/overload/drain and native AT-SPI checks pass |
| F17 | Lifecycle effects describe every committed mutation; one sender/state projection | Inactive acknowledgement and generic committed-state notification | Implemented; 112 lifecycle/control/cancellation checks pass |
| F18 | Share provider I/O lifetimes/deadlines; headless provider builds exclude host rendering | Common fragmented/slow/close transport fixtures and dependency check | Implemented; 40 headless and 38 desktop host/backend checks plus strict Clippy pass |
| F19 | Direct identity lookup and indexed lightweight layout expansion avoid broad snapshot clones | Ordering/identity regressions and representative work/allocation measurement | Implemented; 9,216 → 0 copied reconciliation entries; 548 App checks pass |
| F20 | Shared desktop setup/readiness helpers and maintained acceptance matrix | Two migrated native fixtures, cleanup/artifacts, manifest coverage | Implemented; drawer and motion fixtures pass at 1.25×; 12-scenario manifest validated |

The verification seams above come from the user-approved audit remediation scope.
Use real interfaces and failures; avoid tests that merely repeat implementation.
For structural performance findings, establish work-count/allocation evidence
before and after rather than claiming an unmeasured latency improvement.

## Final gates

- All 20 rows implemented and verified with authoritative evidence recorded here.
- Domain documentation/ADRs updated for actual interface and lifecycle changes.
- `bash scripts/check.sh` and `bash scripts/check.sh --core` pass.
- Relevant isolated desktop/backend acceptance passes; no live user device changes.
- Review all changes for specification and repository standards; resolve findings.
- Commit completed work on the current branch (implement skill).

## Execution record

- Baseline normal validation passed during audit: 1,788 Rust tests and strict Clippy.
- Remediation started from a clean worktree at the audit revision.
- F01: pending-menu → Panel failed before the fix. One pending-opening slot,
  one Panel tree owner and one interaction type now replace duplicated state.
  Pending-menu → dmenu and exactly-once responder completion also pass among
  545 App tests. The remaining isolated AT-SPI check also passed with private
  socket permissions (the sandbox blocks bind), for 546 App checks in total.
- F06: invalid directives were accepted before the fix. Configuration now uses
  Chrono's fallible output formatter (also rejecting parse-only `%#z`), and cadence
  uses Chrono's parsed items. All 37 clock/config/rendering checks pass.
- F08: the private Pulse fixture reproduced 120%/60% → 100%/50% on bar volume-up.
  Both controllers now use one pure channel policy. All seven real private audio
  fixtures pass, including the same policy table at both entry points.
- F07: a real provider transport and private Shell Media D-Bus fixture reproduced
  an indefinitely stale panel after host input cancelled a delayed Snapshot.
  A persistent dirty flag now retains the refresh obligation. All three media
  provider tests pass, including updating without another Changed signal.
- F14: unsendable advertised actions and oversized local edits reproduced in
  protocol and real provider-session regressions. Fixed-action validation and
  visible editable-input rejection pass all 27 protocol checks (including exact
  escaped-byte boundaries, every rich target, and legacy wire compatibility) and
  25 external-panel checks. A shorter edit clears the error and dispatches normally.
- F03: the failing-expiry fixture reproduced a live banner with zero timer records.
  Explicit firing/retry/failed state now preserves work, detaches the firing task
  before commitment, and reports bounded retry exhaustion through control status.
  All 10 timer-resource tests pass, covering recovery, replacement/shutdown during
  retry, terminal status, explicit completion and paused committed publication.
- F11: both unrelated-active-row corruption blocking replacement and inactive-row
  corruption blocking active diagnostics reproduced before their fixes. Replacement
  now performs one fallible point lookup; the counting History adapter observes one
  point read and zero scans with 1, 64 and 1,024 live rows. Active diagnostics query
  only retained active rows; count-only control uses SQL COUNT without decoding even
  corrupt active content. Retained-only legacy semantics are preserved. All 62
  control/lifecycle/replacement tests and eight binary adapter checks pass. Strict
  Clippy also passes for the changed Notify library, binary and integration tests.
- Focused strict Clippy passed for Shell/protocol and Notify core before F11.
  Foreground's final structural adjustment passed 34 focused tests; all 10 panel
  widget tests and seven control transport/resource tests also passed.
- F10: cancelled action/clear waiters lost publication before the fix. Runtime-owned
  completion now covers all mutations and receipt registration, with one admitted
  coordinator task. Three public cancellation/shutdown tests pass; the larger focused
  run also passes 34 control, five replacement and 10 timer checks. All 49 isolated
  D-Bus contract/receipt, notification receipt and configuration intake checks pass.
  File preparation remains cancellable; installation and configuration status finish
  under owned reload serialization. The complete effect path still preserves sender
  order and bounded external activation preparation.
- F17: inactive acknowledgement returned empty effects after its durable write in
  the new failing regression. Attention changes now have a dedicated effect, with
  one runtime application path and one sender projection decorated by activation
  tokens. All 112 library, lifecycle, control and cancellation checks pass on a
  private bus. Four portal unit checks require that private bus; an earlier sandbox
  run failed only because its socket connection was denied.
- F16: a 128-gesture burst caused 61 concurrent token preparations in
  the failing fixture. The bounded queue/four-worker forwarder passes the burst
  and blocked-publication shutdown tests. Native AT-SPI dispatch through the new
  bounded channel also passes. Strict Clippy passes for the Notify library,
  binary and all tests with desktop features enabled after F10/F12/F16/F17.
- F12: 256 small notifications previously caused 257 reads and 88,038,015
  serialized bytes. The same fixture now uses 17 reads and 679,436 serialized
  bytes including two final exact checks, for a 226,649-byte frame. This is work
  reduction evidence, not an end-user latency claim. A frame-owned reader reuses
  permission maps and transient ordering, and the builder accounts for independent
  fragments with one insertion decision. Exact escaped boundaries, split actions,
  rejected controls, policy snapshot coherence and all real v4/v5 pagination,
  stale-target, history and control checks pass: 100 checks in the private run.
- F05: installed-subscription and inert-navigation regressions both failed before
  the fix. Surface input is now unconditional. Navigation uses configured actions,
  available intrinsic handlers and resolved tooltips, preserving inspectable targets.
  Physical bar close also uses the same retirement path as output removal, including
  origin/tooltip cleanup; the end-to-end fixture covers empty, clock-only and
  network-only bars. Inert dummy navigation fixtures now declare real key actions.
  All 546 App tests, nine input mappings and 24 module checks pass. Strict Clippy
  passes for Shell library, binary and all tests.
- F04/F19: real reconciliation reproduced configured-name aliasing and hot reads
  copied 9,216 reconciliation entries. Structural, shared IDs now distinguish a
  configured name from a typed child key and parent; widget keys use full identity.
  Parent ownership no longer has a second mutable origin map or child-module string.
  Store identity duplicates fail explicitly. Name lookup borrows from the ID, and
  cached ordered-child/configured-name indexes drive Bar and Panel placement.
  Both configured orders, tray/workspace actions, widget keys, removal/reappearance
  and retained focus pass. Two old fixtures encoded separate origin overrides or
  addressed children by formatted strings; they now exercise structural ownership
  and configured-only placement. All 548 App checks and strict Shell Clippy pass.
  Evidence: `/tmp/canopa-remediation-module-{identity-red,lookup-red,identity-green,app-green,clippy}.log`.
- F20: the manifest runner executed the complete migrated drawer and animation
  fixtures at 1.25× in disposable Weston/Niri sessions; both passed and completed
  normal process-group cleanup. Drawer cases cover overlay/push-bar, unchanged
  application geometry, inside/outside input, empty desktop and dmenu; seven motion
  cases cover intermediate frames, stale close, disabled animation, idle CPU and
  restored pixels. Logs, screenshots and results are retained beneath
  `/tmp/canopa-remediation-desktop-20261001/`; its `summary.json` records both exits 0.
  The normal check validates that every supported native verifier has a manifest
  entry; the manual desktop workflow runs all 12 entries.
- F18: blocked Wi-Fi output reproduced the missing deadline. Both v1/v5 now time
  out within the shared publication bound. Three shared I/O fixtures pass for
  fragmented/cancelled receives, owned-reader cancellation, partial writes and
  revision retirement. All 39 headless provider/backend/protocol checks pass,
  including credential edit order, close during work, activation cleanup, audio
  recovery and the media cancelled-Snapshot regression. Strict headless Clippy
  and the dependency guard pass. Desktop host compatibility evidence follows below.

- F13: the private 19-player fixture reproduced a 600 ms control timeout while
  unrelated players refreshed. Inventory now reads bus ownership separately from
  bounded per-owner metadata reads, and commands remain serviceable during refresh.
  One in-flight read plus a dirty obligation coalesces bursts; 101 updates required
  two reads, and unrelated players retained their original one read. Unique read
  identities discard retired completions. Failed reads retain last valid metadata
  and manual selection; owner departure retires it. Controls revalidate capabilities,
  owner and track under the request's remaining deadline. Publication projects state
  only after content/selection changes. The target-timeout fixture confirms no late
  control RPC is issued. All 40 headless and 38 desktop library/host checks pass,
  and strict Clippy passes in both feature profiles. Evidence:
  `/tmp/canopa-remediation-media-{refresh-red,refresh-green,registry-green,headless-green}.log`
  and `/tmp/canopa-remediation-providers-{desktop-green,headless-clippy,desktop-clippy}.log`.
- F18 complete: the default desktop host still passes all nine v4 compatibility,
  input and bounded-image checks, plus 29 desktop library/backend checks. The new
  headless guard includes dev dependencies. Cargo lock/attribution inventory updated.
- F02: the private NetworkManager fixture reproduced sending credentials to another
  SSID/security after a selected AP path was reused. Opaque backend-issued
  selections now bind daemon/device/network identity.
  AP paths may roam within that identity but cannot redirect a stale selection.
  Credential-agent callbacks recheck owner, network properties and device, and
  cleanup stays pinned to the original daemon. Provider synchronization retires
  stale prompts and secrets. The prior edit-order fixture seeded a prompt before
  startup scan; it now opens one through actual host input. Fourteen focused
  Wi-Fi checks passed, followed by 34 headless library and ten provider checks.
  Evidence: `/tmp/canopa-remediation-wifi-identity-{red,green}.log` and
  `/tmp/canopa-remediation-network-headless.log`.
- F09: a real Iced layout test reproduced identical horizontal widths with and
  without explicit rotation. Ordinary modules now supply one presentation to a
  shared layout/style/rotation boundary; the App has one Bar entry point. Rich
  visuals remain Module-owned. Dead plain-text render branches were removed.
  The regression and all 704 combinations pass: 11 format-capable module kinds,
  four edges, four rotation policies and four format-inheritance cases. The matrix
  preserves fixed workspace pill geometry and unformatted fallback behavior.
  Native glyph acceptance passes all 36 screenshot checks at 1.0× and 1.25×,
  including 12 ordinary-module rotation/orientation checks alongside focused-window
  title replacement, icon preservation and glyph antialiasing.

- F15: all 12 status checks pass, including one observer switching from absence
  to networkd, NetworkManager, fallback, absence and recovery. The headless
  provider suite also passed open/new-panel selection and owned cancellation.
  Backend documentation now agrees with the common live resolver.
  Evidence: `/tmp/canopa-remediation-network-status.log`.
- Independent standards/spec review: private regressions reproduced F13's response-order tie and
  F16's detached dispatcher on cancelled shutdown. Both fixes are implemented.
  F12 was retracted after checking `NotificationCenterCursor::for_thread`: its
  creation timestamp and ID are immutable, and the interleaved replacement test
  already preserved monotonic order. Retain that regression; no speculative
  pagination production change is needed. Confirmed review totals: Standards 1,
  Spec 2 (two unique defects). Both reviewers rechecked the fixes and reported
  no remaining findings. Regression execution is included in final validation.

- The first full run passed all 1,237 Shell binary tests, including the 704-case
  rendering matrix, and the reviewed media-selection and cancelled-shutdown
  regressions. It exposed a provider process-lifetime interaction: read-ahead after
  Close started an uncancellable Tokio stdin read. Explicitly closing the host pipe
  immediately released the process. The shared input boundary now delivers a
  terminal frame and stops reading. Both Wi-Fi wire versions and new real audio/media
  subprocess regressions exit successfully while host stdin remains open. All
  headless provider targets pass; both reviewers also rechecked this correction.
  Evidence: `/tmp/canopa-remediation-full-initial.log` and
  `/tmp/canopa-remediation-provider-close-{red,green}.log`.

## Final validation

- `bash scripts/check.sh`: passed. Recorded 1,823 successful workspace Rust test
  executions and four vendor tests, along with formatting, strict workspace Clippy, dependency attribution,
  contrast, protocol/dependency boundaries, updater and service-unit checks,
  configuration validation and private settings-portal acceptance.
  Log: `/tmp/canopa-remediation-final-check.log`.
- Native manifest runner: `drawer-modes`, `panel-motion` and `content-rotation`
  all passed. Artifacts and exit-status summary:
  `/tmp/canopa-remediation-desktop-final-20261001/`.
- `bash scripts/check.sh --core`: passed, with 448 successful test executions
  (including subprocess fixtures and doctests). Core and provider dependency guards
  confirm GUI/audio and host-rendering exclusion respectively.
  Log: `/tmp/canopa-remediation-final-core.log`.
- Standards review: no remaining findings after the shutdown correction and
  terminal-input follow-up. Spec review: no remaining findings after initial media
  selection, shutdown and terminal-input corrections. The Center cursor concern
  was retracted; its unchanged behavior has a passing regression.
