# Original Copy Undo restart: validated checkpoint

The entire five-area Files goal remains active. This slice does not complete it.
Next priority: Copy Undo with a saved previous destination, then remaining generic
Forward/Undo cleanup and Trash/Restore restart recovery. Preserve all other areas.

## Authoritative validation

Final source: copy-undo-inputs-11.json,896 inputs,schema55.
- Source07 unfiltered backend library01:245pass/29ignored. Batches10..18 pass18
  integration targets. Final batch19 initially10pass/2fail/2ignored.
- Only tests/operation_recovery.rs changes between source07 and11. All production
  and other tested inputs are byte-identical, proved in source-bridge01. The complete
  corrected integration target passes12/2ignored on source11 (recovery-green02).
  Combined560pass/50ignored over20targets, zero parent filters. Do not claim all20
  reran against one unchanged manifest. Both required active/stopped gates12 pass.
- Normal isolated default desktop-feature library01:326pass/30ignored, zero parent
  filters. Receipt continuation-core-1792838551-receipt.json, all896 inputs unchanged.
  Private HOME/XDG/private D-Bus,no display/system bus/host activation. Terminated0.
- Final default/all-target Clippy02 -Dwarnings,format03,fixture-check03 pass.
- verify-copy-undo.py passed: copy-undo-verification-01.json/log. Protected app/Niri
  hashes unchanged; final required restart-gate hash captured. Every code validation
  process and verifier terminated. Source bundling and independent decoded/live
  verification are the last artifact steps; read source-final-live01 for their result.

No native/workspace/Shell/Notify/Niri-distribution/package/release acceptance claimed.
ADR0147, Files context and ledger are updated. Final source/evidence bundle is
copy-undo-source-01.tar.gz; source-01-verification and source-final-live01 record
its actual SHA/member hashes and independent decoded/live verification. Do not edit
archived members afterward. No commit/worktree/vendor changes made in this slice;
large unrelated Notify work stays intact.

## Implementation

undo/copied.rs retains actual creator descriptors and the selected copied entry
before binding. Pending::UndoCopied and per-lane copy_undo bind original history
generation, canonical Change and pending digest. Prepared/Taking/Detached/Consumed/
Cleared/Preserved phases retire source rename and payload unlink permissions before
single live native operations. Restart never repeats Taking rename or Consumed
unlink, even if the same inode reappears. Prepared may finish or compensate with
canonical history unchanged. Detached checks original and exact snapshot/fingerprint.
Uncertain data stays available for whole-root Recovery. Fingerprint reads remain
outside metadata transactions; strict opened snapshots are checked around reads.
Source-retention changed-path checks include the new intent.

Completion atomically commits reversal plus original empty-only UndoCleanup receipt.
The shared receipt installer now verifies the exact changes row was actually removed
inside that transaction, so an ignored deletion rolls the whole handoff back. No
completed target is reopened by receipt cleanup. Modern roots skip legacy live
clean_completed_stage. Missing/foreign roots and parent generations are not adopted.

Private fresh copy_undo Review can archive the whole original root and retire its
exact pending/binding atomically, preserving uncompleted original Undo history.
Stale roots/reviews and deserialized rows cannot grant Keep. Copy uses its independent
Recovery lane; export and original Undo pass. Raw filenames/link targets survive.
Ordinary retained:Some and changed-copy preservation remain separate legacy paths.

## Preserved attempts

- inputs01:894 inputs,schema54,test-only completion hook + child. red01/02 both101;
  public completed Copy Undo leaves empty root after successful Reconcile, all owners0.
- inputs02:896,schema55,initial implementation. green01:23pass/4ignored across existing
  transfer copy_boundary plus new completion parent.
- inputs03:private Keep/phase tests; green02 compile fails private module visibility.
- inputs04:visibility and actual same-inode reuse. green03:3 focused parents pass.
- inputs05:replacements/tampering/refused writes. green04:5pass/1fail. write-red02
  repeats unchanged. Probe02: PendingUndoCleanup,copy_undo0,changes1 after ignored
  DELETE changes. Probe01 preserves full SQLite schema. Original bytes remain protected.
- inputs06:atomic retired-step fix and Copy/Keep/export. green05:7parents pass.
- inputs07:initial registration and raw-link checks. green06:9parents/1ignored; static
  checks pass. Full library and18integration targets pass; core19 has10pass/2fail.
- inputs08 only preserves failing integration fixtures; recovery-red02 repeats failure.
- inputs09 adds temporary test-only page trace; probe03 proves whole-root available
  row/private Keep, source and copied16MiB intact,newer21byte occupant intact.
- inputs10 modernizes acceptance but places Keep before interrupted-count/refusal
  assertions; recovery-green01 has10pass/2fail. Four later failure fixtures retained.
- inputs11 moves Keep after refused reconcile and removes diagnostics; unfiltered
  recovery-green02:12pass/2ignored. The public workflow verifies whole-folder Copy,
  original journal still present after export, refused reconcile,newer occupant intact,
  fresh Keep,successful reconcile,archived bytes through export Undo and normal work.
  Both queued-runtime and direct-engine/no-scheduling-row variants pass.

First fixture check used a wrong text assertion; check02 passed on source07 and
check03 on final11. Production never changed after source07. Source11's Clippy02,
normal default library and all artifact verification use the final manifest.

## Retained failure evidence

Three decoded and independently live-verified archives preserve8roots/90entries.
All bytes/modes/mtime/original filesystem objects match; no permission borrowing.
01:2roots/24entries SHA
27dedc70e177a8cb8e805e9df5f822443a2d679755d6ea520cfc801051130801.
02:2roots/18entries SHA
89759188d88d94d42d01339cb63b9fd506040c968f60784f7d3b0f1a3daaf497.
03:4roots/48entries SHA
70b44895ee1a3d90ae9a1139a7a3894d3bc4d5ef71fb1545bc141403c12b66a4.
Original orphan roots /tmp/canopa-copy-undo-WU2qvf and -JfmDcc;
ignored-deletion roots -iAk2Uk and -tomnWZ. Runtime workflow roots are
/tmp/canopa-files-recovery-interrupted-{VtLjKq,YH8f1R,fwvdgL,UYolTt}.
Initial broad core19 failures dropped normally; later repeats preserve actual data.
Do not delete or mutate any originals, archives or previous goal evidence.

Predecessor schema54/source undo-cleanup-inputs-13.json:893 inputs,551backend and
317normal private default, both restart gates, all static checks passed. Archive
SHAa6ced6ae47e13c42a40ad81be224f71a532d5cd951d2d25c5ba66733d541c5b9
has1027 decoded/live-verified members. Reverified at this goal turn's start.

## Continuing work

Reproduce the next retained:Some Copy Undo crash through a real public worker before
changing that protocol. Preserve its current copied file and previous saved version;
separate one-shot claims for current removal and previous restoration, bind actual
creator/selected generations, and derive completion cleanup atomically. Do not
pretend existing scalar-only legacy stages supply original execution authority.
Broader Recovery leases/expiry/queued lineage/large-report acceptance remains.

Then finish generic/legacy Forward/Undo/cleanup and Trash/Restore, nav/search/preview
backend separation, product/media/drag/device/accessibility/physical checks,
startup/focus reliability and optimized performance, complete native/workspace and
Shell/Notify regression checks,compatible Niri distribution,package smoke/docs/GTK
retirement. The full goal remains active, not blocked, paused or complete.

Run from workspace root; CARGO_INCREMENTAL=0 TMPDIR=/tmp,-j1,--locked,--offline.
No concurrent Cargo mutation or rebuilding a current_exe helper while children live.
Reclaim only exact completed owned test ELFs with reclaim-leaf-test-cache.py under
both Cargo locks; retain audits, app/Niri/gate/rlibs/evidence. All filesystem tests
use private fixtures. Do not run mutating tests on user files or activate host UI.
