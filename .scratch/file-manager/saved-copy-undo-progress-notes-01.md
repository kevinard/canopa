# Saved-replacement Copy Undo: implementation in progress

Full five-area Files goal remains active. Previous goal turn made verified progress
(schema55 ordinary Copy Undo). New turn verified all896 previous inputs and archive
SHA9f5d631b35b553898735bf820eacb1df91185dbb651f6495d1967e467558b4e9.
Do not mark complete for this slice. No subagents/commits/worktrees/vendor edits.

Current source saved-copy-undo-inputs-05.json,898 inputs,schema56. Current focused
saved_copy_boundary test exec47878 is running; inspect exact handle/log green04,
not a lock or intent. Do not edit inputs until terminal. All earlier handles closed.
No full backend/default/native validation of this new production source yet.

New repro saved-copy-undo-red01/02 (source01,897,schema55) both101: actual public
Undo after Copy Replace restores previous, unlinks copied payload, dies. Parent
reintroduces same copied inode via preexisting alias. Reconcile repeats unlink and
removes original empty root. Source alias and restored-target edits remain intact.
Probe01 preserves actual stage/change,gone root/payload,alias inode/hash and counts0.
Two failure roots preserved + decoded/separately live-verified archive01:22entries,
SHA2f6dace38673b39d423ea4f716a3d0c219465336cfc10a87c50d754ab5a6ebab.

Source02 extends existing undo/copied.rs + new copied/previous.rs:
- Removal selected/selected_version become Option (Some JSON unchanged for old
  schema55 journals). Optional previous Saved proof + previous_returned false skip.
  Restoring consumes previous-name return before native rename; Restored commits
  result. Both source rename and current-copy unlink keep prior one-shot phases.
- Saved proof holds canonical normalized original root, selected original entry
  (Ownership.previous or replacement_facts.original_previous),strict selected version.
  Previous Opened pins root/entry before original binding save; no current-path
  adoption of unknown legacy roots. Actual copied-file creator uses separate fresh
  root so Keep can preserve uncertain copy while canonical previous stays available.
- Absent copied file uses same new bound return protocol, actual Saved selection
  plus staging::Opened::require_created (actual creator/root fd proof), no fake reader.
- Return completion never reads or verifies later target contents. Restoring restart
  recognizes original returned object without repeating native rename. Restored skip
  target entirely; it only consumes still-original copied payload or preserves it.
  Taking/Restoring uncertain outcomes stay protected for Copy/Keep/new fresh Undo.
- Completion derives fresh copy-root plus original saved-root empty-only cleanup.
- reverse_copy routes retained:Some including absent current path through protocol;
  removes old generic live implementation. require_backup still preflights.
- Store/schema assert/fixture versions56; saved cleanup Roots2 in reverse_cleanup.
Source02 green01 original new parent passes.

Source03 adds phase hooks/tests, original-saved check before Taking,source-retention
consumed_stage+phase recheck, and safe error instead of unwrap for unknown saved roots.
Green02 passes5saved parents +1ignored; ordinary-green01 passes all9ordinary parents.
Tests cover prepared/taking/detached/previous-renamed/restored/consumed/unlinked/cleared,
missing copy,foreign roots/saved entries,current target edits/replacement,Copy/Keep/
export Undo + fresh original Undo after consumption. Actual fresh payload root kept
separate from active canonical saved root.

Source04 adds14 refused/ignored phase/completion cases and direct Keep after a
recorded return. Green03:6pass/1fail; keep-red02 repeats unchanged source. Probe01
confirms before phaseRestored/previous_returned=true, afterKeep pending0/copy_undo0/
cleanup0/changes1/garbage1. Copy preserved but completed return metadata not committed.
Failure roots /tmp/canopa-saved-copy-undo-3qGHtx and -ufPjH0 still need archive02.

Source05 fixes this actual handoff: private Keep archives exact fresh root, then if
previous_returned is recorded, reverse_transaction commits canonical return and an
empty-only cleanup receipt ONLY for original saved root, atomically. Fresh archived
copy root never becomes disposable. Completion installer checks actual history row
retirement, rollback ignored/refused writes. Recovery message says Keep finishes
recorded return. Unreturned Keep leaves original Undo unchanged as before. Cleanup
result updates problem state. Root paths include saved stage for observation/preflight.
Removed accidental same-display-parent restriction: both independent original parents
are verified and native no-replace rename chooses correct descriptors. Missing original
root proof still refuses. No production validation beyond pending green04 yet.

Next: inspect green04. Add direct-Keep metadata refusal/rollback cases and older-Undo
lineage if necessary; test raw names/links and source reservations. Run existing
ordinary9 on final source, default all-target Clippy -Dwarnings,format/schema syntax,
broad saved/cross/permissions/Undo/recovery target matrix and normal isolated default
library. Capture every source revision; keep passed-check/source relationships honest.
Do not repeat passing unaffected suites unnecessarily when only a test target changed.
Complete actual bugs/regressions before final checkpoint; no narrowed success criterion.
Record ADR0148 + ledger/context after validation; final verifier/source bundle with
independent decoded/live member/protected app/Niri/gate check. Current ledger stays
on prior verified schema55 until new evidence supports replacing it.

No code wrappers/preloads. Run root CARGO_INCREMENTAL=0 TMPDIR=/tmp,-j1,--locked,
--offline. Reclaim only exact completed owned test ELFs through existing script under
both Cargo locks,retain audits/app/Niri/required gate/rlibs/evidence. Do not overwrite
current_exe helpers while children live. Private default runner needs approved private
D-Bus listener isolation (temporary HOME/XDG,no display/system bus/host activation).
No normal mutating operations on real user files. All new tests private /tmp roots.

## Superseding state after additional compatibility evidence

Source07:898 inputs,schema56,12 focused saved parents pass in green06; Clippy01
all default targets -Dwarnings,format01,fixture-check01 pass. Source06 green05 had
9pass/1fail because public lease admission intentionally rejects selection while a
journal owns retained data. Source07 corrects that test, adds a before-Undo lease
admission parent and verifies schema55 bound ordinary wire compatibility. Failure
root iQDCOh archived03 (14entries) decoded/live SHA
c7e3e413eb850c6cafc1db021d4fc7aea6a1fbc9dbb18a855c69a8301d5044bc.
Source05 green04 had7parents pass; direct Keep metadata fix is verified. Keep failure
roots archived02:28entries SHA2af7ba4081a1118418b20eadbe176ba5a8b688d0f884c5503819d84e5b8d5dad.

Source08 adds legacy-format upgrade test: actual canonical saved root/returned entry
from killed worker, serializes old Pending::Undo and clears new binding. legacy-red01
and red02 both101: old recover_reverse still repeated payload unlink on restart.
Roots5RMm3A and8sRdWB preserved (not archived yet). Original saved root gone, alias
and later target edits survive. This repeats original cause in old-journal decoder.

Source09 removes that unsafe legacy removal/rollback block. copied::recover_legacy
requires exact canonical history/change and matching canonical normalized saved root,
requires original root/parent generations, original saved entry at returned target
(via Ownership.previous or replacement_facts.original_previous), and missing old
previous name. It finishes ONLY metadata with original empty-only cleanup receipt;
never unlinks or renames a payload. Reintroduced payload -> nonempty root stays
protected/whole-root Recovery. Unknown/unfinished legacy claims remain preserved.
Green07 passes13parents/1ignored. Two Store wrappers now dead after unsafe caller
removal: replace_change and replace_change_with_relocation; remove wrappers before
final -Dwarnings; underlying replace_change_with_recovery is used and must remain.

Source10 adds cloned-history refusal test and extracts legacy_record helper; history
red01 compile failed because helper accidentally retained #[test] (corrected).
Source11 only fixes that attribute. Current history-red02 exec51961 is RUNNING:
check its handle/log before editing. Expected test detects legacy metadata upgrading
without independently verifying original history generation. It renames actual
history to original-history and copies bytes back (new inode), leaving original
saved-root proof valid. A replacement DB must not acquire root cleanup ownership:
assert changes remains1, old root payload intact, no completed-target mutation.

If history-red02 fails, repeat unchanged source11 once (red03), preserve roots,
then add context::require_change_history using stored changes.bindings ForwardProof
original plus proof.history/current Store identity and transfer_completion::history_file.
Expose a short Store verifier and call it in recover_legacy before handoff. read()
inside store/context.rs already returns/validates canonical Bindings; no need to
reconstruct Forward digest from unknown old preparation. New modern task already
uses its separate original-history binding. Remove dead wrappers. Capture source12,
run focused13+newhistory parent, ordinary9 and final static then broad backend/default.
No full new backend/default validation yet; ledger/ADR still prior schema55. No live
process except history-red02 (older focused/static handles all terminated). Do not
claim this candidate validated beyond actual scope. Source11 has898 captured inputs.

## Superseding authoritative state: source15 expanded validation live

Current candidate saved-copy-undo-inputs-15.json,898 inputs,schema56.
Only live process now: exec98610, expanded01 log, unfiltered four integration
 targets (request_source_retention,undo_saved_stage_move,undo_saved_versions,
 undo_unavailable_moves). Source15. First3 targets already passed29/24/8 parents
 with1ignored each; unavailable-move target still live at last observation. Verify
 handle before editing/restarting; do not infer terminal from old logs. No other
 live handles; all prior test/static/archival handles terminal.

Source12 history-generation fix: ctx::require_change_history reads canonical
changes.bindings ForwardProof history+original, checks original actual history
file before legacy metadata handoff. Removes two now-dead replace_change wrappers;
underlying replace_change_with_recovery remains used. green08 all14 saved parents
pass. Clippy02 -Dwarnings,format02,fixture-check02 pass. Unfiltered backend lib01
passes259/30ignored. Full driver01 batch10 FAILED source12: undo_saved_stage_move
23pass/1fail, undo_unavailable_moves12pass. All processes terminated (driver36784=101).
Failure already preserved by existing integration Fixture Drop.

Existing open-handle saved-version review regression repeated approval-red02 on
unchanged source12. Both fail at1724: Copy Undo differs from original history step
when user explicitly chose UseSavedVersion. Probe01 captures canonical16bytes vs
actual37bytes,same physical file,canonical recoveryfalse,pending0. Existing
saved_version::review changes ONLY retained.version and retained.recovery after
its guarded explicit answer. No temporary production diagnostics introduced.
Source13 canonical_digest in store/copy_undo normalizes ONLY those permitted fields
for comparing actual selected pending against canonical history, hashes canonical
unchanged, and binds full selected pending separately. Actual creator/selected-file
proof still required before save; changing any root/path/identity field refuses.
Approval-green01 original integration regression passes (source13).

Expanded relevant tests were added to validation (not previously in20target slice):
request_source_retention29pass/1ignored, undo_saved_versions6pass/2fail/1ignored
on source13. Existing a_replaced_saved_version_keeps_older_history_bound... fails
WouldBlock original-item changed. That exact failure repeated in replaced-red02.
The public API explicitly permits a guarded UseSavedVersion selecting a replacement
object in the same original saved root; old canonical leaf generation cannot
substitute for that newly accepted source.
Source14 adds public queued Undo question + approved replacement worker case;
reviewed-red03 fails before prepared SIGKILL (childFailed instead ofsignal9), root
ChWff1 retained. Source15: copied::begin detects changed selected snapshot relative
to exact canonical step (later canonical_digest permits only version/recovery),
previous::Opened::capture(reviewed) pins/captures actual selected generation ONLY
for that fresh guarded live review, still requires original container/parent and
strict selected version. Resume never captures a new entry: it requires this saved
selected generation/version from independent pending binding. Without fresh review,
old canonical previous generation/facts remain mandatory. Green09 passes15parents/
1ignored on source15. Includes prepared/previous-renamed/restored deaths for approved
new physical object, older Undo still covered by existing positive integration test.

Do not claim source15 static/full-backend/default validated yet. Next after expanded
live process: if pass, final default all-target Clippy/format/fixture on15, rerun
unfiltered backend lib (production changed since source12), continue remaining
integration batches11..19 (batch10 covered by expanded01) on15. Reuse already
passed expanded targets on exact same final manifest; total22 target scope if final
lib plus prior19 integrations plus added2, not 'all backend tests'. Consider any
other directly affected tests before broad claims. Run normal private desktop lib
with same isolated runner. No native/workspace/release claim; full goal active.

Seven failure archives decoded + independently live verified,14roots/156entries:
01 22entries/2roots 2f6dace38673b39d423ea4f716a3d0c219465336cfc10a87c50d754ab5a6ebab
02 28/2 2af7ba4081a1118418b20eadbe176ba5a8b688d0f884c5503819d84e5b8d5dad
03 14/1 c7e3e413eb850c6cafc1db021d4fc7aea6a1fbc9dbb18a855c69a8301d5044bc
04 23/2 aed75f3caeb94c0f1b676bc21210b6aee09f3aa2a21a127e18c5028b51bf9ca2
05 28/2 b8f63d29360e56cae52eeb87f48b43baad958f50e2df49b2c4cde1bbf2f9f0c0
06 30/4 9372dd2d45f44ef1d787cc2d6e54c8cd2b56eff429494d6a3770ed39003576cb
07 11/1 450807a79d7e8b4eb8edbb89064a2fcc959ce6fd212c8d80d6d145889e19ea52
06 preserves /tmp/.tmpeC36tP,/tmp/.tmpaPsNPQ plus private project-output
.canopa-saved-stage-return-M1KNzG and -3ZBl4e under crates/canopa-files. 07 preserves
/tmp/canopa-saved-copy-undo-ChWff1. Initial direct replaced-object target dropped
fixtures normally; new public queued fixture preserves equivalent data. All archives
permission-borrow list empty; bytes/modes/mtime/objects matched. Do not mutate any.

ADR0148 drafted/statusinprogress + context index and ledger point to source12/14
focused parents, broad validation in progress. Update them to actual final source/
15parents/review proof only after new verification. Final verification/source bundle
and independent decoded/live/protected binary check still missing for new slice.
Protected app/Niri/gate paths and previous schema55 source/evidence remain intact.

## Latest live state (supersedes prior live-handle statements)

Expanded01 exec98610 TERMINATED0 on source15: request_source_retention29/1ignored,
undo_saved_stage_move24/1, undo_saved_versions8/1, undo_unavailable_moves12/1. All
unfiltered and green,73parents/4ignored across4 targets. Clippy03 all default targets
-Dwarnings,format03,fixture-check03 PASS source15. Source898 still unchanged.
Only currently live process: unfiltered backend library02 exec60579, source15,
near end at last poll. Do not edit source/restart until authoritative terminal.
After it passes, run run-saved-copy-undo-checks-02.py (prepared,not yet executed).
It selects remaining batches11..19 (17integration targets), source15 and reclaims
exact completed lib02+expanded01 ELFs first under Cargo locks,protecting restart gate.
Final integration scope22targets = lib+17remaining+4expanded. Expected counts if
unchanged tests:612 backend pass/53ignored, library260/30; default341/31. Treat these
as EXPECTED until actual logs prove them. Private default run and final proof still
not executed. No native/workspace/release claim. Full goal active.
