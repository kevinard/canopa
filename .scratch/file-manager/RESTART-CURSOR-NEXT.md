# Next: advance the active cursor through its interrupted filesystem step

The current cursor can resume the exact queue only at its matching history
watermark with no journal or directory-access work. It deliberately refuses an
advanced step. Do not remove that refusal or reconstruct work from current names.
The full goal remains Copy/Move, all restart families and the other four areas.

## First regression seam

Add a real public OperationJobs killed-worker gate after Copy publication/append
and before cross-filesystem Move source removal. The existing live engine emits
`TransferEvent::Verifying` there, but an asynchronous snapshot can miss it; use a
small cfg(test) boundary at that exact call site (as with existing publication and
source-removal boundaries), then kill the actual worker process. Preserve an
edited completed prefix, exact target names, and two subsequent opens. Test both
an unchanged original source/output and replaced or edited originals. Retain the
unignored gate until safe continuation passes.

## Missing durable source fact

`Change::Copied` records the original publication but not which actual opened
source supplied it. Initial selection facts cover request roots; they cannot prove
an unselected child after a copy committed. Capture source path, entry/parent
Originals and source fingerprint from `fingerprint::source::Opened` while its
actual pins still exist. Do not observe that path later to manufacture authority.
Ordinary Copy completion can retire the copied task without reading current
completed bytes; Move cleanup must prove the recorded original source and the
original unchanged published copy before removing that source.

`source_removal::Prepared::open/commit` currently need `staging::Prepared`, but
only use `published`, `metadata`, `require_published` and `fingerprint` from that
copy. Introduce a narrow verified-copy interface, implemented by live Prepared
and a reopened original publication. Reopening must prove the recorded publication
against held descriptors before any read; source removal must likewise prove the
recorded original source and unchanged source fingerprint before claiming it.

## Task phase and step ownership

Extend the opaque continuation and normalized ledger with an explicit copied-leaf
cleanup phase. Keep the ordinary queue entry, but consume that phase through
source removal instead of copying again. Persist it in stopped plans as well as
the active ledger. A per-source map is a smaller change than replacing every
existing `(source,destination,finish)` tuple. A new ledger role and checkpoint
codec need explicit versioning and safe legacy behavior.

Bind the active task to its exact Forward intent/slot, including the reviewed
actual destination after Keep both, before filesystem mutation. A matching history
suffix then proves which substep completed; an unrelated or cloned change cannot
advance the head. Existing `active_cursor_refuses_corrupt_borrowed_or_advanced_work`
adds a prior prefix change to a different active task: that must remain refused.
Cursor retirement, counters and task-phase changes must be atomic with their
owned step evidence. Do not count Move complete merely because its copy exists.

## Other required phases

Directory creation publishes before its DirectoryFinish and child queue are
currently persisted. Capture the earlier source and actual created payload
originals, reviewed target and already-read Unix child names before publication;
advance after proved publication. Merge initialization also needs a durable
reviewed folder before children run. Completed unlink/rename can restore metadata
without touching newer occupants; returning/claimed stages retain their limited
rights. Saved-container migration, Preparing/Cleanup binding, queued successor
lineage/admission, large restored reports/paged owners and sanctioned relocation
remain required. Normal end receipts now preserve source remainders and final-file
cancellation/attention, and successful startup Retry Save restores proved owners;
keep their regressions green.
