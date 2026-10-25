# Directory expansion restart recovery

Status: scoped backend and isolated default-library verification.
The full five-area goal and complete Copy/Move restart recovery remain open.
Exact sources and checks are recorded in
[DIRECTORY-EXPANSION-WIP.md](../../../.scratch/file-manager/DIRECTORY-EXPANSION-WIP.md).

## Problem

A directory transfer reads its children, publishes a prepared folder, and only
then updates its in-memory traversal. A crash after publication leaves an advanced
CreatedFolder change without an exact task receipt or durable child enumeration.
Replaying the selected root could revisit completed work or include later source
entries. An accepted Merge also needs to retain its existing destination and
enumeration before any child runs, despite having no creation history step.

## Decision

Schema42 adds normalized expansion and child rows to the existing bounded active
cursor. One descriptor records the exact source and reviewed destination,
DirectoryFinish facts from the actual opened folders, prepared stage and payload
originals, canonical intended creation, operation/ordinal and ordered name digest.
Each raw Unix child name has its own row keyed by the original head position and
child ordinal. The descriptor stays small; ordinary traversal updates still write
only changed rows. Ten counters retain the existing legacy counter decoding, and
the table migration preserves earlier rows and their hashes.

The live cursor scope writes the descriptor, names, counters and Preparing intent
in one transaction. It verifies the exact running request and durable head first.
The head remains unconsumed until the normal traversal checkpoint. Preparing a
Merge instead records the actual approved existing folder without a journal.
Current permission-access records remain owned by the live operation; restart
still requires their reconciliation before restoring execution owners.

Before publication, the exact Forward creation binds both the queue-row hash and
expansion-row hash. The receipt retains the canonical change and stage. Restart
verifies the accepted request, original history file, every scoped row, counters
and exact history suffix before consuming that creation. It expands only to the
captured names, directory facts and Move postorder finish. It preserves the chosen
Keep Both destination, creation count and root report without recreating output.
Merge consumes its independently recorded expansion at an unchanged watermark.
An empty operation record remains while an active cursor references it, so a
Merge before its first child does not lose its operation during startup trimming.

A captured Preparing intent can become Forward only through its independently
proved active directory expansion. Promotion verifies the original accepted
request/history, entire proposed child graph, original source and unchanged
watermark, with no preexisting step receipt. It records Forward, its context and
step receipt atomically. Missing or changed declared expansion data refuses
promotion; it cannot fall into generic cleanup. This grants no authority to
promote unrelated or legacy Preparing records.

Directory Forward reconciliation verifies the actual original parent, work
directory and payload through held descriptors. It publishes that same payload
with NOREPLACE, verifies the original public entry before mode writes, and appends
the exact creation. Replacement objects and devices cannot supply new facts.
Completed append retains only the established empty-stage cleanup rights. All
restored unfinished parts still use the independent stopped-result factory and
its original folder/parent checks; descriptive results grant no execution rights.

Enumeration also records whether the source was empty. This fact lets an actual
empty Merge source removal count once after restart, while an already counted
created folder is not counted again. Continuation checkpoint5 carries true empty
facts; ordinary plans retain checkpoint3/4 encoding. Earlier checkpoints decode
the missing fact as false. Unconsumed expansion rows produce a reviewable error
instead of reaching an unreachable parser branch.

## Verification and limits

Public OperationJobs gates use actual SIGKILL after preparation, publication,
append, accepted Merge and a second crash immediately after promotion commit.
They cover Copy and real cross-filesystem Move, raw names and dangling links,
Keep Both/Merge, repeated opens and Continue, original output inode retention,
later unselected source entries, newer destination contents, empty folders and
completion counts. Refusal gates change or remove descriptors, child rows, keys,
counters and step proofs, replace folders, and move the same original folder into
a replacement parent. SQL triggers verify promotion context/receipt rollback.
A 770-name case verifies bounded normalized rows and safe abandonment; it does
not establish arbitrary-size report or paged-owner support.

The final captured source passes 428 selected backend parents across 20 targets,
33 ignored helpers, and 196 isolated default-library parents, 13 ignored. Clippy
with warnings denied, formatting and schema42 fixture syntax/hash pass. Both
required stopped/active restart gates remain unignored. The WIP and verification
records distinguish exact source relationships and preserve the actual failure
and source archives. These checks
do not establish native, full workspace, release or physical-device acceptance.
Source-directory removal cursor advancement, whole-entry Move, reviewed
replacement and saved-container phases, remaining Preparing/Cleanup provenance,
legacy bindings, queued lineage/admission, large reports/paged owners and
Undo/relocation/expiry remain required. Trash/Restore and the later backend,
product, reliability/performance and release areas remain open.
