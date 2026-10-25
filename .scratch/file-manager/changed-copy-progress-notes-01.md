# Changed-copy preservation: implementation and live validation

Latest validation state: final matrix exec68020 TERMINATED0 on source06:
619pass/54ignored across22 unfiltered targets, including267/31 library. Both
active/stopped restart gates pass. Source899 unchanged. Additional18-target
driver is NOW LIVE exec63604, extra-driver01 log, batches extra01..05 and receipts01.
Only live process is63604; fixed archive probe/control/negative probes/static
and both primary matrices are all terminal. Normal default and final proof/bundle
remain unexecuted. Earlier live-handle statements below are historical.

## Next implementation is now backed by a public Move Undo reproduction

While final source06's matrix68020 ran unchanged, two external public-API probes
reproduced generic same-filesystem Move Undo forgetting a completed saved return.
Both workers actually moved20 files with Replace, then were SIGKILLed by a watcher
after file19's incoming item returned and its saved previous destination returned.
The journal is exactly Undo/Rename ordinal19 at death. A pre-existing hardlink
recreates previous after death; editing the completed target also edits that inode.
Reconcile reports success but changes ordinal19 to RetainedOnly, granting that new
previous name another logical saved-return step rather than retiring completion.
Bytes remain intact; do not describe this as data deletion or an observed repeated
rename. It is a proved completion/reclassification gap in a still-unfinished family.

changed-copy-rename-next-01/02 both101; probe01 JSON records before Rename ordinal19,
after RetainedOnly,20changes/pending0/undo_work0/garbage0, incoming bytes back at
source19 and all three previous/alias/completed-target names sharing the edited inode.
A one-variable control with NO recreated previous (but identical later target edits)
passes0 and retires ordinal19, leaving19 other changes. This isolates the old
Retained::restored_at predicate requiring the previous name absent, followed by
recover_reverse's Rename-to-RetainedOnly fallback. Earlier crash/Fingerprint-only
causes are ruled out. Actual selected-root generations and original-history binding
still need complete modern consumption/return protocol; do not fix by adopting a
current path or merely loosening an identity test.

Prototype sources changed-copy-rename-next-probe.rs and -control-probe.rs are private
debug artifacts, not Cargo inputs. They were rustc-linked against source06's exact
compiled public rlib; verification01 records source/executable/library hashes.
Both negative roots /tmp/canopa-rename-undo-probe-ubhwK5 and -n6UCBY are archived
and decoded/live verified142entries, SHA
1b590f135f4f7a896648eb1e7d178f60808619041d826e78d9b1e6fc30478734
in changed-copy-rename-next-fixtures-01.tar.gz. Keep roots and archives untouched.
Control root is /tmp/canopa-rename-undo-probe-kDfvOk, positive proof in verification01.
No source changes were made by this investigation; all899 captured inputs stay06.
This is explicit remaining full-goal work, not a reason to reject correctly scoped
changed-copy/shared archive verification. Finish the running matrix,18extras,
private default library and artifact proof, then port this real pattern into a
public process-death regression and implement the missing generic Move return seam.

## Superseding current state: source06 archive handoff fix

Latest: focused green03 exec93390 TERMINATED0,7pass/1ignored on06. Clippy03 all
default targets -Dwarnings,format03,fixture-check03 PASS on06 (static handles all
terminal0). Only currently live process is final matrix driver exec68020;
backend-driver02/backend02 logs belong to this run.
run-changed-copy-checks-02.py is now LAUNCHED on06. Do not edit captured source.
After its compile finishes, rebuild/rerun the standalone probe against its new
rlib (preserve old negative executable); then complete18 extras/default/proof.
Prepared changed-copy-build-probe.py does that rebuild after it sees Finished test
in backend02. Then execute /tmp/canopa-changed-copy-archive-probe-fixed into
changed-copy-archive-probe-green-01.log and require0. This helper has NOT run.

Superseding probe status: build02 exec37940 TERMINATED0 against final06's new
rlib; fixed probe green01 TERMINATED0. Green01 JSON independently confirms
changes1/pending1/undo_work1/garbage3, exact39-byte selected payload intact and
present in public Recovery after the ignored insert. Main matrix68020 still live.
No other live handles. Do not rebuild the probe again or overwrite its artifacts.

Authoritative candidate is NOW changed-copy-inputs-06.json,899 inputs,schema57.
Only live process: exec93390, focused module green03 (not yet terminal at this
update). Source04 backend31338 terminated0:618/54ignored across22 unfiltered
targets, both gates pass. HOWEVER source04 is not a verified final candidate:
a separate public API probe twice found uncatalogued selected edits after ignored
archive insertion when three older Recovery rows already exist. Both journal and
canonical Undo retire, garbage count stays3, exact39-byte payload survives in a root
absent from all archive rows. Probe01 JSON and red01/red02 logs prove this.

Probe was built directly with rustc against current source04's compiled public
rlib while backend tests ran; no source/build-cache changes or helper rebuilds.
Build01 JSON preserves actual linked-library hashes. Original prototype source
is changed-copy-archive-probe.rs, clearly private/debug, and original executable
/tmp/canopa-changed-copy-archive-probe remains unchanged. It must be recompiled
against final06 and rerun after the final backend compile produces its new rlib.

Source05 adds a permanent public nonempty-archive regression using three older
protected edits. archive-red01 repeats the failure, retaining cBHLAV. Source06 fixes
generic archive(): require exactly one inserted row, exact stored body at its new
id and both namespace/root ownership indexes before completing reversal. This
prevents an ignored INSERT reusing last_insert_rowid from an older archive.

After focused93390 passes, run default all-target Clippy03 -Dwarnings,format03,
fixture-check03 on06. Then run run-changed-copy-checks-02.py: SAME22-target matrix
on06, backend-02.log/receipt (expected619/54 with library267/31, unproven). The first
run's source04 evidence stays separate. Then run prepared18-target extra checker,
normal private default library (expected348/32) and verify-changed-copy.py. Those
unexecuted templates now reference06/backend02 and three failure archives.
Inspect/fix any verifier assertion from actual evidence; never forge counts.

Three decoded and separately live-verified failure archives preserve7roots/99entries:
01 fourroots44entries SHA c97f16cf7ee3752a4188fdb84ac3693d8bc9ae8c8b6eab17229c65d5b702a278;
02 tworoots38entries SHA 191903ddce972c4368b2dac72f5606f4bf1d4c1276b2daeab385ac2276e1b1f8;
03 oneroot17entries SHA b24878e4efadab657365883c6874c5f92e516a8865f1a961273fd15f4e93b696.
02 roots /tmp/canopa-changed-copy-archive-probe-HWFxJZ and -9cAoQs;03 root
/tmp/canopa-changed-copy-cBHLAV. All originals/archives untouched after live checks.

ADR0149/context/ledger still reference source04/six focused parents and validation
in progress; update them after final06 results. Source archiver and separate final
live verifier are prepared/not run; source archiver now references06. Full goal
active. Everything below describes earlier states and is historical where it
conflicts with this superseding section.

Full five-area Files goal active. Previous saved-replacement Copy Undo checkpoint
was progress and has its own complete schema56 evidence. Do not mark the full goal
complete for this slice. No subagents, commits, worktrees or vendor edits here.
Preserve unrelated Notify work and every retained failure fixture/archive.

## Current authoritative state

Candidate changed-copy-inputs-04.json,899 captured inputs,schema57. Source03 focused
module green02 passes6 parents/1ignored helper. Source04 only simplifies two Clippy
expressions. Default all-target Clippy02 -Dwarnings,format02,fixture-check02 pass on04.
All final build inputs remain unchanged. All old focused/static handles terminated.

Only live process: exec31338, run-changed-copy-checks.py, backend-01.log. It compiles
and runs unfiltered library plus21 integration targets, no default features, -j1,
locked/offline. Poll its handle; do not infer termination from a quiet compile log.
Do not edit captured source or rebuild while current-executable children live.
Expected counts from unchanged existing tests:618pass/54ignored across22 targets,
including library266/31. These are expectations until actual terminal logs prove them.

After backend terminal0, execute run-changed-copy-extra-checks.py. It is prepared,
NOT launched. It covers18 additional public shared-reversal callers in five batches:
Undo, Trash, Restore, Recovery, upgrades/history/identity. It reclaims only exact
completed owned test ELFs under both Cargo locks, keeping application/Niri/gate and
all archives. Keep reclamation audits. Require successful receipts and source04.
The broadened scope is necessary because complete_reversal serves every Undo.
recovery_catalog is gated on local-runtime and is excluded from this no-default
matrix; it needs its own isolated feature-enabled check before acceptance claims.

Then normal private desktop-feature library via atomic-rename-run-core.py
--desktop-lib --source changed-copy-inputs-04.json with a fresh numeric label,
temporary HOME/XDG/private D-Bus and no display/system bus/host activation. Socket
creation requires the same reviewed escalation used by prior isolated runs.
Expected347/32ignored, not yet executed or proved. Final count/source/provenance proof,
final docs and source/evidence archive/independent verification still outstanding.
No native/workspace/release acceptance claimed. Full goal active.

## Implementation

Changed Copy MoveToRecovery now uses existing displaced::Prepared and undo_work
binding. replacement::Reviewed lends its actual held selection through
preservation_selection; Prepared::for_preservation retains actual creator roots,
selected Original and strict reviewed version. Registration checks canonical Change,
exact selected pending and original history-file generation in one transaction.
Prepared failure uses verified empty-only cleanup. After registration,
preserve_changed_copy retains durable root ownership even if the live rename refuses.

undo_work owner/require now cover Copied retainedNone,stageNone,displacedSome.
New generic unbound changed-copy intents receive no restart mutation rights.
recover_removal completes a proved selected previous entry through reversal+archive;
otherwise existing begin_empty_undo handles compensation through durable consumed
namespace/Skipped/Returned states. It never calls legacy rollback_displaced.
The existing empty-copy disposal accepts this specific Copy shape; ordinary
SourceRemoved return-copy behavior remains covered by prior/current tests.

Pending::record exposes the original preservation root at slot0. Fresh private
undo_work Review binds exact pending/binding/access/root and current previous-entry
snapshot. Copy preservation can Keep uncertain contents in the original root, with
no selected-entry execution rights. SourceRemoved reviews retain their stronger
original displaced observations. Stale Copy reviews refuse; fresh Keep archives
available leaf and whole original root atomically, leaving canonical Undo intact.

complete_reversal now checks exact changes-row absence before clearing any owner.
This closes ignored DELETE handoffs even when completion has no empty-root receipt.
Initial/refused/ignored phase/completion and stale Keep acceptance pass focused03.
Schema assertions and native claim-review fixture updated57. No temporary logs.

## Preserved attempts

01 (899inputs,schema56): test-only public worker cuts after registration/preservation.
red01/red02 both101: cloned history reconciles/removes original empty preparation.
Probe01 captures missing work root, pending0/undo_work0, edits intact, distinct DB
inodes. Those early fixtures replaced the main DB while WAL existed; do not claim
their renamed original DB independently retained pending1. The final regression
checkpoints WAL before cloning its committed original history.

02 (schema57): binding/creator/private Keep/empty compensation. Focus green01 has
3pass/1fail/1ignored. Clone, two restarts and uncertain Keep pass. Completion fault
fails BEFORE DELETE ON changes/IGNORE, repeated unchanged in write-red02. Actual
preserved bytes survive, but journal retires while canonical change remains.
03 adds shared retired-step check, initial-write and during-review replacement
tests plus stale Keep. Focus green02 passes6/1 in1.35s. Clippy01 fails two expression
lints (lazy Option fallback and nonminimal boolean),format01/fixture01 pass.
04 simplifies those expressions; final static02 pass; backend31338 live.

Failure archive changed-copy-failure-fixtures-01.tar.gz has four roots/44 entries,
SHA c97f16cf7ee3752a4188fdb84ac3693d8bc9ae8c8b6eab17229c65d5b702a278.
Decoded and independent live check pass; all bytes/modes/mtime/objects match,
permission borrowing empty. Original roots a3BogD,ha4N42,epkOCd,8GHSYK under
/tmp/canopa-changed-copy-. Do not modify these or the archives.

ADR0149/context/ledger currently say implementation/static plus validation in
progress. Update them to actual final results after validation. Previous schema56
source bundle remains saved-copy-undo-source-01.tar.gz, SHA
66d961558381d31c8cb198a8c15b4ff7be850f9e3e553c1bf1ce2d7517a1eb45,
1031 decoded/live-verified members at its checkpoint. New source supersedes its
live input equality; do not rerun that old final-live check expecting equality.

Protected application target/debug/canopa-files SHA
3f638343c9cc5ab73fea245013112edbd0e4e8d5189c55dac58b5e1e2ee95cf8;
Niri target/niri-cancel-upstream/debug/niri SHA
63e39bae347346d7ac5aa90fb974ee8fe09ee59d9bd69238546e990368232da7.
Keep required gate operation_transfer_resume-99a373b9bfb9fe1b; capture its new final
hash after gate pass. Both active/stopped test names are in prior verification JSON.

## Remaining goal work

Unknown/uncompleted unbound legacy disposition; other generic Forward/Undo/cleanup
(including RetainedOnly), Trash/Restore crash recovery; broader Recovery expiry,
queue lineage and reports; backend navigation/search/preview separation; product,
media/drag/device/accessibility/physical acceptance; startup/focus/performance;
complete native/workspace/Shell/Notify/Niri distribution/package/release/docs/GTK
retirement. Use actual public crash tests before granting new resumed namespace
rights. Reuse existing ownership seams instead of duplicating protocols speculatively.
