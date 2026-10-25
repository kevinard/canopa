# Saved-container restart tasks — work in progress

The full five-area user goal remains active. This slice follows ADR0132; it is
not release acceptance or complete Copy/Move recovery.

The public OperationJobs Move of an actual retained-data container reproduced an
Interrupted completed task twice (saved-task-red-01/02.log). Both killed worker
roots are preserved in /tmp. The existing saved Move used scalar path/parent
metadata and had no independently bound active task. Ranked hypotheses and
implementation direction were reported before edits: bind the exact task, move
through held descriptors and original saved-data generations, and derive catalog
parent rebasing from those facts.

Current unverified changes: schema43; strong saved Movement proof holds original
source parent, container, previous entry and destination parent plus whether a
nested saved-container step contributes to item counts. Same-filesystem saved
Moves use existing descriptor-held entry Move claims, with a distinct task
matcher and returned-task disposition. Canonical saved-container owner records
rebase their ownership to the captured destination parent. Previous data is
verified through its existing catalog proof; legacy or recreated data without
proof remains reviewable and cannot acquire authority from current paths.

Validation pending. Preserve all prior archives and protected application/Niri/
restart-gate binaries. Cross-filesystem saved migrations, full Preparing/Cleanup
provenance, legacy and queued lineage, Undo/expiry, Trash/Restore and the remaining
user goal are still unfinished. Temporary compiler files belong under /tmp.

The initial completed-task repro now passes (green01, 870 inputs01 captured after
the terminal build). Five public parents pass phase02: completed result, original
Ready/intent return and Remaining through older Undo, Publishing/published with
new source occupants, previous/parent/work substitution refusal, and deleted or
corrupt receipts. The phase01 missing-receipt failure was reproduced separately
(receipt-red02). Its retained roots show filesystem publication before job-proof
refusal. `require_bound_forward` previously checked only journal context. A common
read-only task binding now validates the existing exact receipt before recovery
prepares directory access or opens mutation targets, using the same head matcher
as initial binding. It cannot insert a receipt. This also covers other supported
modern Forward families. Unsupported legacy shapes remain a separate open audit.

A restored previous child changes the container version; reconciliation returns
the original container safely and Remaining completes it. The first phase01
expectation of immediate Finished was corrected to check this safe disposition
and actual continuation. It did not indicate loss or adoption of the substitute.

Phase03 extends tests to Returning/returned, empty-only return cleanup with a
foreign payload, transaction rollback, raw previous links, Keep Both plus later
batch work and nested saved-container task counts. This run is in progress.

Phase03 passes all nine public parents plus one ignored child helper. The complete
no-default-feature library passes 151 tests, 17 ignored helpers. Formatted source
was captured as inputs02 (870 paths) before broad checks. The first integration
batches (saved-container/Undo, job/checkpoints, active/stopped restart gates and
request execution) pass without input changes. Remaining batches and default
library/static checks are running. The five retained failed-fixture roots were
archived and every decoded file verified in saved-task-failure-fixtures-01.json;
original /tmp roots remain intact.

All 464 selected backend parents pass across 20 Cargo targets, 38 ignored,
on inputs02. Both required active/stopped restart gates are unignored and pass.
Clippy01 refused two collapsible_if guards in store/stages.rs. The original file
is retained with its inputs02 hash; source-relation01 and clippy-source-diff show
only those two equivalent guard simplifications. Final formatted inputs03 has
870 paths. Default library and Clippy02 use inputs03; backend integration remains
explicitly inputs02 rather than claiming those binaries came from inputs03.


Final checkpoint: all nine new public-worker parents pass in the 151-test backend
library (17 ignored) and final 232-test isolated default library (18 ignored).
The selected backend set totals 464 parents/20 targets/38 ignored on inputs02.
Final default library, all-target Clippy with warnings denied, workspace format
and schema43 fixture AST/hash pass on inputs03, after only the two recorded
Clippy guard simplifications. No semantic correction followed the broad suite.
Verification01 validates all current 870 source hashes, exact source relation,
per-target terminal summaries, both required unignored restart gates and all
protected binary hashes. The final source/docs/proofs are preserved by
archive-saved-task.py and its saved-task-source-02-verification.json companion,
which decodes and hashes every archive member. No commits or vendor changes.

Same-filesystem saved-container whole-Move task recovery is verified in this
scope. Cross-filesystem saved migration, broader provenance/queued lineage,
Undo/expiry, Trash/Restore and all remaining user goal areas are unfinished.
See SAVED-TASK-NEXT.md. This is not native/workspace/release/device acceptance.

The first archive decoded correctly but included its own still-growing redirected
output log. Only that log changed after its snapshot. Archive01 and its proof are
preserved; archive02 excludes archiver output logs and emits metadata directly.
Implementation inputs and terminal test records are unchanged. The final proof
also verifies all archived live members after the archiver has returned.
