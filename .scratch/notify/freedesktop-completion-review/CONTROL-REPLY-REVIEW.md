# Control-reply continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Committed comparison: `git diff df88ecd...HEAD`; cumulative pending comparison: `git diff df88ecd`, including initially untracked executable fixtures and ADR 0039. Commits since baseline: `8be0e99`, `9289e34`, `4eaa158`, `cf449ee`, `9d62fcf`, `622201a`, `c170b6d`, `a572593`, `2b8a21b`, `24f66bb`, `9f9f129`. This tranche began at `9f9f129`.

The implementation skill required independent parallel Standards and Spec reviews. Both inspected this tranche and cumulative control/provider interactions without edits or competing builds. No review correction was required. Clippy subsequently identified a complex test-case tuple; named fixture fields removed that warning without changing assertions, and the Standards reviewer independently rechecked the cleanup with zero findings. Full-suite verification also triggered diagnosis of an existing fixed-sleep presentation-test race; both reviewers independently rechecked its event-based synchronization correction. Projection/query work, aggregate diagnostic memory, pagination and broader resource/recovery/graphical acceptance remain unfinished; the wire limits are not total-memory guarantees.

## Standards

Zero scoped findings: no documented-standard violations or actionable heuristic smells.

Transport limits, capped serialization and exchange deadlines remain private to `control_adapters/transport.rs` under ADRs 0038–0039. Command-specific validation and formatting stay in `CtlRequest`, preserving ADR 0020's separation of domain results and adapter representations. Shared capped encoding has two existing consumers with distinct budgets. Tests exercise the executable and external socket peers without exposing helpers or adding production testing hooks; real temporary history verifies overflow preserves retained facts. Partial-clear diagnostics preserve ADR 0030's committed counts.

Worst outstanding issue: none.

## Spec

Zero scoped findings: no missing scoped requirement, unrequested scope expansion or incorrectly implemented requirement.

Ordinary reply serialization reserves newline space within its 8 MiB cap before output; overflow returns a fixed error without changing history. Typed and legacy active JSON share this boundary while panels retain their separate limit. CLI request admission counts encoded bytes before connecting; reply validation covers bounded complete framing, UTF-8 and expected type before stdout. Connect/write/read share the total deadline with the shorter write allowance retained. Invalid replies cannot leak their contents, valid partial-clear replies preserve committed counts, and no automatic retry path exists.

Executable regressions cover exact/excessive reply and request sizes, escaped request bytes, malformed/incomplete/mismatched replies, compatible success formatting, slow-drip timeout without retry and history preservation. The presentation regression subscribes before acknowledgement and verifies both committed expirations and history states under an explicit executor stall; it cannot pass when a visible timer is lost. Its bounded event wait replaces an assumed completion delay without changing production timeout semantics. Verification evidence is recorded in [IMPLEMENTATION.md](IMPLEMENTATION.md); reviewers did not independently run builds or tests.

Worst outstanding issue: none.

Summary: Standards — 0 findings, worst none; Spec — 0 findings, worst none.
