# Compositor-sized surface continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked ADR 0045 and evidence artifacts. This tranche began at `559cdfa`. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`, `6260b01`, `aae365b`, `77d3aaf`, `60d733f`, `6397b9e`, `1bf5698`, `7ce2a0f`, `c681dfe`, `559cdfa`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewed the pending compositor-sized surface tranche and relevant cumulative presentation/runtime interactions without edits or builds/tests, then rechecked final evidence/documents and screenshots with zero new findings. Neither certifies W10/W11 or full-plan completion. All 102 focused presenter library/integration tests and 390 final private-bus workspace tests pass, with no failures or ignored tests. Typecheck, Clippy, formatting, whitespace and protocol-drift checks pass with existing warnings.

The initial full run stopped at an unchanged icon-index test's one-second elapsed assertion (125 library tests passed, one failed). It passed ten unchanged isolated runs at 0.05–0.06 seconds, followed by the successful full rerun above. The initial cause is not established; no production code, test deadline or assertion was changed for the rerun.

## Standards

No actionable Standards findings in the compositor-sized surface tranche or relevant cumulative interactions since `df88ecd`.

Geometry and pointer-region ownership remain private to the presenter; lifecycle and timer ownership remain unchanged. ADR 0045 explicitly supersedes the old measured-surface strategy and records the larger transparent-buffer trade-off. Shared latest-region callbacks prevent deferred delivery from restoring obsolete bounds without leaking geometry into the domain.

Tests exercise actual renderer layout, Iced tasks and Wayland-region wire requests. The test-only dependency exposes the existing renderer-consumer seam and introduces no new locked package/version. No actionable baseline smell was found.

Standards: **0 documented-standard violations; 0 actionable smell findings.** Worst issue: none.

## Spec

No actionable scoped Spec findings.

The change matches W10 measured layout and no-focus behavior, and ADR 0045 compositor-owned geometry. Cards remain top-right and width-capped; scrolling and captured acknowledgement ownership remain intact. Input covers the visible stack, clears old extent and resolves current bounds when deferred callbacks execute. The buffer trade-off is documented; no unrelated production dependency or capability expansion was found.

W10's requirement that graphical fixtures validate frame/display timing, output fallback and interaction hit regions still calls for acceptance evidence. The fresh [graphical run](OUTPUT-FIT-EVIDENCE.md) covers constrained-output timing, tested scales, actions, focus and click-through; multiple-output fallback, broad scale/long-label coverage and full keyboard/accessibility remain explicitly unfinished.

Spec: **0 findings; no worst issue.**

Summary: Standards — 0 findings, worst none; Spec — 0 scoped findings, worst none. W10/W11 remain incomplete.
