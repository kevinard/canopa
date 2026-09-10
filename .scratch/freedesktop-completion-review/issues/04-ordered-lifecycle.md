# W04: Serialized mutations, revision-safe lifecycle and UI callbacks.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W04. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-10 continuation: Implement revision-bearing banner targets through widget events, asynchronous activation-token callbacks and the daemon interaction handler. Reject stale interactions (including a reused action key) without mutating replacement content or publishing a token. Keep Notification Center/CLI commands as deliberate current-state operations. Test at the approved Daemon/presentation interfaces, including retained/transient replacements, duplicate interactions and delayed display acknowledgements. Review baseline: `df88ecd`.
