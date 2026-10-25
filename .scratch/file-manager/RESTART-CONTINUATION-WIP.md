# Unfinished Copy/Move continuation after process death

Started2026-10-21 after the verified schema27 completion checkpoint. The full
five-area goal remains active, with unfinished Copy/Move the next priority.

Public red: a_killed_stopped_batch_restores_both_exact_parts_and_keeps_completed_work
really SIGKILLs a child after a directory batch saves failed and remaining work.
The first Copy parent reopens Stopped with both descriptive plans and completed
Undo, but all transfer actions false. It fails as expected in
restart-continuation-red-01.log. Its private dataset is retained at
/tmp/canopa-transfer-restart-bEYC7r and
crates/canopa-files/.canopa-transfer-restart-output-U6YQz1. The parent already edits
the completed prefix and repairs the unselected failed child before reopening.

The same acceptance test requires both part orders for Copy and real cross-
filesystem Move, preserving Keep Both targets and prefix identity/content, with
independently undoable successors. It is deliberately unignored and remains red
until safe execution is actually implemented. No compatibility-only review state
is being substituted for the requested endpoint.

Initial implementation captures a bounded persistent handle digest from original
O_PATH descriptors, paired with boot/unique-mount scope. Selected entries retain
their original parents; destination roots retain their first captured folder.
Unavailable evidence stays unavailable. Checkpoint v2 and schema28 persist these
facts in the initial accepted plan and stopped parts. Decoding still restores no
process execution owner. ADR0112 records the remaining safety gates.

The first library compile caught sibling-module private-field access. Snapshot
construction now stays in the observation owner and passes typed facts through a
narrow constructor. Foundation tests are in progress. Exact step/journal, folder,
stage, cleanup and descriptor-relative mutation proof are still required before
hydration can enable Retry/Continue. The public gate remains authoritative.


## Current evidence,2026-10-22

Core06 passes134 selected parents across8 targets,7 ignored inventory entries.
It deliberately excludes the unignored required public restart gate, which is
run separately in restart-continuation-red-02.log and remains failing. The final
all-target default Clippy (restart-authority-clippy-01.log) and formatting pass.
All832 entries in restart-authority-foundation-inputs-01.json remain unchanged.
This is foundation verification, not full backend/native/workspace acceptance.
The current application executable still belongs to protected schema27 checkpoint
b379372fb3f1; no new schema28 native/build claim is made.

Original capture now opens each selected leaf relative to the same opened
parent whose facts it records, avoiding a parent replacement window. Directory
normalization uses the original opened type, and unsorted/nested selections keep
their initial checkpoint without losing their request intent. New tests bind a
moved original leaf to its old parent and prove source, parent and destination
facts match their actual descriptors. Immutable unavailable facts do not become
available merely because a pathname later appears. v1 input carries no invented
facts; v1 cannot smuggle the v2 original field. Unknown versions still refuse.
The old codec test had incorrectly hardcoded2 as a future version; core02 caught
it, and its unknown-version input is now255. All intermediate logs remain.

The exact engine-start transaction now seals original source/root facts and
SHA256 alongside the accepted Running revision/body association. An SQL guard
in the core test refuses any filesystem journal before that record exists. The
same record survives a later stopped-report checkpoint and replacement source
path; it does not authorize execution from the report. Storage remains charged
to the existing budgets. Initially unavailable root binding and broader step
reconciliation still need their own execution evidence.

Red02 retains /tmp/canopa-transfer-restart-sInMwv and
crates/canopa-files/.canopa-transfer-restart-output-WR1ufh. Read-only inspection in
restart-authority-red-02-inspection.json proves schema28, checkpoint v2, request1
at revision3/phase4, filesystem operation1 at accepted revision2, two completed
changes, one failure and one remaining entry. Both saved parts equal the sealed
original snapshot, all selected/root observations are available, and no stopped
end receipt exists. The surviving prefix contains the parent's newer edits.
No dataset or earlier proof archive was removed.

Next: record exact stopped/step results and their ownership independently of the
application description, then validate original traversal/stage/cleanup objects
and perform mutations through opened objects before minting a restored
continuation. Preserve completed-prefix Undo, both handoff orders, independent
source owners, original device requirements and new-process clipboard intent.
The public gate must pass as implemented behavior before this work is complete.

## Exact stopped-result receipts, 2026-10-22

ADR0113 adds schema29 engine-returned stopped receipts. The filesystem core now
owns the existing report/stopped-result format, with the old application API
paths re-exported. The receipt binds the exact stopped report and both returned
parts to the accepted execution. Startup can restore a lost application result
without running filesystem work. Fatal/uncertain returns and cancellation before
an execution reservation do not invent stopped receipts or execution owners.

Metadata failure retains the actual live result, work/source holds and dispatch
barriers. Retry Save cannot execute a continuation. The receipt uses the existing
4 MiB request allowance rather than the 128 KiB journal-step limit. Its duplicate
full body is retired atomically with the application result, retaining report,
part and saved-body hashes. An engine-scoped witness can save receipt/result in
one transaction when a duplicate would exceed the budget. No limit is raised.

The current source manifest restart-stop-inputs-01.json records834 Cargo/crate/
vendor inputs. Core04 passes139 selected parents across8 targets with7 ignored
inventory entries. The required public restart test remains unignored and is
explicitly excluded from this regression command. Core03 separately passes47
library and18 request-execution parents. New cases cover real killed-process
stopped Copy and cross-filesystem Move, replacement source/destination roots,
newer prefix edits, exact report/Undo across two starts, refused end/result saves,
activity/dispatch holds, corrupt/stale/missing/schema28 receipts, a real stopped
report above128 KiB, atomic rollback and the near-limit live-witness fallback.

Core01 exposed a new test assertion that expected a child destination rather
than the reported top-level directory. Core02 caught Arc serialization and path
comparison mistakes in the new assertions. Corrected checks pass in03/04; all
attempt logs remain. Default-feature all-target Clippy and formatting pass on
this source. The private desktop library also passes115 parents with2 ignored, using
isolated HOME/XDG directories and a private bus. Its runner verifies all834
inputs unchanged after the run. restart-stop-verification-01.json records the
exact commands and evidence; the application binary is still b379372fb3f1.

The required gate fails again in restart-continuation-red-03.log. Its actual
private data remains at /tmp/canopa-transfer-restart-quRBK7 and
/home/kevinard/projects/canopa/.canopa-transfer-restart-output-lgacWA.
Read-only restart-stop-red-03-inspection.json proves schema29, accepted revision2,
saved revision3/phase4, two completed changes, one failed part and one remaining
part. The new stopped receipt is present with its duplicate body retired; all
report/part/saved-body hashes match, and both parts retain the sealed original
snapshot. The completed prefix still has the parent's newer edits. Startup has
no execution owner, so the required Retry/Continue assertion correctly fails.

Next: prove original traversal folders, stages and cleanup objects at their real
creation/observation boundaries; use opened original objects for each mutation;
verify exact journal/step ownership and typed handoff/abandon lineage before
minting restored execution ownership. Then pass both part orders for Copy and
cross-filesystem Move and the full crash matrix. The complete five-area goal
remains active. Current-source native/full-workspace/release verification is not
claimed; the application executable still belongs to the protected schema27
checkpoint.
# Follow-on checkpoint

The subsequent schema30/checkpoint-v3 opened-folder work, receipt codec upgrade
and separately diagnosed history-lock race are tracked in
[RESTART-DIRECTORY-WIP.md](RESTART-DIRECTORY-WIP.md). The required public restart
execution test still fails; this note does not replace or narrow the full goal.
