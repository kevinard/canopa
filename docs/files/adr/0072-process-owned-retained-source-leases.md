# 0072: Process-owned retained-source leases

Status: implementation in progress. Continues ADR0068/0069 and the entire PRD.
This does not accept source execution, complete consuming-operation ownership,
Retry/Continue or restart reconciliation.

An accepted request already reserves catalogued retained versions. An owned
Copy/Cut selection needs protection before Paste accepts a request. Capture
remains a non-owning observation; explicitly acquire a core-owned source lease
before publishing the clipboard selection. Acquisition uses the same short
transaction and stale-capture/journal checks as request admission, without
waiting for a transfer or Undo question's mutation lock.

Represent the lease using an internal request reservation tagged as a source
lease. It is not executable scheduling intent and never appears in Operations
or the public request page. It shares request budgets, expiry/Undo protection
and atomic physical-stage relocation with durable request reservations. Schema
22 makes older binaries refuse these new records rather than interpreting them
as ordinary queued work.

The public lease is an opaque, cloneable process-owned handle. Its final drop
queues release on a dedicated metadata worker; dropping it performs no database
or filesystem I/O on the Iced thread. An explicit worker-only release can wait
for acknowledgement. A failed deletion retains protection and is retried before
new acquisition; failure is reported rather than silently removing ownership.
The release worker shares the exclusive history-lock ownership guard until
its channel closes and queued releases are handled (ADR0115). A lease cannot outlive
the lock that makes its process ownership meaningful.

On opening history under that exclusive lock, remove abandoned source-lease
rows before reconciliation and expiry. A previous process's clipboard authority
does not survive restart. Durable request rows and their reservations remain;
accept a Paste while retaining its source lease until admission replies, so
there is no unprotected gap when clipboard ownership changes concurrently.

Prepare a lease on the metadata worker alongside selection validation. Keep it
with the exact process-local clipboard ownership token, pending reads and
admissions. Superseded preparations, failed publications, external owners and
completed empty Cut selections release only their own handles. Another owner or
an accepted request keeps its own protection. Ordinary files and similarly named
unowned folders gain no retained-data authority.

Verify through the established OperationEngine, request and clipboard seams
with real private files: expiry, clones/independent owners, consuming Undo,
admission during review, request handoff, physical-stage migration, failed
release, restart after process death and absence from scheduling pages. Native
acceptance uses only private Niri/clipboard sessions. Journal-created containers,
other consuming Rename/Trash/Move, source descriptors, original media, exact
saved-plan association and restart execution retain their separate gates.

Scoped verification now covers the core ownership lifecycle and Iced clipboard
wiring. The metadata actor prepares leases during a live Undo question; queued
Paste owns a guard through durable admission even if its reply is abandoned.
Concurrent explicit releases use final-owner coordination so one caller observes
the final deletion result. Public tests cover clones, independent owners,
nonblocking Drop, failed deletion, shared budgets, migration and killed-process
cleanup while preserving accepted requests.

Candidate `bf3f9f2f` passes all 430 core tests, 51 Iced library tests, all-target
Clippy and 28 native checks across four cases in both maintained themes. The
preserved `5906ee19` baseline first reproduced loss of copied retained bytes
after 100 native folder operations. The candidate preserves them through that
expiry, pastes their actual bytes, and cleans them up after ownership changes.
Cleanup uses bounded further public operations because release is asynchronous.
Ordinary clipboard Copy/Cut/Paste, conflicts, Undo and closed-Browser lifetime
also pass. All four application logs are empty and narrow captures were inspected.
See the [verification record](../../../.scratch/file-manager/iced-source-leases-verification.json).
This native retained-data case exercises Copy; consuming-source execution and
the broader gates above remain open. The complete product brief is not accepted.
