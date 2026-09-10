# Completion plan execution

Baseline: `d3f12df1888acbeb1cf05fd81baa00fb500e6a89`.

Status: in progress. Implements [PLAN.md](PLAN.md). Tests use the plan's executable/D-Bus, Notification Lifecycle/History, and presentation interfaces. Native portal v2 (W15) remains a separately selected optional target.

| Package | State | Evidence / remaining work |
| --- | --- | --- |
| W01 | In progress | Regression tests and private-bus/CI baseline. |
| W02 | In progress | Duplicate-startup and XDG-state regressions pass; socket/database ownership guards and recoverable legacy import implemented; remaining permission/import/readiness verification. |
| W03 | In progress | D-Bus ID collision regression reproduced then fixed; no history alias fallback or revival of closed rows; concurrent fixed-token regression reproduced and resolution moved into the receipt boundary; ADR 0029 records compatibility boundary. Typed registry and retirement remain. |
| W04 | In progress | Revision race reproduced then fixed; mutation/effect coordinator added; activation-token acquisition follows non-mutating action validation and precedes revision-guarded commitment. Rejected-action regression passes. Banner UI callback revisions remain. |
| W05 | In progress | Renderer failure regression reproduced and fixed. 130-closure loss reproduced then fixed with ordered bounded transport publication and synchronous ID invalidation. Transport/presentation error exposed; coalescing/readiness/recovery remain. |
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

## Verification of this tranche

- Final Notify private-bus suite: 236 tests passed (including 17 executable D-Bus tests), no failures or ignored tests. Command: `bash scripts/test.sh`. Temporary run log: `/tmp/canopa-notify-final-tests.log`.
- Notify workspace/all-targets typecheck and Clippy passed. Formatting and whitespace checks passed in both repositories. Existing lint and test-helper dead-code warnings remain; this is not a warning-free baseline.
- Final applied Niribar suite: 1,141 tests passed, no failures or ignored tests, including the new process transport round trip, invalid-frame checks, image/keyboard tests and existing Wi-Fi provider checks. Command: `cargo test --offline --locked` in Niribar. Temporary run log: `/tmp/niribar-final-tests.log`.
- Canonical protocol source matches the Niribar vendor snapshot via `bash scripts/check-panel-protocol.sh /home/kevinard/projects/niribar`.
- Independent Standards and Spec reviews completed; their three documented mismatches were corrected and independently rechecked in the actual worktrees. See [IMPLEMENTATION-REVIEW.md](IMPLEMENTATION-REVIEW.md).
- No running desktop service or bar was replaced, no configuration was installed into the user's session, and no new freedesktop capabilities were advertised. The graphical acceptance checklist remains outstanding.
