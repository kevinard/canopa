# Revision-safe banners and coalesced presentation review

Baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Review command: `git diff df88ecd` (uncommitted continuation). Scope: the W04/W05 continuation described in [IMPLEMENTATION.md](IMPLEMENTATION.md), not completion of every work package.

The implementation skill required independent Standards and Spec reviews. Both reviewers inspected the actual worktree while the full private-bus suite ran.

## Standards

No documented-standard breaches or heuristic code-smell findings. Revision targets, lifecycle revalidation, coalescing and frame semantics follow CONTEXT.md and ADR 0030.

Additional correctness concern: renderer shutdown clears the pending watch value before removing its receiver. Concurrent publication can refill the slot and retain a callback-owned runtime indefinitely. Reversing cleanup alone is insufficient because the watch sender's receiver check and value replacement are not one atomic operation. Publication and closure need shared synchronization.

## Spec

One scoped implementation defect: the shutdown/publication race violates issue 05's renderer-lifetime cleanup requirement. A deterministic concurrent-exit regression is required alongside synchronization of publication and closure.

No scope creep or other missing scoped requirements. Graphical readiness, proactive health, restart recovery, transport/shutdown fault coverage and the rest of the completion plan remain explicitly unfinished.

Summary: Standards — 0 documented breaches, 0 heuristic smells, 1 additional correctness concern; Spec — 1 defect. The issue in each report was the same renderer-retirement race.

Correction: publication and closure now share a mutex-protected sender slot. Closure makes the slot terminal before clearing retained callbacks, so a concurrent update either precedes cleanup and is cleared, or is rejected afterward. The controlled regression holds receiver removal while publishing after callback retirement; it failed before the fix and passes afterward.

Both reviewers independently rechecked the correction and found no remaining defect. All 18 focused presentation-adapter tests pass. Final workspace suite and lint results are recorded in [IMPLEMENTATION.md](IMPLEMENTATION.md).
