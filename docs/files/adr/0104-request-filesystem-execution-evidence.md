# 0104: Associate accepted requests with filesystem executions

Status: scoped execution-evidence verification, 2026-10-20. Continues ADR0056/0102 and Phase2
of the continuation handoff. Exact restart continuation remains unfinished.

## Evidence

A public OperationJobs fixture refuses the result checkpoint with a private SQL
trigger after an actual Rename or a partially successful directory Copy. The
child waits for SaveFailed and kills itself with SIGKILL. The filesystem and Undo
survive, but the previous restored Running request contains no association with
those changes (source12/core13/receipt57, failing first on Rename).

## Decision

Schema23 adds request_executions. The application mutation and ordinary transfer
workers associate the exact Running scheduling revision and payload SHA256 with
the new filesystem operation in one SQLite transaction, before its forward journal
or committed mutation. Existing temporary preparation is not newly journalled by
this change. Parent operation IDs already identify the journal and its confirmed
steps. A refused association rolls back operation creation before forward work.

Require the original history, current revision, Running phase and exact stored
payload. Each accepted request can acquire at most one execution, including after
a later scheduling revision or Undo-history expiry. Retry/Continue already creates
a distinct successor request and operation under ADR0102. Saving an actual result
again does not acquire another execution.

Association lifetime follows its scheduling record, independently of Undo expiry.
Finished-record eviction removes the evidence with that record; unfinished records
remain protected by the existing1024/64MiB bounds. Association UTF8 label/hash overhead
also counts toward the existing4MiB per-request and64MiB total budgets. A near-full
record can therefore refuse execution before forward mutation. No budget is raised.

The public read-only request_effects interface reports the original revision/hash,
operation, currently retained confirmed-step count and at most128 sampled change
paths with an explicit truncation flag, history availability and a
pending journal. Empty history or an absent journal does not imply completion.
Restored Running application rows show this evidence and remain Interrupted,
without execution controls. Old rows receive no guessed link from labels or paths.
Schema22 upgrades add an empty association table and preserve existing records.

## Boundaries and verification

This connects ordinary Transfer and Mutation (Create Folder/Rename). Undo,
Trash/Restore, Recovery Copy, retained-source execution ownership, full result
reconciliation and durable original-object authority require their own integration.
The association is descriptive evidence, not permission to replay a stored request.
It does not reconstruct lost traversal or create a private execution owner.
[ADR0105](0105-retained-rename-ownership-and-dismissal.md) subsequently uses this
association for queued Rename retention and keeps the evidence after explicit
ordinary-request dismissal. General ownership and restart reconciliation remain
separate gates.

Source14/core15/receipt58 passes the current focused targets, including actual
subprocess death after Rename/partial-Copy commit and refused result save. Edited
post-crash output bytes and inode remain intact, with the associated Undo visible.
Frozen source18's full suite passes577 backend parents across67 standard targets
and the custom device harness. It includes stale/foreign/duplicate/newer-revision
refusal, transaction rollback, UTF8 byte budgets, at-most128 path sampling with
truthful total/truncation, history expiry, exact payload digest and schema22
migration.83 default Iced library tests, formatting, all-target Clippy and the
default build also pass. All721 frozen inputs match through native acceptance.

The preserved ELF613871c85757 passes six maintained native cases/40 grouped checks
in both appearances. Those establish current operation/continuation/interrupted-
Recovery compatibility. The new result-save crash association itself is verified
through actual subprocesses at the public backend seam; no dedicated native
presentation case is claimed. Native31's dark case passed five checks before its
parent received unexplained SIGTERM143; its incomplete batch is preserved. A
runner-only sender diagnostic accompanied native33's successful two-case batch,
with no SIGTERM observed. Its cause remains open, without changing assertions or
timeouts. The diagnostic self-test confirms sender capture and normal termination.
Exact receipts and limitations are recorded in EXECUTION-AUTHORITY-WIP.md.

Subprocess death during preparation, partial file copy, quarantine and continuation
handoff still needs its complete matrix. Descriptor-relative mutation, stable media
and file identity across restart, proven durable continuation and explicit review/
new-authority workflows remain Phase2 gates. Later product/release phases remain
open. No desktop or native verification is implied by backend results alone.
