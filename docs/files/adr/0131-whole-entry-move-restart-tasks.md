# Whole-entry Move restart tasks

Status: scoped backend and isolated default-library verification.
The full five-area goal and complete Copy/Move restart recovery remain open.

## Problem

An ordinary whole-entry Move can append its Rename and finish private cleanup
before the traversal checkpoint. Two actual public-worker crashes leave one
confirmed Move, the unchanged queue head and no task receipt. Restart refuses
the uncovered history suffix. Adding the receipt alone exposes a second gap:
the traversal decoder understands copied leaves and directory phases, but cannot
consume this already completed Move.

## Decision

The actual ordinary Move Forward intent binds its durable queue head before
mutation. The matcher requires a non-postorder task, exact source, destination
parent, Move facts and matching entry identity. The typed prepared operation
retains the selected descriptor and both original parents across conflict review;
Keep Both can change the destination leaf. The receipt stores that exact chosen
path, all canonical Move facts, queue key, scoped task hash and step digest.
Copied-leaf matching stays unchanged. Replacement and saved-container movements
retain their separate phases and cannot use this matcher.

Restart validates the original accepted request/history, cursor graph and exact
committed suffix against its pre-mutation receipt. A dedicated decoder consumes
one whole-entry Move, counts one completed item and records its actual root
destination. It leaves byte counts as live whole-entry Move reports do. Restoring
this completed metadata grants no new rights to inspect or move either entry;
new source occupants, edited or replaced completed output and later unrelated
batch work remain independent. No schema or continuation encoding changes.

Ready and Returning ordinary Move claims use the dedicated returned-source
transition. It verifies the exact normalized empty-only Preparing intent,
independent Forward context and uncommitted head receipt, then retires that
receipt and writes Preparing in one transaction. SQL refusal rolls back both.
A second crash leaves only empty-stage cleanup rights. A Publishing claim with
no private payload cannot infer a return from its old source name, even if the
same original inode has been put back there.

A phase update of an existing Forward Move must find its original receipt and
unchanged canonical change. It cannot insert a replacement for missing evidence.
The original implementation's INSERT OR IGNORE otherwise recreated a deleted
receipt while recording Returning. Basic returns concealed this because an
unchanged history watermark bypassed committed-suffix verification.

Live descriptor-held Moves continue to work when generation capture is
unavailable. Their exact committed receipts can restore metadata results;
unfinished restart mutation and continuation factories still require the original
generation proofs. No path observation supplies fresh authority over a replacement.

## Verification and limits

Nine public worker parents cover real SIGKILL at intent, Ready, Publishing,
publication, completed cleanup and return, including a second crash after the
atomic disposition. File, raw-name dangling link and folder cases preserve
original inode/content, edited output and new source entries. Other cases cover
Keep Both with later batch work after completed entry replacement, missing and
changed committed/uncommitted proofs, return transaction rollback, foreign private
payloads, and a missing/replaced publication with its original inode reintroduced
at the source name. Repeated opens, explicit Reconcile and Continue use the public
OperationJobs service. Failure roots, decoded archives and exact source/check
records are retained in WHOLE-MOVE-WIP.md and its referenced verification files.

The selected backend checks pass 446 parents across 20 Cargo targets, 35 ignored
helpers; the isolated default library passes 214 parents, 15 ignored. Default
all-target Clippy with warnings denied, formatting and schema42 fixture syntax/
hash pass on the same 868 captured inputs. Both required stopped/active restart
gates remain unignored. The first integration attempt exhausted temporary build
space; a fresh attempt with /tmp temporaries passed without source changes.
[WHOLE-MOVE-WIP.md](../../../.scratch/file-manager/WHOLE-MOVE-WIP.md) preserves exact
source relationships, failed/retry evidence, archives and protected binary hashes.

Each transfer family needs public-worker acceptance for its initial task receipt,
phase updates without fresh evidence, committed suffix projection, returned-step
disposition and second-crash cleanup. Filesystem-journal reconciliation alone
does not prove that its unfinished application task can safely continue.

Reviewed replacement and saved-container task recovery remain next. Broader
Preparing/Cleanup provenance, weak legacy binding handling, queued admission and
lineage, arbitrary-size reports/paged owners, sanctioned Undo/relocation/expiry
and Trash/Restore remain. Backend navigation/search/preview separation, product/
device/accessibility/physical acceptance, reliability/performance and complete
native/workspace/release/Shell/Notify/Niri integration and GTK reference retirement
remain open. Focused checks cannot establish those wider acceptance requirements.
