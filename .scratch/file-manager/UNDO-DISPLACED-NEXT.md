# Displaced Undo work

The previous goal turn made verified progress. The full five-area goal remains
active and unchanged. Current work is the interrupted Undo Replace flow; do not
claim all Undo or restart recovery complete. No agents, commits, vendor edits or
main app/Niri rebuilds. Preserve the large unrelated dirty worktree and Notify.

Predecessor: undo-disposal-inputs-04.json,886 inputs,schema51, verified before edits.
Its source archive SHA is
683e69ad419794e33cc85998c7eef33e0173138be12655d8f386a8b3ef593575;
944 members independently decoded/live verified. Final532 backend/298 normal
private desktop library, both gates12, Clippy/format/fixture01 passed. Root/Files/
context docs and ADR0143 were read this turn. Diagnose skill remains in use; red
runs and ranked hypotheses were shown before fixing.

Current source: undo-displaced-inputs-04.json,889 inputs,schema52. New files:
operations/undo/displaced.rs; operations/store/undo_work/displaced.rs;
operations/undo/abort_boundary/displaced_boundary.rs. Existing abort_boundary,
rename_boundary, staging, undo, undo_work, schema asserts and desktop fixture changed.

Actual red01/red02 on source01: public cross-filesystem Move completes; Undo sees
newer source-location work and accepts Replace; it dies at64KiB of its return copy.
The parent edits the partial. Reconcile preserves it but public Keep is false.
Both failed roots have undo_work1, return-root generation proof, and no displaced-
root ownership. red-probe01 records these. Four roots are retained:
/tmp/canopa-undo-abort-MuCHnx, /tmp/canopa-undo-abort-iwJRA7 and crate-cwd private
.canopa-undo-abort-output-tOlfHn / .canopa-undo-abort-output-8pLpYe.
These are archived and independently live verified:4 roots/22 entries, SHA
de09df4e5de20a103d7ada743c78902d1af1ae55a078efd31506e917f2820dd9.
Originals remain; no permission borrowing was needed. Preserve any new failures.

Implementation:
- SourceRemoved conflict observation opens the actual original-location item with
  O_PATH/NOFOLLOW before the question; its Version comes from that held descriptor.
- New undo::displaced Observed/Prepared creates original owned work roots and saves
  the selected item's Original as previous-origin proof. It never captures recovery
  paths as new authority. Prepared uses verified empty-only cleanup on Drop, not a
  recursive TempDir deletion; keep after successful registration retains its root.
- New register_undo_displaced accepts actual Prepared, validates its record, writes
  exact initial PendingUndo(stageNone,displacedSome) and undo_work binding atomically
  against original history generation/canonical step. No source descriptor is
  claimed until the actual copy-reader registration extends this same owner.
- undo_work owner supports this pre-copy state. require includes SourceRemoved
  displacedSome even with stageNone; it requires modern displaced root/previous
  facts. Old unproven displaced journals cannot gain cleanup authority from current
  paths. Return and displaced stages must be distinct. The copy registration may
  extend sourceNone only from its own initial stageNone binding; later source proof
  must remain unchanged.
- Fresh Keep review captures return root/payload and displaced root plus either
  original previous entry or original still-unmoved selected entry. Stale/replaced
  roots or previous entries refuse. Keep archives available previous leaf and entire
  displaced root alongside return work, including extra children, then verifies
  exact atomic journal/binding retirement. Original Undo history remains unchanged.
- Modern snapshot_displaced handles a proven root whose previous entry has not yet
  moved by returning its unchanged record. The existing rollback path still needs
  complete durable one-shot handling; do not claim it finished. The initial empty
  pre-copy preparation can reconcile without reusing its old Replace answer.
- Schema52 prevents old executables from interpreting new pre-copy Undo ownership.
  Current non-displaced disposal stays schema51's Reserved/Claimed/Consumed protocol.
  Displaced auto-disposal/rollback is still protected, not yet automatic.
- Recovery slot3 remains existing source quarantine. Keep from return slots0/1 or
  displaced leaf2 archives the whole displaced root; the root is copyable as a normal
  archived item afterwards. No fresh Keep review is attached to slot3.

Attempts:
source02 / green01 did not compile: Ownership is boxed and Option<&Box<Ownership>>
needed a closure. Source03 fixes those two types. green02 passes14 parents/2 ignored,
including the new edited Replace Copy/Keep/retry and the prior13 parents.
Source04 adds three parents and two real SIGKILL hooks: after independent initial
registration before copy and before payload publication after the original occupant
was actually moved. The new parents cover initial replacement-root refusal/empty
reconciliation/fresh Replace answer, two restart Keep/Undo/whole-root Copy after
actual displacement with extra children, and stale replaced previous/root refusal.
No new subprocess helper; existing undo_abort_child is extended via private env.

Final focused green03 on source04 passed 17 parents/2 ignored. Final validation
passes 536 backend tests/49 ignored across 20 targets (221 library/28 ignored, batches
10..19) and 302 normal isolated desktop-feature library tests/29 ignored. All results
have zero failures/filters. Both required active/stopped batch restart gates pass
in12. Default all-target Clippy01 with warnings denied, Files format01 and fixture-
check01 schema52/syntax pass. All 889 source04 build inputs stayed unchanged. The
normal private runner receipt is continuation-core-1792820459-receipt.json.

All test/static execution handles are closed: source01 red10959/31800 terminated101,
source02 compile58013 terminated101, source03 green94915 terminated0, source04
focused23635 terminated0, Clippy64715 terminated0, format31093 terminated0,
backend60725 terminated0, default61038 terminated0. No children remain from those
completed runs. Source02's failures are compile types, not a runtime safety finding.

verify-undo-displaced.py records final results, attempt/source relations, protected
app/Niri/restart-gate hashes and the 4-root/22-entry failure archive. Final source
and attempt evidence are tracked by undo-displaced-source-01-verification.json;
independently decoded/current-hash verified by undo-displaced-source-final-live-01.json
after the archiver terminates. Helpers are archive-undo-displaced-source.py and
verify-undo-displaced-source.py. Do not change included files after archiving without
preserving a fresh source record. No native/workspace/release or physical acceptance
is claimed here.

Do not overstate scope. Durable displaced rollback/automatic continuation and
completed-reversal empty cleanup remain, then other generic Forward/Undo paths,
broader Undo/expiry/queued lineage/large reports and Trash/Restore. All backend
separation, product/media/drag/device/accessibility/physical, reliability/performance,
native/workspace/Shell/Notify/Niri distribution/release/GTK retirement remain open.

Use CARGO_INCREMENTAL=0 TMPDIR=/tmp, -j1 --locked --offline; no Rust wrappers and no
concurrent Cargo mutations. Never replace an ELF while current_exe children live.
Reclaim only exact completed own test ELFs under both Cargo locks, with the audited
helper. Preserve app SHA
3f638343c9cc5ab73fea245013112edbd0e4e8d5189c55dac58b5e1e2ee95cf8,
Niri SHA63e39bae347346d7ac5aa90fb974ee8fe09ee59d9bd69238546e990368232da7,
the required operation_transfer_resume gate and all prior source/evidence archives.
