# Source-directory removal restart recovery

Status: scoped backend and isolated default-library verification.
The full five-area goal and complete Copy/Move restart recovery remain open.

## Problem

A cross-filesystem Move removes an emptied source folder after its children
commit. A crash after that removal's append, before the traversal checkpoint,
left a confirmed RemovedDirectory change without an exact durable task receipt.
Restart refused the uncovered suffix instead of restoring the finished result
or offering only later batch work. A returned directory claim also needed to
retire its own uncommitted receipt before restoring its postorder task.

## Decision

Before mutation, the actual RemovedDirectory Forward intent binds its durable
DirectoryFinish head. The matcher requires the exact source and destination,
source/parent/destination identities, captured original folder/parent generations,
mode and operation range/creation facts. An unchanged operation retains the
original first/created ordinals. A resumed operation binds its actual new range
and cannot claim the earlier folder creation. The receipt includes the exact
canonical change, queue key, scoped task hash and step digest. Ordinary copied
leaf matching stays unchanged. No schema or continuation encoding changes.

Restart validates the original accepted request/history and the complete cursor
graph, then checks the exact committed suffix against that pre-mutation receipt.
Only that proved removal consumes the postorder head. Its completion count uses
the existing before-count and captured was_empty facts: an empty Merge counts
once; an already counted created folder gains no second count. Later tasks and
durable reports remain. Finished output and the old source name need no new
filesystem inspection to restore this metadata result.

Ready and Returning directory claims use the dedicated returned-source
transition. It validates the exact normalized intent, independent Forward
context and uncommitted task proof, then retires the receipt and saves empty-only
Preparing in one transaction. Claimed cannot use this transition. A second crash
leaves no payload deletion rights; unexpected stage contents require review.

A Claimed directory with an absent payload records its completed removal without
examining the old source name. A newer entry, including the same original inode
manually placed back at that name, cannot be inferred to be a returned claim.
Original parent/work-directory proofs still protect the actual cleanup namespace.

## Verification and limits

Public OperationJobs checks use real SIGKILL at Ready, Returning, returned,
Claimed, unlink and append boundaries, including a second crash after the return
transaction and a removal in a resumed operation. They cover repeated opens,
edited completed output, recreated sources, a later batch item with a replaced
completed destination folder, empty Merge counts, changed/missing task proofs,
transaction refusal/rollback and foreign private payload preservation.

The repeated original failures and their actual fixtures are archived and
retained. The selected backend checks pass 437 parents across 20 Cargo targets,
33 ignored helpers, and the isolated default library passes 205 parents, 13
ignored. Clippy with warnings denied, formatting and schema42 fixture syntax/hash
pass on the same 867 captured inputs. Both required stopped/active restart gates
remain unignored. [DIRECTORY-FINISH-WIP.md](../../../.scratch/file-manager/DIRECTORY-FINISH-WIP.md)
records exact source/check evidence, the review patch and protected binary hashes.
Broader
Preparing/Cleanup provenance, weak legacy bindings, whole-entry Move, reviewed
replacement and saved-container task phases, queued lineage/admission, large
reports/paged owners, Undo/relocation/expiry and Trash/Restore remain required.
The later backend, product/device/accessibility, physical acceptance,
reliability/performance and release areas remain open. Focused checks do not
establish native, full-workspace, release or hardware acceptance.
