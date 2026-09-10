# W05: Reliable ordered sender events and recoverable current-state presentation.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W05. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-10 continuation: Reproduced renderer FIFO overflow and unbounded undisplayed acknowledgements. Implement a latest-state stream, bounded pending frame acknowledgement and renderer-lifetime cleanup. Preserve acknowledgements from already queued frames, but never acknowledge skipped undisplayed states. Tests use the existing presentation adapter/renderer-consumer seam. Startup readiness, proactive health, restart recovery and full shutdown/transport-failure verification remain separate unfinished criteria.
