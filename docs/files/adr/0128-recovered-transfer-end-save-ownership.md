# Recovered transfer-end save ownership

Status: scoped backend and isolated default-library verification; exact results
are recorded in [ENGINE-END-RETRY-WIP.md](../../../.scratch/file-manager/ENGINE-END-RETRY-WIP.md). Complete restart recovery and the five-area product goal
remain unfinished.

## Problem

Cursor recovery can prove a completed or stopped transfer from its original
accepted request, history file, scoped rows and exact history suffix, then fail
while recording the engine end or retiring the cursor. The transaction correctly
rolls back, but the caller loses the proved result and reports Interrupted. Files
must restart or review again to retry a metadata write. Ordinary live transfers
already retain an engine-owned Retry Save witness at this boundary.

The public OperationJobs regression reproduces the gap twice after an actual
SIGKILL at the final Copy append. An SQL refusal leaves the original cursor and
null end intact, yet the saved result has no Retry Save or activity ownership.

## Decision

Recovery validates the complete cursor and proposed end before attempting any
metadata writes. Validation failures retain Interrupted and cannot construct a
pending witness. A failure after validation instead preserves the typed end and
write error. The engine binds that result to its process-local completion scope
and exact accepted request through the existing PendingTransfer witness.

The witness is not serializable and grants metadata retry only. It cannot run
filesystem work, reopen completed paths or itself own an unfinished continuation.
The worker carries the actual confirmation through startup and successful public
Reconcile, instead of inferring Recorded from the saved result's variant.
SaveFailed retains the application activity hold and exposes Retry Save.

Completed retry uses the existing atomic engine-end/cursor-retirement transaction,
then independently saves the application result. Stopped retry uses the existing
witnessed end/result transaction, preserving the request budget and retiring the
cursor atomically. Only after a successful stop save can the independent original
object factory restore unfinished parts. Their activity hold persists through
Continue, Retry or Abandon as before. Failed metadata retry keeps its witness and
hold. Changed accepted request bytes, revision or reservation refuse the retry.

Completed and stopped end recording share their evidence validation with restart
preflight. Existing recorded ends remain idempotent. No database or checkpoint
format changes; schema41 and continuation versions3/4 remain.

## Verification and limits

Public-service gates kill actual workers, refuse end/stop recording and cursor
retirement, reopen twice, retry failures in process, then remove the refusal and
save successfully. They cover startup Copy and reviewed cross-filesystem Move,
completed output edits and inode retention, unfinished owners unavailable before
save, activity retention/release, later Abandon and reopen, stale request bodies,
revisions and reservation intents. Earlier corrupt, missing and borrowed cursor
evidence continues to refuse a witness. Application-result save failure remains
separately covered. Exact source, profiles, logs and failure archives are recorded
in the WIP record. The initial source passes 419 selected backend parents across 20
targets. After two equivalent guard rewrites requested by Clippy, the final source
passes 36 restart/execution integration parents and 187 isolated default-library
parents, plus all-target Clippy with warnings denied, formatting and schema41
fixture syntax/hash. The source relationship is preserved explicitly. Library
checks do not establish native, full workspace,
release or physical-device acceptance.

Directory publication/child queues, source-directory removal and other task
phases, complete Preparing/Cleanup provenance, queued lineage/admission, legacy
bindings, large reports/paged owners and Undo/relocation/expiry remain required.
Trash/Restore and all other goal areas remain unfinished.
