# Verified disposal of interrupted Undo returns

Status: schema51 implementation verified on undo-disposal-inputs-04.json.
Continues ADR0142 and ADR0137–0139. The complete Files goal remains active.

A real public worker twice reproduced unchanged Undo return work remaining blocked
after restart. It died after writing the first 64 KiB of a 128 KiB return copy.
Both original source generations were recorded and the partial matched its source
prefix, but Reconcile required manual retention rather than permitting retry.

Reuse the existing Reserved/Claimed/Consumed disposal protocol for a non-displaced
SourceRemoved Undo. Disposal carries the exact original Undo intent. Its private
phase transition advances the independent Undo binding atomically with the journal,
preserving original history generation, operation/ordinal, canonical history body,
source reproduction proof, relocation and original work root. Generic intents cannot
create or replace this owner. Other transfers retain their existing stage binding.
Schema51 prevents older executables from interpreting these records as ordinary
transfer disposal.

Before reserving another namespace, verify that the Undo partial can be reproduced
from its original opened source. Edited partials, missing sources and identical-byte
replacement sources remain in the original work folder. After claiming the partial,
repeat the comparison and hold both readers through the consumed save and final
current-name/version checks. Cancellation remains bounded during comparison. Live
Undo compensation uses a separate cancellation token so cancellation can finish
verified cleanup and preserve its original outcome.

Only the live consumed transition may unlink once. Restarted Consumed work has
empty-root rights only, even if the same original inode is linked back into a work
name. A still-present partial remains available through Recovery Copy and fresh
Keep review. Keep verifies the exact Undo binding and both root snapshots, archives
available leaves and whole roots, and atomically retires the journal/binding. It
does not consume original Undo history or grant file-operation execution.

New acceptance twice reproduced another actual bug: ignoring the journal DELETE
committed binding deletion and reported successful reconciliation with the old
journal still present. Journal retirement now verifies absence of all own-lane
journal/context/stage/Undo binding rows before the surrounding transaction commits.
Ignored journal or binding deletion rolls back the entire metadata retirement.
Refused and ignored phase saves retain their exact earlier journal/binding too.

Thirteen focused public acceptance parents pass, including the prior edited-partial
workflow and six new parents: unchanged retry, six process-death cuts, consumed and
reintroduced entries, unavailable/replaced sources, failed phase/retirement saves,
and late source/payload edits. A changed source returned after Keep requires its
ordinary explicit Undo review. The completed moved destination remains intact until
that Undo runs.

Final verification passes 532 backend tests across 20 targets (49 ignored helpers)
and 298 normal isolated desktop-feature library tests (29 ignored), with zero
failures or filters. Both required active/stopped batch restart gates, default
all-target Clippy with warnings denied, Files formatting and fixture schema/syntax
checks pass. All 886 captured build inputs stayed unchanged. Exact attempts are in
`.scratch/file-manager/undo-disposal-verification-01.json`. Eight failed-fixture
roots retain 38 decoded and separately live-verified entries. Final source and
evidence are tracked by `undo-disposal-source-01-verification.json` beside it.

This does not finish displaced Undo preparation/rollback, durable empty cleanup
after a completed reversal, or other generic Forward/Undo cleanup. Broader Undo,
Trash/Restore, backend separation, product/device/accessibility/physical acceptance,
reliability/performance and full native/workspace/release integration remain open.
