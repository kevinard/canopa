# W01: Executable specification matrix and private-bus runner.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W01. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-11 continuation from `60d733f`: Add a dedicated executable/private-bus contract suite for exact method/signal introspection, method reply types and identity, empty invalid-closure errors, malformed calls and ordered signal envelopes. Use `dbus-send` as an independent decoder for the four top-level server-information strings. Keep optional graphical capabilities and local ID/admission policies explicitly separate from specification requirements; do not infer graphical completion from wire tests. Review baseline remains `df88ecd`.

2026-09-11 verification: Six new contract tests and the strengthened single-stream activation/action/closure test pass. A reproduced unexpected-argument acceptance gap in discovery methods is fixed by enabling zbus's generated empty-argument validation. All 31 focused and 366 full-workspace tests pass; typecheck, Clippy, formatting, whitespace and Niribar protocol-snapshot checks pass with existing warnings. Independent Standards/Spec reviews report zero scoped findings; see [DBUS-CONTRACT-REVIEW.md](../DBUS-CONTRACT-REVIEW.md). Named evidence and local error/allocation conventions are recorded in the compliance matrix. Clean-checkout CI and broader graphical/release acceptance remain unverified locally; W01 and the overall plan are not marked complete.
