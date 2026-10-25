# Copied-leaf filesystem steps after restart

Status: scoped backend and isolated desktop-library verification. Full Copy/Move
restart recovery and the five-area product goal remain unfinished.

## Problem

A worker killed after publishing a copy can have a committed history change
beyond its last traversal cursor. The old watermark refusal preserves data but
cannot continue that leaf. A cross-filesystem Move also needs the actual opened
source which supplied the committed copy; fresh observations cannot grant
permission to remove a replacement source or consume a different output.

## Decision

Schema40 records a copied leaf's source path, original entry and parent,
identity, fingerprint and accounted byte count while the live source pins are
still held. Its existing publication proof identifies the original destination
parent and entry. Missing original facts keep legacy/unverifiable work refused.
No source is selected anew during restoration.

Before filesystem mutation, a Forward intent can bind the durable queue head to
its exact operation ordinal and canonical change. The receipt includes the head's
position and scoped row digest. The scope already binds the original history,
request revision/body, operation and cursor descriptor. A Keep Both choice may
change the final leaf name in that same admitted parent. Recovery checks every
receipt against the committed history suffix; an unrelated appended change or
missing/corrupt/borrowed proof cannot advance the cursor. Receipts are bounded,
accounted within the cursor budget and cleared at each traversal checkpoint.
Ordinary per-file progress still changes only affected rows.

A proved Copy advances the completed task without reopening its output or source.
A Move whose source removal also committed does the same. Otherwise checkpoint4
retains an explicit copied-source cleanup task, in the stopped result and the
normalized cursor. Its execution reopens the recorded original source and copy,
checks both fingerprints and uses the existing opened-parent source-removal
claim. It never recopies, replaces the output, or adopts a new source/device.
Changed bytes/modes retain both objects for Retry/review. Cancellation cannot
roll back the copy completed before the restart. Normal empty cleanup maps retain
checkpoint3 encoding, and schema39 cursor progress with six counts remains
readable. The task-table migration preserves its prior rows and digests.

Pending journals still require the established explicit Recovery review. After
that captured journal reconciles successfully, the worker may restore an exact
running transfer through its independent cursor/end proof. It does not wrap that
request as merely acknowledged and discard the opportunity to continue. The same
proof checks and owner reopening used at startup apply; failed saves retain
Retry Save and the activity hold. Other reviewed operations keep their existing
behavior.

## Verification and limits

The public OperationJobs child-process gates use actual SIGKILL after ordinary
publication, Copy append, source unlink and source-removal append. They exercise
Copy and real cross-filesystem Move, reviewed folder and leaf Keep Both names,
raw Unix names and dangling links, explicit public Recovery review, two later
opens and another crash during resumed cleanup. Completed edited prefixes retain
their bytes/inodes. A completed unlink consumes metadata without changing a new
source occupant or edited output. Replacement source/output entries or parents
are refused, including byte-identical files and a hard link in a new parent.
Original objects with changed bytes/modes remain preserved on failed cleanup.
Missing, changed, borrowed and mismatching step receipts and failed result saves
keep the original cursor/data across two opens.

The first captured source passes 404 selected backend parents across 20 targets.
An equivalent retry-helper signature simplification and an umask-independent
permission fixture have 36 focused integration rechecks and 172 isolated default
desktop-library passes on the final captured source. Default all-target Clippy
with warnings denied, formatting and the schema40 fixture syntax/hash pass.
Both required stopped/active restart gates remain unignored. See
RESTART-COPIED-LEAF-WIP.md and the implementation ledger for exact source
relationships, profiles and receipts. This is not complete restart recovery: final-leaf terminal
receipt recovery, directory publication/children and directory-removal cursor
advancement, whole-entry Move and reviewed-replacement task phases, saved-container
migration, Preparing/Cleanup binding, queued admission/lineage, large reports and
paged owners, and sanctioned Undo/relocation/expiry remain required. Trash/Restore,
backend separation, product/device/accessibility acceptance, performance and
release integration remain part of the active goal. Native, full workspace,
release and physical-device acceptance are not established by library tests.
