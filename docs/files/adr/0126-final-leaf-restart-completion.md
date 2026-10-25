# Final-leaf restart completion

Status: scoped backend and isolated desktop-library verification. Complete
Copy/Move restart recovery and the five-area product goal remain unfinished.

## Problem

A worker can die after its last Copy or cross-filesystem source removal commits,
while the traversal cursor still points at that leaf. Proved step advancement
consumes the task, but the old decoder requires an unfinished part and refuses
its now-complete result. Restart then shows Interrupted despite completed work.

The final-leaf regression reproduces this through the public OperationJobs
service with an actual SIGKILL. History and its scoped task receipt are intact,
the journal is clear, and no end has been recorded. Edited completed paths are
not the cause; the exact error is “The active transfer has no exact unfinished
part.” Normal stopped-result validation correctly refuses empty work and should
retain that invariant.

## Decision

The internal cursor decoder returns a typed completed or stopped result. It
validates every row, count and the exact bound history suffix before either
result. A completed cursor needs an operation and no unresolved saved-source
receipt. It does not reopen completed sources, outputs or their parents. A Move
whose copy alone committed still has its exact copied-source cleanup task and
remains unfinished.

Both ordinary execution and proved cursor completion use the same transaction
body to record the existing CompletedTransfer end and retire its cursor. Either
both commit or both roll back. The worker then uses the established independent
application-result save and recorded confirmation. Failed application saves keep
Retry Save and activity ownership; retries perform metadata work only. Missing,
borrowed or changed task/step evidence cannot mint completion.

Pending filesystem journals still require explicit captured Recovery review.
After successful reconciliation, the original running request can restore its
proved completed result through the same path. A second crash during a resumed
final source cleanup consumes only its actual completed removal, with no recopy
or another unlink of a newer source occupant.

Schema41 adds normalized source-remainder report rows. A completed nonempty
original source-folder task records that descriptive observation before later
tasks. The cursor checkpoints only appended report rows, using its existing scoped
digests, counts and budget. This preserves earlier new source files and their
report through final-leaf recovery without inspecting those completed folders.
Ordinary live returns retain their existing final report refresh. This metadata
grants no execution or deletion permission. The task-table migration retains
older rows/digests; decoding still accepts six/seven-counter headers. Ordinary
continuation checkpoints remain version3, with version4 for copied-source cleanup.

A Claimed source with an absent payload is completed namespace removal, even if
another hard link reintroduces the same inode at its original name. Recovery must
not interpret inode equality as an engine return. Only Ready can use that
no-claim inference; an authorized return has its own durable Returning phase.
This applies equally to original and resumed cleanup and leaves both links intact.

## Verification and limits

Backend library checks cover the final regular leaf with edited/replaced sources
and outputs, replaced/missing output parents, raw names and dangling links, two
opens, explicit journal review, a second crash during final resumed cleanup,
earlier source-folder remainders, corrupted/missing/borrowed evidence, transaction
rollback at end recording or cursor retirement, and application-result Retry Save.
Completed metadata leaves newer occupants and edits untouched. Existing stopped
and active restart gates remain required and unignored.

The final source passes 413 selected backend parents across 20 targets and 181
isolated default desktop-library parents. All-target Clippy with warnings denied,
formatting and the schema41 fixture syntax/hash pass. The earlier snapshot
remains separately identified with its 412/180 results; the hard-link guard and
regression are the only later source changes. Both required restart gates remain
unignored. See TERMINAL-LEAF-WIP.md and the implementation ledger for exact profiles,
summaries, source relationships and preserved failure/source archives. Library
checks do not establish native, workspace, release or physical-device acceptance.
Directory publication/child-queue durability, directory-removal and whole-entry
Move task advancement, reviewed replacements, saved-container phases, complete
Preparing/Cleanup binding, queued admission/lineage, legacy and large-report/paged
owner behavior, and Undo/relocation/expiry remain required. Trash/Restore and all
other areas of the full goal remain unfinished.
