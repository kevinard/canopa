# Project completion follow-up — 2026-09-21

Scope: the four remaining items in the [completion brief](../../.scratch/notify/project-completion/PRD.md).
The static Notifications 1.3 feature scope is unchanged. Review started from
Notify `80f2134` and Niribar `e4280a1`.

## Storage failure coverage

- `killed_migration_rolls_back_and_restarts_with_history_and_policy_intact` holds a
  real SQLite reader lock while the executable writes a v2 migration rollback
  journal, then sends SIGKILL. SQLite restores schema v1; executable retry migrates
  successfully and preserves notification identity/content, pending attention,
  DND and user rules. The restart also recovers process locks and a stale socket.
- `interrupted_import_preserves_source_and_retries_without_partial_copies` limits
  the child process's file size with `prlimit`. SIGXFSZ terminates a real `VACUUM
  INTO` snapshot write after data reaches the temporary file. The source remains
  byte-identical, a partial copy is never published, and retry preserves content
  and policy without restoring sender actions.
- This exposed a defect: retry left the interrupted temporary copy behind. Imports
  now use a reserved `.history-import-*` namespace. The next default-path startup
  reclaims owned regular snapshots and sidecars while holding the destination
  lock, including after successful publication or removal of the old source.
  Separate tests preserve unrelated files and reject symlinks without deleting
  their target. Generic temporary files from older versions are not removed.
- A competing SQLite `BEGIN IMMEDIATE` produces actual error code 5 for receipt,
  replacement, dismissal, DND and user-rule writes. Failed operations preserve the
  public snapshot, sender events and policy. Releasing the lock permits retry;
  restart verifies persistence of the successfully retried changes.

All nine focused storage/import tests pass. These are process interruption and
lock-contention tests; they do not certify power-loss or filesystem durability.

## Configuration failures under load

The unchanged 11-test configuration suite passed five repetitions with four CPU
workers (55 test executions), and five during a Niribar build/Notify Clippy run
(another 55). Durations were 23.94–31.00 seconds with CPU workers and 15.98–19.54
seconds with build load. [CPU-load results](evidence/completion-2026-09-21/configuration-cpu-load.json)
and [build-load results](evidence/completion-2026-09-21/configuration-build-load.json)
include Linux pressure measurements.

An initial concurrent release build reproduced process-startup deadline failures
in the legacy-import suite: children were killed before producing diagnostics.
Those tests passed after the competing build stopped. This is evidence of a
broader startup/resource-pressure problem, but does not establish the exact cause
of the earlier configuration/FIFO failures. No production or test deadline was
relaxed. The original configuration-specific failure remains unreproduced.

`scripts/stress-configuration.py` makes further repetitions reproducible using a
compiled suite, private buses, bounded CPU workers and per-run logs. SIGTERM
cleanup was exercised after two seconds; its [partial result](evidence/completion-2026-09-21/configuration-cancel.json)
records interruption and the script terminates its workers and test process group.

## Builds and local checks

- Default-feature private-bus workspace suite: **435 passed, 0 failed, 0 ignored**.
  [Result](evidence/completion-2026-09-21/full-tests.json).
- Core-only private-bus workspace suite: **336 passed, 0 failed, 0 ignored**.
  [Result](evidence/completion-2026-09-21/core-tests.json).
- All-target Clippy with `-D warnings`, formatting, whitespace and the core
  dependency boundary pass. The shared panel protocol exactly matches Niribar.
- Notify was built with Rust 1.95.0, `--release --locked --offline -j1`.
  Niribar was built with its existing development profile and `--locked --offline
  -j1`; its 16 pre-existing warnings remain. Its lockfile needed one stale
  `serde_json` dependency entry removed from the vendored protocol package,
  committed in Niribar as `0dbd269` (`Fix locked Canopa protocol dependency graph`).
- The actual paired binaries passed the disposable Niri 26.04/Sway 1.12 fixture:
  native banner action, opening the host Center, native Center action and image
  fallback. [Result](evidence/completion-2026-09-21/desktop-result.json),
  [disposable desktop](evidence/completion-2026-09-21/desktop.png).
  This run does not repeat the separately recorded Orca acceptance.

Two compilation attempts exhausted disk space before tests ran. Superseded
project Cargo outputs were removed and compilation resumed. Temporary release
compilation caches were also cleared to release tmpfs memory; the optimized
binary was retained. Fixture socket-denial attempts inside the sandbox were
rerun outside it and are not counted as test results.

## Live rollout

Both binaries are installed in `~/.local/bin`. `canopa-notify.service` and
`niribar.service` are enabled and running under the active
`graphical-session.target`; both were restarted successfully. Dunst is disabled
and masked. The original custom Dunst unit, Niribar configuration and service
states are backed up in
`~/.local/state/canopa-rollout/20260921-213137`.

The provider now explicitly selects `canopa-v4` and uses installed Notify plus
the absolute `/run/user/1000/canopa-notify/control.sock` path. The old argv string
contained an unexpanded `$XDG_RUNTIME_DIR`. Both panel preferences use height 720.
A labeled Notifications button was added to the top bar; the previous left-bar
button displayed only `>`, now `N`.

Live libnotify and Gio notifications reached the installed daemon. The session's
accessibility bus initially advertised a socket that refused connections; its
service and the installed pair were restarted. The user confirmed that the new
labeled top-bar button opens the Center. A revision-bound synthetic notification
link opened `https://example.org/` through the live OpenURI portal; a monitor
restricted to this daemon captured response code 0, and status returned zero
pending requests with no opener error. Synthetic test notifications were recalled
or acknowledged. [Live results and binary hashes](evidence/completion-2026-09-21/live-result.json).

During concurrent builds, one diagnostic Center projection timed out at five
seconds and the host was slow to open. Linux I/O pressure was high (full avg60
29.33%, memory full avg60 0%). The diagnostic was repeated with a 30-second
client allowance and passed; this does not establish a live latency bound and
production deadlines were not changed. The labeled button and larger reading
area resolve the user-reported missing/broken Center for this session.

A real logout/login has not been performed; service restart
and target wiring are not recorded as fresh-session evidence.

## Hosted CI and backlog

The user confirmed that there is no GitHub repository yet. No hosted run or
registered graphical runner is claimed. [Setup instructions](hosted-ci.md),
read-only runner preflight, manual dispatch for both workflows, a pinned Rust
1.95.0 toolchain and serialized desktop jobs are prepared. Repository creation,
runner-machine selection/registration and passing run URLs remain open.

Older policy/reload tickets and PRD stories now follow ADR 0027's future-request
semantics. The action ticket and domain glossary expose all accepted named
actions under ADR 0048. Historical ticket filenames/comments remain; completion
package statuses now match the final selected-profile implementation tracker.

Independent Standards and Spec reviews found two issues in the new tooling:
inconsistent AT-SPI executable lookup and SIGTERM cleanup bypass. Both were fixed
and rechecked: **Standards 0 outstanding; Spec 0 outstanding**.
