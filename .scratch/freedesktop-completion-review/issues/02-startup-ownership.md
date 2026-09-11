# W02: Safe ownership, XDG paths, recoverable legacy history import.

Status: In progress

Specification and acceptance criteria: [PLAN.md](../PLAN.md), work package W02. Execution evidence and remaining work: [IMPLEMENTATION.md](../IMPLEMENTATION.md).

## Comments

2026-09-10: Began implementation under the approved completion plan. An in-progress status does not claim that all package acceptance criteria have passed.

2026-09-11 continuation from `6397b9e`: Verify legacy import at the approved executable/private-D-Bus and filesystem/SQLite fixture seams. Require the source database to be unowned even when its owner uses another socket; preserve live source state on rejection. Verify stopped-source import, committed WAL contents, source preservation, existing-destination protection and recoverable failed imports. Keep graphical startup readiness and broader migration/permission work separate. Review baseline remains `df88ecd`.

2026-09-11 verification: Reproduced and fixed live-source import through a different socket by taking the existing database ownership lock until publication. Four new executable tests cover ownership, WAL-aware content/policy preservation, destination precedence and prepublication cleanup/retry. All 12 focused and 370 full-workspace tests pass; typecheck, Clippy, formatting, whitespace and Niribar protocol-snapshot checks pass with lint/shared-helper warnings. Independent Standards/Spec reviews report zero scoped findings; see [LEGACY-IMPORT-REVIEW.md](../LEGACY-IMPORT-REVIEW.md). Abrupt import interruption/orphan cleanup, noncooperating writers/path aliases, broader permission/path cases and graphical readiness remain open; W02 is not marked complete.
