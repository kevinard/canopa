# Undo partial disposal

The previous goal turn made verified progress. The full five-area goal is active
and unchanged. This turn adds verified disposal for non-displaced Undo returns.
No agents, commits, vendor edits or main application/Niri rebuilds. Preserve the
large unrelated dirty worktree, especially Notify.

Authoritative source: undo-disposal-inputs-04.json, 886 inputs, schema51.
The predecessor undo-abort-inputs-09.json (884 inputs) and source archive
ac71335d97405d29f56ff79a2fa5f7bc9e10421e06760ba0fd9d054dc85d1e3d were
verified before editing. Its final results were 526 backend/292 default library,
Clippy02, format02, fixture02, both gates22 and live-decoded 976-member source archive.
Current root/Files/context instructions and ADR0137/0139/0142 were read.

Current source changes:
- staging/disposal.rs: optional exact Undo origin in Disposal; validate only
  SourceRemoved, stage present, displaced None, matching id/target/original stage.
  prepare/advance retain it. begin_undo preflights reproduction before reservation;
  Claimed repeats existing full partial-prefix/raw-link comparison. Consumed keeps
  original one-shot semantics. Existing transfer disposal remains unchanged.
- store/undo_work.rs normalizes the disposal origin for canonical comparison but
  hashes the entire pending disposal body. New child undo_work/disposal.rs privately
  advances the existing binding and reproduces through its original source proof.
- Store::disposal_transition selects Undo or transfer binding; stage_intent require
  delegates Undo disposal to undo_work, forbidding secondary-lane ownership.
- disposal_review selects original Undo binding, supports existing fresh Keep for
  both disposal roots, and checks binding retirement. source_retention checks both
  Undo paths; interrupted paths expose the original history references too.
- undo::recover_reverse accepts cancellation and begins verified disposal for a
  non-displaced unreturned SourceRemoved payload. Live Undo error compensation uses
  a fresh token. Displaced work remains protected but not yet implemented.
- clear_intent verifies own-lane pending/context/stage/Undo rows are absent before
  commit; ignored deletes cannot strand a journal without its owner binding.
- schema51 assertions and desktop fixture updated. No new source adoption for
  schema49/50 legacy unbound preparations.

Two new files (over predecessor) are operations/store/undo_work/disposal.rs and
operations/undo/abort_boundary/disposal_boundary.rs. Existing abort_boundary adds
an unchanged-partial workflow; stage_boundary delegates its cfg(test) phase hook.
All private fixtures use existing /tmp/canopa-undo-abort-* roots and crate-cwd
.canopa-undo-abort-output-* temporary folders; failures are retained. A successful
source-parent replacement case restores its held sibling; its path is recorded in
held-source-parent.txt and printed if a future failure occurs.

Evidence:
- red01/red02, source01: public Undo dies at64KiB/128KiB; unchanged Reconcile fails
  with manual Copy/Keep message. Both bindings retain original source proofs and
  exact prefix bytes. red-probe01 records this. Source01 changes tests only.
- green01, source02: schema51 production implementation,8 parents pass/1 ignored.
- green02, source03:11 parents pass; failed-disposal saves test catches ignored
  retirement reporting Finished after deleting undo_work while pending remains.
  retirement-red02 reproduces; retirement-red-probe01 records pending1/binding0.
- green03, source04:13 parents pass/2 ignored helpers. Six new parents cover
  unchanged retry, six death cuts (reserved/after-claim/claimed/after-unlink/
  holding-gone/roots-gone), consumed/relinked entries (including same inode),
  missing/identical-byte entry/parent replacements, refused/ignored consumed and
  retirement saves, and source/payload edits after Consumed. Changed-source Undo
  after Keep requires explicit ReturnItem. Original destination bytes are checked.

Final validation passed on source04's 886 unchanged build inputs:
532 backend tests /49 ignored across 20 targets, including 217 library/28 ignored and
all batches10..19; 298 normal isolated desktop-feature library tests/29 ignored.
All results have zero failures/filters. Both required active/stopped batch restart
gates pass in12. Default all-target Clippy01 with warnings denied, Files format01
and fixture-check01 schema51/syntax pass. The normal private runner receipt is
continuation-core-1792817787-receipt.json. Backend78964, format87538, Clippy45526 and
default25949 all terminated 0; no test children remain from those completed runs.

verify-undo-disposal.py records final results, attempt/source relations, protected
application/Niri/restart-gate hashes and the 8-root/38-entry failure archive. Final
source and attempt evidence are tracked by undo-disposal-source-01-verification.json;
independently decoded/live checked in undo-disposal-source-final-live-01.json after
the archiver terminates. Do not change included files after archiving without a
fresh source record. Helpers are archive-undo-disposal-source.py and
verify-undo-disposal-source.py.

Eight failure roots,38 entries, are archived and independently live verified:
undo-disposal-failure-fixtures-01.tar.gz SHA
5655029246a16a59b6d68d27567836e7308fdf3142aae1a206aacd2a6a378d05.
All bytes/modes/mtime/objects match; no permission borrowing was needed. Originals
remain in place. Do not confuse them with predecessor failure archive12roots/67entries.
Final source/evidence archive is tracked by the verification records above.

Next: complete displaced Undo retention/rollback and durable empty cleanup after
completed reversals, then other generic Forward/Undo abort paths, broader Undo/
expiry/queued lineage/large reports and Trash/Restore. All backend separation,
product/media/drag/device/accessibility/physical, startup/focus/performance,
native/workspace/Shell/Notify/Niri distribution/release/GTK retirement requirements
remain open. Do not treat this slice as the whole goal.

Use CARGO_INCREMENTAL=0 TMPDIR=/tmp, -j1 --locked --offline. No concurrent Cargo
mutations or replacing any ELF while its current_exe children live. Reclaim only
exact completed own test ELFs under both Cargo locks, with existing audited helper.
Preserve target/debug/canopa-files SHA
3f638343c9cc5ab73fea245013112edbd0e4e8d5189c55dac58b5e1e2ee95cf8 and
target/niri-cancel-upstream/debug/niri SHA
63e39bae347346d7ac5aa90fb974ee8fe09ee59d9bd69238546e990368232da7,
the required operation_transfer_resume gate, all archives, manifests and source.
Finish actual terminal checks, update docs, verify results/unchanged inputs, archive
final source/attempt evidence, then independently decode and verify live hashes.
