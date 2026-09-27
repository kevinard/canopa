# Input-budget continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Cumulative command: `git diff df88ecd`; committed portion: `git diff df88ecd...HEAD`, including `8be0e99` (revision-safe banners/coalesced presentation), `9289e34` (wire registry extraction/retirement), and `4eaa158` (typed core IDs/restart fixtures). Both reviewers inspected the new, initially untracked input-budget module, integration tests and ADR 0031. The current W07 tranche began at `4eaa158`.

The implementation skill required parallel, independent Standards and Spec reviews. Scope is the input-budget continuation in [IMPLEMENTATION.md](IMPLEMENTATION.md), not completion of every work package.

## Standards

No documented-standard breaches or actionable heuristic smells were found in the cumulative diff against `df88ecd`, including the new files.

The private input-budget module centralizes byte/count accounting while preserving the ownership boundaries in CONTEXT.md: decoding remains transport adaptation, Notification Intake interprets content, and Notification Lifecycle owns mutation. Raw wire fields are checked before normalization, normalized inputs include derived action-icon references, and rejection precedes policy decisions, receipt registration and lifecycle effects.

Content-free errors follow the documented diagnostics rules. ADR 0031 accurately distinguishes admission rejection from markup degradation and identifies resource categories outside this tranche.

Parser traversal makes progress without repeated suffix scans. Depth and segment limits apply during parsing; plain fallback preserves decoded text and image alternatives without retaining image references. UTF-8-safe legacy backfill bounds interpretation without rewriting stored raw/plain fields, consistent with the history compatibility rules.

The cumulative changes continue to respect ADRs 0029/0030: typed internal identity remains separate from wire identity; registration and retirement preserve the mutation boundary; banner interactions retain revisions; replaceable presentation updates remain separate from ordered sender events.

Tests use the agreed daemon, history and D-Bus seams. Legacy-row SQL is fixture setup, not an alternative production lifecycle path. No additional concrete correctness concern was identified. Tooling-enforced matters were excluded.

## Spec

No scoped findings were found in the cumulative diff against `df88ecd`, including the new input-limit module, tests and ADR 0031.

Text/action admission matches the documented budgets at both raw-wire and normalized daemon boundaries. Rejection precedes lifecycle mutation, receipt registration and publication; error messages contain fixed categories rather than sender content. The decoder's intentional Rust API change is documented without changing D-Bus signatures or capabilities.

Markup depth and provisional segment budgets are enforced during parsing. Excessive complexity removes styling/image references while retaining safe text and image alternatives. Malformed scans advance without repeated suffix traversal; legacy backfill uses a UTF-8-safe prefix without rewriting stored raw/plain bodies.

The cumulative implementation preserves typed internal identity, guarded wire-ID allocation/retirement, revision-safe interactions and coalesced presentation semantics.

No scoped requirement is missing or incorrectly implemented, and no scope creep was found. Aggregate resources, asset/worker budgets, sender authentication and graphical/recovery acceptance remain explicitly unfinished; this tranche does not claim their completion.

Summary: Standards — 0 documented breaches, 0 actionable smells, 0 additional correctness concerns; Spec — 0 missing/partial scoped requirements, 0 scope-creep findings, 0 incorrect implementations. No worst issue on either axis and no review correction required. Final verification evidence is recorded in [IMPLEMENTATION.md](IMPLEMENTATION.md).
