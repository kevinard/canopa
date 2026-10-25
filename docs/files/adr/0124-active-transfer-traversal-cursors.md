# Active transfer traversal cursors

Status: scoped backend and desktop-library verification. Full Copy/Move restart
recovery and the wider product goal remain unfinished.

## Problem

A process killed while a live transfer waits on its next conflict has no returned
stopped result. Its initial request cannot identify the remaining children under
a reviewed Keep both directory, or distinguish them from the completed prefix.
Replaying the selected roots would revisit completed work.

## Decision

Schema39 persists an incremental traversal ledger before each next task. Ordered
queue rows, traversed directory facts, failures, skipped paths, root outputs and
saved receipts are separate from a small progress header. Ordinary completion
updates only changed rows. Scope digests bind each row's request revision/body,
physical history, operation, descriptor, role and position; header counts detect
lost entries. A nonserializable private token owns live updates. Byte accounting
uses row triggers rather than a queue scan. The serial cursor has a separate
64MiB budget; existing scheduling/result budgets remain unchanged.

Startup may turn a cursor into a typed stopped receipt only when the exact
accepted execution and original history verify, both journal lanes and directory
access records are clear, and the current history watermark matches the cursor.
This transaction retires cursor rows and records the exact stopped end together;
failure restores the original rows. The established stopped-result transaction
and original-object reopening then restore Retry/Continue/Abandon owners.
Completed data is never reopened to derive a new task or changed to restore a
report. Unprocessed children retain the existing folder-selection semantics.

Normal terminal receipts retain informational source remainders and final-file
cancellation/attention flags. These are finished parts with no execution plan;
their progress cursor must retire without losing the report or blocking later
parts.

## Verification and limits

Public service tests kill an active worker at a conflict after completed work,
then resume Copy and real cross-filesystem Move in either part order. They edit
the completed prefix after death, preserve its inode and bytes, retain reviewed
names and undo successors independently. Refusal cases mutate or borrow ledger
rows, remove entries, change descriptors/positions/watermarks, advance history or
refuse stop saving; reopening twice preserves the data and original evidence.

Startup result-save failure/retry also has a public killed-worker regression:
a successful Retry Save restores the proved execution owners and activity guard
without another restart. The library regression for cancellation after the last
file verifies exact terminal flags and cursor retirement.

The captured implementation passes 385 selected backend parents across 19 targets.
A subsequent equivalent Clippy condition collapse has 120 focused rechecks on its
own captured source, followed by 165 isolated default desktop-library passes.
Default all-target Clippy with warnings denied and formatting pass. Both required
restart gates remain unignored. No native/full workspace/release or hardware
acceptance is implied; receipts distinguish the two source snapshots.

The implementation still refuses an unresolved filesystem step or history that
advanced beyond its last traversal checkpoint. Closing that gap requires typed
step advancement and original copied-source facts, not reobserving current
pathnames. Directory/saved-container phases, queued admission and handoff lineage,
large restored reports and paged owners, Undo/relocation/expiry integration,
Trash/Restore, native/hardware acceptance and all other goal areas remain open.
See the implementation ledger and RESTART-CURSOR-WIP.md for actual current checks.
