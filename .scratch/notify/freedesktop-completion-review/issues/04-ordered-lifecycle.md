# W04: Serialized mutations, revision-safe lifecycle and UI callbacks.

Status: complete for the selected static Notifications 1.3 profile

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W04. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-10 continuation: Implement revision-bearing banner targets through widget events, asynchronous activation-token callbacks and the daemon interaction handler. Reject stale interactions (including a reused action key) without mutating replacement content or publishing a token. Keep Notification Center/CLI commands as deliberate current-state operations. Test at the approved Daemon/presentation interfaces, including retained/transient replacements, duplicate interactions and delayed display acknowledgements. Review baseline: `df88ecd`.

2026-09-11 continuation: Extend captured-revision safety to Notification Center actions under ADR 0042, superseding the center-only portion of the earlier current-state policy. Bind displayed controls to daemon lifetime as well as thread/revision, preserve stable panel item IDs and ordinary CLI commands, and verify at the approved control/provider/D-Bus interfaces. No host protocol or capability change. Review baseline remains `df88ecd`.

2026-09-11 verification: Center-target continuation complete. Six new regressions and all 360 private-bus workspace tests pass; all-target typecheck/Clippy, formatting, whitespace and host snapshot checks pass. Independent Standards/Spec reviews report zero scoped findings; see `../CENTER-TARGET-REVIEW.md`. W04 remains in progress for broader interleaving and graphical acceptance evidence.

2026-09-21 maintenance: Status reconciled with the final [implementation tracker](../IMPLEMENTATION.md) and [release verification](../../../../docs/notify/notification-release-verification.md). Earlier dated comments retain historical evidence. Operational rollout, additional failure verification, and hosted CI are tracked separately in [project completion](../../project-completion/PRD.md); hosted execution remains open until a repository and runner exist.
