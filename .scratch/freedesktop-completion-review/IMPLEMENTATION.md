# Completion plan execution

Baseline: `d3f12df1888acbeb1cf05fd81baa00fb500e6a89`.

Status: in progress. Implements [PLAN.md](PLAN.md). Tests use the plan's executable/D-Bus, Notification Lifecycle/History, and presentation interfaces. Native portal v2 (W15) remains a separately selected optional target.

| Package | State | Evidence / remaining work |
| --- | --- | --- |
| W01 | In progress | Regression tests and private-bus/CI baseline. |
| W02 | In progress | Duplicate-startup and XDG-state regressions pass; socket/database ownership guards and recoverable legacy import implemented; remaining permission/import/readiness verification. |
| W03 | In progress | D-Bus ID collision regression reproduced then fixed; no history alias fallback or revival of closed rows; concurrent fixed-token regression reproduced and resolution moved into the receipt boundary; ADR 0029 records compatibility boundary. Typed registry and retirement remain. |
| W04 | In progress | Revision-safe lifecycle and ordered mutation/effects implemented. Banner widgets and asynchronous token callbacks now carry captured thread/revision targets; stale activation, dismissal and reused action keys are rejected. Delayed cancelled-revision acknowledgements and current resident actions have regression coverage. Remaining command/interleaving and graphical acceptance evidence is still open. |
| W05 | In progress | Ordered bounded sender publication and ID invalidation implemented. Banner updates now coalesce to one latest projection; a paused renderer recovers the final state after a burst. Pending display callbacks are bounded and queued frames retain their own acknowledgement. Graphical startup readiness, proactive health, restart recovery and full shutdown/transport-failure evidence remain. |
| W06 | In progress | Atomic RETURNING/write/prune transactions and partial clear preserve committed effects; structured partial-clear counts verified over the control socket. Transactional version-1 baseline migration, future-schema rejection, private database permissions, and focused active queries implemented. Twelve adapter upgrade/storage tests pass; interruption/backfill-scale verification remains. |
| W07 | In progress | Raw channel narrowing, bounded diagnostics, 64 KiB control frames and 64 concurrent control clients implemented with regressions. Asset, parser, active-resource and provider-stream budgets remain. |
| W08 | In progress | Installed XDG configuration, transactional reload, configuration health/generation, typed status and durable DND CLI implemented; command and failure-path verification continues. |
| W09 | Pending | Safe asset/theme resolution. |
| W10 | Pending | Reachable actions and banner geometry. |
| W11 | Host port implemented; graphical verification pending | Applied to `/home/kevinard/projects/niribar`, baseline `d7870c2d20cff5d0f84da03296e15bf65115cc90`. Explicit Canopa-v3 transport, shared source snapshot/drift check, sections/controls/metadata/actions, b/i/u, bounded local images, generation-safe interactions and keyboard selection implemented. Existing framed-v1 Wi-Fi protocol retained. See `docs/notification-center-verification.md` for required graphical evidence; screen-reader integration remains open. |
| W12 | Pending | Links; external host verification required before advertisement. |
| W13 | Pending | Persistence contract and implementation. |
| W14 | Pending | Packaging, architecture, CI, final verification. |
| W15 | Optional, not selected | Native portal v2 backend. |

## Initial tranche verification (`df88ecd`, Niribar `b5d1b54`)

- Final Notify private-bus suite: 236 tests passed (including 17 executable D-Bus tests), no failures or ignored tests. Command: `bash scripts/test.sh`. Temporary run log: `/tmp/canopa-notify-final-tests.log`.
- Notify workspace/all-targets typecheck and Clippy passed. Formatting and whitespace checks passed in both repositories. Existing lint and test-helper dead-code warnings remain; this is not a warning-free baseline.
- Final applied Niribar suite: 1,141 tests passed, no failures or ignored tests, including the new process transport round trip, invalid-frame checks, image/keyboard tests and existing Wi-Fi provider checks. Command: `cargo test --offline --locked` in Niribar. Temporary run log: `/tmp/niribar-final-tests.log`.
- Canonical protocol source matches the Niribar vendor snapshot via `bash scripts/check-panel-protocol.sh /home/kevinard/projects/niribar`.
- Independent Standards and Spec reviews completed; their three documented mismatches were corrected and independently rechecked in the actual worktrees. See [IMPLEMENTATION-REVIEW.md](IMPLEMENTATION-REVIEW.md).
- No running desktop service or bar was replaced, no configuration was installed into the user's session, and no new freedesktop capabilities were advertised. The graphical acceptance checklist remains outstanding.

## Continuation from `df88ecd`

Scope: W04 revision-safe banner interactions and W05 coalesced presentation/display acknowledgement lifetimes. No protocol or Niribar source changes; no new advertised capabilities.

- Reproduced a queued action invoking replacement content under the same action key; added revision-bearing Banner Targets and one daemon interaction handler.
- Reproduced renderer queue overflow during 130 updates; replaced the FIFO with a bounded latest-state stream.
- Reproduced retention of 130 undisplayed acknowledgement closures; retain only the latest pending acknowledgement, with already queued frame events owning their original callback.
- Fixed early renderer-exit cleanup and the review-discovered concurrent publication/retirement race; a deterministic blocked-retirement regression now passes.
- Focused verification: 44 presentation integration tests and 18 renderer-adapter tests pass. Workspace/all-targets Clippy, formatting, whitespace and protocol-snapshot drift checks pass; existing lint warnings remain.
- Independent Standards and Spec reviewers rechecked the corrected race and found no remaining scoped defect. See [CONTINUATION-REVIEW.md](CONTINUATION-REVIEW.md).
- Final private-bus workspace suite: **245 passed, 0 failed, 0 ignored** (nine more tests than the initial tranche), via `bash scripts/test.sh`. Temporary run log: `/tmp/canopa-notify-continuation-final-tests.log`. No desktop services were restarted or installed.
