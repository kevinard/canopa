# Legacy-import ownership continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked `crates/canopa-notify/tests/legacy_import.rs`. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`, `6260b01`, `aae365b`, `77d3aaf`, `60d733f`, `6397b9e`. This tranche began at `6397b9e`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewed the pending tranche and relevant cumulative startup/history behavior without editing files or running builds/tests. Verification results are recorded separately in [IMPLEMENTATION.md](IMPLEMENTATION.md); neither review certifies graphical acceptance or completion of W02/the overall plan.

## Standards

Zero scoped findings. The source lock reuses the existing ownership mechanism and stays within startup, preserving the documented separation between Notification History persistence and Notification Lifecycle recovery (CONTEXT.md; ADRs 0004, 0012 and 0030).

The four new tests exercise approved executable/control and filesystem/SQLite seams. Their fixtures isolate user state and clean up child processes. Documentation distinguishes cooperative path-based ownership from alias or unrelated-writer protection, and explicitly leaves interruption recovery unfinished. No documented-standard breaches or actionable baseline smells were identified.

Worst outstanding issue: none.

## Spec

Zero scoped findings. W02 requires import only if the source still exists and its old daemon is stopped. The source ownership guard uses the same database lock as ordinary startup and remains alive through snapshot creation, verification and publication. Existing-destination precedence remains unchanged.

The four executable tests substantiate cross-socket owner rejection, retained source contents and live identity, committed WAL import, history/policy preservation, stale-action recovery, failed-import cleanup/retry and destination reuse. They exercise approved interfaces without changing production interfaces. Documentation limits guarantees to cooperative path-based ownership and ordinary prepublication failures. Abrupt interruption/orphan cleanup, filesystem aliases and graphical readiness remain explicit open work.

Worst outstanding issue: none.

Summary: Standards — 0 findings, worst none; Spec — 0 findings, worst none.
