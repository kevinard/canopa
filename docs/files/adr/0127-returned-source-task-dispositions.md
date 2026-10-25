# Returned-source task dispositions

Status: scoped backend and isolated desktop-library verification. The full restart and
five-area product goal remain unfinished.

## Problem

A process killed after claiming a Move source, before verified removal, leaves a
Ready journal. Recovery safely returns the source. The old generic Preparing
write then cleared the Forward context but retained the uncommitted SourceRemoved
receipt. Cursor recovery found the committed Copy plus that extra receipt and
refused continuation. Returning claims had the same missing task disposition.
The actual public-service SIGKILL regression reproduces this twice, with unchanged
returned source, intact copy and no pending journal after review.

## Decision

Source-return cleanup uses a dedicated backend transition. It accepts only an
exact Ready or Returning SourceRemoved Forward intent normalized into its own
empty-only Preparing stage. The original source path, operation and stage facts
must match. The independently bound Forward context must verify before its proof
can be retired. Claimed removal is never treated as a return.

Within one SQLite transaction, the transition verifies the durable active head
and exact uncommitted removal receipt, retires only that receipt, and saves the
Preparing intent/context. It refuses a committed or out-of-order removal and
changed bodies, task positions, row hashes or step hashes. The preceding committed
Copy proof remains. Unscoped synchronous operations retain their existing cleanup
behavior. No new row format or continuation codec is introduced.

The active-head verifier is shared by live Forward binding and returned-step
retirement. Retirement never inserts or recreates a missing receipt. Once the
verified return and empty-stage cleanup reconcile, recovery can advance Copy to
its original copied-source cleanup task. It does not recopy output, replace a
source or infer task completion from current path existence.

## Verification and limits

Public OperationJobs gates use actual SIGKILL at Ready and after an authorized
return, then explicit Reconcile, two opens and continued cleanup. Other gates
kill the review process after the atomic Preparing transition; verify the Copy
receipt remains; reject unexpected private payloads; and preserve identical-byte
replacement sources/outputs. SQL refusal triggers test rollback at receipt deletion
and Preparing save, including later successful retry. Changed Forward contexts,
head hashes and receipt bodies/hashes cannot mint an execution owner. All earlier
Claimed/hard-link guards remain required.

Current verification passes 417 selected backend parents across 20 targets,
31 ignored helpers (including 104 library parents), and 185 isolated default
desktop-library parents, 11 ignored. All-target Clippy with warnings denied,
formatting and schema41 fixture syntax/hash pass. See SOURCE-RETURN-WIP.md and the implementation ledger for
actual final profiles, source hashes, broader/desktop checks and preserved failure
records. These checks do not establish native/workspace/release or hardware
acceptance. Complete Preparing/Cleanup provenance, legacy loose bindings,
metadata-record Retry Save ownership, directory and other filesystem task phases,
queued lineage/admission, large reports/paged owners, Undo/relocation/expiry,
Trash/Restore and the other goal areas remain required.
