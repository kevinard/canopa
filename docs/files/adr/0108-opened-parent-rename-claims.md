# 0108: Claim ordinary Rename entries through opened parents

Status: scoped backend, Iced and native verification, 2026-10-20. Continues ADR0103–0106
and Phase2. Other operation families and durable execution remain unfinished.

## Evidence

The cfg(test) boundary hook runs once in the mutation's own thread immediately
before its real rename syscall. It is absent from application builds and has no
environment-controlled product path. Core02/receipt98 replaces the selected leaf
after the exact Running execution and Forward journal exist: Rename reports
success, but the target contains the unselected replacement. The original inode
remains parked. Observation03 confirms a committed history row claiming the
original inode and no pending journal. Core05/receipt99 swaps the whole parent at
the same boundary and reproduces mutation of its replacement child. Both private
fixtures and source-bound failed attempts are preserved.

These failures require neither hostile processes nor kernel tracing. Another
filesystem user changing the entry during an ordinary operation is sufficient.
The earlier proposed seccomp diagnostic was never implemented or run; ordinary
Rust synchronization provides the correct test seam.

## Decision

Core29/receipt107 adds actual public prepare_rename/submit_rename queue tests at
the earlier validation-to-engine handoff. Both leaf and parent replacement are
still adopted: the old source remains parked, replacement moves, row is Finished.
Observation30 confirms the committed change stores the replacement inode rather
than the prepared original. The staging check worked against freshly recaptured
authority. Carry immutable original entry/source-parent/destination-parent facts
from the existing pinned ObservedSources into the exact request execution. Check
the opened entry/parents against those facts. A repeated pathname check cannot
replace this handoff. Direct low-level OperationEngine::execute still establishes
its own observation at that deliberately fresh call boundary.

Ordinary OperationRequest::Rename pins the final entry itself with O_PATH and
NOFOLLOW, opens both followed parent directories, and compares the entry with the
execution's selected identity. Original paths retain Unix bytes; final symlinks
are renamed rather than followed. Parent aliases remain supported. Equal parent
identities and filenames identify the same entry and are refused before creating
an operation. Core11/receipt102 preserves a failing implementation probe that
would otherwise have turned this alias case into a false success/Undo entry.

Create private0700 work through the destination directory descriptor, preserving
the existing random stage naming. Before claiming the source, durably journal
Forward/Rename with that stage and synchronize its parent. Claim with descriptor-
relative NOREPLACE into owned work, then verify the actual identity and current
parent bindings. Files and final links use a payload inside the private stage;
directories use a hidden sibling name derived from that stage in the requested
destination parent. Publish only the selected entry through the opened destination.
Check actual publication and parent bindings, synchronize the held directories,
then checkpoint to the established stage-free Forward format before appending
the completed change. Normal completed Rename and its shared Undo format remain
unchanged; no request payload or descriptor budget increases.

Full36 and minimal38 reproduce a compatibility error in the first implementation:
moving a readonly directory into a staging subdirectory requires permission to
update its '..' entry, whereas an ordinary same-parent Rename succeeds. The
[Linux rename manual](https://man7.org/linux/man-pages/man2/rename.2.html) documents
this permission requirement. The sibling location adds no parent transition to
the user's requested rename and changes no permissions. Stage.sibling_payload
defaults to false and is omitted from ordinary records. The claimed data remains
journalled before it moves. Preserve the private receipt directory while the
sibling exists, expose the sibling to Recovery Copy, and protect its whole tree
from a recovery destination within itself. Generic stage cleanup and other
journal families reject this role; they cannot interpret it as disposable work.

On refusal, return the actual claimed entry through the opened source parent
with NOREPLACE. Pin that actual entry during return. A new occupant must never be
overwritten. Conflicts or uncertain outcomes keep the journal and private data.
Cleanup is descriptor-relative rmdir of the original empty stage only; it never
recursively deletes a claimed file or directory. If a parent moves during
publication, return through the held directories rather than following the new
pathname. Existing Transfer, Trash/Restore and Undo mutation helpers retain their
own behavior and still require their full check/use audit.

Synchronize the stage and source directories immediately after every successful
claim, before identity refusal/return, so an unexpected entry whose return is
blocked receives the same ordering. Synchronize all affected held directories
before clearing a returned claim. The [Linux fsync manual](https://man7.org/linux/man-pages/man2/fsync.2.html)
requires explicit directory synchronization for directory entries. Relative
paths passed to [renameat2](https://man7.org/linux/man-pages/man2/rename.2.html)
use the supplied directory handles; NOREPLACE refuses an existing target.
The actual child tests establish process-interruption behavior, not hardware
power-loss acceptance.

Schema24 reserves a staged Forward/Rename as an unresolved entry claim. Earlier
executables would regard such staging as disposable; the version gate prevents
them from opening it. Upgrade from23 changes only the version marker, preserving
existing completed changes and scheduling bodies. This is independent of the
previously rejected Undo-stage promotion proposal.

## Interruption and limits

A staged Rename is never sent through generic forward cleanup or inferred to be
completed from a new path observation. Recovery leaves it intact, reports the
original paths and exposes an available staged entry for explicit Recovery Copy.
Tests cover blocked return of an unexpected replacement, reopening, Recovery
Copy, and an actual child killed after claim/before publication. Editing the
staged bytes after process death must not cause replay or deletion. Normal
mutations wait while the claim remains unresolved.

This slice supplies no automatic replay authority across restart. Inode
observations alone are not durable original-object/media proof. Explicit claim
reconciliation/review completion, the full crash-boundary matrix, exact execution
end/step receipts, other mutation families and broader Phase2 remain open.
Same-user deliberate interference with private staging is not a sandbox security
claim. Current handles and verification limits are in ATOMIC-RENAME-WIP.md;
the full product and restart contract remain unfinished.

## Current verification

Frozen source45/746 inputs passes606 backend parent tests across67 standard
targets plus the custom device_runtime harness, with40 ignored entries not all
independently executed. The final focused library suite passes31 parents and
the complete directory_permissions target passes41, including the original
readonly retained-history regression under both umasks. The actual interruption
parent kills both a file claim and a readonly-directory claim. Default Iced99
pass/2 ignored, formatting, all-target Clippy with warnings denied and the desktop
build pass with all inputs unchanged.

ELF2ae7fea5867b (134742240 bytes) is protected in a decoded-verified gzip. Six
maintained native cases/40 grouped checks pass on that same executable, dark100%
and light125%, with pinned Niri63e39bae3473. They cover F2 authority, original
return/fresh device selection, Create/Rename/shared Undo and actual commit/save
death with reviewed dismissal. Four critical captures were inspected. No native
microsecond-boundary injection is claimed; that seam is exercised by real public
queue/engine tests with cfg(test) hooks. Six failed private fixtures are archived
with exact members/hashes and their originals retained. Every failed attempt,
compile/lint failure and intermediate source remains reviewable. See
ATOMIC-RENAME-WIP.md, atomic-rename-verification.json and primary review59.
