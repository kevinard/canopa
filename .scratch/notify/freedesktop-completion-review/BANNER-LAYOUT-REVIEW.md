# Measured banner layout continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked `presentation/tests/layout.rs`, the evidence document and screenshots. This tranche began at `7ce2a0f`. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`, `6260b01`, `aae365b`, `77d3aaf`, `60d733f`, `6397b9e`, `1bf5698`, `7ce2a0f`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewed the pending measured-layout tranche and relevant cumulative presentation/lifecycle interactions without editing files or running builds/tests. Their rechecks included [graphical evidence](BANNER-LAYOUT-EVIDENCE.md); neither certifies completion of W10/W11 or the full plan. Separately, the final private-bus workspace suite passed all 383 tests with no failures or ignored tests; typecheck, Clippy, formatting, whitespace and protocol-drift checks pass with existing warnings.

## Standards

Zero outstanding scoped findings after recheck. The issue comment now appends correctly, obsolete acknowledgement hashing support is removed, and the shared software-renderer fixture avoids duplicated setup without hiding test behavior. Viewport reconciliation remains within the presenter and preserves generation-owned acknowledgements.

The screenshots support the documented clipping correction. Evidence distinguishes processed redraws, surface fit and physical output visibility, and explicitly records the remaining ADR 0028 constrained-output limitation. It makes no unsupported keyboard, accessibility or capability-completion claims.

Resolved: one low-severity documented-convention breach (comment order) and one low-severity Speculative Generality judgement (orphaned subscription hashing). Worst outstanding issue: none.

## Spec

One P2 scoped defect was corrected and independently rechecked: asynchronous resize tasks can arrive out of order, leaving the latest card clipped if only the cached requested size is compared. `LayoutFrame` now carries the actual viewport; current measured geometry repairs outdated compositor resize delivery. The persistent-widget regression reproduced zero repair tasks instead of one before the correction, then passed and verified settled frames do not loop. Documentation calls this self-correction, not rejection of every stale command.

Zero outstanding scoped findings; no scope creep. The screenshot supports the compact-layout improvement. W10's supported-scale/long-label/output-fallback requirements, ADR 0028 per-card display timing under constrained outputs, and W10/W11 full graphical keyboard/accessibility acceptance remain explicitly partial. Pointer/focus/expiry checks do not claim compositor presentation feedback.

Worst outstanding scoped issue: none.

Summary: Standards — 0 remaining findings (2 cleanup corrections); Spec — 0 remaining scoped findings (1 P2 correction). W10/W11 remain incomplete.
