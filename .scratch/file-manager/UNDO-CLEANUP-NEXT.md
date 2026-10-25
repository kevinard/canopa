# Completed Undo cleanup: validated checkpoint

## Authoritative result

The full five-area Files goal remains active and unchanged. This is verified progress
for original-generation Undo work folders; it does not finish legacy/generic cleanup,
Copy/Move, Undo, Trash/Restore or the full product.

Final source: undo-cleanup-inputs-13.json,893 unchanged inputs,schema54.
- Backend attempt03:551pass/49ignored across20 targets, zero failures/filters in
  parent runs. lib03 and batches30..39 pass; both required active/stopped gates32 pass.
- Normal isolated desktop-feature lib01:317pass/29ignored, zero failures/filters.
  Receipt continuation-core-1792833005-receipt.json. Private HOME/XDG/private D-Bus,
  no display/system bus/host session activation. No native/workspace/release claim.
- Default/all-target Clippy05 -Dwarnings, Files format03 and fixture-check03
  schema54/syntax pass. Temporary parent diagnostics/environment flag absent in Rust.
- Separate unfiltered cross-green01 passes19parents/1ignored on final source,
  including killed reviewed/saved-only returns. Six new cleanup parents are covered
  by final full library and normal desktop-feature runs.

Backend26250, desktop63834, Clippy45713, format51542 and verification13022 all
terminated0. No validation process remains live. undo-cleanup-verification-01.json
records exact attempts, source relation, counts, gates, receipt and protected hashes.
Source/evidence bundle: undo-cleanup-source-01.tar.gz. The source verification JSON
records its member hashes; source-final-live01 independently verifies decoded and
live members/protected binaries after archiver completion. Consult actual records
for the final archive SHA. Do not edit captured members after that verification.

## Final implementation

Pending::UndoCleanup plus per-lane undo_cleanup binding derives modern empty-only
roots from exact pending Undo/history. Reversal+journal+independent history-generation
binding commit atomically. Immediate/restarted cleanup verifies original parent/root
and only rmdirs empty roots, without accessing or replaying completed return targets.
Unexpected contents/replacement roots stay protected. Failed/ignored saves preserve
exact earlier ownership. Whole-root Recovery Copy uses its separate journal; a fresh
private Keep review archives original folders before exact receipt retirement.

Directory-return plans record destination Original before children move. Direct
saved-container handoff propagates that proven parent while retaining the original
container generation. Copied migrations keep their actual created-copy rebind.
identity_ref_origins follows exact surviving identity_refs (matching expected identity,
FK lifetime), so older Undo can use a parent created by a later Undo after its frame
retires. Optional saved_stage::Parent is boxed with the same JSON wire shape.

Saved-only restoration completes SourceRemoved using the exact paired Copied step's
retained root. Shared cross::saved_only validation supplies that original owner to
both cleanup derivation and metadata completion, preserving all existing pairing,
path, replacement identity and generation constraints.

Six new parents cover committed/roots-gone deaths, two restarts, return edits,
remaining original Undo, reintroduced output/extra children, Copy/Keep/export Undo,
replacement roots, seven refused/ignored writes and three tamper cases. Existing
four-case merge test covers both filesystems, Skip/resume and restart. Existing
public Undo child reused; no new ignored helper. Production store/reverse_cleanup.rs;
new tests undo/abort_boundary/cleanup_boundary.rs. ADR0146 and ledger updated.

## Preserved attempts

- Source01/red01,red02: actual public death after SourceRemoved Undo commit leaves
  original empty root with pending0/undo_work0/stage_intents0. Matching device/inode,
  one Copied step left, returned-file edits intact. Both runs terminated101.
- Source02/green01: original public parent passes after durable receipt.
- Source03/green02: six parents pass; Clippy01 passes.
- Source04: canonical saved-root comparison; static02/01/01 pass. Backend01 has
  lib01 PASS236/28ignored, core10 saved target23pass/1fail, unavailable12pass. Existing
  folder-merge failure repeats unchanged in merge-red02; broad driver terminated1.
- Source05 changes ONLY test Fixture Drop preservation. merge-probe03 repeats failure,
  retaining source .tmpkRByyW/output3kXQES. PendingUndoCleanup op2/0, original empty
  root, older bytes returned, op1/0 left. Corrected handle probe02: root true,parent
  false; scalar parent had changed without its original generation.
- Source06 records parent handoff but applies direct-source proof to recreated
  copied container. merge-green01/red04 repeat forward failure. Actual selected
  parent and created CONTAINER match; old source does not.
- Source07 skips direct updates for copied migrations. green02/red05 still fail
  last same-filesystem Skip/resume case.
- Source08 temporary trace01 shows3 combinations pass; op4 creates parent then
  retires graph, op3 later gets only scalar parent, original=false.
- Source09 persists generation on surviving refs; trace-green01 passesall4 and last
  op3 handoff original=true.
- Source10 removes diagnostics; Clippy03 fails large_enum_variant from inline Parent
  proof (680vs448 bytes).
- Source11 boxes Parent; Clippy04/format02/fixture02 pass. Backend02 lib02 PASS236,
  core20..26 allPASS/gates22; core27 UndoCross17pass/2fail, directoryCross6pass. Saved-only
  pair mismatch repeats in isolated saved-only-red02. Broad driver terminated1.
- Source12 ONLY preserves one public cross-test error fixture. probe03 repeats101,
  retaining .tmpBKLbnb/.canopa-cross-undo-9fzB6Y. Probe01 confirms actual RetainedOnly
  op2/1 versus canonicalSourceRemoved without retained root; exact pairedCopied op2/0
  root/identity/path/replacement match. All restored/newer/external bytes intact.
- Source13 shares exact paired validation: saved-only-green01 and unfiltered19-parent
  cross target pass; final full551backend/317default and all static checks pass.

CAUTION: separate exec sandboxes have distinct bind-mount unique IDs. merge-probe01
full Original comparison cannot establish root replacement. Corrected probe02 uses
persistent handle OBJECTS: root=true,parent=false, supported by actual in-process
refusal. forward-probe02 compared created_copy with previous FILE payload; probe03
corrects it to CONTAINER proof/identity=true. Original probes and corrections remain;
do not reinterpret confounded comparisons as production findings.

## Failure and predecessor evidence

Four decoded and separately live-verified bundles preserve18roots/168entries.
Original bytes/modes/mtime/objects match, no permission borrows; originals preserved.
- 01:4roots/24entries SHA
  7f53c21ac0b023c74caa629da4ee4985be91a4161e7c2efd6d8de7c3cca65a60.
- 02:2roots/22entries SHA
  6b5bdc6fa6ae73fa2e4dd79910a9ae5f43ada1afc5fa5cdeb702a53103e883c0.
- 03:10roots/108entries SHA
  a9d164f3bb2f820be768049d78b5393af3ac3bd467dd090e32823ec31b319805.
- 04:2roots/14entries SHA
  ad4910c33a591ba895ffd995f00c8cda762e4e94bd75409db34d59724ff30db2.
Manifests list every root; private outputs are under crates/canopa-files. Earlier
broad failure fixtures before preservation dropped normally; logs remain and later
equivalent reproductions retain actual data. Do not delete any originals/archives.

Predecessor schema53/source undo-rollback-inputs-09.json:891 inputs,545 backend/311
normal private default and both restart gates passed. Archive
4452c654e235a9924e06e99026044dea4542fc748ce81bcb3d83469535e2dfab has960
members decoded/live verified; source/archive reverified at this turn start.

## Next implementation priority

Legacy Stage::prepare roots still lack Original generations and keep separate
clean_completed_stage; modern roots never reopen consumed names there. Build a real
public-worker crash loop for ordinary Copy Undo and the other legacy completion/
abort paths before upgrading their creator/selected-root and independent task proofs.
Current cleanup death helper assumes SourceRemoved return shape; adapt the public
fixture for Copy Undo rather than forcing it through that assumption. Do not capture
current paths at restart as fresh authority or call legacy cleanup finished merely
because original-generation receipts are verified.

Continue broader Undo/expiry/queued lineage/large reports and Trash/Restore recovery,
then nav/search/preview backend separation, batch/cross-filesystem Restore and Recovery
retry/expiry, media/drag/device/accessibility/physical acceptance, first-window/focus
reliability, optimized startup/idle/large-directory/search measurements, full native/
workspace/Shell/Notify checks, compatible Niri distribution, release/package smoke,
documentation and GTK reference retirement. Keep the entire goal intact.

Preserve unrelated dirty work, especially Notify. No agents, commits, vendor changes,
worktrees or app/Niri rebuilds. Root/Files/domain instructions read; diagnose skill
used with repeated actual failures, ranked hypotheses and corrected probes. No
speculative race findings. CARGO_INCREMENTAL=0 TMPDIR=/tmp, -j1 --locked --offline,
no Rust wrappers, no concurrent Cargo mutation or ELF replacement while current_exe
children live. Reclaim exact completed own ELFs only via audited helper under both
Cargo locks. ApplicationSHA
3f638343c9cc5ab73fea245013112edbd0e4e8d5189c55dac58b5e1e2ee95cf8;
Niri63e39bae347346d7ac5aa90fb974ee8fe09ee59d9bd69238546e990368232da7.
Preserve required operation_transfer_resume gate, raw Unix paths and all private
fixtures. Cross-filesystem sources survive until commit. No auto-review rejection.
