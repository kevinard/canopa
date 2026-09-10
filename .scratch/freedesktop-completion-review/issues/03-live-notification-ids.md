# W03: Session-safe live sender identity separated from retained history.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W03. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-10: Continuing from `8be0e99`: extracting the D-Bus adapter and typed live registry, retiring issued IDs, and bounding allocation/exhaustion. The retired-token regression fails on the baseline. ADR 0029 explicitly tightens closed fixed-token compatibility; session-global identity is unchanged and the separate trustworthy-identity decision remains open.

2026-09-10: Registry continuation implemented and independently reviewed: 251 workspace tests pass, including six new public D-Bus seam tests. Clippy, typecheck, format and protocol drift checks pass with existing baseline warnings. See [ID-REGISTRY-REVIEW.md](../ID-REGISTRY-REVIEW.md). This issue remains in progress for core typed-ID propagation, restart/service-owner-change fixtures and the separate identity decision.
