# W07: Bound requests, diagnostics, parser, assets, and task growth.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W07. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-10: Continuing from `4eaa158` with text/action admission budgets and bounded markup parsing at the approved daemon/D-Bus seams. ADR 0031 records rejection versus safe-text degradation. Aggregate assets, active resources, workers and provider-stream budgets remain outside this tranche.

2026-09-10: Input-budget continuation implemented and independently reviewed against `df88ecd`. All 41 focused tests and 267 workspace tests pass (11 new), with zero failures or ignored tests. Typecheck, Clippy, formatting, whitespace and Niribar protocol drift checks pass with existing warnings. Regressions cover atomic over-limit rejection, D-Bus errors/ID preservation, exact byte/count boundaries, malformed/complex markup and UTF-8-safe legacy backfill without rewriting stored text. See [INPUT-BUDGET-REVIEW.md](../INPUT-BUDGET-REVIEW.md). This issue remains in progress for aggregate active resources, assets, workers and provider streams.
