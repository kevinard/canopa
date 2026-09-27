# Sound-resource continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed diff command: `git diff df88ecd...HEAD`; cumulative worktree command: `git diff df88ecd`, including initially untracked sound tests and ADR 0034. Commits since the baseline are `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf` and `622201a`. This W07 sound-resource tranche began at `622201a`.

The implementation skill required independent parallel Standards and Spec reviews. Both reviewers inspected the selected tranche and its interactions with the cumulative implementation. Neither edited source or ran a competing Cargo build. Scope is bounded sound admission, audio ownership, file/PCM limits and cancellation, not completion of all W07/W09 requirements.

## Standards

No remaining documented breaches, actionable heuristic smells or production correctness findings. Admission, cancellation and PCM ownership stay behind `SoundPresenter`, consistent with CONTEXT.md's deep-module rule and ADRs 0025/0034. Reservations cover workers and queued audio without reference cycles; reopening preserves outstanding accounting. Diagnostics omit sender content. Deferred work remains explicitly documented.

One test-reliability finding was corrected: waiting for any mixed sample did not establish that all four workers had queued their sources. Cancellation could legitimately release still-decoding jobs before the test asserted saturation. The fixture now waits for all four known-amplitude voices before cancellation. The reviewer rechecked the correction, extraction of tests to `sound/tests.rs`, concrete PCM wrapper and clarified best-effort trait contract.

## Spec

No remaining actionable findings in the selected tranche. Admission, worker/source ownership, cancellation generations, encoded/PCM limits, fallback and runtime receipt handling conform to W07/W08 and ADRs 0025/0034. No unintended scope expansion or capability change was found. Theme traversal/semantics, image decoding, provider streams and broader recovery remain explicitly unfinished.

The independent Spec reviewer also identified the same cancellation-fixture race and confirmed the corrected four-voice observation establishes source-owned reservations before disable/reopen. This was one shared test-synchronization defect, not two separate production defects.

Summary: Standards — 0 outstanding findings, worst issue none; Spec — 0 outstanding findings, worst issue none. One test-synchronization issue was corrected and independently rechecked. Final test/check evidence is recorded in [IMPLEMENTATION.md](IMPLEMENTATION.md).
