# 0115: End history locking with logical ownership

Status: accepted and regression verified, 2026-10-22.
Continues ADR0072. This does not close restart execution or native startup gates.

A combined source-retention regression intermittently fails when reopening
history immediately after releasing the last source lease. The failure occurs
at the next loop's engine open, even though explicit lease release waited for
its worker to finish. Repeating only the original test does not reliably expose
the surrounding concurrent process-launch window.

The ranked hypotheses were an inherited descriptor in a fork/exec child,
premature release-worker completion signalling, and a surviving lease owner.
A controlled child holds inherited descriptors while the parent drops the real
engine. The same EWOULDBLOCK occurs before a lease worker is even created. It
reproduces on all three recorded attempts. The test then covers the lease-worker
path and verifies that an active engine or source lease still excludes reopening.

Linux flock locks belong to open file descriptions shared by fork/dup copies.
Closing the parent's last descriptor can leave a child's copy holding the lock;
an explicit unlock releases it. See [flock(2)](https://man7.org/linux/man-pages/man2/flock.2.html).
Close-on-exec does not remove the inherited descriptor before exec.

Represent history ownership with a shared guard. The engine and its source-
release worker hold that same guard; the last logical owner explicitly unlocks
before closing its file. A nonowning child cannot extend ownership merely by
retaining a descriptor. Active leases keep the worker and guard alive. The
existing worker-exit acknowledgement still follows release of history ownership.
Acquisition keeps its original regular-file, no-follow, close-on-exec and
nonblocking exclusivity checks. No on-disk format changes are needed.

The regression uses anonymous close-on-exec pipes for deterministic readiness
and release. Only async-signal-safe C calls execute in the forked child, followed
by _exit; Rust and SQLite destruction run only in the parent. This replaces the
first fixture's socket timeout setup, which the test sandbox refused.

After the fix, the controlled fork regression passes twenty consecutive runs;
three complete source-retention suite runs and the final combined regression
also pass. Active leases still prevent a second history owner. Evidence lives
in restart-history-lock-stress-01.json and restart-directory-core-09.log under
.scratch/file-manager/. The broader native reliability issues remain separate.

This fixes the demonstrated ownership boundary. The native first-window timeout
and activation/focus issues still require their own reproduction and verification.
Files restart execution, filesystem mutation/cleanup ownership and the remaining
product/release acceptance gates also remain open.
