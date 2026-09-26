# Timer-resource continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed diff command: `git diff df88ecd...HEAD`; cumulative worktree command: `git diff df88ecd`. The committed portion includes `8be0e99` (revision-safe/coalesced banners), `9289e34` (wire registry/retirement), `4eaa158` (typed IDs/restart fixtures), `cf449ee` (request/markup bounds), and `9d62fcf` (live-resource admission). Both reviewers inspected the initially untracked timeout module, public-seam tests and ADR 0033. This W07 tranche began at `9d62fcf`.

The implementation skill required independent parallel Standards and Spec reviews. Scope is the timer-resource continuation in [IMPLEMENTATION.md](IMPLEMENTATION.md), not completion of every work package. Neither reviewer edited files or ran a competing build.

## Standards

No documented breaches, actionable heuristic smells or concrete correctness findings against `df88ecd`, including the current untracked timer files.

The timer extraction follows CONTEXT.md's deep-module ownership rule and ADRs 0028/0030/0033: lifecycle retains expiration meaning, ordered committed effects register/cancel timers, and acknowledgements only consume existing registrations. Scheduling and cancellation share one lock; firing tasks remove their matching registration before publication, avoiding self-abortion.

Typed identities and revision-bearing banner targets remain consistent with ADR 0029. Operational diagnostics are content-free, and tracker comments follow the append-only convention. Deferred recovery, shutdown and other resource work remain explicitly documented without unsupported completion claims.

## Spec

No scoped spec findings. The implementation satisfies W07's requirement: "Reclaim timer tombstones only when old callbacks cannot become valid again."

Only committed lifecycle effects register timers; delayed acknowledgements cannot reconstruct removed entries. Starting and cancellation use the same lock, closing the check-then-schedule race. Replacement revisions reject old frames; dropped replacements preserve current timers. Failed receipt/user/sender transition writes leave registrations untouched; partial clear applies committed cancellations only.

Firing tasks detach their matching record before expiration, avoiding self-abortion during asynchronous sender publication. Pending/scheduled registration count remains bounded by live admission. `timeout_records` reports only that bookkeeping and preserves existing plain-text control output. Display-based duration semantics and the cumulative typed-ID, resource-admission and revision-safety invariants remain intact.

No missing scoped requirements, unrequested scope creep, or implemented-but-incorrect scoped behavior identified. Expiration-write recovery, shutdown, workers/assets/provider streams and graphical acceptance remain explicitly deferred.

Summary: Standards — 0 documented breaches, 0 actionable smells, 0 concrete correctness concerns; Spec — 0 missing/partial scoped requirements, 0 scope-creep findings, 0 incorrect implementations. No worst issue on either axis and no review correction required. Final verification evidence is recorded in [IMPLEMENTATION.md](IMPLEMENTATION.md).
