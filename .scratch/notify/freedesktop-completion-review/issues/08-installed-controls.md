# W08: Installed configuration, durable DND, atomic reload and health status.

Status: complete for the selected static Notifications 1.3 profile

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W08. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-11: Continuing CLI failure-path verification jointly with W07 under ADR 0039: bounded complete replies, expected-command validation, a total exchange deadline and preserved committed partial-clear counts. No installed configuration settings or control commands are added; broader concurrent configuration and recovery acceptance remain open.

2026-09-11: CLI transport failure-path continuation is implemented. Successful status/active/DND/reload/clear/delete output is preserved, complete partial-clear replies retain committed counts, and invalid/incomplete/mismatched replies fail without leaking payloads or retrying uncertain commands. All 66 affected tests and 337 final workspace tests pass. Independent Standards and Spec reviews against `df88ecd` have zero scoped findings; see [CONTROL-REPLY-REVIEW.md](../CONTROL-REPLY-REVIEW.md). Broader configuration/command acceptance remains in progress.

2026-09-11: Continuing from `6260b01` with bounded regular-file configuration intake, explicit type-annotation rejection, content-free diagnostics and executor-independent serialized candidate preparation under ADR 0040. Tests use the approved daemon, executable/control and real private D-Bus interfaces. Existing child-node rejection is preserved, not a newly discovered implementation defect. Parser-internal nesting/work limits, aggregate projection memory and broader recovery/graphical acceptance remain separate.

2026-09-11: Configuration-intake continuation is implemented. A real XDG-started daemon and private D-Bus clients verify that rejected CLI reloads preserve committed path, generation and policy, while valid reloads affect subsequent notifications. Public-daemon fixtures cover serialized concurrent preparation, cancellation, responsive controls and presentation-installation rejection. All 99 affected tests and 348 final workspace tests pass; typecheck, Clippy, formatting, whitespace and Niribar protocol drift checks pass with no new warnings. Independent Standards and Spec reviews against `df88ecd` have zero scoped findings; see [CONFIGURATION-INTAKE-REVIEW.md](../CONFIGURATION-INTAKE-REVIEW.md). Parser-internal resource limits and broader command/recovery/graphical acceptance remain in progress.

2026-09-21 maintenance: Status reconciled with the final [implementation tracker](../IMPLEMENTATION.md) and [release verification](../../../../docs/notify/notification-release-verification.md). Earlier dated comments retain historical evidence. Operational rollout, additional failure verification, and hosted CI are tracked separately in [project completion](../../project-completion/PRD.md); hosted execution remains open until a repository and runner exist.
