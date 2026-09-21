# W03: Session-safe live sender identity separated from retained history.

Status: complete for the selected static Notifications 1.3 profile

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W03. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-10: Continuing from `8be0e99`: extracting the D-Bus adapter and typed live registry, retiring issued IDs, and bounding allocation/exhaustion. The retired-token regression fails on the baseline. ADR 0029 explicitly tightens closed fixed-token compatibility; session-global identity is unchanged and the separate trustworthy-identity decision remains open.

2026-09-10: Registry continuation implemented and independently reviewed: 251 workspace tests pass, including six new public D-Bus seam tests. Clippy, typecheck, format and protocol drift checks pass with existing baseline warnings. See [ID-REGISTRY-REVIEW.md](../ID-REGISTRY-REVIEW.md). This issue remains in progress for core typed-ID propagation, restart/service-owner-change fixtures and the separate identity decision.

2026-09-10: Continuing from `9289e34`. Reproduced a public request-decoder path treating a wire ID as an internal replacement target; content decoding must not resolve IDs. Propagating the nonzero internal type through core interfaces without changing numeric JSON/SQLite encodings, and adding executable restart/service-owner-change coverage. Authentication is not selected by this tranche.

2026-09-10: Core-ID continuation implemented and independently reviewed against `df88ecd`: 62 focused tests and all 256 workspace tests pass, including the compile-fail boundary check and two-process owner-change fixture. Signed JSON and legacy zero replacement metadata compatibility have regression coverage. Typecheck, Clippy, format, whitespace and protocol drift checks pass with baseline warnings. See [CORE-ID-REVIEW.md](../CORE-ID-REVIEW.md). This issue remains in progress for the separate trustworthy-identity/evidence decision and enforcement work; session-global sender behavior is unchanged.

2026-09-21 maintenance: Status reconciled with the final [implementation tracker](../IMPLEMENTATION.md) and [release verification](../../../docs/notification-release-verification.md). Earlier dated comments retain historical evidence. Operational rollout, additional failure verification, and hosted CI are tracked separately in [project completion](../../project-completion/PRD.md); hosted execution remains open until a repository and runner exist.
