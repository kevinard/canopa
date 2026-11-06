# Session locking, interactive authentication, and lockscreen composition

Status: complete (implementation and scoped acceptance); the full workspace test
run has the unrelated failures recorded below.

## Required outcomes

1. An event-driven user companion handles the actual logind session's Lock signal,
   including loginctl lock-session/lock-sessions. Direct and requested locks report
   LockedHint only after compositor lock acknowledgement, and clear it only after
   confirmed authenticated unlock. Crashes retain the locked hint. Session discovery
   checks ownership and avoids guessing among multiple graphical sessions.
2. Interactive PAM conversations support successive password/PIN/OTP prompts,
   visible prompts, hardware-touch/fingerprint informational messages, and successful
   device authentication without a typed password. PAM and account policy remain
   authoritative. Authentication runs in one cancellable child process; cancellation,
   timeout, worker death and stale responses never unlock or reuse a password.
3. Configuration places clock/date/authentication and optional widgets in named
   viewport regions, orders them, and controls spacing/alignment. Authentication
   remains present and usable at supported sizes. Existing configurations keep a
   useful default layout; preview exercises the same geometry.
4. Named custom widgets use explicit argv/string commands, interval, timeout,
   bounded output and formatting/style. They are passive, share reusable data-side
   execution code, never receive credentials, and stop when the locker exits.
   Validation/preview do not execute configured commands by default.

## Evidence required

Focused parser/layout/IPC/auth tests, private D-Bus logind fixture tests, isolated
PAM scenarios for multiple factors and fingerprint-style no-password completion,
private compositor lock/readiness/state/crash tests, custom-widget lifecycle tests,
visual previews at varied sizes/themes, packaging/unit checks and the workspace gate.
No real session locking, host PAM changes or hardware enrollment is performed.

## Implementation

- `canopa-lock daemon` and its Type=notify/KillMode=process user service subscribe
  to the real owned logind session path, with explicit/PID/unambiguous discovery,
  listener exclusion, reconnect backoff and compositor-confirmed hints. New lock
  requests during authenticated helper cleanup are preserved; duplicate requests
  during an active lock are coalesced. Direct lockers report their own state too.
- Interactive PAM runs in a cancellable child with one private socket and distinct
  responses per challenge. Authentication, account validation and PAM teardown
  must succeed. Cancellation, timeout, worker death and stale generations stay
  locked. Parent-death/process-group cleanup prevents blocked worker leaders from
  surviving the locker. No host PAM configuration was edited.
- Nine layout anchors, ordered row/column contents, bounded spacing/offsets,
  authentication placement validation and overlap priority use the same planner
  for preview and the live lock. Existing layouts retain their defaults.
- Custom passive script widgets share a bounded command observer in
  `canopa-widgets`; argv, interval, timeout, format, font size, color and preview
  placeholders are configurable. Preview/check never execute commands. Commands
  start after locking, stop with the consumer and never inherit auth descriptors.
- Setup, PAM control-flow guidance, architecture, packaging, updater and the native
  acceptance matrix include the new listener and widget/layout contract.

## Validation

- Focused Rust checks: 7 session library tests, 5 configuration/preview tests and
  2 shared widget tests passed. These cover secret erasure, fresh mixed-style PAM
  prompts, invalid replies, image/IPC bounds, geometry, custom parser rejection,
  config-relative argv/cwd and command-free preview.
- All 20 updater tests, 8 check-selection tests, 8 acceptance-runner tests, systemd
  graph/order checks (including notify readiness and active-lock preservation),
  Settings portal activation/reads/reload signals passed. Packaged documentation:
  30 files and 46 working offline links. Python/C/shell syntax checks passed.
- Strict full-workspace Clippy, formatting, fork provenance, panel isolation, theme
  contrast and generated dependency inventory passed. The first workspace gate
  stopped on new script-return/renderer-argument Clippy findings; those were fixed
  and the remaining gate is being run separately, without repeating updater builds.
- Native private Weston/Niri: all 14 wallpaper/lock regression checks and all 23
  logind/PAM/widget checks passed. Real loginctl transport targets a test-owned
  D-Bus service; PAM policies/modules are mounted only inside bubblewrap. Includes
  OTP, ECHO_ON, device-only fingerprint-style success, touch/cancel, timeout, worker
  death, logind restart, Unlock refusal, locker crash, listener stop, direct hint
  reporting, command cleanup and preview placement. See `evidence/summary.json`.
- Inspected dark 1920x1080, light 3840x2160 and small 640x480 previews under `visuals/`.
  The updated wallpaper idle sample measured 13,048 KiB RSS and zero ticks in two
  seconds (one virtual output, debug build; not a release benchmark).

The final native recheck passes all 23 scenarios after the PAM teardown/lint fixes
and the additional stale-hint ownership regression. A crashed child leaves the bus
hint intact but retires its listener's private restoration state; a direct recovery
locker can unlock without an old listener resurrecting true on a later logind
restart. `native-final.log` and `binaries.sha256` record the run/artifacts.

The full workspace gate is **not green**. Its test continuation used
`dbus-run-session -- cargo test --workspace --all-targets --locked --no-fail-fast`
so the initial Files failure did not hide downstream checks. Five targets fail:

1. Files `recovery_catalog`: `pages_remain_loaded_and_saved_identity_survives_active_undo_expiry`
   times out with “Recovery catalog did not settle” (3 siblings pass). This is the
   timeout already observed during the preceding wallpaper feature.
2. Files `undo_unavailable_moves`: 7 tests fail with “Saved return differs from its
   original Undo history” or a consequent expected-kill assertion (5 pass, 1 ignored).
   `an_external_move_does_not_let_older_copy_undo_own_an_independent_restored_destination`
   reproduces alone in the already-built named target in 0.06 seconds; see
   `files-undo-isolated.log`.
3. Files `undo_unavailable_saved`: `source_only_replace_preserves_newer_original_work_while_saved_restoration_remains_pending`
   fails with “Undo return work differs from its original history step”
   (8 pass, 1 ignored).
4. Notify's native AT-SPI unit fixture: the host launcher rejects
   `--launch-immediately --a11y=1` and its accessibility connection times out
   (154 other Notify unit tests pass).
5. Shell's native Center accessibility fixture: the same host launcher rejection,
   then “AT-SPI never exposed Unread” (1268 other Shell binary unit tests pass).

No Files or Notify implementation files, Files dependency versions or failing
accessibility fixture sources changed. The Undo failures were newly exposed by
continuing beyond the earlier gate's first failure; they are recorded, not claimed
fixed or used as evidence of full workspace/native acceptance. See
`workspace-tests.log`; successful tooling/initial gate output is in
`workspace-check.log`, with final strict lint in `clippy.log`, focused tests in
`focused-final.log`, and vendor tests in `vendor-tests.log`. Settings portal,
configuration migration/isolation, generated attributions, formatting and script
syntax checks also pass. The final PAM-missing unit condition and packaged guide
were rechecked with the systemd graph and offline documentation validators.

## Acceptance boundaries

Physical token/fingerprint devices, enrollment and a distribution's actual PAM
policy are not tested by synthetic module fixtures. The locker implements the PAM
conversation and respects the installed policy; it does not race independent PAM
stacks or change expired passwords. Explicit user configuration may run trusted
commands; deliberately detached command descendants own their own lifecycle.
No live session/unit installation or host PAM changes were performed.
