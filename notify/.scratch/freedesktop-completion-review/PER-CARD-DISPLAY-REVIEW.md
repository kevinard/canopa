# Per-card display continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked `presentation/display.rs` and ADR 0044. This tranche began at `c681dfe`. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`, `6260b01`, `aae365b`, `77d3aaf`, `60d733f`, `6397b9e`, `1bf5698`, `7ce2a0f`, `c681dfe`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewed the pending per-card tranche and relevant cumulative presentation/runtime interactions without edits or builds/tests. Neither certifies completion of W10/W11 or the full plan. Separately, all 100 focused presenter library/integration tests and all 388 final private-bus workspace tests pass, with no failures or ignored tests. Typecheck, Clippy, formatting, whitespace and protocol-drift checks pass with existing warnings.

## Standards

No Standards findings in the per-card-display tranche or its reviewed cumulative interactions since `df88ecd`.

The changes preserve documented ownership: viewport geometry remains private to presentation; timeout policy, registration and revision validation remain in the Notification Runtime (CONTEXT.md; ADRs 0028/0030/0033). Captured acknowledgements execute outside their lock and cannot clear a newer projection's pending callbacks.

The widget delegation is necessary Iced integration, not an actionable Middle Man smell. The callback map supports existing whole-presentation consumers alongside the new per-card path without exposing timer policy. ADR 0044 records the containment threshold and explicitly excludes physical-output acceptance.

Issue comments follow the append-only convention; compliance and verification updates distinguish software evidence from earlier graphical evidence.

Standards: **0 documented-standard violations; 0 actionable smell findings.** Worst issue: none.

## Spec

No scoped Spec findings.

The per-card change matches ADR 0044: "the renderer reports each card when its full compact layout is inside the drawing viewport." Independent callbacks preserve unrelated pending cards, tolerate the documented scroll rounding, and retain captured revision/generation ownership. Runtime registration checks preserve ADR 0033's requirement that acknowledgements "may start an existing pending record, never insert or reconstruct one."

The observer delegates layout, scrolling, input, operations and overlays without conditionally changing widget structure. Consumed callbacks cannot restart timers; older frames cannot clear a newer projection's pending acknowledgement.

No unrequested scope expansion found. Physical output fit, oversized cards, supported scales and complete graphical/keyboard acceptance remain explicitly deferred—not falsely claimed complete. Existing synchronous whole-presentation acknowledgement remains supported.

Spec: **0 findings; no worst issue.**

Summary: Standards — 0 findings, worst none; Spec — 0 scoped findings, worst none. W10/W11 remain incomplete.
