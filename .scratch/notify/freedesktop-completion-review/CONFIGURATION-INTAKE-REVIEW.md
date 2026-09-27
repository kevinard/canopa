# Configuration-intake continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked executable fixtures and ADR 0040. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`, `6260b01`. This tranche began at `6260b01`.

The implementation skill required independent parallel Standards and Spec reviews. Both inspected the configuration tranche and cumulative runtime/control interactions without edits or competing builds. No review correction was required. Parser-internal nesting/work/memory limits, aggregate projection/query budgets and broader recovery/graphical acceptance remain unfinished, not implied by these file/admission bounds.

## Standards

Zero scoped findings: no documented-standard breaches or actionable heuristic smells.

Descriptor safety stays in the existing private file reader; KDL validation and sanitized diagnostics stay in configuration; committed policy installation remains coordinated by Notification Runtime. Shared reload-guard ownership serves a concrete lifetime requirement through caller cancellation and installation without introducing a public scheduling abstraction. Tests use the approved daemon, executable/socket and private D-Bus interfaces. No conflict was found with AGENTS.md, the domain terminology, or ADRs 0010, 0027, 0030 and 0040.

Worst outstanding issue: none.

## Spec

Zero scoped findings: no missing scoped requirement, scope expansion or incorrectly implemented requirement.

The regular-file UTF-8 reader enforces the 1 MiB source limit. Existing child-document rejection remains intact and active node/entry annotations now reject explicitly. Background preparation keeps file reading and parsing off the executor; caller/job ownership preserves serialized reload preparation through cancellation. Only a surviving caller installs its candidate. Preparation and installation failure preserve committed settings and emit content-free diagnostics; future-request policy semantics and user-rule preservation remain unchanged.

Tests cover byte boundaries, malformed UTF-8, regular-file symlinks, FIFO rejection, schema/diagnostic behavior, concurrent reloads, cancellation, queued preparation, failing presentation installation and installed-XDG policy through real D-Bus clients. Final verification evidence is recorded in [IMPLEMENTATION.md](IMPLEMENTATION.md); reviewers did not independently run builds or tests.

Worst outstanding issue: none.

Summary: Standards — 0 findings, worst none; Spec — 0 findings, worst none.
