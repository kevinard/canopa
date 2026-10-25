# Next restart recovery work

The complete five-area user goal remains active. Schema40's current copied-leaf
slice is described by ADR0125 and RESTART-COPIED-LEAF-WIP.md. Preserve the source
archive, captured input manifests, check receipts, red fixtures, previous source39
archive and protected source27 application/Niri/required gate executables.
No commits or vendor edits were made in this slice.

## Next concrete regression

The current normalized cursor decoder requires at least one unfinished part.
Add an actual public-service SIGKILL after the last ordinary copied leaf (and
after the last cross-filesystem source unlink/append). Prove that a completely
consumed queue becomes the existing typed CompletedTransfer end atomically,
without another filesystem operation or reads of edited/replaced completed
outputs. Do not manufacture a stopped part or relax the missing-step refusal.
Use the bound suffix proof already added; missing/corrupt suffixes still refuse.
The existing Store.record_transfer_end transaction may be factored into a
connection helper so cursor retirement and completion share a transaction.
Keep the normal StoppedTransfer invariants unchanged. Explicit pending journal
review should then use the same typed completion path and owner/result-save logic.

## Continue filesystem phases

Directory creation still publishes before DirectoryFinish/child-queue durability.
Capture actual opened source/created-payload facts, reviewed destination and the
already-read Unix child names before publication; persist that typed task phase
and use proved publication to advance. Merge admission needs an equally durable
reviewed folder. Then advance whole-entry Move, reviewed replacement and empty
source-directory removal through their own exact task proofs. Do not expand the
copied-leaf matcher to accept arbitrary changes or merely match pathnames.

Finish saved-container migration, Preparing/Cleanup binding, queued successor
lineage/admission, legacy data, large reports/paged owners, sanctioned Undo and
relocation/expiry integration. General Trash/Restore and Undo restart handling,
renderer separation, product/device/accessibility acceptance, first-window/focus
reliability, optimized performance, native/workspace/release/Niri packaging and
GTK adapter retirement remain required by the full goal.

## Current proofs worth retaining

The copied-leaf Source is captured from held source entry/parent descriptors. A
Move cleanup verifies those originals and the original published copy, both
fingerprints, then uses source_removal::Prepared through staging::VerifiedCopy.
A completed Copy/removal advances only metadata. Checkpoint4 carries cleanup
maps; ordinary empty maps retain version3. The cursor's role6 carries the same
phase; legacy six-count headers still decode. Exact Forward receipts bind current
queue position/hash, accepted execution and canonical committed change. Each next
traversal checkpoint clears obsolete step receipts, including after a rollback
reuses an ordinal. Never drop the unrelated-advanced-history regression.

Public worker startup intentionally refuses automatic journal mutations. The
explicit Reconcile action now includes exact running transfer plans in its
captured reports, then restores them through their independent cursor/end proof
before generic acknowledgment. Other operation families keep generic behavior.
Fresh owners and SaveFailed retries retain activity holds while required.

The public gates cover actual ordinary publication, Copy append, source unlink
and SourceRemoved append; two opens; a second crash while a resumed cleanup
runs; raw names/dangling links; Keep Both folder/leaf names; replacement source,
copy and parents; changed bytes/modes; missing/borrowed/corrupt/mismatching step
proofs and failed stop-save rollback. Retained failure roots are in the decoded
red-fixture archive receipt and must remain available.
