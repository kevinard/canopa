# 0084: Backend-owned Trash and Restore

Status: backend workflow scoped-verified, 2026-10-19. The follow-on Iced catalog
and interface are recorded in [ADR0085](0085-iced-trash-catalog-and-window.md).
Continues ADR0008/0065/0074.

Trash and Restore enter the same renderer-independent OperationJobs queue as
Rename, transfers and Undo. The existing engine remains their only filesystem
implementation and history writer. Iced owns list selection, drafts and
presentation; it must not select storage, move payloads or reconcile metadata.

A Trash request captures its raw selected paths and the application's data-home
placement. The worker captures process-local selected-entry observations before
acknowledging durable admission, and retains them until the work settles. A later
replacement at a queued pathname must be preserved. Original failed observations
remain failures. Storage checks and the engine's private, same-filesystem Trash
selection still apply. These observations do not grant restart authority or
complete the descriptor-relative traversal/race audit.

Publish actual completed, failed and unstarted items as a typed, durable result.
Cancellation preserves completed changes and records only those in shared Undo.
Individual failures do not prevent later items. Stopped results retain no
executable request, and reopening never retries them automatically. Explicit
safe Retry/Continue remains an independent gate under ADR0056.

Recovery can confirm a move after an initial synchronization error. Count that
confirmed journal change as completion, rather than reporting it as a failed
source. A later recovery/finalization error retains the known result and marks
unconfirmed items for review. That interrupted result does not authorize more
mutations. Cancellation between dispatch and first execution also produces a
typed cancelled result with its original unstarted items.

Restore carries the actual selected TrashItem and optional raw destination,
preserving its engine-owned metadata and payload identity checks. It never
overwrites an occupied destination or invents information for an orphaned item.
Its actual completed change enters the same shared Undo history.
Its typed durable result preserves the confirmed destination and operation even
when finalization fails. A recovered committed move is successful; an uncertain
journal requires review. An intent refusal with a confirmed absent journal is an
ordinary failure and does not prevent later work. Both stopped Trash and Restore
records contain results rather than executable requests.

Verification uses the established public queue/filesystem seam from ADR0001 and
ADR0065: real private files, admission/completion, Trash listing, shared Undo,
queued replacements, individual failures, cancellation and reopening. Native
Iced Trash/Restore controls and their previews require separate native evidence;
these backend tests do not satisfy it. See ADR0085 for that follow-on evidence.

Twelve new Trash/Restore tests, a concurrent snapshot regression and the existing
queue/attention/retention/Trash tests pass 66 parents, with actual private child
fixtures. They cover raw names and final symlinks, completed/partial/cancelled
result persistence, queued replacements and initial-read failures, cancellation
after a real move, shared Undo, occupied names and chosen Restore destinations.
A private C fixture built with `cc` interposes only in an isolated child: one
exact source-parent fsync fails after rename, or the next armed worker waits
while the coordinator acknowledges cancellation. A private SQLite trigger makes
final trimming fail after completion. These regressions failed before the fixes
and now preserve the actual result, reopening and Undo/review semantics for both
Trash and Restore. A retained-source lease regression also reproduces an intent
refusal before any move, then verifies unchanged payloads, unrelated work and a
fresh Restore after releasing the lease.

The attention regression also exposed a snapshot publication gap: a reader could
see 101 Finished rows before trimming to 100. Completion, attention and bounded
rows now publish together under one lock. The original 1024-notice stress test
and concurrent-reader regression pass. Existing durable trimming already applies
the row bound on reopening; no new startup policy was needed. The temporary
diagnostic probe was removed before acceptance.

This backend checkpoint left the resident executable unchanged and claimed no
new native Iced acceptance. Exact attempts, source identity, checks and reviews are
in [the verification record](../../../.scratch/file-manager/trash-queue-verification.json).
