# W08: Installed configuration, durable DND, atomic reload and health status.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W08. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-11: Continuing CLI failure-path verification jointly with W07 under ADR 0039: bounded complete replies, expected-command validation, a total exchange deadline and preserved committed partial-clear counts. No installed configuration settings or control commands are added; broader concurrent configuration and recovery acceptance remain open.

2026-09-11: CLI transport failure-path continuation is implemented. Successful status/active/DND/reload/clear/delete output is preserved, complete partial-clear replies retain committed counts, and invalid/incomplete/mismatched replies fail without leaking payloads or retrying uncertain commands. All 66 affected tests and 337 final workspace tests pass. Independent Standards and Spec reviews against `df88ecd` have zero scoped findings; see [CONTROL-REPLY-REVIEW.md](../CONTROL-REPLY-REVIEW.md). Broader configuration/command acceptance remains in progress.
