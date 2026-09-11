# W07: Bound requests, diagnostics, parser, assets, and task growth.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W07. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-10: Continuing from `4eaa158` with text/action admission budgets and bounded markup parsing at the approved daemon/D-Bus seams. ADR 0031 records rejection versus safe-text degradation. Aggregate assets, active resources, workers and provider-stream budgets remain outside this tranche.

2026-09-10: Input-budget continuation implemented and independently reviewed against `df88ecd`. All 41 focused tests and 267 workspace tests pass (11 new), with zero failures or ignored tests. Typecheck, Clippy, formatting, whitespace and Niribar protocol drift checks pass with existing warnings. Regressions cover atomic over-limit rejection, D-Bus errors/ID preservation, exact byte/count boundaries, malformed/complex markup and UTF-8-safe legacy backfill without rewriting stored text. See [INPUT-BUDGET-REVIEW.md](../INPUT-BUDGET-REVIEW.md). This issue remains in progress for aggregate active resources, assets, workers and provider streams.

2026-09-10: Continuing from `cf449ee` with aggregate live count/text/raw-image admission and commit-aware cleanup under ADR 0032. Rejected requests must preserve existing work, replacements receive existing-footprint credit, and successful terminal effects release capacity. Asset decoding, workers, provider streams and timer-tombstone reclamation remain separate.

2026-09-10: Live-resource continuation implemented and independently reviewed against `df88ecd`, with zero scoped findings. All eight new focused regressions and 275 workspace tests pass, with zero failures or ignored tests. Typecheck, Clippy, formatting, whitespace and Niribar protocol drift checks pass with existing warnings and no new warnings. Coverage includes exact byte/count boundaries, replacement credit, failed storage writes, committed partial-clear cleanup, concurrent last-slot admission, resident actions, restart-loaded stale history and D-Bus error/ID preservation. See [LIVE-RESOURCE-REVIEW.md](../LIVE-RESOURCE-REVIEW.md). W07 remains in progress for assets, workers, provider streams and safe timer-tombstone reclamation.

2026-09-10: Continuing from `9d62fcf` with safe timer-record reclamation under ADR 0033, verified at the existing daemon/control and presentation seams. This removes lifetime cancellation tombstones without allowing old display callbacks to recreate timers. Asset decoding, sound workers, provider streams and graphical/recovery acceptance remain separate.

2026-09-10: Timer-resource continuation implemented and independently reviewed against `df88ecd`, with zero scoped findings. All 58 focused tests and 280 workspace tests pass, with zero failures or ignored tests. Typecheck, Clippy, formatting, whitespace and Niribar protocol drift checks pass with no new warnings. Seven public-seam regressions replace two private implementation tests and verify churn reclamation, late/duplicate frames, concurrent acknowledgement/closure, registration bounds, failed writes, partial clear and delayed sender publication. JSON control status now exposes content-free `timeout_records`; plain-text availability output is unchanged. See [TIMER-RESOURCE-REVIEW.md](../TIMER-RESOURCE-REVIEW.md). Asset decoding, worker/provider-stream bounds and broader failure recovery remain unfinished.

2026-09-11: Continuing from `622201a` with sound job/source admission, generation cancellation and file/PCM budgets under ADR 0034. Tests use the existing public presenter/daemon seams with real offline mixer output; no audio device is opened. Sound overload remains best-effort and preserves notification receipts, retained/transient history behavior and sender closures. Sound-theme traversal/semantics, image decoding, provider streams and broader recovery remain separate work.

2026-09-11: Sound-resource continuation implemented and independently reviewed against `df88ecd`. All 23 focused sound tests and 292 workspace tests pass, with zero failures or ignored tests. Typecheck, Clippy, formatting, whitespace and Niribar protocol drift checks pass with existing warnings and no new warnings. Both reviewers identified the same test-synchronization race; the corrected four-voice fixture was independently rechecked and rerun, leaving zero outstanding scoped findings. See [SOUND-RESOURCE-REVIEW.md](../SOUND-RESOURCE-REVIEW.md). This issue remains in progress for image decoding, sound-theme traversal, provider streams and broader resource/recovery acceptance.
