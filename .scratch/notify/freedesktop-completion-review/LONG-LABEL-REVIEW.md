# Long-label continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked evidence/review artifacts. This tranche began at `2394849`. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`, `6260b01`, `aae365b`, `77d3aaf`, `60d733f`, `6397b9e`, `1bf5698`, `7ce2a0f`, `c681dfe`, `559cdfa`, `2394849`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewed the current wrapping changes and relevant cumulative geometry/acknowledgement interactions without edits or builds/tests, then rechecked final documents and all four screenshots with zero new findings. All 104 focused tests and 392 final private-bus workspace tests pass with no failures or ignored tests. Workspace/all-targets typecheck, Clippy, formatting, whitespace and protocol-snapshot checks pass with existing warnings.

The initial full-suite command terminated during compilation with exit 143, before test results. Its cause was not established; the unchanged rerun produced the full pass above. No source, assertion or deadline was changed for the rerun.

## Standards

No actionable Standards findings in the long-label tranche or its relevant cumulative interactions since `df88ecd`.

Wrapping remains private to banner presentation, consistent with daemon ownership (ADR 0005) and deep-module boundaries (`CONTEXT.md`). Action callbacks retain opaque keys and captured Banner Targets (ADR 0030). Natural height continues through compositor-constrained scrolling and full-card display acknowledgement (ADRs 0044–0045); no lifecycle or timeout policy moves into widgets (ADR 0028).

Tests exercise rendered layout, actual hit regions and emitted region requests without introducing production test hooks. The W10 comment follows the append-only issue-tracker convention. No actionable baseline smell was found; the small repeated input-event fixture keeps the tests independently readable and does not warrant extraction.

Standards: **0 findings; worst severity: none.**

## Spec

No actionable scoped Spec findings.

W10 requires rich-body/action combinations not to clip required controls and preservation of in-place replacement/non-focus-stealing behavior. The changes remove fixed summary/action height constraints while retaining single-line geometry, opaque action keys, captured targets and the existing two-action compact projection. Tests exercise multiline, word/glyph wrapping, icon-bearing actions, last-line activation and replacement shrinking.

ADR 0044's full-card visibility requirement remains intact: larger wrapped cards defer acknowledgement until their natural layout fits. No timeout policy, focus behavior, capability advertisement or unrelated scope expansion was introduced. Broader supported-scale, oversized-card, output-fallback and keyboard/accessibility acceptance remains explicitly unfinished, not incorrectly claimed complete.

Spec: **0 findings; worst severity: none.**

Summary: Standards — 0 findings, worst none; Spec — 0 scoped findings, worst none. W10/W11 remain incomplete.
