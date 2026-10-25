# File manager implementation ledger

Goal: implement the entire [product brief](PRD.md), including native behavior
and verification. This ledger tracks progress; intermediate slices do not
replace the full goal.

## uutils Copy migration, 2026-10-25

[ADR0153](../../docs/files/adr/0153-uutils-prepared-copy.md) selects direct
synchronous uucore 0.12.0 descriptor copying with a narrow root vendor patch for
per-call progress/cancellation and pipe isolation. The path uu_cp API was
rejected by the runnable private proof. All actual byte-copy consumers use one
prepared-copy lifecycle; source validation and the unchanged SHA-256 receipt stay
separate from transfer. Schema60, publication, source removal, saved versions,
Undo and Recovery ownership stay intact. No schema61 candidate was imported.

[Migration evidence and limitations](UUTILS-IMPLEMENTATION.md) records exact
checks and debug measurements. The full Files goal, generic incoming-Move/Undo,
Trash/Restore/legacy cleanup, native acceptance and the 500-item/20-second gate
remain open. Sparse/reflink, ACL/xattr and hard-link-group improvements are not
claimed. This checkpoint is a Copy-backend migration, not full goal completion.

## Current Phase2 work — proved saved-return refusal, 2026-10-24

[ADR0152](../../docs/files/adr/0152-proved-saved-return-refusal.md) tracks schema60
on refused-return-inputs-02.json (903 inputs). A new Refused phase distinguishes a
proved live permission refusal from an uncertain consumed native call. Restart
reopens the exact original selection/version and parents before another attempt;
unknown Returning work still cannot replay. Refusal-write failures retain the old
owner, and retained-source leases defer continuation. Saved-only cross-filesystem
returns now bind their exact SourceRemoved and paired Copy rows together.

Current source02 passes365 default-feature library parents/34 ignored on an
isolated private bus, and102 parents/seven ignored across eight unfiltered
integration targets, including both restart gates. Proper default all-target
Clippy with warnings denied, formatting and fixture syntax pass. Source01's
79 no-default Undo crash parents/seven ignored and source02's new lease parent
have an explicit test-only bridge; production inputs are identical. Four affected
public cases and nine saved-return crash parents passed privately before installation.
Exact proof is [refused-return-verification-01.json](refused-return-verification-01.json).
The source02 archive has903 decoded/live-verified members; protected binaries and
retained failure fixtures remain intact. Source07's broader checks had exposed four failures
in reviewed-Move permission continuation and saved-only canonical pairing, alongside
its73 passes/seven ignored. Do not count private checks as installed acceptance.

The original500-item20-second gate remains open: source06 measured29.84s; a private
canonical-only optimization measured26.89s. Only the canonical-byte optimization
was installed. Compact encoding and profiling experiments stayed private; commits
accounted for less than one second, so durability barriers remain intact.
Generic incoming Move consumption, remaining legacy/cleanup and Trash/Restore,
and every broader five-area requirement remain unfinished. Full goal active.
[Current handoff](MOVE-RETURN-NEXT.md) records exact handles and evidence.

## Previous Phase2 work — original Undo displacement preparation, 2026-10-24

[ADR0151](../../docs/files/adr/0151-original-undo-displacement-preparation.md)
tracks schema59. Public Undo Replace now captures its selected entry and actual
parent before the question, creates its original work root through that parent,
and registers an independent exact task/history binding before displacement.
Direct interrupted compensation uses the consumed cleanup protocol. Saved-return
handoff supersedes that binding atomically; completion context uses the recorded
incoming identity. Both previously failing additional-pass cases now pass, with
all13 parents in undo_directory_additions and undo_replace passing on source03.
Source06 passes76 Undo crash parents (including six new direct tests), seven
ignored. Eight unfiltered integration targets pass82 parents and fail two, seven
ignored: the original500-item gate takes29.84 seconds against20, and a deliberate
legacy row with removed bindings refuses for missing original history proof.
Modern deferred-return control passes. Private diagnosis reproduces29.50 seconds;
canonical-byte reuse alone reduces it to26.89, still above the unchanged gate.
These failures remain open; no current static/native/release acceptance is claimed.

The reviewed saved-return permission regression also passes whole-folder Copy,
fresh Keep, restart and history expiry on source02. The source58 six saved-return
crash parents and proper default all-target Clippy/format pass remain scoped to
that earlier source. Source01 related tests finished with three failures; the two
additional-pass failures are fixed above and the permission/Recovery flow has a
separate focused pass. No combined current-source acceptance is claimed yet.

Incoming Move name consumption, broader legacy/cleanup and Trash/Restore restart
remain open, followed by all backend extraction, product/acceptance, reliability/
performance and release work. The complete five-area goal remains active.
[Current handoff](MOVE-RETURN-NEXT.md) records authoritative handles and evidence.

## Previous Phase2 diagnosis — changed-copy preservation restart, 2026-10-24

[ADR0149](../../docs/files/adr/0149-changed-copy-preservation-restart.md) tracks
schema57 on changed-copy-inputs-13.json (899 inputs). Two worker crashes reproduced
replacement history acquiring cleanup of original changed-copy preparations.
Changed-copy preservation now uses the existing independent displaced Undo binding,
actual reviewed selection and created-root proof. Restart finishes proved preserved
data or compensates an unused preparation without replaying a legacy rollback.
Fresh private Keep retains uncertain original-root contents and leaves Undo history
available; replacement roots and stale reviews refuse.

An ignored history deletion reproduced a second handoff gap twice. Shared reversal
now verifies that its exact step retired before clearing the owner. Source04 passed
618 tests across22 targets, but two independent public probes then exposed ignored
archive insertion reusing an older SQLite row id and leaving selected edits
uncatalogued. Shared archive insertion now verifies affected row, exact stored body
and ownership indexes. Its permanent regression proves safe retry with all four
versions retained. Expanded tests exposed payload-only Recovery expectations and
a repeated500-item Undo timing failure above its existing20-second bound. Whole-folder
Copy/Keep/export-Undo acceptance now passes. Reusing canonical byte digests and
deriving immutable transition fields removes redundant serialization; measured
Undo phases fall22.61→18.48 seconds with the same63,000 identity captures, and the
unprofiled20-second gate passes. Profiling code is removed. A new canonical-format
compatibility/tamper parent and static checks passed source11. The broader rerun
then reproduced stale saved-container parent ownership after directory Merge.
Merge preparation now captures the catalogued container and both actual parent
generations, so return commits update its owners to the verified receiving parent.
All10 directory Merge cases pass. Source12 static checks pass; its matrix
passed619 tests and failed one cross-filesystem saved-folder Keep both case. The
new direct movement record was incorrectly applied after a created-copy migration
had rebound ownership. Source13 restricts direct movement records to same-filesystem
children; the Merge and full saved-folder suites pass34 tests/1ignored together,
and its broader validation is complete. The15-entry
Merge refusal and42-entry cross-filesystem collision fixtures are archived and
independently live verified. Initial fixed-case archives preserve7roots/99entries; expanded
acceptance archives preserve7roots/4573entries, all decoded and independently
live verified.

Next generic Move Undo work is now reproduced: recreating the previous name after
an actual saved return makes restart reclassify the completed step as RetainedOnly.
Two public deaths fail; the one-variable no-recreated-name control passes. Bytes
survive. Its two roots/142entries are separately archived and live verified. Finish
current validation, then implement durable generic Move return consumption;
[CHANGED-COPY-NEXT.md](CHANGED-COPY-NEXT.md) has the exact repro and live handles.
All remaining five-area work
and the full goal remain active; no native/workspace/release acceptance is claimed.

Schema57 source13 diagnosis is complete:743passed/2failed/65ignored across40
backend targets;349passed/32ignored in the isolated desktop library. Static06 and
both required batch gates pass. Failed additional-pass cases use public Undo Replace,
which still creates an unproved saved root. Two public retained repros repeat them;
32 entries are archived/live verified. Fix generic displacement creation through
actual selected descriptors and independent task binding; do not adopt a catalog
path as original proof. This is not a green restart-recovery checkpoint.

The next Move saved-return candidate is being tested in a private source copy so
source13 stays unchanged for its archive. Six real worker-death parents pass on
private build07; a seventh direct child is ignored. A further parent first exposed
recorded completion rereading the source and letting older Copy Undo delete a
replacement; the recorded returned identity fixes this. Main source remains57 until
the diagnostic archive is decoded/live verified, then the candidate can be installed
and its broader cases checked. See the current handoff for authoritative handles.

## Previous Phase2 checkpoint — saved-destination Copy Undo restart, 2026-10-24

[ADR0148](../../docs/files/adr/0148-saved-copy-undo-restart.md) tracks schema56 on
saved-copy-undo-inputs-15.json (898 inputs). Actual killed-worker tests reproduced
repeated unlink of reintroduced Copy Undo payloads and a separate Keep metadata
handoff gap. Saved returns now have a separately consumed claim and durable return
receipt. Copied-payload disposal and original empty-root cleanup retain their own
permissions. Keep after a recorded return commits its metadata and preserves the
copy root atomically; earlier Undo continues to follow the returned file.

Legacy journals can finish only an independently proved completed return, with
empty-only root cleanup. They cannot repeat unlink or rollback rename. Canonical
original history-file generation is required before this handoff; a cloned history
cannot acquire original work-folder cleanup rights. Unknown/uncompleted legacy data
stays protected for Recovery review.

Fresh guarded Use Saved Version decisions can select edited or replacement saved
entries within their original root. The canonical comparison permits only reviewed
version/Recovery changes, and the actual selected generation is recorded before
registration. Restart requires that recorded selection; it never recaptures approval.

Fifteen focused public parents pass. The final unfiltered scoped backend suite
passes612 parents/53 ignored across22 targets on the same898 unchanged inputs.
Normal isolated desktop-feature library passes341/31 ignored, receipt
continuation-core-1792847076272108370-receipt.json. Both active/stopped restart gates,
default all-target Clippy with warnings denied, Files formatting and schema56 fixture
syntax pass. Exact source/attempt relationships are verified in
[saved-copy-undo-verification-01.json](saved-copy-undo-verification-01.json). Seven
decoded and independently live-verified failure archives retain14 roots/156 entries.
Source/evidence bundling and its separate decoded/live check are recorded by
saved-copy-undo-source-01-verification.json and saved-copy-undo-source-final-live-01.json.
Application/Niri binaries and the required restart gate remain protected.

Next: changed-copy preservation, unknown/uncompleted legacy disposition, other
generic Forward/Undo/cleanup and Trash/Restore. Exact next work is in
[SAVED-COPY-UNDO-NEXT.md](SAVED-COPY-UNDO-NEXT.md). No full native/workspace/release
acceptance is claimed. Every remaining five-area requirement and the full goal
remain active.

## Previous Phase2 checkpoint — original Copy Undo restart, 2026-10-24

[ADR0147](../../docs/files/adr/0147-original-copy-undo-restart.md) tracks schema55.
An unchanged Copy Undo without a saved previous destination now binds its selected
copy and actual created work folder before consuming either name. Durable phases
never repeat a consumed rename or unlink after restart. Completion hands original
empty-root cleanup to its receipt. Uncertain data remains available through whole
folder Recovery Copy and a fresh private reviewed Keep decision.

Nine new focused public parents pass. They cover eight death boundaries, two
restarts, safe compensation/fresh Undo, reused names, replacement roots/parents/items,
tampering, initial/phase/completion/Keep refused and ignored writes, Copy/Keep/export
and original Undo, and raw Unix symlink names/targets. An ignored history deletion
reproduced a shared metadata handoff gap twice; receipt installation now checks that
the exact history step is retired inside its transaction before committing.

Final source: copy-undo-inputs-11.json (896 inputs). Backend coverage passes560
parents across20 targets (50 ignored helpers). Source07 passed the library and18
integration targets; only the final integration target changed to exercise the new
whole-folder Recovery contract. That complete target passes on source11. Production
and every other tested input remain byte-identical; source-bridge01 records this
relation. These20 targets were not all rerun against one unchanged manifest.

Normal isolated desktop-feature library passes326 parents (30 ignored), receipt
continuation-core-1792838551-receipt.json. Both active/stopped restart gates, final
default all-target Clippy with warnings denied, Files formatting and schema55 fixture
syntax pass. Exact source relations and attempts are in
[copy-undo-verification-01.json](copy-undo-verification-01.json). Eight retained failure
roots (90 entries) have decoded and separately live-verified archives. Source and
evidence are tracked by
[copy-undo-source-01-verification.json](copy-undo-source-01-verification.json); its
independent decoded/live check records final bundle verification. Application/Niri
binaries and the required restart gate are preserved. Next work is in
[COPY-UNDO-NEXT.md](COPY-UNDO-NEXT.md).

Copy Undo with saved replacements, changed-copy preservation, legacy/generic
Forward/Undo cleanup, Trash/Restore and every broader five-area requirement remain
unfinished. No native/workspace/release acceptance is claimed. Full goal active.

## Previous Phase2 checkpoint — completed Undo cleanup, 2026-10-24

[ADR0146](../../docs/files/adr/0146-completed-undo-cleanup.md) tracks schema54.
Two public-worker crashes reproduced orphaned empty work folders after a reversal
committed and cleared its cleanup owner. Modern completed Undo roots now retain an
independent empty-only receipt in the reversal transaction. Restart removes only
original empty roots and never replays the return or inspects its later target.
Unexpected contents and replacement roots remain protected; fresh Recovery Keep
archives whole original folders before retiring the exact receipt.

Six focused public acceptance parents cover actual deaths, Copy/Keep, export Undo,
remaining original Undo, refused/ignored metadata writes and binding tampering.
Saved-folder merge regressions exposed missing parent generations across Skip/resume
Undo; those generations now follow surviving history references. Saved-only
cross-filesystem restoration uses the exact paired copy's retained root through
the shared pair check. Both regressions pass the final full matrix.

Final validation on undo-cleanup-inputs-13.json (893 unchanged inputs) passes 551
backend tests across 20 targets (49 ignored helpers) and 317 normal isolated
desktop-feature library tests (29 ignored), with zero failures or filters in the
parent runs. Both required active/stopped restart gates, default all-target Clippy
with warnings denied, Files formatting and schema54 fixture syntax pass. Temporary
diagnostics are removed. Default receipt: continuation-core-1792833005-receipt.json.
Exact attempts and source relations are in
[undo-cleanup-verification-01.json](undo-cleanup-verification-01.json). Eighteen failed
fixture roots retain 168 decoded/live-verified entries across four archives.
Final source and evidence are tracked by
[undo-cleanup-source-01-verification.json](undo-cleanup-source-01-verification.json).
Application/Niri binaries and the required restart gate are preserved. Exact next
work is in [UNDO-CLEANUP-NEXT.md](UNDO-CLEANUP-NEXT.md).

Legacy preparation/cleanup, other generic Forward/Undo paths and all broader
five-area requirements remain unfinished. The full goal remains active.

## Previous Phase2 checkpoint — displaced Undo compensation, 2026-10-24

[ADR0145](../../docs/files/adr/0145-displaced-undo-compensation.md) tracks schema53.
Two public-worker crashes reproduced unchanged Undo Replace work requiring manual
Keep despite valid source proofs. Verified disposal now preserves its original
Undo owner through copy cleanup and a separate one-shot displaced-item return.
Consumed returns never rename again; Returned receipts have empty-root rights and
leave later target occupants untouched. Failed saves preserve the exact old owner.

A missing-payload test twice reproduced the old fallback moving a reintroduced
previous-name hardlink again. Explicit empty-copy compensation now handles absent,
unallocated and disappearing payloads without copy-file unlink rights. Original
Undo history remains available and retry requires a new conflict answer.

Twenty-six focused public parents pass. Recovery Copy/Keep covers all working
folders and preserves new entries in consumed names. Existing pending identifiers
remain compatible. Final validation on undo-rollback-inputs-09.json (891 unchanged
inputs) passes 545 backend tests across 20 targets (49 ignored helpers) and 311
normal isolated desktop-feature library tests (29 ignored), with zero failures or
filters in the parent runs. Both required active/stopped batch restart gates,
default all-target Clippy with warnings denied, Files formatting and fixture checks
pass. The default receipt is continuation-core-1792825465-receipt.json. Exact attempts
and source relations are in [undo-rollback-verification-01.json](undo-rollback-verification-01.json).
Twelve failed fixture roots retain 74 decoded/live-verified entries. Final source
and evidence are tracked by
[undo-rollback-source-01-verification.json](undo-rollback-source-01-verification.json).
Application/Niri binaries and the required restart gate are preserved. Exact next
work is in [UNDO-ROLLBACK-NEXT.md](UNDO-ROLLBACK-NEXT.md).

Completed-reversal cleanup, other generic/legacy paths and every broader five-area
requirement remain unfinished. The full goal remains active.

## Previous Phase2 checkpoint — displaced Undo retention, 2026-10-24

[ADR0144](../../docs/files/adr/0144-displaced-undo-retention.md) tracks schema52.
Two public-worker crashes reproduced edited Undo Replace partials with no safe Keep
disposition. The displaced folder lacked original generation proof. Preparation now
holds the selected entry before the question, captures created roots and binds the
actual initial Undo owner before copying or displacement. The return-copy reader
can extend only that same owner. Recovery cannot adopt current replacement roots.

Fresh Keep review preserves available leaves and both whole work folders, including
extra children, then retires only the exact journal/binding. Original Undo history
remains available and retry uses a fresh conflict answer. Seventeen focused public
parents pass, including actual deaths before copying and after displacement,
two restarts, Copy/Keep/retry, extra children and stale/replacement refusal.
Final validation on undo-displaced-inputs-04.json (889 unchanged inputs) passes
536 backend tests across 20 targets (49 ignored helpers) and 302 normal isolated
desktop-feature library tests (29 ignored), with zero failures or filters. Both
required active/stopped batch restart gates, default all-target Clippy with warnings
denied, Files formatting and fixture schema/syntax pass. The default receipt is
continuation-core-1792820459-receipt.json. Exact attempts and source relations are in
[undo-displaced-verification-01.json](undo-displaced-verification-01.json). Four failed
fixture roots retain 22 decoded/live-verified entries. Final source and evidence are
tracked by [undo-displaced-source-01-verification.json](undo-displaced-source-01-verification.json).
Application/Niri binaries and the required restart gate are preserved. Exact next
work is in [UNDO-DISPLACED-NEXT.md](UNDO-DISPLACED-NEXT.md).

Automatic displaced rollback/disposal, completed-reversal cleanup and other generic
Forward/Undo paths remain unfinished. All broader five-area requirements remain
open and the full goal stays active.

## Previous Phase2 checkpoint — Undo partial disposal, 2026-10-24

[ADR0143](../../docs/files/adr/0143-undo-partial-disposal.md) tracks schema51.
Two public-worker crashes reproduced unchanged Undo partials requiring manual Keep
before retry despite having original source proofs and matching partial bytes.
Non-displaced Undo return work now uses the existing Reserved/Claimed/Consumed
disposal protocol, with its independent Undo binding advancing atomically. Source
generation and prefix checks run before reservation and again before the live
one-shot unlink. Edited or unavailable-source data remains protected.

Consumed restart work has empty-root rights only. Fresh Keep can preserve both
Undo disposal roots and their available leaves without consuming underlying Undo
history. New acceptance twice reproduced ignored journal deletion reporting success
after deleting the binding; retirement now verifies own-lane journal/binding absence
before commit, rolling back refused or ignored retirement together.

Thirteen focused public acceptance parents pass. Final validation on
undo-disposal-inputs-04.json (886 unchanged inputs) passes 532 backend tests across
20 targets (49 ignored helpers) and 298 normal isolated desktop-feature library
tests (29 ignored), with zero failures or filters. Both required batch restart
gates, default all-target Clippy with warnings denied, Files formatting and fixture
schema/syntax pass. The default receipt is continuation-core-1792817787-receipt.json.
Exact results are in [undo-disposal-verification-01.json](undo-disposal-verification-01.json).
Eight failed-fixture roots retain 38 decoded/live-verified entries. Final source
and evidence are tracked by [undo-disposal-source-01-verification.json](undo-disposal-source-01-verification.json).
The application/Niri binaries and required restart gate are preserved.

Exact next work is in [UNDO-DISPOSAL-NEXT.md](UNDO-DISPOSAL-NEXT.md). Displaced Undo,
completed-reversal cleanup, other generic cleanup and all broader five-area
requirements remain open. The full goal remains active.

## Previous Phase2 checkpoint — Undo return retention, 2026-10-24

[ADR0142](../../docs/files/adr/0142-undo-return-work-retention.md) tracks schema50.
Two actual public-worker crashes reproduced loss of edited cross-filesystem Undo
return partials: Reconcile deleted the only edited bytes, removed their work folder,
cleared the journal and reported success. Undo return preparation now has an
independent creator-bound history/step/journal/source record before data writes.
Recovery verifies that record and preserves payloads rather than blindly cleaning
them. Incomplete returned files cannot falsely complete their history step.

Fresh Keep review can retain a bound non-displaced Undo return's available leaf and
whole work folder atomically, including extra children, then retire only the exact
journal/binding. Recovery copies and the original remaining Undo follow existing
newest-first history ordering. Changed journals, replacement work roots and refused/
ignored saves remain protected. Seven focused public parents pass, including legacy
copies, two restarts, full Copy/Keep/Undo retry, and Keep-both relocation. Broader
acceptance reproduced rejection of explicitly reviewed cross-filesystem returns.
Registration now checks the selected version against the actual opened source,
normalizes only approved review fields, and retains the original history hash.
Original-location edits are refused before preparing a return payload, after the
original parent is validated; installation still rechecks later edits.

Final verification on undo-abort-inputs-09.json (884 unchanged inputs) passes 526
backend tests across 20 targets (48 ignored helpers), including both required batch
restart gates, and 292 normal isolated desktop-feature library tests (28 ignored).
All results have zero failures/filters. Default all-target Clippy with warnings
denied, formatting and fixture schema/syntax pass. The default receipt is
continuation-core-1792815539-receipt.json. Exact attempts are in
[undo-abort-verification-01.json](undo-abort-verification-01.json). Twelve private
failure roots retain 67 decoded/live-verified entries. Final source/evidence is
tracked by [undo-abort-source-01-verification.json](undo-abort-source-01-verification.json).
The application/Niri binaries and required restart gate are preserved.

This does not finish generic Forward/Undo cleanup. Automatic verified partial
disposal, displaced Undo work, completed-reversal cleanup and other legacy/generic
paths remain. Exact attempts and current handles are in
[UNDO-ABORT-NEXT.md](UNDO-ABORT-NEXT.md). The complete five-area goal stays active,
including Trash/Restore, backend separation, product/device/accessibility/physical,
reliability/performance, complete native/workspace and release integration.

## Previous Phase2 checkpoint — nested restricted-folder access, 2026-10-24

[ADR0141](../../docs/files/adr/0141-nested-recovery-access.md) continues schema49.
Two public runs reproduced nested Continue failing before unfinished work because
only its immediate destination parent was borrowed. Transfer now opens owned created
ancestors outermost first, proving the recorded folder and parent generations before
permission changes. A separate actual process-death fixture twice reproduced an
inner read grant staying 0500 after its already-restored outer folder hid it at 0400.
Startup now borrows proven ancestors durably, restores the target and unwinds all
opened ancestors through descriptors. Failure preserves original grant evidence.

The inputs07 matrix passed 517 backend tests, then added acceptance reproduced two
live regressions twice: Continue replaced external modes 0701 and 0444 with the older
reader's borrowed 0500. Borrowing now uses a reader's original mode only for its
actual recorded grant. Writer restoration requires matching reader mode/object/
parent context; fresh Recovery can acquire newer context while older selections live.

Final verification on nested-access-inputs-09.json (882 unchanged build inputs)
passes 519 backend tests across 20 targets (47 ignored helpers) and 285 normal
isolated desktop-feature library tests (27 ignored), with zero failures/filters.
All eight new acceptance parents, both required batch restart gates, default
all-target Clippy02 with warnings denied, Files formatting03 and fixture schema/
syntax03 pass. The default receipt is continuation-core-1792809594-receipt.json.
[nested-access-verification-01.json](nested-access-verification-01.json) records exact
attempts, input relations, results and protected binaries. Two failure archives
preserve nine roots and 195 decoded/live-verified entries, with original bytes,
modes, mtime and objects; archive-only permission borrowing records ctime changes.
Final source/evidence is tracked by
[nested-access-source-01-verification.json](nested-access-source-01-verification.json).
The application/Niri binaries and required restart gate are preserved. Complete
native/workspace or release acceptance is not claimed. Exact remaining work is in
[NESTED-ACCESS-NEXT.md](NESTED-ACCESS-NEXT.md).

Next: generic Forward/Undo abort cleanup, broader Undo/expiry, queued lineage, large
reports and Trash/Restore. All backend separation, product/device/accessibility/
physical, startup/focus/performance, native/workspace/Shell/Notify/Niri, release and
GTK-retirement requirements stay open. The complete five-area goal remains active.

## Previous Phase2 checkpoint — restricted-parent Recovery access, 2026-10-24

[ADR0140](../../docs/files/adr/0140-recovery-folder-read-access.md) tracks schema49.
An actual queued Copy/Continue worker dies while writing beneath a Files-created
folder whose original mode is0400. Explicit Reconcile preserves an edited partial
and restores that mode, but repeated public acceptance reproduced all Recovery rows
being unavailable because they could not search the folder.

Recovery selections now borrow owner read/search access through original directory
and parent generations, backed by durable permission grants. Leases are shared by
live selections, omitted from serialization, and restored on a dedicated worker or
after process death. Reader/write borrowing shares original modes; replacements,
external mode edits and refused/ignored saves remain protected. Keep also retires
its restored write-access record so the original accepted cursor can regain Continue.
Fresh Recovery pages can inspect archived retained data after a second restart.

Final verification on restricted-access-inputs-09.json (881 unchanged build inputs)
passes 511 backend tests across 20 targets (44 ignored helpers) and 277 normal
isolated desktop-feature library tests (24 ignored), with zero failures/filters.
This includes all four new acceptance parents, both required batch restart gates,
permission restoration, live cancellation and source-change batch continuation.
All-target default Clippy with warnings denied, formatting and fixture schema/syntax
checks pass. The default receipt is continuation-core-1792805716-receipt.json.
[restricted-access-verification-01.json](restricted-access-verification-01.json)
records exact attempts, input relations, results and protected binaries. Seven failed
private roots retain 134 decoded and live-verified archive entries. Bytes, modes,
mtime and object identities match; ctime changes from controlled archive permission
borrowing are recorded. The application/Niri binaries and required gate are preserved.
No complete native/workspace or release acceptance is claimed. Exact remaining work
is in [RESTRICTED-ACCESS-NEXT.md](RESTRICTED-ACCESS-NEXT.md).

Next: nested restricted ancestors and deaths between ancestor/descendant permission
releases, then generic Forward/Undo abort cleanup. Broader Undo/expiry, queued lineage,
large reports, Trash/Restore and all backend separation, product/device/accessibility/
physical, startup/focus/performance, native/workspace/Shell/Notify/Niri, release and
GTK-retirement requirements remain open. The complete five-area goal remains active.

## Previous Phase2 checkpoint — reproducible partial disposal, 2026-10-24

[ADR0139](../../docs/files/adr/0139-reproducible-partial-disposal.md) tracks schema48.
Two public-worker crash tests reproduced loss of the remaining partial after its
source disappeared. Separate public Recovery tests also reproduced loss of edited
partial bytes. The captured state contained no retained version or published target;
cleanup had retired both the journal and its binding and reported success.

Copy preparation now binds a reproduction descriptor from the actual opened source
into the independent stage intent. Claimed disposal requires the original source
parent/entry generations, original version and a byte-for-byte partial-prefix or raw
symlink-target comparison. Verified handles and strict live snapshots survive the
consumed save, with current-name/source/payload checks immediately before unlink.
Cancellation is forwarded through reconciliation and checked between bounded reads.
Missing/changed originals and divergent partial bytes remain available for Recovery
Copy/Keep; legacy missing proofs are not replaced with observations of current paths.

The inputs05 backend library passed 191 tests (21 ignored helpers), including six
new acceptance parents for Copy/cross-filesystem Move, two restarts, Recovery Copy/
Keep, original returns versus identical-byte replacements, old bindings, cancellation
and late source/payload changes. Its integration run stopped at operation_transfers:
a cancelled copy token incorrectly aborted verified compensation and produced
Interrupted instead of Stopped. The exact test reproduced again; improved terminal
state diagnostics confirmed the cause. Live compensation now uses a separate token,
while explicit Recovery comparison remains cancellable. The exact regression passes.

The inputs07 run passed library02 and batches50..54, then found a second regression:
source-change failure retained its partial but escaped before reporting the failed
item, preventing unrelated batch work. A typed reproduction-problem marker now lets
only the exact live failed task retain its proven work roots atomically and proceed.
Restart reconciliation still requires explicit review. The strengthened direct-engine
batch test passes and also verifies Recovery Copy and complete root cataloguing.

Final verification on partial-disposal-inputs-11.json (879 unchanged build inputs)
passes 507 backend tests across 20 targets (42 ignored helpers) and 273 normal
isolated desktop-feature library tests (22 ignored), with zero failures/filters.
This includes 192 backend library tests, all seven new public acceptance parents,
both required batch restart gates, live cancellation and changed-source batch
continuation. All-target default Clippy03 with warnings denied, workspace format03
and fixture schema/syntax checks pass. The default receipt is
continuation-core-1792800566-receipt.json.
[partial-disposal-verification-01.json](partial-disposal-verification-01.json) records
exact source, failed/green attempts, results and protected binaries. Final source
and attempt manifests/logs are decoded in
[partial-disposal-source-01-verification.json](partial-disposal-source-01-verification.json).
The data-loss failure archive preserves four private roots and 35 verified entries;
transient integration-test roots were not retained and are not claimed archived.
These checks do not constitute full native/workspace or release acceptance.

Next: finish scoped verification and restricted created-parent access acceptance,
then remaining generic Forward/Undo abort cleanup. Broader Undo/expiry, queued lineage,
large reports/paged owners, Trash/Restore and all backend separation, product/device/
accessibility/physical, startup/focus/performance, native/workspace/Shell/Notify/Niri,
release and GTK-retirement requirements stay open. Prefix-verification startup costs
still need measurement. The full five-area goal remains active.

## Previous Phase2 checkpoint — public disposal Keep, 2026-10-24

[ADR0138](../../docs/files/adr/0138-reviewed-disposal-retention.md) tracks schema47.
Public Keep in Recovery now retains both original disposal work roots, including
extra children, in one metadata transaction. It preserves available leaf identities,
exposes whole-root Recovery copies and derives continuation only from the accepted
request and independent cursor. A fresh nonserialized review checks both original
parent/root generations, observed versions and exact pending/task/access context.
Changed review or substituted roots refuse retention; no filesystem data is moved.

A private ignored-DELETE test reproduced false Finished twice: the pending journal
remained while its task binding disappeared. Keep now verifies retained records,
namespace/physical indexes and journal retirement before commit. Aborted or ignored
writes roll back all retention metadata. Failure evidence and manifests are kept
under disposal-keep-*; the preceding schema46 checkpoint remains independently
preserved. Final validation on 878 unchanged inputs in disposal-keep-inputs-06.json
passes 500 backend tests across 20 targets (42 ignored helpers) and 266 normal
isolated desktop-feature library tests (22 ignored), with zero failures/filters.
Both required active/stopped batch restart gates pass. Default all-target Clippy
with warnings denied, workspace formatting and fixture schema/syntax checks pass.
[disposal-keep-verification-01.json](disposal-keep-verification-01.json) records exact
source, attempts, results and protected binaries. The decoded source/evidence snapshot
is recorded in [disposal-keep-source-01-verification.json](disposal-keep-source-01-verification.json).
Three failed private roots retain 43 decoded verified entries in the failure archive.
These checks do not constitute complete native/workspace or release acceptance.

Next: cover edited partial/source disappearance and restricted created-parent access. Replace remaining generic Forward/Undo abort
cleanup, then legacy/reverse Undo, expiry, queued lineage, large reports/paged
owners and Trash/Restore. Backend separation, product/device/accessibility/physical
acceptance, startup/focus reliability and measurements, complete native/workspace
checks, Shell/Notify regressions, Niri distribution, release packaging/docs and GTK
reference retirement remain required. The complete five-area goal stays active.

## Previous Phase2 checkpoint — preparation disposal namespaces, 2026-10-23

[ADR0137](../../docs/files/adr/0137-preparation-disposal-namespaces.md),
[STAGE-DISPOSAL-WIP.md](STAGE-DISPOSAL-WIP.md) and
[STAGE-DISPOSAL-NEXT.md](STAGE-DISPOSAL-NEXT.md) track schema46.
Two actual public-worker deaths reproduced cleanup deleting a newly recreated
folder name linked to the already unlinked original inode. The captured journal
and task binding were unchanged and the actual payload proof remained valid.

Pending::Disposing now retains the original preparation and a separately created
holding Stage through Reserved, Claimed and Consumed phases. Private creator and
transition witnesses advance the original journal/task binding atomically. Save
Consumed before unlink: only that live transition can unlink once; after restart,
only empty-root retirement remains. Unknown or reused entries in either namespace
stay intact for review. Public Recovery copies work for retained known-leaf data,
foreign replacement generations and links to the original inode. Phase-save abort,
ignored writes and retirement failures preserve the original durable state.

Validation passes 495 selected backend tests across 20 targets with 42 ignored
helpers and zero filters. Normal isolated desktop-feature library passes 261 with
22 ignored helpers and zero filters. Both required batch restart gates were rebuilt
and rechecked after cache changes: 17 target tests pass, with 3 ignored helpers.
Default all-target Clippy with warnings denied, formatting and fixture schema/syntax
checks pass on 877 captured inputs in stage-disposal-inputs-06.json.
[stage-disposal-verification-01.json](stage-disposal-verification-01.json) records
exact source, failed/green attempts, fresh gates and artifact-state changes.
[stage-disposal-source-01-verification.json](stage-disposal-source-01-verification.json)
records the decoded source/evidence snapshot. Failure archive retains 3 roots/40
verified entries; original temporary fixtures disappeared during environment changes,
so they are not claimed live. Two desktop builds failed on disk/quota before tests;
the third normal build passed. The existing Files executable was preserved at its
newly observed hash; missing older test caches are recorded honestly.

Next: a public Keep disposition for retained disposal data. Current Keep in Recovery
supports Rename only; tests use external fixture moves after public copies. That
is not finished user-facing review/continuation. Also verify/implement arbitrary
extra-child/whole-root Recovery copies, edited partials/source disappearance and
restricted created-parent access. Replace remaining generic Forward/Undo abort
cleanup, then finish legacy/reverse Undo, expiry, queued lineage, large reports,
Trash/Restore and all other backend, product, native/device/accessibility,
performance, workspace, Niri distribution, release and GTK-retirement requirements.
The complete five-area goal remains active.

## Previous Phase2 checkpoint — preparation and cleanup task bindings, 2026-10-23

[ADR0136](../../docs/files/adr/0136-preparation-and-cleanup-task-bindings.md),
[STAGE-TASK-WIP.md](STAGE-TASK-WIP.md) and [STAGE-TASK-NEXT.md](STAGE-TASK-NEXT.md)
track schema45. Actual stage creators supply independent history, lane, operation,
task and pending-body bindings before recovery directory access. Verified Forward
transitions supply atomic empty-cleanup bindings; returned work retains empty-only
rights. Original history/source/parent/work/payload generations and raw Unix paths
remain intact. Missing proofs preserve staged data for Recovery review.

Interrupted reports now have bounded separate storage against the exact accepted
revision/body digest. Journal failures remain Interrupted; failed report saves use
SaveFailed and retry metadata only. Neither replaces the accepted Plan or grants
execution ownership. Startup requires explicit review before deriving continuation
from the original Plan and independently verified cursor/end. This fixes both the
fatal-journal status regression and final-trim startup resumption regression.

Eight public crash parents cover actual deaths, Copy/cross-Move/raw-link remaining
work, metadata/receipt refusal, substitutions and original returns, unknown data,
payload-save rollback, completed output/source changes, and atomic retirement
failure. Worker integration also covers report-save abort/retry, two restarts,
confirmed-folder history and no actions before review. Selected backend validation
passes 488 tests across 20 targets with 41 ignored helpers and zero filters; both
required active/stopped batch restart tests pass unignored. Isolated default-feature
library passes 254 with 21 ignored helpers and zero filters. Default all-target Clippy
with warnings denied, workspace formatting and fixture syntax/schema45 pass.

[stage-task-verification-01.json](stage-task-verification-01.json) records exact
875 inputs in stage-task-inputs-14.json, failed/green attempts, input relations,
protected binaries and test results. Failure archive 02 preserves 113 private roots,
755 verified entries including raw symlinks, hardlinks and empty directories.
[stage-task-source-01-verification.json](stage-task-source-01-verification.json)
records the decoded source/evidence archive and its hash.

Next: durable generic-preparation disposal claims. A second crash after unlink
must preserve a newly reused folder name, including the same original inode.
Metadata task bindings do not finish this consumed-name protocol. Legacy/reverse
Undo, expiry, queued lineage, large reports/paged owners, Trash/Restore and the
remaining backend, product, native/device/accessibility, performance, workspace,
Niri distribution, release and GTK-retirement work stay open. Full goal stays active.

## Previous Phase2 checkpoint — copied saved-container restart claims, 2026-10-23

[ADR0135](../../docs/files/adr/0135-copied-saved-container-restart-claims.md),
[SAVED-CLAIMS-WIP.md](SAVED-CLAIMS-WIP.md) and
[SAVED-CLAIMS-NEXT.md](SAVED-CLAIMS-NEXT.md) track the current schema44 work.
Copied saved-container Moves now bind an immutable original recipe/context and
an exact task receipt, publish through held verified directories with NOREPLACE,
and commit catalog/reservation changes atomically before source cleanup.
Verified root and file claims protect cleanup, including new source namespaces,
phase-save rollback and interrupted reconciliation. After proven source deletion,
only original empty-root cleanup remains; completed output edits/replacements and
destination parent replacements are preserved without reopening their names.

New Undo Move-to-Recovery archives now record selected original entry/container
proof at creation. Broader Undo verification exposed six missing-proof regressions;
the creation fix restores all six without adopting legacy paths. Folder Keep-item
review also remains available when a folder replaces a copied file. The consumed-
claim completion bug was separately reproduced twice and fixed. Actual public
worker crash parents cover archived versions, raw links, receipt/context/phase
refusal, substitutions, unknown private work, batch/nested work and second crashes.
The selected backend passes 479 tests across 20 targets, with 40 ignored helpers.
This includes 165 passed and 19 ignored in the full no-default library, with zero
filters. Both required active and stopped batch gates and the formerly red completed-cross gate pass. Default
all-target Clippy with warnings denied, workspace formatting and fixture syntax
pass on saved-claims-inputs-06.json (872 captured inputs). Isolated default-feature
library passes 246 tests with 20 ignored helpers and zero filters.
[saved-claims-verification-01.json](saved-claims-verification-01.json) records exact
inputs, test results and required unignored restart gates;
[saved-claims-source-01-verification.json](saved-claims-source-01-verification.json)
records the decoded source archive and final artifact hash.

This scoped progress does not complete Copy/Move restart recovery. Generic
Preparing/Cleanup provenance, legacy/reverse Undo and expiry, queued lineage,
large reports/paged owners, Trash/Restore and all remaining goal areas remain
required. Native/device/accessibility/performance/workspace/release acceptance
is still open. The full five-area goal stays active.

## Previous Phase2 foundation — cross-filesystem saved-container recovery, 2026-10-22

[ADR0134](../../docs/files/adr/0134-saved-copy-descriptors-and-created-ownership.md),
[SAVED-CROSS-WIP.md](SAVED-CROSS-WIP.md), [SAVED-CROSS-DESIGN.md](SAVED-CROSS-DESIGN.md)
and [SAVED-CROSS-NEXT.md](SAVED-CROSS-NEXT.md) track active work. Forward copy now
uses the pre-review selected source directory and destination parent handles.
Saved-copy metadata restoration uses the created file/link FD; an actual race
after the final name check previously overwrote a substituted file's timestamp.
Target ownership now carries its already captured created directory/payload proof
through catalog cutover, enabling later moves and successive older Undo without
freshly adopting current paths. Missing proof cannot downgrade a strong owner.

The actual completed-cross public restart gate remains RED, unignored, on the
same 871 captured inputs: uncovered completed operation#2. It was the sole
explicitly filtered test in the 152-pass/18-ignored backend library check.
Saved-container integration passes 24/1, Clippy with warnings denied and formatting
pass. Both metadata and ownership regressions were reproduced twice. Failure and
source archives preserve raw links/empty directories, fixture corrections and
unchanged protected binary hashes. This is a foundation, not completed restart,
full backend/native/workspace/release or device acceptance. The full goal stays active.

Next: original recipe/context and exact task receipts, descriptor publication,
explicit source cleanup claims, actual crash continuation and refusal checks.
Broader Preparing/Cleanup, legacy/queued lineage, large reports/paged owners,
Undo/expiry, Trash/Restore and all remaining user goal areas are still open.

## Previous Phase2 checkpoint — saved-container whole-Move recovery, 2026-10-22

[ADR0133](../../docs/files/adr/0133-saved-container-whole-move-restart-tasks.md),
[SAVED-TASK-WIP.md](SAVED-TASK-WIP.md) and
[SAVED-TASK-NEXT.md](SAVED-TASK-NEXT.md) track same-filesystem saved-container
Move recovery. Actual catalog-owned data now uses descriptor-held entry claims,
original source/container/previous/target proofs, a distinct exact task receipt,
atomic catalog parent rebasing, and returned-task disposition. Completed metadata
preserves newer data and source occupants; nested saved steps do not add counts.
A deleted-receipt repro exposed mutation before job-proof refusal. Supported
modern Forward steps now validate their existing receipt before recovery prepares
any directory access or mutates filesystem data. Weak legacy audit remains open.

Nine new public-worker parents pass across actual claim/publication/return/cleanup
crashes, raw links, Keep Both and later batch/nested work, substitutions, bad
receipts and metadata rollback. The selected backend passes 464 parents across
20 targets, 38 ignored, on inputs02. The final isolated default library passes
232 parents, 18 ignored, on inputs03. Its only later change collapses two equivalent
parent-rebasing guards per Clippy; the original and exact diff are preserved.
Clippy with warnings denied, workspace formatting and schema43 fixture syntax/
hash pass. Verification and archive proofs check the 870 final inputs, both
required unignored restart gates, retained failures and protected binary hashes.

Next: cross-filesystem saved-container migration and cleanup phases, broader
Preparing/Cleanup provenance, weak legacy bindings/phase reinsertion, queued
admission/lineage, large reports/paged owners, sanctioned Undo/relocation/expiry
and Trash/Restore. Backend separation, product/device/accessibility/physical
acceptance, reliability/performance and complete release integration remain open.
The full five-area goal remains active; these checks do not establish native,
full-workspace, release or physical-device acceptance.

## Previous Phase2 checkpoint — reviewed replacement recovery, 2026-10-22

[ADR0132](../../docs/files/adr/0132-reviewed-replacement-restart-tasks.md),
[REPLACEMENT-TASK-WIP.md](REPLACEMENT-TASK-WIP.md) and
[REPLACEMENT-TASK-NEXT.md](REPLACEMENT-TASK-NEXT.md) track replacement tasks.
An exact reviewed-replacement matcher binds the selected source, actual target,
old destination and retained stage. Descriptor-verified Refreshed values authorize
only their exact canonical before/after change. One transaction updates the
existing receipt and byte budget, appends history and records empty-only cleanup.
Phase updates cannot recreate missing proof. Returned replacements retire their
uncommitted receipt atomically; continuation asks for a fresh conflict answer.
Cross-filesystem Move retains only its original copied-source cleanup.

Nine public worker parents pass actual Copy/Move/cross-filesystem crash phases,
raw links, batch continuation, saved data, bad proofs, metadata rollback, source/
parent substitution refusal and second-crash private-payload preservation.
The final library passes 142 parents, 16 ignored. The selected backend set passes
455 parents across 20 Cargo targets, 37 ignored; the isolated default library
passes 223 parents, 17 ignored. A full-library regression at an
unused Ready claim was reproduced and fixed by explicitly recording its verified
Returning subject; the public Ready gate covers it too. Clippy with warnings
denied, formatting and schema42 fixture syntax/hash pass on the same 869 final
formatted inputs. Both required restart gates remain unignored. Source/relation,
verification and archive records preserve the failed attempts, Ready correction,
format capture correction and unchanged protected binary hashes.

Next: saved-container task phases, broader Preparing/Cleanup provenance, weak
legacy bindings and equivalent reinsertion in other claim families, queued
admission/lineage, large reports/paged owners, sanctioned Undo/relocation/expiry
and Trash/Restore. Backend separation, product/device/accessibility/physical
acceptance, reliability/performance and complete release integration remain open.
The full five-area goal remains active. Focused checks do not establish native,
full-workspace, release or physical-device acceptance.

## Previous Phase2 checkpoint — ordinary whole-entry Move recovery, 2026-10-22

[ADR0131](../../docs/files/adr/0131-whole-entry-move-restart-tasks.md),
[WHOLE-MOVE-WIP.md](WHOLE-MOVE-WIP.md) and
[WHOLE-MOVE-NEXT.md](WHOLE-MOVE-NEXT.md) track actual ordinary Move tasks.
Forward binds the exact durable head and canonical Move before mutation; restart
advances only its proved committed suffix to one count and the actual chosen
destination. Completed output and newer source occupants are not reopened.
Keep Both with later batch work remains resumable after completed entries change.
Ready/Returning retires its exact uncommitted receipt atomically with empty-only
Preparing. A Forward phase update cannot recreate a missing receipt. Publishing
with an absent payload cannot infer a return from a newer source name.

Nine public worker parents pass across actual file/link/folder crash phases,
repeated opens/Reconcile/Continue, bad committed and uncommitted task proofs,
Keep Both and later batch work, rollback and foreign private payload preservation.
The backend library passes 133 parents, 14 ignored helpers. The selected backend
set passes 446 parents across 20 Cargo targets, 35 ignored; the isolated default
library passes 214 parents, 15 ignored. Default all-target Clippy with warnings
denied, workspace formatting and schema42 fixture syntax/hash pass on the same
868 captured inputs. Both required restart gates remain unignored. The WIP and
verification records retain source relationships, archives, failed build/retry
evidence and unchanged protected binary hashes. No schema or
continuation encoding changes. Existing unfinished generation checks stay strict.

Next: reviewed replacement and saved-container task recovery, then broader
Preparing/Cleanup provenance and weak legacy bindings, queued admission/lineage,
large reports/paged owners, sanctioned Undo/relocation/expiry and Trash/Restore.
Backend separation, product/device/accessibility/physical acceptance,
reliability/performance and complete release integration remain unfinished.
Focused checks do not establish native, full-workspace, release or hardware
acceptance. The full five-area goal remains active.

## Previous Phase2 checkpoint — source-directory removal recovery, 2026-10-22

[ADR0130](../../docs/files/adr/0130-source-directory-removal-restart-recovery.md),
[DIRECTORY-FINISH-WIP.md](DIRECTORY-FINISH-WIP.md) and
[DIRECTORY-FINISH-NEXT.md](DIRECTORY-FINISH-NEXT.md) track the exact postorder task.
RemovedDirectory now binds the actual original source/parents/destination,
mode and operation/range/creation facts before mutation. Restart advances only
its independently proved committed suffix, preserving counts, completed output
and newer source entries. Later unrelated batch tasks remain resumable even if a
completed folder has since been replaced. Ready/Returning return retires only
its uncommitted receipt atomically with empty-only Preparing; a Claimed absent
payload never infers a returned source from the old name.

Eighteen public directory boundary parents pass, including nine added here:
actual SIGKILL through removal/return phases, empty Merge counts, later batch
work, changed/missing proofs, return rollback, a second crash with foreign stage
contents, and a crash in a resumed operation. The full backend library passes
124 parents, 12 ignored. The complete selected backend set passes 437 parents
across 20 Cargo targets, 33 ignored helpers; the isolated default library passes
205 parents, 13 ignored. Clippy with warnings denied, formatting and schema42
fixture syntax/hash pass on the same 867 captured inputs. Both required restart
gates stay unignored. The WIP and verification records preserve exact source
relationships, review patch, archives and unchanged protected binary hashes.

Next: whole-entry Move, reviewed replacement and saved-container task phases;
complete Preparing/Cleanup provenance, weak legacy bindings, queued admission/
lineage, large reports/paged owners and Undo/relocation/expiry. Trash/Restore,
backend separation, product/device/accessibility/physical checks,
reliability/performance and complete release integration remain unfinished.
These focused checks do not establish native, full-workspace, release or hardware
acceptance. The full five-area goal remains active.

## Previous Phase2 checkpoint — directory expansion recovery, 2026-10-22

[ADR0129](../../docs/files/adr/0129-directory-expansion-restart-recovery.md),
[DIRECTORY-EXPANSION-WIP.md](DIRECTORY-EXPANSION-WIP.md) and
[DIRECTORY-EXPANSION-NEXT.md](DIRECTORY-EXPANSION-NEXT.md) track directory expansion.
Schema42 records original opened folder/stage facts, reviewed destinations and
raw child names in bounded normalized rows before publication or child work.
Creation receipts bind the exact head and expansion. Restart advances only that
proved creation, or the accepted no-history Merge, to its captured child queue
and Move postorder facts. Original outputs, later source entries and newer
unrelated destination contents remain intact. Empty operation records stay while
an active cursor needs them; checkpoint5 records true empty-folder facts and
retains ordinary checkpoint3/4 otherwise.

A proved Preparing expansion can promote its own Forward/context/receipt in one
transaction. Directory reconciliation verifies actual original parent/work/payload
before NOREPLACE publication, mode writes and append. Missing/borrowed/changed
proofs and replacement folders or parents cannot supply new execution owners.
Weak legacy folder journals remain in review without chmod/publication/deletion.
The real readonly crash gate verifies successful recovery with original proofs.
Nine new public-service parents cover real SIGKILL, Copy/cross-filesystem Move,
Keep Both/Merge, repeated opens/Continue, empty counts, replacement parents, bad
rows, promotion rollback, a second crash, 770 long names and readonly modes.

The final captured source passes 428 selected backend parents across 20 targets,
33 ignored helpers (including 115 library parents), and 196 isolated default-library
parents, 13 ignored. All-target Clippy with warnings denied, formatting and schema42
fixture syntax/hash pass on the same 867 inputs. Both required stopped/active
restart gates remain unignored. The WIP and verification records preserve exact
source relationships, legacy fixture changes, archives and protected binary hashes.

Next: source-directory removal task proofs and advancement; then whole-entry
Move, reviewed replacement and saved-container phases. Broader Preparing/Cleanup
binding, legacy bindings, queued lineage/admission, large reports/paged owners,
Undo/relocation/expiry and Trash/Restore remain. Backend separation, broader
product/device/accessibility and physical checks, first-window/focus reliability
and performance, complete native/workspace/release/Shell/Notify/Niri integration
and GTK reference retirement remain unfinished. No native, full-workspace,
release or hardware acceptance is claimed by these focused checks.

## Previous Phase2 checkpoint — recovered end Retry Save, 2026-10-22

[ADR0128](../../docs/files/adr/0128-recovered-transfer-end-save-ownership.md) and
[ENGINE-END-RETRY-WIP.md](ENGINE-END-RETRY-WIP.md) track engine-end metadata retry.
A fully verified restart cursor now retains its typed result on end-record or
cursor-retirement failure. The engine binds a process-local metadata-only witness
to its completion scope and exact accepted request. Startup and public Reconcile
carry the actual confirmation into SaveFailed and retain application activity.
Corrupt/unproved cursor evidence remains Interrupted and cannot mint a witness.

Completed Retry Save records the end/retirement atomically before the independent
application save. Stopped Retry Save uses the existing witnessed end/result
transaction; unfinished continuation owners remain unavailable until it succeeds.
New public-service checks cover actual SIGKILL, startup/review, Copy/cross-filesystem
Move, repeated write failures, rollback, activity holds, edits/inodes preserved,
Abandon/reopen and changed accepted request/reservation refusal. Schema41 and
continuation3/4 remain unchanged. The repeated original failure and its data are
preserved. The initial captured source passes 419 selected backend parents across 20
targets, 31 ignored helpers. Clippy then requested only two equivalent let-chain
forms. The final source passes 36 restart/execution integration parents, 4 ignored,
and 187 isolated default-library parents, 11 ignored. Clippy with warnings denied,
formatting and schema41 fixture syntax/hash pass. Both required restart gates are
unignored. The WIP and verification records preserve exact source relationships,
commands, archives and protected binary hashes.

Next: directory publication and accepted Merge child-queue durability, directory
removal and other task phases, complete Preparing/Cleanup provenance, legacy loose
bindings, queued lineage/admission, large reports/paged owners and Undo/relocation/
expiry. Trash/Restore, backend separation, broader product/device/accessibility
acceptance, first-window/focus reliability and performance, complete native and
workspace/release/Niri integration and GTK reference retirement remain unfinished.
No native, full-workspace, release or physical-device acceptance is claimed here.

## Previous Phase2 checkpoint — returned-source task dispositions, 2026-10-22

[ADR0127](../../docs/files/adr/0127-returned-source-task-dispositions.md),
[SOURCE-RETURN-WIP.md](SOURCE-RETURN-WIP.md) and
[SOURCE-RETURN-NEXT.md](SOURCE-RETURN-NEXT.md) track Ready/Returning continuation.
Recovery's proved source return now retires only its exact uncommitted removal
receipt atomically with the empty-only Preparing intent. The committed Copy
proof remains, so continued cleanup preserves its output inode and never recopies.
The dedicated transition verifies its Forward context, phase, operation, stage,
active head and step. It cannot retire a committed or changed removal or treat
Claimed as returned. No row format or checkpoint version changed.

Four new unignored public-service parents cover actual Ready/Returning SIGKILL,
explicit review, repeated opens, continued cleanup, a second crash after Preparing
commits, unexpected private contents, byte-identical replacement source/output
files, damaged contexts/head/step proofs, and transaction refusal/rollback at
receipt deletion or Preparing save. The original public continuation test failed
twice before the fix; both failure pairs remain preserved.

All 417 selected backend parents across 20 targets pass, 31 ignored helpers,
including 104 library parents. The isolated default desktop library passes
185 parents, 11 ignored. Clippy with warnings denied, formatting and schema41
fixture syntax/hash pass on the same 864 captured input paths. Both required
restart gates remain unignored. Exact profiles, archives and unchanged protected
binary hashes are in the WIP record. No native/full-workspace/release or hardware
acceptance is claimed.

Next: an engine-owned metadata Retry Save witness when recording an end fails;
directory publication/child queues and source-directory removal cursor phases;
whole-entry Move, reviewed replacements and saved-container phases. Complete
Preparing/Cleanup binding, legacy loose-binding handling, queued lineage/admission,
large reports/paged owners and Undo/relocation/expiry remain. Trash/Restore,
renderer separation, product/device/accessibility checks, first-window/focus
reliability and performance, release/Niri integration and GTK reference retirement
remain part of the full active goal.

## Previous Phase2 checkpoint — final-leaf completion, 2026-10-22

[ADR0126](../../docs/files/adr/0126-final-leaf-restart-completion.md),
[TERMINAL-LEAF-WIP.md](TERMINAL-LEAF-WIP.md) and
[TERMINAL-LEAF-NEXT.md](TERMINAL-LEAF-NEXT.md) track schema41 terminal recovery.
An exact proved final Copy/removal now records the existing CompletedTransfer
end and retires its cursor in one transaction. Stopped results retain their
strict nonempty invariant. Completed paths are not reopened, so edited/replaced
outputs, new source occupants and missing/replaced parents remain untouched.
Pending journals still require explicit captured Recovery review. Independent
application-result failure retains working Retry Save and activity ownership.

Normalized role7 rows retain earlier source-folder remainder observations before
later tasks. They are descriptive metadata with no execution rights. Normal live
returns keep their existing final report refresh. A late real SIGKILL regression
also caught a Claimed source incorrectly treated as returned when the original
inode was reintroduced through a hard link. Only Ready may infer that no claim
happened; explicit Returning retains its own proof. Claimed with absent payload
now consumes removal metadata without inspecting or changing the new name.

The initial captured source passes 412 selected backend parents and 180 isolated
default library parents. The final source changes only the claim guard and its
public hard-link regression; it passes 413 selected backend parents across 20 targets, 30 ignored helpers,
and 181 isolated default library parents, 10 ignored. All-target Clippy with
warnings denied, formatting and the schema41 fixture syntax/hash pass. Exact
profiles and receipts are in the WIP record. Both snapshots capture 864 Rust/
Cargo/vendor/fixture paths. Red datasets, prior sources and protected binaries
remain. No native/full-workspace/release or hardware
acceptance is claimed.

Next: persist exact Ready/Returning task dispositions when a verified return
clears its uncommitted removal binding; add an engine-owned metadata Retry Save
witness when recording an end fails; finish directory publication/children and
source-directory removal cursor phases, whole-entry Move, reviewed replacements
and saved-container phases. Keep exact step, original-object and unrelated-history
refusals. Preparing/Cleanup binding, queued lineage/admission, legacy and large-
report/paged-owner behavior, Undo/relocation/expiry, Trash/Restore, renderer
separation, product/device/accessibility checks, first-window/focus reliability
and performance, release integration and GTK retirement remain in the full goal.

## Previous Phase2 checkpoint — copied-leaf filesystem steps, 2026-10-22

[ADR0125](../../docs/files/adr/0125-copied-leaf-restart-steps.md),
[RESTART-COPIED-LEAF-WIP.md](RESTART-COPIED-LEAF-WIP.md) and
[RESTART-COPIED-LEAF-NEXT.md](RESTART-COPIED-LEAF-NEXT.md) track schema40's exact
copied-leaf crash continuation. Actual opened source/parent facts now accompany
a copy; a Forward step binds its durable queue head before mutation. Recovery
can consume a proved Copy or completed source unlink as metadata, preserving
completed edited data. An unfinished Move carries checkpoint4 copied-source
cleanup and verifies both original objects and fingerprints before removal;
it never recopies or adopts a replacement. Ordinary checkpoints stay version3.
Pending journals still require explicit Recovery review, which now reopens an
independently proved running transfer after successful captured reconciliation.

Seven new unignored public-service parents exercise actual SIGKILL at publication,
Copy append, source unlink and removal append, two opens, a second crash during
resumed cleanup, raw names/dangling links, Keep Both names, changed bytes/modes,
replacement entries/parents, damaged step proofs and stop-save rollback. Completed
unlink preserves a new source occupant and edited output. The original missing
step gate failed twice before its fix. Red failure datasets and proof/source
snapshots remain available.

The first captured source passes 404 selected backend parents across 20 targets,
30 ignored inventory helpers. The final source has an equivalent retry-helper
signature simplification for Clippy and an umask-independent permission fixture.
The final source rechecks 36 integration parents and passes 172 isolated default
desktop-library parents, 10 ignored. All-target Clippy with warnings denied,
formatting and the schema40 fixture syntax/hash pass. Both snapshots capture863
Rust/Cargo/vendor paths; exact source relationships, receipts and verified archives
are in the WIP record. Protected binaries remain unchanged. No native/full-workspace/
release or hardware acceptance is claimed.

Next: terminal receipts for a proved final leaf with no remaining part; directory
publication/children and source-directory removal cursor phases; whole-entry Move,
reviewed replacement and saved-container phase advancement; Preparing/Cleanup
binding, queued lineage/admission, legacy and large-report/paged-owner behavior.
Keep exact step, original-object and unrelated-advanced-history refusals. General
Undo/relocation/expiry, Trash/Restore restart, backend separation, product/device/
accessibility acceptance, first-window/focus reliability and performance, release
integration and GTK reference retirement remain part of the full active goal.

## Previous Phase2 checkpoint — active traversal cursors, 2026-10-22

[ADR0124](../../docs/files/adr/0124-active-transfer-traversal-cursors.md),
[RESTART-CURSOR-WIP.md](RESTART-CURSOR-WIP.md) and
[RESTART-CURSOR-NEXT.md](RESTART-CURSOR-NEXT.md) track schema39 active Copy/Move
traversal persistence. The new unignored active-worker SIGKILL gate passes Copy
and real cross-filesystem Move in both part orders, retaining exact reviewed
names, edited completed bytes/inodes and independent successor Undo.

The normalized ledger changes only affected tasks, directories and result rows.
Scope hashes bind their history, request revision/body, operation, role and key;
counts detect missing entries, and the history watermark must match with no
pending journal/directory-access records. Cursor retirement and the typed stopped
end are atomic. Original-object owner reopening still refuses replacements.
Tests cover borrowed/corrupt/missing/reordered entries, changed descriptors and
watermarks, advanced history and failed stop-save rollback across two opens.
A real SQL refusal trigger protects unchanged queued tails from rewrites.

Terminal receipts now preserve source remainders and cancellation/attention even
when no executable part remains, so completed parts retire their cursors. Startup
Retry Save also restores proved owners and retains the activity guard until
release. The final-file cancellation and result-save retry regressions failed
before their fixes. The captured inputs 01
passes 385 selected backend parents across 19 targets (including 84 library),
with 26 ignored inventory helpers. The only later input change is an equivalent
Clippy condition collapse; inputs 02 rechecks 120 focused parents and passes 165
isolated default desktop-library parents,8 ignored. Both snapshots capture 861
Rust/Cargo/vendor/fixture paths. Default all-target Clippy with warnings denied,
formatting and the schema fixture syntax pass. Source relationships and the
verified archive are in the WIP receipt; protected binaries are unchanged.
No native/full workspace/release or physical-device acceptance is claimed.

Next: prove and advance the interrupted filesystem step beyond the traversal
watermark, starting with the actual copied-source origin and Move source-removal
phase; persist directory publication/children and saved-container phases before
mutation; finish Preparing/Cleanup binding, queued lineage/admission, legacy and
large-report/paged-owner behavior. Do not replace this with pathname replay or
relax the watermark check. General Undo/relocation/expiry, Trash/Restore restart,
backend separation, product/device/accessibility checks, reliability/performance
and release integration remain required by the full active goal.

## Previous Phase2 checkpoint — stopped Copy/Move execution owners, 2026-10-22

[ADR0123](../../docs/files/adr/0123-restored-stopped-transfer-owners.md) and
[RESTART-OWNER-WIP.md](RESTART-OWNER-WIP.md) record actual restart owners for exact,
durably stopped transfer parts. The required unignored SIGKILL gate now passes
Copy and real cross-filesystem Move in both Retry/Continue orders. Completed,
edited output keeps its inode and bytes; each successor remains independently
undoable. Another killed worker between the parts proves that the consumed part
cannot be handed off twice and the other part survives.

Schema 38 binds new accepted intents to the original physical history file.
Restoration verifies the independent result seal, current request revision/body,
report and exact part hashes, then reopens only proved original selections and
destinations. Replacement entries, parents, traversed destinations, copied
histories, changed plans and stale revisions are refused. A proved independent
part can continue; sealed unavailable work can be abandoned without touching
files. Handoff checks its exact successor and advances the seal atomically;
generic scheduling writes cannot advance it. Restored owners retain the activity
guard until explicit release, including while accepted work is stopped.

The final captured source passes 381 selected backend tests across 19
no-default-feature targets, including 83 library parents; 25 ignored entries are
inventory helpers. Inputs04 captures 859 Rust/Cargo/vendor/fixture paths, with the
schema fixture checked separately. The isolated default desktop library
passes 164 tests, with eight ignored inventory helpers. Default all-target Clippy
with warnings denied and formatting pass; protected app/Niri binaries remain
unchanged. The final source archive is recorded in the WIP receipt. This is scoped
verification;
full native/workspace/release and physical-device acceptance remain open.

Next: active-execution and queued restart continuation, metadata-save retry owner
restoration, per-child source identity between copied publication and Move source
removal, saved-container claims, complete Preparing/Cleanup/migration binding,
and durable successor lineage for queued recovery. Older intents without original
history proof remain reviewable and cannot mint owners. General Undo/relocation/
expiry still need newer original facts and directories recreated by earlier Undo.
Trash/Restore restart, backend separation, product/device/accessibility checks,
reliability/performance and release integration remain part of the active goal.

## Previous Phase2 checkpoint — empty-source-directory claims, 2026-10-22

[ADR0122](../../docs/files/adr/0122-empty-source-directory-claims.md) and
[RESTART-DIRECTORY-REMOVAL-WIP.md](RESTART-DIRECTORY-REMOVAL-WIP.md) record
schema37 source-folder cleanup claims. Live boundary tests exposed deletion of
replacement folders/parents, and a killed worker exposed false completion before
the removal syscall. Cleanup now uses the earlier source/destination originals,
claims through held directories and removes only a verified empty original.
Ready/Claimed/Returning state persists actual return direction before NOREPLACE
return. Changed or occupied objects remain intact; completed unlink restores
metadata without touching newer source occupants or destination data.

The scoped library passes82 parents,7 ignored. Fifteen actual killed-worker
states use raw folder bytes and two opens each, alongside the original live
races and pre-syscall crash regression. Selected backend376 pass across19
no-default-feature targets; the required unignored restarted Retry/Continue test
remains the sole failure (24 ignored inventory helpers). Inputs04 captures859
Rust/Cargo/vendor/fixture paths. Default all-target Clippy, formatting and schema37
fixture syntax pass; isolated desktop163 pass,8 ignored, on unchanged captured
inputs. The earlier video timeout remains undiagnosed. Protected app/Niri binaries
are unchanged. No native, complete backend/workspace/release or physical-device
acceptance is claimed.

Next: source identity between copied publication and interrupted Move removal,
saved-container claims, complete Preparing/Cleanup/migration binding, unsupported/
legacy safety and typed handoff/abandon lineage. Restore actual execution owners
and pass both Copy/real cross-filesystem Move part orders of the required gate.
General Undo/relocation/expiry must consume newer original facts while accounting
for directories recreated by earlier Undo steps. Keep every goal area active;
descriptive stopped reports alone remain unfinished behavior.

## Previous Phase2 checkpoint — ordinary prepared-copy publication, 2026-10-22

[ADR0121](../../docs/files/adr/0121-ordinary-prepared-copy-publication.md) and
[RESTART-PUBLICATION-WIP.md](RESTART-PUBLICATION-WIP.md) record schema36 ordinary
Copy publication recovery. Actual public-transfer crashes reproduced accepting
a replacement parent into history and deleting newer prepared bytes. Recovery
now verifies the recorded parent/work/payload generations first, finishes
unchanged prepared output with NOREPLACE, and preserves changed or occupied
entries. Completed publication restores only metadata, preserving newer output
and modes. New Copy steps retain their original publication facts before the
syscall; completion carries those facts forward before empty-only cleanup.

Scoped coverage passes34 actual killed-worker cases for Copy and real
cross-filesystem Move, files/raw links and two opens each. Library78 parents pass;
selected backend372 pass across19 no-default-feature targets. The required
unignored restarted Retry/Continue test remains the sole failure (23 ignored
inventory helpers). Inputs04 captures857 Rust/Cargo/vendor/fixture paths.
Default all-target Clippy, formatting and schema36 fixture syntax pass; isolated
desktop159 pass,7 ignored, on unchanged captured inputs. The earlier video
timeout remains undiagnosed. Protected application/Niri binaries are unchanged.
No native, complete backend/workspace/release or physical-device acceptance is
claimed by these checks.

Next: source-removal continuation identity, empty-folder and saved-container
claims, complete Preparing/Cleanup/migration binding, unsupported/legacy safety
and typed handoff/abandon lineage. Then restore actual execution owners and pass
both Copy/real cross-filesystem Move part orders of the required gate. Persisted
selection currently names original request roots; audit per-child source proof
for a copy committed before an interrupted Move removes its source. General
Undo/expiry still need newer original-object facts. All five goal areas remain
active; descriptive stopped reports alone remain unfinished behavior.

## Previous Phase2 checkpoint — forward intent and completed cleanup rights, 2026-10-22

[ADR0120](../../docs/files/adr/0120-forward-intent-and-completed-cleanup-rights.md)
and [RESTART-STEP-WIP.md](RESTART-STEP-WIP.md) record schema35 forward recovery
checks before directory access or claimed-file movement. New intents persist
independent context binding the history, journal lane and complete canonical
intent. Recovery requires that proof, an existing operation and its next
unoccupied history slot. Missing or changed ownership preserves staged data.

A real post-append crash also exposed deletion of completed output moved back
to its former private name. Committing a step now retires all payload deletion
rights from cleanup. Immediate and restarted cleanup receive empty-directory
rights; occupants including the original published object remain in Recovery.

Eight binding parents exercise Copy and Move with16 actual killed workers and
two opens each. Returned completed-output checks cover edited/unmodified files
in both journal lanes. The library passes75 parents; selected backend coverage
passes 369 across19 no-default-feature targets. The required unignored restarted
Retry/Continue test remains the sole failure (22 ignored inventory helpers).
Inputs05 captures855 Rust/Cargo/vendor/fixture paths. Default all-target Clippy,
formatting and schema35 fixture syntax pass. The isolated desktop library passes
156,6 ignored, on unchanged captured inputs. The earlier intermittent video
timeout remains undiagnosed. Native, complete backend/workspace/release and
physical-device acceptance are still open; protected app/Niri binaries are intact.

Next: audit plain Copy's published-object proof; bind Preparing/Cleanup/migration;
finish empty-folder and saved-container claims, unsupported/legacy safety and
typed handoff/abandon lineage. Then restore actual execution owners and pass
both Copy/real cross-filesystem Move part orders of the required restart gate.
General Undo/expiry must still consume the newer original-object facts. Keep
all five goal areas active; restoring descriptions alone remains unfinished.

## Previous Phase2 checkpoint — reviewed replacement Copy/Move claims, 2026-10-22

[ADR0119](../../docs/files/adr/0119-reviewed-replacement-claims.md) and
[RESTART-REPLACEMENT-WIP.md](RESTART-REPLACEMENT-WIP.md) record schema34 reviewed
replacement claims. Files retains the old entry and its parent before conflict
review, backs it up through the actual descriptors, and saves its return direction
before rollback. Copy owns generated-output publication facts; same-filesystem
Move composes the backup with its entry claim. Unclaimed targets are never
returned: a preclaim conflict cleans only proved generated work. Completed
publication restores history metadata while preserving newer bytes.

Scoped replacement coverage exercises66 actual killed-process cases with two
opens each and both original parent races. Inputs02 captures849 Rust/Cargo/vendor
paths. The selected no-default-feature backend run passes 360 parents across19
targets, with the required unignored restarted Retry/Continue gate still the
sole failure (22 ignored inventory helpers). This profile excludes13 optional
local-runtime library tests from the preceding checkpoint. Default all-target
Clippy, formatting and schema34 fixture syntax pass. The isolated desktop library
rerun passes147,6 ignored; its first run had an undiagnosed video timeout, while
the isolated video rerun passed. Keep that media reliability issue open.

Next: pre-mutation operation/ordinal/journal/cleanup binding; saved-container and
empty-folder claims; typed handoff/abandon lineage and actual restored execution
owners. General Undo/expiry still need to consume the new original-object facts.
The required gate is enabled and still fails at Copy/Failed-first because startup
restores descriptions without continuation ownership. Keep both Copy and real
cross-filesystem Move part orders required. The full five-area goal, native and
workspace acceptance, physical-device checks and release integration remain
active. The protected application and Niri binaries are unchanged.

## Previous Phase2 checkpoint — whole-entry same-filesystem Move claims, 2026-10-22

[ADR0118](../../docs/files/adr/0118-whole-entry-move-claims.md) and
[RESTART-ENTRY-MOVE-WIP.md](RESTART-ENTRY-MOVE-WIP.md) record ordinary whole-entry
Moves without replacement or catalogued saved-container movement. They retain
the selected entry and both parents before conflict handling, and journal Ready,
Publishing and Returning phases in schema33. Startup returns unverified/changed
claims without replacement, finishes proved publication, and records already
published output without replaying newer work. Missing or inconsistent claim
roles and object values preserve the journal. Cleanup has empty-stage rights.
Available admitted source/parent and destination generation facts now constrain
live checks, including a replacement parent linking the same selected inode.

Current inputs 02 freezes845 inputs. Selected backend coverage passes 367 parents
across19 targets; the sole failure remains the required unignored restarted
Retry/Continue ownership test (21 ignored inventory helpers). The isolated
desktop library passes142,5 ignored. Formatting and schema33 fixture syntax pass;
default all-target Clippy passes with warnings denied. No application rebuild or native,
full-backend/workspace/release acceptance is claimed. Verification01 and the
decoded-verified source 01 checkpoint retain exact evidence and scope.

Next: destination replacement and saved-container/empty-folder claims; unsupported
and legacy stage safety; complete step/journal/cleanup and handoff/abandon binding;
then actual restored execution owners and both Copy/real cross-filesystem Move
part orders of the required gate. New completed Move/source-removal facts still
need general Undo and relocation consumption. Preserve the full five-area goal,
including other operation families, backend separation, product/device acceptance,
native reliability/performance and release integration. The verified app/Niri
binaries remain protected.

## Previous Phase2 checkpoint — opened cross-filesystem source claims, 2026-10-22

[ADR0117](../../docs/files/adr/0117-opened-source-removal-claims.md) and
[RESTART-SOURCE-CLAIM-WIP.md](RESTART-SOURCE-CLAIM-WIP.md) record ordinary leaf
Move source removal through the actual source/copy descriptors and parents.
Schema32 journals Ready, Claimed and Returning phases. Restart returns unverified
claims without replacement, verifies original source/copy generations and bytes
before finishing removal, and records already-finished unlink without replaying
copied output. Missing return subjects and inspection failures preserve Recovery
state. Completed removal retains original facts; normalized cleanup has only
empty-stage rights. Undo consumption of those new facts remains open.

Inputs03 freezes843 inputs. Selected backend coverage passes 361 parents across19
targets with the required unignored Retry/Continue ownership gate still the sole
failure (20 ignored inventory helpers). The isolated desktop library passes136
with4 ignored helpers; default all-target Clippy, formatting and schema32 fixture
syntax pass. Clippy03 ran out of build space; audited completed test-cache
reclamation allowed Clippy04 to pass. Exact terminal evidence is in
restart-source-claim-verification-01.json; the decoded-verified source 01 archive
preserves this unfinished checkpoint. No application rebuild or native/full
backend/workspace/release acceptance is claimed. The protected app/Niri hashes
remain unchanged.

Next: destination replacement, same-filesystem Move and empty-folder claims;
legacy/unsupported stage safety; complete step/journal and typed handoff/abandon
lineage; then restore actual runtime owners and pass both Copy/real cross-filesystem
Move part orders of the required gate. Restoring descriptions alone is unfinished
behavior. Preserve the full five-area goal, including the other operation families,
backend separation, product/device acceptance, reliability/performance and release.

## Previous Phase2 checkpoint — opened copied leaves/stages and interrupted cleanup, 2026-10-22

[ADR0116](../../docs/files/adr/0116-opened-copy-leaves-and-stages.md) and
[RESTART-LEAF-WIP.md](RESTART-LEAF-WIP.md) track the next implementation work.
Source/link reads and prepared payload writes now use their actual opened
objects. Schema31 records available stage/parent/payload facts before byte writes
and an atomic post-append Cleanup intent. Cleanup refusals keep the journal;
committed output is not replayed. Real Copy/cross-filesystem Move deaths verify
owned partial cleanup and preservation of replacement leaves/stages/parents.
Actual post-append death in both journal lanes preserves newer output/Undo, and
a refused cleanup save rolls back the change and retains its Forward intent.

Selected coverage passes355 parents across18 targets; the sole failure is the
required unignored killed-process Retry/Continue gate, included in the run.
The isolated desktop library passes132, 3 ignored; default all-target Clippy,
formatting and schema31 fixture syntax pass. Source03 has841 unchanged inputs;
it differs from source02 only in a test-helper closure style, with the library
rerun. Verification-01 records exact sources, logs and19 ignored inventory
entries without double-counting subprocess helpers. No new application binary,
complete backend/native/workspace check or release acceptance is claimed.

Actual restored Retry/Continue ownership remains unfinished. Next: source and
replacement claims, complete cleanup/step ownership and typed handoff/abandon
lineage, then both part orders of the required Copy/Move restart gate. The full
five-area goal remains active; the remaining operation families and broader
native/workspace/release acceptance are not replaced by these checks. The
protected schema27 application remains intact. See restart-leaf-verification-01.json
and the decoded-verified source03 archive for this unfinished checkpoint.

## Previous Phase2 checkpoint — opened transfer folders, 2026-10-22

[ADR0114](../../docs/files/adr/0114-opened-transfer-folder-facts.md) and
[RESTART-DIRECTORY-WIP.md](RESTART-DIRECTORY-WIP.md) record descriptor-bound folder
enumeration, merge observations and created-folder publication. Checkpoint v3 /
schema30 persist original source/destination folders and both parents when
available. Publication refusals preserve journals and foreign namespaces;
permission restoration uses the actual created-folder descriptor. Schema29/v2
stopped receipts verify their original JSON fragments before an atomic upgrade
binds the saved current codec, preserving the original end hash and Undo.

[ADR0115](../../docs/files/adr/0115-logical-history-lock-lifetime.md) fixes a
separately reproduced history-reopen race: the last logical owner explicitly
unlocks its shared guard so inherited child descriptors cannot prolong ownership.
The controlled failure reproduced3/3 before the fix;20 controlled passes and3
complete source-retention suite passes verify the correction. This does not
establish a fix for native first-window or activation-focus failures.

Frozen source07 has836 unchanged Cargo/crate/vendor inputs. Selected backend
coverage passes250 parents across14 targets (11 ignored inventory entries;
child helpers are not counted twice). The isolated desktop library passes121,
2 ignored; default all-target Clippy and formatting pass. The required unignored
killed-process Retry/Continue gate still fails in red05 because startup restores
no execution owner. Read-only inspection confirms exact receipts, both parts and
newer completed bytes; different sandbox mount identities grant no authority.
See restart-directory-verification-01.json for precise evidence and scope.

Next: opened child I/O, destination installation and owned stage/cleanup claims,
then exact step/journal validation and typed handoff/abandon lineage. Only that
proof may restore safe execution and pass both Copy/Move part orders. The full
restart matrix, Trash/Restore, Undo, interrupted cleanup and all later goal areas
remain active. No source07 application build, complete native/workspace check or
release acceptance is claimed; the protected schema27 executable remains intact.

## Previous Phase2 checkpoint — recorded stopped Copy/Move, 2026-10-22

[ADR0112](../../docs/files/adr/0112-durable-selected-entry-facts.md),
[ADR0113](../../docs/files/adr/0113-recorded-stopped-transfer-results.md) and
[RESTART-CONTINUATION-WIP.md](RESTART-CONTINUATION-WIP.md) track the required
public restart gate. The original entry/parent/root facts persist in checkpoint
v2 and are sealed to the exact execution start before journalling. Schema29 now
also records the exact engine-returned stopped report and each failed/remaining
part, independently of the application result save. Startup restores that report
through metadata alone. Save failures retain the live result and activity/source
barriers; large reports retain the existing request budgets and atomic rollback.
The current focused regression passes139 parents across8 targets, including
cross-filesystem stopped Move, replacement directories, stale/corrupt/legacy
receipts, metadata-only Retry Save and large-report budget boundaries.

No decoded plan gains execution ownership. The required unignored killed-process
Retry/Continue test still requires original traversal/stage/cleanup and opened-
object mutation proof, plus typed handoff/abandon lineage. It remains a separate
acceptance check. Complete step/journal recovery, the other operation families,
and all later product/release phases remain active. Default all-target
Clippy, formatting and the isolated desktop library pass (115 passed, 2 ignored),
on834 unchanged inputs. The required gate fails again in red03, with its new
stopped receipt and exact part hashes confirmed by read-only inspection. See
restart-stop-verification-01.json. No new native/full-workspace claim.

## Previous Phase2 checkpoint — recorded Copy/Move completion, 2026-10-21

[ADR0111](../../docs/files/adr/0111-recorded-transfer-completion.md) and
[TRANSFER-COMPLETION-WIP.md](TRANSFER-COMPLETION-WIP.md) add schema27 exact typed
successful Copy/Move ends and backend metadata-only startup restoration. A real
killed-process regression first reproduced completed Copy restoring Interrupted.
Current focused coverage preserves newer files/original occupants and Undo across
two restarts; failed end/result saves retain metadata-only Retry and activity
holds. Missing, legacy, stale, corrupt, partial and uncertain work stays reviewable.
Selected backend coverage passes162 parents across8 targets; default desktop
library107/2 ignored, fmt/Clippy/build pass. Six distinct native cases/16 checks
pass across runs02/03 in both appearances on ELF b379372fb3f1 and pinned Niri.
Four final captures inspected. Sandbox-bus refusal and one screenshot timeout
remain recorded; neither is treated as an application reliability fix. This
checkpoint does not claim full-backend, complete-native, workspace or release
acceptance. See transfer-completion-verification.json for exact evidence.

Executable restart continuation remains open: durable original source/device,
traversal, stage and cleanup proof must precede safe remaining-step execution.
Trash/Restore, Undo and interrupted cleanup, then original phases3–6 remain active.

## Previous Phase2 checkpoint — exact ordinary mutation completion, 2026-10-21

[ADR0110](../../docs/files/adr/0110-recorded-mutation-completion.md) and
MUTATION-RECEIPTS-WIP.md track exact typed completion before application result
save, then metadata-only startup reconciliation without repeating filesystem
work. Schema26 reserves typed completion within existing bounds. Actual live
success after cleanup establishes the end; missing/legacy/stale/corrupt work
remains reviewable. Failed end/result saves keep the actual outcome and holds.

Source11 passes complete grouped backend coverage:618 parents/67 standard targets
plus custom device_runtime,40 ignored not all independently run. Constructor-only
startup parameter grouping then passes37 queue/crash parents, 105 Iced (2 ignored),
fmt/Clippy/default build on source16. The full618 result belongs to source11.
ELF26c804b48f55/134894840B is protected by decoded-verified gzip21. Native22/24
passes6 selected cases/24 groups in both appearances on that same ELF and pinned
Niri63e39bae3473;4 final captures inspected. Source23 only corrects the claim
fixture's exact schema expectation; all591 compiled crate/vendor/Cargo inputs
equal source16. Failed and intermediate attempts remain, including the initial
bad quota-test boundary,8-argument Clippy finding and2 native schema assertions.
Only audited regenerable Files test caches were reclaimed. The real red dataset
is protected by exact10-member failed-fixture21, with its original retained.

Verification25/scope-diff-review26 and finalsource27 are followed by external
preservation28/final-audit29; only their successful completion establishes sealing.
Other families' end/step reconciliation, stable original/media/traversal/stage/
cleanup proof, executable restart continuation and the broader matrix remain
Phase2 work. The complete brief and original handoff phases3–6 remain active.

## Previous sealed Phase2 checkpoint — Keep interrupted Rename data, 2026-10-20

[ADR0109](../../docs/files/adr/0109-keep-interrupted-rename-in-recovery.md) adds
backend-owned Keep in Recovery for a freshly reviewed, available ordinary Rename
claim. One transaction transfers its protected payload ownership and retires only
that primary journal; the failed request and other reservations remain separate.
No replay, file deletion/movement or successful Rename/Undo is fabricated. Schema25
persists payload roles. Receipt-only movement is refused; sibling source protection
preserves final entries and dangling links without adopting their referents.

Frozen source23 passes612 backend parents/67 standard targets plus custom
`device_runtime`;40 ignored entries are not all independently executed. Iced105
pass/2 ignored, fmt/Clippy/default build pass. All588 crate/vendor/Cargo inputs
match through native-source33 and final documentation. ELF9cd8f4e26e0d,
134829384 bytes, is protected in decoded-verified gzip30. Native34/35 pass6 cases/
26 groups on that same ELF and pinned Niri63e39bae3473, both appearances. Four
final captures inspected; inventory186 remains broader than this scoped run.

CLAIM-REVIEW-WIP.md retains all failed/partial attempts and exact receipts. Native31
light fails at premature post-copy Browser focus; the fixture now waits for modal
retirement/owner focus, without application/timeout changes or repeated focus IPC.
Two failed core datasets/20 exact members are archived36, originals retained.
Native temporary datasets follow Session cleanup; emitted proof artifacts remain.
Only exact owned regenerable Cargo outputs were reclaimed under locks (14/26).

Verification37, primary scope/diff38 and review39 lead to finalsource40. Successful
external preservation41/final-audit42 establish sealing; this forward reference
alone does not. Earlier300 archives/proofs and unrelated work remain protected.
Phase2 still needs other claim families, exact execution end/step receipts and
result-save reconciliation, stable originals/media/traversal/stages/cleanup and
safe durable continuation. Original handoff phases3–6 remain open. Saved plans
never obtain execution authority from presentation or fresh availability.

## Previous sealed Phase2 checkpoint — opened-parent Rename, 2026-10-20

[ADR0108](../../docs/files/adr/0108-opened-parent-rename-claims.md) carries the
prepared original entry/both-parent facts into execution and claims through opened
parents. Actual leaf, parent and engine-handoff replacement failures are preserved.
Unexpected claims return without overwrite; uncertain work remains in Recovery.
Readonly directory sibling claims preserve permissions and normal Undo. Schema24
prevents older cleanup interpretation; malformed-journal inspection is guarded.

Frozen source45 passes606 backend parents across67 standard targets plus custom
device_runtime;40 ignored entries are not all independently executed.99 Iced tests
pass/2 ignored, fmt/Clippy/default build pass. ELF2ae7fea5867b is protected in a
decoded-verified gzip. Current native56/57 pass6 cases/40 groups in both appearances
on that ELF, with pinned Niri63e39bae3473 and four critical captures inspected.
Inventory184 remains broader than this scope. Six failing private datasets and
all intermediate/failing attempts remain; only20 exactly audited regenerable
Files test outputs were reclaimed after the pass. No source/proof/shared work lost.

ATOMIC-RENAME-WIP.md, atomic-rename-verification.json and primary review/scope/
diff59 carry exact evidence. Finalsource61/external preservation62/final-audit63
establish sealing. This does not close full-product checkboxes: unresolved claim
review completion/owner transfer, exact end/step receipts, durable restart proof,
other mutation families and the full interruption matrix remain Phase2 work.
Original handoff phases3–6 remain. Saved descriptions never recreate execution.

## Earlier Phase2 work — Rename execution boundary, 2026-10-20

[ADR0108](../../docs/files/adr/0108-opened-parent-rename-claims.md) and
ATOMIC-RENAME-WIP.md record actual leaf/parent replacement at the filesystem
mutation boundary. Ordinary Rename now claims through opened parents, verifies
the claimed entry and returns unexpected data without overwrite. Blocked or
interrupted claims stay journalled for Recovery review/Copy. Schema24 protects
this interpretation from earlier cleanup.24 focused library parents pass; broader
backend, desktop and native verification is pending. Unresolved claim completion,
other-family mutation authority, durable restart and remaining phases stay open.

## Previous sealed Phase2 checkpoint — prepared Rename, 2026-10-20

[ADR0106](../../docs/files/adr/0106-prepared-rename-authority.md) moves original
entry, followed-parent, storage and retained-source authority before the Iced
name draft. Shared single-use capabilities refuse replacements/runtime mismatch,
preserve admission refusals, and atomically transfer source references. Original
return and fresh deliberate device selection remain usable; independent owners
stay protected. Actual object/device adoption failures are retained. The public
lifetime regression also found ordinary concurrent admission/journal lock failures;
[ADR0107](../../docs/files/adr/0107-history-write-transaction-ordering.md) acquires
the writer before reading in all19 mutating Store transactions, without replay,
retry loops, schema/budget/timeout increases.

Frozen source26/739 inputs passes591 backend parents across67 standard targets
plus custom device_runtime,39 ignored entries not all independently executed.
Iced84 pass/1 ignored, fmt/Clippy/default build pass. Final ELF240396644fda is
protected/decoded verified; current native37/38 pass6 cases/40 groups, both
appearances and pinned Niri63e39bae3473. Four final captures inspected. Inventory
184. Earlier intermediate-ELF evidence is preserved and counted separately.

PREPARED-RENAME-WIP.md, prepared-rename-verification.json, primary review/scope/
diff40 and finalsource42/external preservation43 carry exact terminal evidence.
Only documented regenerable Cargo outputs were reclaimed; all earlier proofs,
protected binaries/probes, source and unrelated work remain. Full product remains
open: atomic execution, exact end/step receipts, durable original/media/traversal/
stage/cleanup proof, other-family review and actual interruption matrix; then
phases3–6 of the original handoff. No full-product checkbox is closed by this slice.

## Earlier Phase2 checkpoint — 2026-10-20

Queued Rename now retains selected entries/parents and its catalogued source
through Undo expiry. The exact linked Forward Rename may use its own reservation;
independent owners and reverse/expiry protection remain. Failed/restored ordinary
Create Folder/Rename rows expose backend-owned Dismiss request with atomic
journal/revision/save/source-release ordering, preserving actual files, Undo and
execution evidence. Journal parents survive first-step recording failure; final
explicit leases retire closed-engine history ownership before returning.

Source29 passes584 backend parents across67 standard targets plus custom device
harness;39 ignored entries are not all independently executed.84 default Iced
tests pass/1 ignored, with fmt/Clippy/default build. ELF73cc8dfa2c9d is protected
and decoded verified. Native47/49/50/51 pass8 cases/46 groups, both appearances,
pinned Niri63e39bae3473, with583 compiled inputs equal through source46.

Native41's intentional-exit fixture registry was fixed;43/48 interruptions are
preserved.48's diagnostic identifies Codex as SIGTERM sender, trigger unknown;
TTY retries are no proven tooling fix. Only five verified private orphan services
were retired. All249 inherited and15 intermediate source archives were checked;
all failures/partial evidence remain. Decision ADR0105, RETAINED-RENAME-WIP.md,
retained-rename-verification.json and source56/preservation58 carry exact evidence.

Phase2 remains open: prepared Rename authority and atomic mutation, other-family
ownership/review, exact end/step receipts and reconciliation, proven durable
original objects/media/traversal/stages/cleanup and explicit new-authority paths,
and the wider real-interruption matrix. Phases3–6 remain product/release work.
None of the full-product checkboxes is closed by this scoped evidence.

## Requirements and completion evidence

- [ ] Independent native Canopa Files application, shared appearance and
  configuration, installation and desktop integration.
- [ ] Real local directory browsing and removable drives, readable responsive
  columns, list and icon views, stable focus/selection/scroll while resizing.
- [ ] Keyboard navigation and multi-selection, parent restoration, editable
  location, type-to-select, opening files and folders, hidden-file controls.
- [ ] Multiple windows and tabs with preserved independent browsing state.
- [ ] Floating source-bound Quick Look that follows selection and preserves
  browsing input; image zoom, PDF paging, text/code, audio and video controls.
- [ ] Custom actions through context menus, shortcuts and toolbar; graphical
  editor for labels/icons/commands/conditions; actual command execution.
- [ ] Configurable toolbar, bookmarks, column widths/details and shortcuts;
  per-folder view/sort/details overrides and Use defaults.
- [ ] Cancellable current-folder and recursive filename search, result paths,
  Quick Look, reveal in folder, restoration of prior browsing state.
- [ ] Create folders, rename, copy, move, Trash and restore; keyboard and actual
  inter-window/external clipboard and drag-and-drop behavior.
- [ ] File Replace/Keep both/Skip and folder Merge/Keep both/Skip, metadata
  comparison and batch choices; no silent overwrite on a race.
- [ ] Shared Undo for actual completed operations, including replaced-file
  retention, safe conflict handling and partial-batch reversal.
- [ ] Background transfers survive closing every browser; shared Operations
  view, progress, cancellation, isolated errors/retry, unavailable destination
  pause, desktop notifications, source retention for incomplete moves.
- [x] Downloads acceptance workflow on real temporary files under native Niri,
  narrow and wide tiling, keyboard and preview focus, move/Undo/custom action.
- [ ] Workspace checks, meaningful filesystem regressions, code review,
  packaging/configuration/dependency attribution updates and documentation.

## Iced migration status

These are implemented slices and remaining acceptance gates, not completion of
the full requirements above. The supported default is Iced with `canopa-ui`;
the GTK adapter remains migration reference code until replacement is complete.

| Area | Current Iced state | Remaining work |
| --- | --- | --- |
| Browsing | Responsive columns, list/icons, tabs/windows, keyboard navigation and filename search have scoped native coverage. The complete Downloads design-test workflow passes both appearances. | Complete browsing, accessibility and performance acceptance. |
| Actions and configuration | Custom actions, their editor, interface/folder preferences and live configuration are implemented and exercised. Managed actions capture prepared storage and validate worker-side availability, with selected-entry and symlink regressions and native coverage in both appearances. | Complete integration, original selected-source identity and accessibility acceptance. |
| Quick Look | Source-bound floating text/source/image/PDF previews, syntax colors and live palette changes have scoped native coverage. Renderer-independent audio/video playback and Iced controls pass both appearances and common-format native checks. | Complete preview, accessibility, performance and release acceptance. |
| File operations | Create/Rename/Copy/Move, shared Undo, native clipboard, progress and conflict controls are connected. Backend-owned live Retry/Continue/Abandon, queued Rename entry/parent observations and exact request/filesystem execution evidence have scoped backend/Iced verification. | Exact request/journal/restart authority, descriptor-relative mutation, wider continuation/native acceptance and complete question coverage. |
| Trash and Recovery | Backend catalogs, Iced Trash/Restore, Recovery for retained files, directories and interrupted Undo, exact in-memory continuation, windowless questions, shared Undo and source-bound previews have scoped native coverage. | Retry/expiry and long-catalog acceptance, general restart reconciliation, batch/conflict restoration, cross-filesystem Restore and complete acceptance. |
| Devices | Prepared catalog, backend Open/mount/unmount, Power off/Eject, current-root lookup, attention and Iced sidebar/Locations/Operations/floating questions pass scoped private-provider verification in both appearances, including stale-origin and windowless behavior. Device-row drops and the mounted root in a 340px tile pass Move/Copy/Undo and unavailable/replacement refusal checks. | Expanded credential/busy-question/helper-loss acceptance, physical devices and complete acceptance. |
| Desktop exchange | Native clipboard exchange and same-runtime window activation have scoped native coverage. Backend drag capture, prepared destinations and admission feed the shared transfer/Undo queue. Actual Iced gestures pass 18 native checks across dark/100% and light/125%, including scrolling, narrow columns and buffered-Escape refusal. A further 16 native checks cover GTK exchange, quiet foreign cancellation, clipboard coexistence and windowless incoming/outgoing receipts. Twelve additional gestures cover real cross-filesystem choices and icon/search/header/bookmark/ancestor sources or targets, plus two private cleanup checks. Sixteen device-drop checks cover mounted targets and refusals. The complete Downloads workflow includes native Cut/Paste and pointer Move/Undo. | Other delayed/stale geometry and complete retained-offer/session/focus integration remain required. |
| Background runtime | Optional resident/headless startup, primary-instance activation, backend-owned workflow composition and FileManager1 cold activation have scoped coverage. | Remaining platform ownership, activation-token focus, execution/restart authority and production idle/warm performance acceptance (ADR0076). |
| Distribution | Iced is the default build. | Full native matrix, shared-stack/workspace/release checks, packaging acceptance and retirement of the GTK reference adapter. |

Earlier retention work is tracked in [RETAINED-FORWARD-WIP.md](RETAINED-FORWARD-WIP.md)
and [retained-forward-verification.json](retained-forward-verification.json).
Source27 passes442 core tests,51 Iced library tests, all-target Clippy and the
default build. Native44018 passes10 checks across both themes on candidate
c2b9e792; both scoped review axes have no unresolved findings. The full PRD,
source execution and continuation gates remain active.

## Current progress

Live application Retry/Continue and explicit Abandon now use backend-owned exact
plans and original observations, with atomic handoff, truthful historical
dispositions, queued cancellation, save-failure protection and windowless holds.
Cut settlement preserves other parts, skipped roots and newer clipboard intents.
The successor saves its opaque descriptive checkpoint without recreating restart
execution rights. Two new regressions first reproduced transient shared-folder
Cut retention and loss of the exact queued successor checkpoint; both are fixed.

The complete source40 backend run passes564 parent tests/66 standard targets plus
its custom device harness. After the final serialization correction, all87 tests
across nine affected application targets pass on source45; unchanged filesystem
engine inputs are bound to that full run. Final78 default library tests, fmt,
warning-free all-target Clippy and the default build pass. New ELF51f509f7f26f
passes four current native cases/ten grouped checks across both appearances;
the prior80575eab052f candidate passes eight/28. The final cases verify actual
Copy Retry/Continue in opposite orders, zero windows and Quit, external clipboard,
exact targets, original destination return and independent Undo. Move/Cut branching
has backend coverage; broader native acceptance remains open. Primary review only.
See [ADR0102](../../docs/files/adr/0102-live-application-transfer-continuation.md),
[work notes](APPLICATION-CONTINUATION-WIP.md) and
[the complete scoped evidence](application-continuation-verification.json).
Restart execution authority and every later handoff phase remain unfinished.

The following paragraphs retain the preceding scoped milestones and their own
source/executable evidence; their earlier missing-work statements are historical.

Before exposing application Retry/Continue, a new public regression reproduced
both Copy and Move adopting a replacement destination root between executions.
The transfer core now retains the opened original destination through live
continuations, with bounded shared root pins independent of the selected-entry
budget. Initially unavailable roots bind once when they first become available.
Saved application results remain descriptive. The frozen source11 full backend
suite passes 556 parent tests plus its custom device harness; 78 default library
tests, formatting, all-target Clippy and the default build pass. New executable
84168e8d2ee9 passes six native cases/22 grouped checks across both appearances,
including replacement refusal and original-folder return. Primary review has no
unresolved scoped findings. Exact evidence and limits are recorded in
[destination verification](continuation-destination-verification.json); see
[ADR0101](../../docs/files/adr/0101-live-transfer-destination-observation.md) and
[destination work](CONTINUATION-DESTINATION-WIP.md). This prerequisite does not
complete application continuation or restart authority.

Live transfer continuations now retain the original selected-entry observation.
A public regression first reproduced Continue copying an unrelated replacement.
The engine fix preserves Copy/Move source identity through repeated pauses,
failed-item outcomes and original-object return, without GUI checks. Kind and
destination remain tied to captured intent. Recorded directory completion keeps
Retry/Continue idempotent in either order. Review-only stopped results explicitly
release process pins, preserving clipboard replacement capacity. The full private
local-runtime suite passes 552 parent tests plus its custom device harness;
78 default Iced library tests, all-target Clippy and the default build pass.
Eight native cases pass 24 checks across both appearances, including transfer
inspection, parent closure, compact windows and the unchanged descriptor limit.
An outdated keyboard fixture is corrected with before/after and old-application
evidence. The independent first-window timeout remains open. Primary review has
no unresolved scoped findings. This is not complete application or durable
restart authority. See
[ADR0100](../../docs/files/adr/0100-live-continuation-selected-sources.md) and
[continuation work](CONTINUATION-SOURCES-WIP.md), with exact
[verification and limits](continuation-verification.json).

The owning runtime now implements FileManager1 before Iced starts. Typed core
presentations share CLI admission, pending-opening holds and shutdown policy;
the CLI wire remains version 1. ShowFolders opens tabs, ShowItems groups and
reveals requested items in their parents, and ShowItemProperties opens an
independent read-only metadata window. Raw Unix names and explicitly hidden
selections survive narrow presentation. The shipped optional activation file
starts the same `--daemon` owner on demand without changing the live session or
folder MIME defaults. Root ADR0025 and Files ADR0099 record the decisions.

Twenty core parent tests pass, plus the private-bus helper; the later bus-only
rerun also passes headless refusal for every method. The default Iced library
passes 78 tests, all-target Clippy passes with warnings denied, and the default executable
is built and protected. Native17 passes 26 checks across both appearances with
actual cold activation, same-owner CLI, committed 340px reveal, Properties-only
startup/windowless lifetime, actual End scrolling and exact Quit/name release.
Initial fixture failures and the superseded resize-sensitive scroll assertion
are retained and explained. Focused existing workflows pass 68 Files checks and
five shared-transport probe checks. Regression19 was interrupted during its
last light preview; the completed individual reports remain, and that unfinished
case passes separately in Regression20. Primary review has no unresolved scoped
findings. Evidence is recorded in [the verification](file-manager-verification.json).
Complete assembly, platform focus, optimized/hosted release,
original source/restart authority, accessibility and performance remain open.
See [FileManager1 work](FILE-MANAGER-WIP.md).

Native acceptance now selects and preserves its compositor explicitly. The
runner and preflight share `--niri`/`--niri-sha256`; the runner uses one checked
executable copy for every Niri server and IPC command, records its identity and
checks it again at completion. Replacing the original build cannot change a later
scenario. Hosted dispatch requires the same path/hash in preflight and execution,
builds the drag example, and retains the complete inventory with a twelve-hour
job budget. This configures the gate; no hosted or distribution pass is claimed.

Eight CLI/actual-executable contract checks pass. Native07 passes eighteen checks
through an artifact path containing spaces: transport5, three cancellation
controls, headless4 and resident activation6. Default selection of installed
Niri in native08 still fails the unchanged Leave-before-release control, with
the stock executable identity recorded. Both batches preserve and recheck their
exact executable bytes. No application Rust, Cargo or compiler patch changed.
Full shared-stack/workspace/release checks, compatible distribution, original
source/restart authority, accessibility and production performance remain open.
See [selection evidence](niri-selection-verification.json) and root ADR0024.

The pinned Niri companion now corrects Smithay's dead-source grab lifetime.
The exact unmodified upstream build reproduces the GTK-only failure; the patched
companion passes the original five-check transport workflow and three strict
controls requiring receiver Leave while the pointer is held and no later Drop.
Destruction of an older accepted source during a newer held grab preserves the
newer drag and its completion. Actual Files adds twenty-five checks: eight per
appearance for independent GTK exchange, selected clipboard coexistence and
windowless work/questions, plus nine dark gesture checks for ordinary/buffered
Escape, narrow columns and shared Undo. All native runners are terminal.

The canonical six-file Smithay patch, pinned source archives, licences and
integrity/build tooling live under the shared vendor/tooling ownership. Builds
use a disposable separate upstream workspace, preserving every other locked
dependency. The Canopa lockfile, appaf0 and clean probead7 are unchanged. The
candidate and sources are archived. Installed `/usr/bin/niri` is unchanged and
still reproduces the baseline failure. Compatible distribution selection,
physical touch, broader retained-offer/shared-stack acceptance and the full
product remain open. Primary review only; no external message was sent. See
[ADR0024](../../docs/adr/0024-niri-drag-source-lifetime.md) and
[the correction evidence](niri-cancel-verification.json).

The following paragraphs preserve the preceding cancellation diagnosis.

Strict native cancellation now has a smaller reproducer and a compositor
comparison. The current clean transport probe fails twice on Niri 26.04 after
exact-source destruction acknowledgement. Disabling only Iced's ordinary clipboard
connection leaves one receiver data device and still fails. A pure GTK source
and GTK receiver, one data device each and no Iced, also fail twice. Pointer wire
traces prove acknowledgement precedes release and Drop follows it. The same GTK
control passes twice on private Sway 1.12. This isolates a Niri compositor-path
failure, not its source-level cause or fix. The strict gate stays open.

Temporary shared-adapter instrumentation was archived and completely restored,
including the original dependency patch/provenance and clean probe. All 668
baseline inputs were rechecked and published-archive vendor integrity passes.
The original transport regression completes four earlier checks then fails
cancellation; its aggregate is not a pass. Only fixture controls and diagnosis
documentation remain changed. See root ADR0023, [the focused issue](issues/01-niri-source-destruction-cancellation.md)
and [the exact evidence](dnd-cancel-verification.json). Full goal active; primary
review only. Source-level compositor correction and retained-offer coexistence
remain concrete work, alongside the other product requirements.

Mounted-device drag targets now have native coverage in both appearances: eight
checks each cover default Move, Ctrl Copy, shared Undo, separate removal-control
rejection, held device replacement at the same directory inode, a fresh reviewed
replacement drag, unmounted-row refusal without implicit mount, explicit GIO
remount and a mounted-root drop in a 340px tile. A further nine existing light
device UI checks pass, including windowless questions. No production correction
was needed: the existing token/root lookup and backend observations enforce the
intended behavior. All files, provider state and desktop sessions are private;
physical devices remain unverified. The unchanged af0924853110 candidate retains
the build/Clippy/77 library results recorded by the preceding Downloads stage.
Primary review only. See [the device-drop evidence](dnd-devices-verification.json)
and ADR0095. Strict native peer cancellation, broader retained-offer interaction,
additional geometry cases and the remaining full-product requirements stay open.

The complete Iced Downloads design test now passes in dark/100% and light/125%:
fifteen grouped checks per appearance cover narrow/wide columns, source-bound
floating previews, independent tiled browsers, resized multi-selection, native
Cut/Paste, shared Undo, a real pointer Move/Undo, graphical custom-action creation,
an actual thumbnail subprocess and captured output in 360-pixel Operations.
Original bytes, destination contents, native focus/geometry and painted previews
are checked together. This satisfies the explicit Downloads checkbox above; the
other full-product requirements remain open.

The walkthrough exposed a real focus-return defect, reproduced twice: closing a
focused preview activated the other browser rather than its owner. The native
transient parent was correct, but dismissal sent no activation. One presentation
method now requests the live owner's focus before closing the input surface,
only when the preview owns focus. The inactive-close countercase preserves fresh
input in the other browser. ADR0098 records the rule. No backend/vendor/Cargo
change was needed. Existing preview regression adds seventeen checks, making
forty-seven scoped native checks on candidate af0924853110. Default build,
warning-free all-target Clippy and 77 unfiltered library tests pass (one helper
ignored and invoked by its parent). Primary review only. See
[the exact Downloads evidence](downloads-iced-verification.json). Mounted-device
drop targets were still open at this checkpoint and are verified above. Remaining
full-product gates stay active. Earlier
checkpoints below retain their historical acceptance status.

A real held-destination replacement bug is corrected in the backend. A view
refresh used to discard the original prepared hover and capture the replacement,
allowing an unintended Move. PreparedDrag now retains each pathname's first
successful storage/binding/directory observation (bounded, never evicted), checks
it on worker-side re-preparation, and releases unused pins at admission. Fresh
reviewed gestures and ordinary contents arrivals still work. ADR0097 records the
rule and diagnosis; no GUI/vendor/serialized authority change was needed.

The public regression and two native attempts reproduced the unsafe Move before
the correction. Final headless coverage passes fourteen drop, twenty-seven
transfer and twenty-seven retained-source parents; the device test also covers
re-preparation after storage identity changes with an unchanged directory inode.
Five new native checks pass per appearance, and earlier dark controls9 plus light
choices7 pass on the same app85a candidate. The seven includes one private cleanup
check. Build, all-target warning-free Clippy and 77 library tests pass (one helper
ignored and invoked by its parent). Primary review only. See
[the exact stale-drag evidence](dnd-stale-verification.json). Device drop targets,
other delayed/stale geometry, strict native cancellation, broader coexistence
and all remaining full-product gates stay active. The Downloads requirement was
still open at this earlier checkpoint and is now verified above.

Cross-filesystem native choices and additional presentations now pass twelve
actual gestures across dark/100% and light/125%, plus two private cleanup checks:
Copy default/Shift Move on different actual filesystems, icon/header, recursive
search source, sidebar bookmark and ancestor folder, each with shared Undo.
Focused Alt+S had inserted text into the query; the Iced adapter now handles safe
search-local commands before text insertion while preserving query editing.
The full sixteen-check search workflow passes in each appearance, including
50,005 matching files. All-target Clippy with warnings denied, the default build
and 77 unfiltered library tests pass (one helper ignored and invoked by its
parent). No backend/vendor change; primary review only. See ADR0096 and
[the choices evidence](dnd-choices-verification.json). Native13's mixed overall
failure is retained; final gesture evidence combines dark13 and light15. Device
and stale/delayed targets, strict native cancellation, broader coexistence and
all remaining original PRD gates stay active. Resident/headless ownership remains
as recorded in root ADR0022/Files0076, including its same-process crash limit.

Actual external GTK/Files exchange passes 16 native checks across dark/100% and
light/125%, including raw URI rereading, refusal of cancelled hover bytes, text
and owned-file clipboard coexistence, zero-window foreign conflict deferral and
same-owner reactivation/Replace/Undo, plus outgoing Move receipt completion after
every Files window closes. A rendered regression exposed a misleading persistent
Operations error after cancellation; empty native reads now end the gesture
quietly, and before/after operation facts match in both appearances. The default
build and all-target Clippy pass. No vendor change; primary review only. See
[the external evidence](dnd-exchange-verification.json) and ADR0095. The choices
stage above extends this evidence. Device targets, stale/delayed gestures, strict
native peer cancellation, broader coexistence and every remaining original PRD
gate remain active.


Actual Iced drag controls are connected under
[ADR0095](../../docs/files/adr/0095-iced-drag-gestures-and-transfer-lifetime.md).
Eighteen real Files checks pass across both appearances: Move/shared Undo, live
Ctrl-Copy, folder-row priority, raw selected groups, scrolled targets, normal and
buffered Escape, and 360px columns. A stopped-process unwanted Move was reproduced
before correcting logical-Escape ordering in native dispatch. Fresh-layout
geometry collection also closes a cache refresh gap exposed by a fractional-scale
source refusal after Undo. All-target Clippy, the default build and shared
Shell/Notify/layer compilation pass. Primary review only; external
exchange and receipt evidence is extended above, while the complete PRD stays open.
See [the controls evidence](dnd-controls-verification.json). The transport and
backend checkpoint descriptions below are historical.


Shared ordinary-window Iced/Wayland drag transport is implemented under
[ADR0023](../../docs/adr/0023-wayland-drag-transport.md), with a standalone Iced
probe and independent GTK peer. Six pipe tests, all-target Files Clippy, root
formatting, exact published-archive provenance and shared Shell/Notify/adapter
compilation pass. Four native checks cover raw-byte URI exchange, two retained
dropped offers, Copy/Move with held completion, zero-window source completion
and stale-grab refusal. Strict cancellation remains red, independently reproduced
with a GTK source after acknowledged native destruction. The existing text
clipboard's second data device also needs coexistence investigation. Files gestures
were not connected at that checkpoint; ADR0095 now covers scoped integration. Primary review only; the full
PRD and all remaining gates stay active. See
[the transport evidence](dnd-transport-verification.json).

Backend-owned drag capture and drop admission have scoped verification under
[ADR0094](../../docs/files/adr/0094-backend-owned-drag-capture-and-drop-admission.md).
Prepared sources, per-hover cancellation, original storage/browsing bindings,
directory pins, action negotiation and one-shot admission feed the existing
transfer queue and shared Undo. Regressions reproduced and corrected a cancelled
drop waiting behind removal and a Move entering a replacement local destination
folder. Twelve new public tests cover actual files and cross-filesystem work.
Sixty headless parent tests pass; default validation passes 77 library, 12 drop
and 27 transfer tests, all-target Clippy, the build and formatting. Ten native Iced
clipboard regression checks pass on the resulting executable. That backend
checkpoint preceded the controls in ADR0095. The full Downloads workflow, original
source/restart authority, full acceptance and the PRD remain active. Primary
review only; see [the evidence](drop-workflow-verification.json).

Power off/Eject now have actual private GDrive and Iced acceptance under
[ADR0093](../../docs/files/adr/0093-iced-device-controls-and-questions.md).
Twelve headless checks cover capabilities, busy errors, cancellation, unplug,
stale requests, no-force completion and owner EOF. Sixteen UI checks exercise
wide/narrow navigation, completion after all windows close, notifications,
same-owner reopening and explicit Quit in both appearances. Existing device UI
and helper/catalog regressions add 33 checks, totaling 61 across nine selected
scenarios. One outer runner terminated with status 143 after its four new UI cases passed;
unfinished regressions completed in separate terminal-zero runs. Its cause is
unknown. No application Rust/Cargo code changed; the same preserved executable
is used. The native inventory now has 160 cases. Primary review only; physical
devices, full native matrix, workspace/release checks and the full PRD remain
open. See [the evidence](device-drives-verification.json).

Iced device controls have scoped verification under
[ADR0093](../../docs/files/adr/0093-iced-device-controls-and-questions.md). Sidebar
and narrow Locations use the same backend Open interface; explicit removal,
shared Operations and floating credential/choice forms retain backend ownership.
Current-token controls, separate row identity, exact question tokens, redacted
draft messages and backend anonymous normalization preserve that separation.
Mount completion never redirects newer or closed browsing context. Default
validation passes 77 library tests and the public runtime program's nine groups,
the build, all-target Clippy with warnings denied and formatting. Sixty-two
selected native checks pass across both device appearances, actions, helper,
catalog, headless attention and resident lifecycle. A light fixture coordinate
error was corrected using actual logical dimensions. An interrupted outer runner
was followed by complete remaining lifecycle and isolated action runs; no full
matrix pass is claimed. This stage has primary review only. See
[the evidence](iced-device-ui-verification.json). The full PRD remains active.

Device current-root lookup and backend attention now have scoped verification
under [ADR0091](../../docs/files/adr/0091-backend-owned-device-catalog-and-commands.md).
A completed mount is revalidated against its exact prepared mount instance and
Storage epoch; unrelated catalog changes preserve it, while remount and actual
unplug/replug invalidate it even with the old directory present. Navigation is
disabled during preparation and observer teardown. The attention host owns a
separate backend receiver and holds actual delivery responsibility through reply
or visible refusal. It never depends on a frontend or answers a question itself.
Nine headless library tests, 103 default parents and the public runtime program
pass. A private-bus device fixture passes four checks, including a real held
Notify reply refusing shutdown and an explicit credential answer. The default
build, all-target Clippy with warnings denied, formatting and inventories pass;
the same executable passes 25 native helper/catalog/headless/resident checks.
This stage received primary review only. See
[the evidence](device-attention-verification.json). At that backend checkpoint,
Iced controls remained open; their scoped coverage is recorded above. Remaining
full-product gates stay open.

Managed action placement has scoped verification under
[ADR0092](../../docs/files/adr/0092-managed-custom-action-storage.md). The public
Application regression reproduced a real command and output after successful
private-device unmount with its old directory still present. The guard now
captures original storage at submission and checks the working folder, selected
entries and referents after the removal wait. Forty renderer-free integration
parents, 73 default Iced library tests and the public runtime program's eight
check groups pass. Coverage includes both symlink directions, remount without
replay, unrelated local work and observer failure. The default build, all-target
Clippy with warnings denied, formatting and inventories pass. The same executable
passes 55 native checks across custom actions in both appearances, device helper
and catalog, headless residency and same-process reopening. This stage received
primary review only. Selected symlink targets are resolved at worker validation,
not pinned at invocation; original-source, restart authority and full-product
gates remain open. At that checkpoint device UI was still pending. See
[the evidence](action-placement-verification.json).

Backend-owned device catalog work continues under
[ADR0091](../../docs/files/adr/0091-backend-owned-device-catalog-and-commands.md).
The helper now observes local unmounted volumes and mounted roots with opaque
revision tokens and capabilities, publishing only after Storage preparation.
It also listens for volume/drive changes without a mounted root. Verification
passes 68 library parents, four startup tests, all-target Clippy and the headless
build; the private-provider catalog has five native checks. Existing Iced operation
coverage passes 12 checks and initial headless residency passes four. Both scoped
review axes have zero findings. That catalog checkpoint left mount/removal
execution, credential attention and Iced controls to implement. Shared backend
admission now covers filesystem
work and arbitrary custom actions, including queued requests, questions,
cancellation, save failures and actual worker retirement. Scoped validation passes
150 parent tests, Files all-target Clippy with warnings denied, formatting and a
renderer-free local-runtime check. Twenty-five native checks cover unchanged
headless/resident behavior and custom actions. The implementation scope has zero
findings on both independent review axes; final fixture/documentation deltas were
reviewed by the primary agent after follow-up reviewers hit the account usage
limit. That coordination checkpoint did not expose device commands.

The internal helper now dispatches async mount/stop/eject/unmount and owns
generation-bound credential/choice interaction, cancellation and callback
retirement. Eight focused local-runtime tests, the default build, Files all-target
Clippy with warnings denied, formatting and inventories pass. The same executable
passes ten private-provider command checks, five catalog checks and four headless
lifetime checks. At that helper checkpoint, parent guard retention/public state,
Iced controls and provider execution of drive stop/eject remained open. The helper
stage received primary review; independent follow-up reviewers remain unavailable at the account usage
limit. See [ICED-DEVICES-WIP.md](ICED-DEVICES-WIP.md).

DeviceRequests now owns atomic parent admission, prepared response validation,
sequence-bound answers and actual request/helper teardown independently of Iced.
Work survives frontend disposal; cancellation retains the shared guard; helper
loss becomes uncertain after reaping and closes storage authority. Device work
does not enter filesystem Undo. Scoped default regressions pass101 parents plus
the public runtime acceptance program; final headless8 and that program pass after
Clippy normalization, along with the build, all-target Clippy, formatting and
dependency inventory. The same executable passes25 native helper/catalog/host
checks, including resident reopening. Device attention, backend current-root
lookup, Iced controls and stop/eject/physical-device acceptance remain pending.
This parent stage received primary review only; no independent review is claimed.


The media slice implements a renderer-independent playback backend under
[ADR0090](../../docs/files/adr/0090-renderer-independent-media-playback.md).
GStreamer bindings decode the prepared opened-file reader on a private worker,
with bounded input/frames, ordered transport commands and snapshots. Application
ownership holds through actual decoder teardown. Iced owns rendering, focus and
slider drafts. The core local-runtime library checks without Iced or GStreamer.

All 64 library parents and the isolated lazy-init child pass, as do 10 public
preview tests, Files all-target Clippy with warnings denied, workspace formatting,
Python syntax and the dependency-inventory check. A real regression first showed
two rapid mute activations leaving mute enabled. Backend relative/toggle commands
fix it; consecutive mute, volume and seek inputs pass without a frontend refresh.

Candidate03ed1b63 passes four native media cases and 34 grouped checks in dark and
light/1.25×. They verify actual private PCM, volume/mute, paused seek/rewind,
advancing VP8/H.264 pixels, repeated tagged-MP3 preparation, MP3/FLAC/Opus/AAC,
narrow floating controls, independent source windows, replacement/owner closure,
corrupt-media Open and final process cleanup. Error text and Open were inspected
in both appearances. The completed dark case in46 is retained separately from
its later runner SIGTERM; light47 and formats48/49 finish successfully. Both
scoped review axes have no outstanding implementation findings; final checkpoint
review and preservation are recorded with the evidence.

The nested software input diagnostic remains a performance concern, with intact
long-path entry taking19.45s during playback. No production performance result,
full native-matrix pass, release/packaging acceptance or full PRD completion is
claimed. The earlier fc23 fallback remains preserved. Exact attempts and limits
are in [ICED-MEDIA-WIP.md](ICED-MEDIA-WIP.md) and
[the media verification](media-verification.json).


Iced now renders the existing backend's bounded lexical spans under
[ADR0089](../../docs/files/adr/0089-iced-prepared-source-highlighting.md).
Character-to-UTF-8 line conversion stays on the preview worker. The adapter
matches exact editor lines, rejects stale source revisions and restyles cached
content when the shared palette changes. Keywords use bold; strings, numbers
and comments use shared semantic colors with contrast fallback. Existing
read-only behavior and bounded large-text initialization remain intact.

All 56 Iced library tests, default all-target Clippy with warnings denied,
formatting and the default build pass. The same executable passes 17 native
preview checks per theme, including actual colors, exact Unicode clipboard text,
live theme changes, plain-text reset and image/PDF/source ownership. Clipboard
copy follows the native editor's LF separator convention; preparation retains
source endings and copying leaves the file unchanged. The large-text interrupted
Recovery workflow passes five groups in each theme. A combined runner was
terminated after dark passed; a fresh light run passes after exact private process
cleanup. No application change or diagnostic pause is used. See
[the receipt](syntax-verification.json) for sources, attempts and limits.

Sixteen fresh startup sessions on the preceding executable passed the original
readiness deadline, including the exact large-file setup. This reproduced no
startup failure and claims no repair or production startup benchmark. A suspected
missing Refresh label was resolved as a visual misreading by exact pixel equality;
no GUI fix was made. Audio/video, devices, drag-and-drop, Downloads, accessibility,
performance, restart/source authority, workspace/release checks and GTK retirement
remain open. No full PRD requirement checkbox is closed by this checkpoint.


Interrupted Recovery startup attention and actual interrupted Undo reconciliation
have scoped verification under [ADR0088](../../docs/files/adr/0088-interrupted-recovery-startup-attention.md).
The backend answers a lightweight committed-journal/uncertain-report query;
Iced presents Recovery once a Browser exists. Manual dismissal consumes startup
attention even when the query reply arrives later. It never reconciles or
replays work automatically, including legacy journals without request rows.

Twelve focused core parents, 52 Iced controller tests, default all-target Clippy
with warnings denied and the default application build pass. The same executable
passes five native groups in each theme: actual Undo quarantine and restart,
respected dismissal with ordinary mutations blocked, original-name floating
preview of actual bytes, non-consuming salvage, a second restart, 340-pixel
controls, occupied-name refusal, explicit reconciliation and independent Undo.
The large-text case also exposed unbounded editor initialization and quadratic
bulk insertion. Empty-editor layout followed by small complete-line appends
fixes both while preserving text, Unicode, line endings and the existing limit.
The final native fixture has no diagnostic delays or debugger. See
[the receipt](iced-interrupted-verification.json) for exact sources, binary,
failed attempts and verification limits. Separate pre-Browser startup timeouts,
general restart/source/journal authority and the full PRD remain open.

The coalescing Recovery catalog and retained-file Iced workflow have scoped
verification under [ADR0087](../../docs/files/adr/0087-iced-recovery-catalog-and-window.md).
The backend owns bounded pages, deduplication, cancellation and observation;
Iced owns selection, viewport and captured destination drafts. Opaque row
identity survives owner expiry, while observed versions invalidate previews.
Restored ancestors renew watch coverage. A shared 150 ms one-shot coalesces
filesystem events; closing the reader cancels inspection, not admitted work.

Recovery is available from Places, Operations and its configurable command.
Enter/Ctrl+R offers a copy, Space opens the source-bound floating preview, and
Refresh preserves loaded pages. Destination refusal keeps the same draft;
folder-only changes preserve the original filename bytes, and full local file
URIs support raw destinations. Recovery, reconciliation and exact in-memory
Retry/Continue all submit through the existing queue. Shared result inspection
uses the backend's transfer report; the view never rebuilds executable plans.

Validation passes 67 focused core parents across eight targets, 48 unfiltered
Iced library tests and default all-target Clippy with warnings denied. Native
Recovery dark100/light125 passes four check groups each, exercising actual saved
preview bytes and external edits, occupied-output refusal and corrected drafts,
raw folder/full-path destinations, Undo and owner-preview closure. Dark Trash
passes six groups and Operations twelve, including unanswered Undo after the last
Browser closes and loss of its exact private GIO monitor. Captures were inspected.
See [the evidence](iced-recovery-verification.json).

The private debug workload with 270 saved versions improved two-page catalog
startup from about 15.6 s to 0.7 s by avoiding quadratic unrelated-history decode.
Coalescing reduced 20 observed folder operations from 11 s to 2.1 s (about 1 s
without a catalog), with zero idle refreshes. These are scoped diagnostic results,
not production performance acceptance. The initial native Operations run timed
out before its first Browser; a later run passed, without fixing startup. One
runner received SIGTERM after its dark scenario passed; its cause is unknown.
The long-URI driver timeout and changed Operations Tab order were fixture issues
corrected without changing the application executable.

Directory Recovery now passes five native check groups in each theme on the new
`bed94800` executable. Actual child collisions exercise Cancel, replaced-output
root refusal and exact Continue. Closing every window leaves the same request
Running; reopening and Review preserve its durable receipt, and explicit Keep
both completes that request without a successor. Separate Undo entries reverse
the continuation and prefix while preserving independently created output,
retained source bytes and read-only modes. Native 340-pixel geometry and a fully
visible floating preview are checked at both scales. The fixture observes only
private read-only scheduling metadata, never execution payloads.

The 48 Iced library tests, all-target Clippy with warnings denied, formatting and
the default build pass for the three presentation corrections. Existing core
inputs and their source42 evidence are unchanged. Both scoped review axes are
clear. One repeated light attempt raced Browser focus against question retirement;
the fixture now waits for actual retirement. A preview resize wait also prevents
an early fractional-scale capture. The same application then passes both cases;
neither fixture correction is an application focus or startup fix. See
[directory evidence](recovery-directory-verification.json).

Reconciliation, retry failures, expiry and long-catalog Recovery acceptance remain,
alongside restart/source execution authority, devices, media, DnD, Downloads,
accessibility, production performance, full workspace/release checks and GTK
retirement. The full PRD and all requirement checkboxes remain open.

Previous backend checkpoint:

Recovery copy-out, exact in-memory continuation and explicit interrupted-journal
review now use OperationJobs under
[ADR0086](../../docs/files/adr/0086-backend-owned-recovery.md).
The shared engine remains the only history writer. Bounded cancellable pages
stay readable during work and questions. The queue captures retained-source
ownership and original storage observations before acknowledging admission.
Retry/Continue hands off an opaque plan and its source references atomically;
saved reports do not reconstruct executable work after reopening. Recovery
questions reject Replace, and ordinary occupied/stale-source refusals allow
unrelated work. Initial cancellation preserves the unstarted selection.

Explicit reconciliation compares the captured journals and temporary-access
records before retrying them. Copy-out never replays the primary journal.
Successful review preserves historical partial results and only acknowledges
the uncertain reports captured by that request. Result-save failures retain the
report and hold later work until Retry save succeeds.

Final validation passes 116 focused core parents, 47 unfiltered Iced library
tests, warning-free default all-target Clippy and formatting. Real private-file
fixtures cover interrupted Undo salvage, newer occupied names, partial directory
output, queued and running cancellation, Retry/Continue ownership, raw path bytes,
save/finalization errors, conflicts and reopening without replay. Both scoped
review axes are clear. At that backend-only checkpoint the binary was still the preserved Trash
candidate and native Iced Recovery had not been exercised. The catalog and
retained-file frontend progress above supersedes that limitation; safe restart
execution, full workspace/release and the entire PRD remain open. See
[the evidence](recovery-runtime-verification.json).

The Iced Trash workflow is connected under
[ADR0085](../../docs/files/adr/0085-iced-trash-catalog-and-window.md).
A backend catalog owns discovery, scope, raw-name sorting, warnings and filesystem
observation, including new volume stores. Its coalescing reader is usable without
Iced. The view owns selection, bounded row construction, scroll and destination
drafts. Browser Delete submits to the existing queue; Trash Restore captures the
actual item and original raw basename. Restore never silently overwrites an
occupied name. Orphans remain copyable through the native clipboard. Quick Look
reads the stored path, follows only its Trash owner and refreshes prepared content
when the backend payload version changes.

Final validation passes 67 focused core parents, three catalog parents without a
display, 47 unfiltered Iced library tests, all-target Clippy (three existing
warning groups), formatting and fixture checks. Binary `6565ad76` passes 34 native
checks: six Trash checks in each theme, 12 operation checks and ten clipboard
checks. Actual bytes, raw metadata, Undo, edited preview text, occupied/chosen/raw
Restore destinations, orphan Paste and external GIO updates are verified. The
long list remains keyboard-accessible in a narrow tile with its own floating
preview. A title-only dialog-close wait exposed a focus race; the fixture now
waits for native retirement and owner focus, and the adapter orders focus before
close only while the dialog is focused. Both scoped review axes are clear.

One clipboard attempt timed out before its first Browser appeared. The unchanged
fresh attempt passes; the independent startup issue remains unresolved. Recovery,
broader Restore and execution/restart authority, full workspace/release and the
entire PRD remain open. See [the evidence](trash-ui-verification.json). This slice
does not complete a full requirement checkbox.

Trash and Restore now use OperationJobs under
[ADR0084](../../docs/files/adr/0084-backend-owned-trash-and-restore.md).
The existing engine remains the only filesystem implementation/history writer.
Trash captures process-local selected-entry pins before durable acceptance,
retains source dependencies while queued, and publishes typed completed, failed
and unstarted paths. A queued replacement is preserved; original read failures
do not become permission to operate on files that later appear. Individual
failures leave other items runnable. Cancellation preserves actual completed
work and shared Undo, including when cancelled after an observed payload move.
Stopped Trash and Restore results contain no executable request and reopen without replay.
Restore carries the selected item's metadata/identity, refuses occupied names,
supports a chosen destination and records reversible actual completion.
Its typed result retains confirmed completion through a finalization error,
reports recovered commits as successful, and distinguishes a known pre-intent
refusal from an uncertain journal. A known refusal leaves later work runnable.

Twelve new Trash/Restore tests, a concurrent snapshot regression and existing
queue/attention/source-retention/Trash regressions pass 66 parent tests, including
the existing private killed-worker fixtures. Actual fault injection exposed and
now verifies recovered completion, finalization-result retention and dispatch
cancellation. Interrupted results grant no new mutation authority. The attention
stress test also exposed 101 Finished rows before trimming; completion, attention
and the bounded rows now publish atomically. That backend checkpoint did not
rebuild the native executable; its checks do not substitute for the Iced evidence
above. Broader source/journal/restart authority and safe Retry/Continue remain
required. See [the queue evidence](trash-queue-verification.json).
This scoped backend work does not complete the Trash/Recovery requirement or
any full PRD, workspace, native or release gate.

Optional resident hosting and same-runtime activation are implemented under
[root ADR0022](../../docs/adr/0022-files-resident-host-and-activation.md) and
[Files ADR0076](../../docs/files/adr/0076-window-independent-and-resident-runtime.md).
Application composes configuration, volume/operation and custom-action owners
before Iced. `--daemon` starts without a Browser and waits without GUI initialization;
later launches open windows on those same handles. `--quit` releases residency
while preserving windows, pending decisions, accepted work, clipboard ownership
and unsaved-result holds. A start without display variables refuses browser
requests without killing its backend. Later GUI use needs the graphical session's
environment at startup. GUI and backend still share the owning process.

One state-directory lock precedes workers/history; a bounded versioned private
socket carries launch intent with original Unix path bytes. Unspecified
configuration inherits the owner, and conflicting explicit selections are refused.
Opaque opening receipts retain the hold through frontend handoff; exit admission
and pending openings share a lock. Independent socketpair wakeup and bounded
connection establishment fix real endpoint-replacement and saturated-backlog
regressions. Orderly shutdown also awaits history teardown, final observation
updates and the exact GIO helper. The paused-helper native regression failed
before that wait was added and now passes.

Scoped verification passes six activation tests with three private subprocess
fixtures, including idle-owner crash/stale-endpoint recovery; this grants no
filesystem execution continuation authority. The local-runtime library builds
without Iced. Default validation passes 47 unfiltered Iced tests, all-target
Clippy (three existing warning groups), formatting and the normal build.
Binary `8acb44cc` passes 49 native checks across headless hosting, graphical
residency/reopening, operation workflows in both themes and light-theme actions.
The same PID retains raw-file Rename/Undo and an unanswered decision plus queued
work through every window closing, then exits after explicit review and settled
holds. The review capture was inspected. Both code-review axes report zero
unresolved findings.

The debug fixture measured its first opening at 5.37 seconds and warm openings
at 1.02–1.21 seconds. Short owner/helper observations recorded 60–61 MiB summed
RSS before GUI and 190 MiB after window closure, initial CPU around 1% of one
core and no post-window CPU ticks.
These fixture measurements include shared pages/provider overhead and do not
complete release performance acceptance. A Niri startup timeout happened before
Files launched; an unchanged retry passed. The independent Files first-window
timeout remains open. See [resident verification](resident-verification.json).
Remaining full PRD/session/workspace/release gates stay open; no entire requirement
checkbox was completed by this slice.

Filesystem attention is extracted under
[ADR0083](../../docs/files/adr/0083-backend-owned-filesystem-attention.md).
OperationJobs retains semantic attention independently of trimmed rows and
coalesced snapshots. Its unique observer keeps opaque receipts protected through
explicit delivery acknowledgement, so direct queue shutdown cannot bypass the
hold. LocalOperations starts the renderer-independent OperationAttention host
before Iced; Iced retains refresh and typed question presentation.
The initial take-and-release handoff failed a stronger shutdown regression and
was replaced with explicit opaque acknowledgement. The inbox retains the latest
unread state per operation, bounds admission at 1024 outstanding identities and
hands off at most 17 receipts concurrently. Acknowledging notification delivery
never releases protection for an unsaved filesystem result.

Scoped backend checks pass 81 parent tests plus isolated subprocess checks:
four attention tests, the display-free completion/decision-refusal workflow and
existing queue/startup/storage/transfer/retained-source/clipboard tests. Real
private-bus acceptance passes eight checks across filesystem and action attention,
with no display or Iced artifacts. A held Notify reply protects direct queue
shutdown; an unanswered Undo preserves later file bytes and blocks queued work
until its exact answer. All four operation notices then settle before shutdown.

Default validation passes 47 unfiltered Iced library tests, all-target Clippy
(three existing warning groups), formatting and the normal build. The initial
library filter selected zero tests and is excluded from acceptance. Binary88a116cd
passes 39 graphical checks: operations in both themes and light-theme actions.
Operation logs are empty; the action log has only four expected unavailable
remote-monitor messages on the isolated bus. Representative window-closure,
question and running-command captures were inspected. Both scoped review axes
report zero findings; final source12 changes documentation only. See
[verification](operation-attention-verification.json).
Resident launch, activation, remaining platform ownership, execution/continuation
authority, the independent startup timeout and the entire PRD/workspace/release
gates remain unfinished.

Custom-action startup and completion attention are extracted under
[ADR0082](../../docs/files/adr/0082-backend-owned-action-attention.md).
LocalActions starts before Iced, owns receipt delivery and its lifetime hold,
and forwards bounded notices to the frontend. The background-actions feature
builds without Iced. Its narrow commands retain completion ownership; the shared
attention lock and serialized shutdown prevent a lifetime gap during handoff.
Transport failure is observable, with at most 17 delivery futures and bounded
error history. The unchanged command queue retains its existing execution limits.

Source07 passes 4 existing action queue tests and 1 new workflow test without
Iced. Maintained private-bus acceptance passes 4 checks without a display or GUI:
20 actual commands finish, 17 notification replies are held and then delivered,
and 3 busy failures remain visible. The receiver verifies the delivered titles
and all actual output bytes; shutdown refuses while replies are held and succeeds
after release. The build has only the background-actions Files feature and no
Iced artifacts. Default checks pass 47 Iced library tests, all-target Clippy
(3 existing warning groups), formatting and the normal build.

Binary ab3caff0 passes 42 native checks across actions in both themes and
light-theme operations, including editing, raw arguments/output, process-group
cancellation and command lifetime after the last Browser closes. The operation
log is empty; each action log has only 4 expected unavailable remote-monitor
messages on the isolated bus. Representative narrow output, background-work and
result captures were inspected. Both scoped review axes report 0 findings.
Final source08 changes documentation only. See
[the scoped evidence](action-host-verification.json). This does not implement a
resident CLI, activation or filesystem attention extraction; the separate startup
timeout and full PRD/workspace/release gates remain open.

Volume and operation startup are extracted under
[ADR0081](../../docs/files/adr/0081-backend-owned-operation-startup.md).
LocalOperations owns the platform monitor and operation queue before Iced
initialization. The queue waits for the first prepared Storage before opening
history or processing work; Iced attaches to the existing handles and observes
snapshots. Startup failure, disconnect and original-media loss remain observable.
Final-owner drop releases a waiting worker and checks shutdown again after the
readiness handoff, before opening history.

The unchanged core passes 76 parent tests plus 2 isolated helpers across startup,
operations, storage, transfers and clipboard ownership. Clean source20 passes the
47 Iced library tests, all-target Clippy (3 existing warning groups), formatting
and both 16-file reviews with zero remaining findings. Clean binary91a298fd passes
5 native workflows/43 checks: original operations in both themes, 16 rapid
new-window URI Rename/shared-Undo cycles, and native text Copy with Location
rebound to Ctrl+C. All 5 application logs are empty. The input transition and
native-editing fixes belong to root ADR0020; temporary diagnostics are removed.
See [the scoped evidence](operation-startup-verification.json).

The separate intermittent first-window timeout remains unresolved; passing
readiness probes do not establish its cause or a reliability fix. Resident
startup, primary-instance activation, remaining host ownership/notifications,
execution and continuation authority, full workspace/native/release acceptance
and the full product brief remain active. No daemon interface is implemented or
advertised by this extraction.

Configuration coordination is extracted under
[ADR0080](../../docs/files/adr/0080-core-owned-configuration-coordination.md).
ConfigurationJobs owns ordered Save/SaveSettings/Reload, external polling,
bounded nonconsuming publications and pending work independently of Iced. Its
owner starts before Iced; frontend/reply/notice detachment does not cancel
accepted saves while that owner remains. Iced retains drafts, focus and rendered
error treatment. The three prior Iced worker tests now use the public core seam.

Scoped review found coalesced rejection hiding a valid candidate in presentation
and foreign bases redirecting the original source. Complete publications now carry
the current candidate and error together; the adapter tracks applied-candidate
and publication revisions separately. Requests bind to the original selection
before reading/writing. Public regressions cover both; foreign Reload97915
reproduced actual redirection before the fix. Final focused60509 passes 6
coordination tests plus 5 existing reload tests.

Source10/binary35198d84 passes 461 parent core tests plus
2 isolated descriptor helpers (40272), 47 Iced library tests
(99884), all-target Clippy (87882), formatting and the normal build (74302).
Standards and Spec reviews each report 0 unresolved scoped findings. Native95796
passes light actions and both full preference workflows; its first dark launch
timed out before any window. Unmodified dark repeat56136 passes. Together the
four workflows pass 46 checks, including actual saves, raw bookmark navigation,
stale-source rejection, dirty-draft preservation and command lifetime. The initial
startup timeout is retained with no established cause; no oracle/timeout changed.
Each application log has only four expected unavailable GIO remote-monitor
messages on the isolated bus. Narrow draft/error and running-command captures
were visually inspected. All 553 inputs, source archives and the binary gzip
are hash-verified; source11 changes documentation only. See
[verification](configuration-runtime-verification.json). Resident startup,
primary-instance activation and the entire PRD remain unfinished.

Owned clipboard replacement is verified under
[ADR0079](../../docs/files/adr/0079-owned-clipboard-replacement-capacity.md).
Native13697 reproduced a full32-file Copy refusing a deliberate one-file Copy at
a private128-descriptor limit. Core replacement now retains the original opaque
intent while admitting bounded overlap; ordinary capture limits remain intact.
The public parent/helper regression94083 covers full-to-full replacement, old
identity protection, failed partial cleanup, normal-budget restoration, foreign
runtime refusal and actual Paste/Undo.

Source04/binary1ea23cab passes455 parent core tests plus2 isolated
helpers (13990),50 Iced tests (78543), all-target Clippy (23992), formatting and
a normal build (70455). Both scoped reviews report0 findings. Native65632 passes
26 checks: two capacity workflows and four stale-source Copy/Cut workflows across
dark100/light125; all six application logs are empty. Scoped visual inspection
confirms successful replacement and narrow light125 Undo. All550 source hashes,
source archives and the binary gzip are verified. Source05 changes documentation
only. See [verification](clipboard-capacity-verification.json). Resident/headless
host ownership and activation remain unfinished, and the entire PRD stays active.

The custom-action queue is extracted under
[ADR0078](../../docs/files/adr/0078-core-owned-custom-action-queue.md). ActionJobs
owns admission, four-worker scheduling, cancellation and compact completion
handoff independently of Iced. The adapter submits requests and renders snapshots;
output editing, focus and native notification delivery remain presentation/host
work. Runtime39000 passes4 public tests, including1024-slot refusal/reuse, queued
and running cancellation,100-row expiry, observer-free queue advancement and
working-folder retarget refusal. Existing action-domain tests pass5.

Production source07/binary6b46a987 passes454 parent core tests plus one isolated
descriptor helper (87573),50 Iced tests (56583), all-target Clippy (91764),
formatting (35961) and a normal default build (51420). Both scoped review axes
report0 unresolved findings. Native47605 passes30 checks in dark100/light125 on
fixture09. The earlier native62086 failed because a stale menu index selected
Copy; fixture09 corrects that observed navigation without weakening actual
filesystem/output/cancellation/lifetime assertions. Each passing application log
contains four expected missing GIO remote-monitor messages on the private bus.
Scoped visual inspection covers focused action invocation, live commands without
a Browser and narrow paged output. Source10 is documentation only;547 frozen
inputs, source archives and the binary gzip are preserved/hash-verified. See
[action-runtime verification](action-runtime-verification.json). Resident hosting,
activation and the entire product brief remain active.

The clipboard workflow in [ADR0077](../../docs/files/adr/0077-core-owned-prepared-clipboard.md)
is verified for this scope. Native baseline66042 reproduced stale Copy copying
unrelated replacement bytes and stale Cut consuming them. PreparedSelection now
carries original source observations and Storage through Paste; the core owns
Cut claims and durable partial completion. Public regression49204 exposed an
obsolete retained-data capture after physical catalog migration; admission now
inherits the original live lease's current reservations atomically.

Candidate f3d9874b passes450 parent core tests plus an isolated descriptor helper
(56606),51 Iced library tests (34392), default all-target Clippy (38843), formatting
and a normal Iced build (83388). Both scoped review axes report0 unresolved
findings. Native74038 passes20 checks across Copy/Move in dark100 and light125;
existing clipboard/source-lease native73303 passes30 checks across both themes.
All eight application logs are empty. The source-refusal, fresh-Undo, background
question and retained-data lifetime captures have scoped visual inspection.
Source15 and the binary are preserved/hash-verified; source16 changes only the
reviewed native notification oracle. See [verification](clipboard-source-verification.json)
and [current work](CLIPBOARD-SOURCE-WIP.md). This does not complete resident
hosting, execution authority or the full product brief.

The user's architecture review identified application coordination coupled to
the historical GTK adapter. [ADR0074](../../docs/files/adr/0074-renderer-independent-application-runtime.md)
records the direction: reusable domain modules plus renderer-independent workflow
coordination, with Iced rendering state and handling interaction. `OperationJobs`
is the existing partial extraction. Further migration must consolidate workflows
behind core interfaces instead of copying GTK scheduling and decision policy.
This documentation change does not claim the extraction is implemented.

The core captures opaque selected-source observations before admission and
rechecks them during execution. The queued replacement regression and private
descriptor-budget regression pass without added Iced policy. A full-suite
remainder regression was fixed by separating execution identity from final
original-placement inspection. Source05 passes447 core summary checks (446
parent tests plus one isolated descriptor helper),51 Iced library tests, default
all-target Clippy, formatting and a normal Iced binary build. Both scoped review
axes report0 unresolved findings. Candidate d2f3a1a8 and source05 are preserved
and hash-verified; no new native acceptance is claimed for this slice. See
[current execution work](QUEUED-SOURCE-WIP.md) and
[verification](queued-source-verification.json).

The user's daemon/headless proposal is recorded in
[ADR0076](../../docs/files/adr/0076-window-independent-and-resident-runtime.md).
It adds optional idle residency and a runtime host independent of Iced. Process
layout and activation transport remain implementation decisions; no daemon mode
is currently advertised or accepted as implemented.

ADR0073 now protects held catalogued payloads against unrelated consuming
mutations, including nested aliases, replacement writes, source removal, empty
folder Undo and enclosing-container migration. Whole-container Rename preserves
catalog cleanup metadata and refuses unsupported names before mutation. The
shared predicate orders journal admission and source acquisition together.

Candidate c2b9e792 (118802464B) builds from frozen source27 with Iced and canopa-ui,
with no GTK widget links. All442 core tests (39112),51 Iced library tests (34611),
all-target Clippy (33728), formatting and the default build (23395) pass. Three
pre-existing unused helpers remain. Native44018 passes10 declared checks in both
themes: actionable Rename refusal,100 actual history operations, exact-byte Paste
and cleanup after clipboard replacement. Both application logs are empty; dark
expiry and light release captures were inspected. Both scoped review axes have0
unresolved findings. The earlier bf3 baseline2108 reproduced actual Rename loss.

The binary gzip and source27 archive are preserved and hash-verified. Final
source28 changes only decision/context/fixture documentation. See
[verification](retained-forward-verification.json),
[current work](RETAINED-FORWARD-WIP.md) and
[ADR0073](../../docs/files/adr/0073-retained-payload-mutation-ordering.md).
The next investigation is [queued source execution](NEXT-EXECUTION-NOTES.md),
including replacement of a selected source while its Move waits behind another
question. Safe Retry/Continue, descriptor/media/restart authority and every
remaining migration/acceptance gate in the table stay active. No full product
requirement checkbox is completed.

Previous process-owned source-lease checkpoint follows.

ADR0072 now has verified process-owned retained-source leases and Iced
clipboard integration. Copy/Cut prepares a lease on the metadata actor before
publishing; owned reads and queued Paste retain it through durable admission.
Opaque clones protect catalogued bytes without granting execution authority.
Async final Drop avoids UI metadata I/O; explicit release acknowledges its final
deletion, including concurrent clones. Failed deletion preserves protection and
is retried before acquisition. Schema22 removes dead-process leases under the
exclusive history lock while preserving accepted request reservations.

Candidate bf3f9f2f,118780768B, builds from source09 (533 inputs) with Iced and
canopa-ui and no GTK widget links. All430 core tests (25668),51 Iced library tests
(41704) and all-target Clippy -D clippy::all (28336) pass. Three existing unused
helpers remain. Native34302 passes28 declared checks across4 dark/light cases:
retained Copy survives100 actual history operations, pastes exact old bytes and
releases expired data after supersession; ordinary clipboard Copy/Cut/Paste,
conflicts, Undo and Browser closure also pass. All4 application logs are empty;
dark expiry and light cleanup captures were inspected. Baseline37554 on exact
5906ee19 genuinely lost the copied version after100 operations. Earlier67612 was
only a capitalization mistake in the new fixture oracle, corrected before the
real baseline reproduction.

Both review axes have0 unresolved scoped findings. Review fixed concurrent
Arc::try_unwrap acknowledgement with Arc::into_inner, replaced private-basename
lookup with unique payload discovery and made cleanup allow bounded public trim
opportunities after asynchronous release. Formatting, vendor integrity,67 script
ASTs,115-case matrix parsing and whitespace checks pass. Final source10 adds only
decision/fixture documentation; implementation ledger and verification record
retain exact handles and preservation evidence. See
[verification](iced-source-leases-verification.json),
[current work](ICED-SOURCE-LEASES-WIP.md) and
[ADR0072](../../docs/files/adr/0072-process-owned-retained-source-leases.md).
Safe Retry/Continue and source/journal/descriptor/original-media/restart
authority, further typed questions, DnD, activation, Iced Trash/Recovery/devices/
media/Downloads, accessibility/performance/shared-stack/workspace/release and
final review/commit remain active. Native retained-data acceptance covers Copy;
this does not complete consuming-source execution. No product checkbox is done.

Previous mixed-conflict checkpoint follows.

ADR0070 now separates remembered batch choices for folder pairs, file/link
pairs and mixed or unsupported kinds. Native39069 on the unchanged e3bf5c3e
baseline reproduced file Replace turning two mixed collisions into failures
without asking. Public service red9232 independently reported2 completed and2
failed. The engine now asks a fresh safe mixed question and keeps its choice
independent from later ordinary file Replace. No Iced presentation or public
request format changed.

Candidate5906ee19 passes44 focused transfer engine/service tests (32577), plus36
declared native checks across8 cases in both maintained themes. Mixed Copy and
Move (13119) verify both type orientations, metadata at320×240, independent
Keep both batching, later file Replace, actual bytes/kinds/identities and Undo.
Ordinary Move and Copy conflicts (58141) verify nested Keep both, Cancel/Skip,
three two-folder Merge batches and shared Undo. Move confirms skipped leaves stay
at source, completed source roots are cleaned up and moved leaf identities return
through Undo. Existing destination-folder identities and unrelated files stay
intact. All8 application logs are empty; mixed and Move captures were inspected.

Build5995/source03,118355928 bytes, is preserved with no GTK widget links.
Source04 extends only the native Move fixture/CLI/matrix; source05 records the
final decision and fixture documentation. All530 final inputs are verified.
All-target Clippy -D clippy::all (82250), formatting, vendor integrity,38 Python
ASTs and113-case matrix parsing pass. Three existing unused-helper warnings
remain. Both independent review axes have no unresolved scoped findings.
The prior420-test core and51-test Iced library proof belongs to e3bf5c3e; those
full suites are not claimed as rerun on this batch-only engine change.
See [verification](iced-mixed-verification.json),
[current work](ICED-MIXED-CONFLICTS-WIP.md) and
[ADR0070](../../docs/files/adr/0070-iced-transfer-questions-and-cut-completion.md).
Further typed-question acceptance, retained-source leases, safe Retry/Continue,
source/journal/descriptor/original-media/restart authority, real DnD, activation,
Iced Trash/Recovery/devices/media/Downloads, full accessibility/performance/shared-
stack/workspace/release and final review/commit remain active. No product checkbox
is completed.

Previous parent-focus and live Retry checkpoint follows. ADR0070 now has verified parent-focus and live destination Retry progress. An
explicit answer focuses the surviving captured Browser or Operations parent
before retiring its question. Native92387 first reproduced focus returning to
the wrong Browser. Native80810 then reproduced Retry copying remaining work into
an unrelated replacement destination; public service red31008 independently
reproduced it for the established transfer interface. Retry now retains the
original observed destination identity through missing and replacement folders.
Initially unobserved destinations retain their first reviewed-folder behavior.

Candidate e3bf5c3e uses Iced and `canopa-ui`, with no linked GTK widget libraries.
It passes all 420 core tests, all 51 Iced library tests and 42 declared native
checks across eight cases in both maintained themes. Native9931 verifies missing
and replacement-folder refusal, original-folder restoration and actual shared
Undo, three-window parent focus, common folder/file batch conflicts and the
existing name/Undo/Operations workflows. All eight application logs are empty;
question and minimum-size captures were inspected. The public regression covers
both Copy and Move; native unavailable acceptance uses Copy and ordinary private
directory relocation, not real device removal.

Build80086, full core45094, library12727 and native9931 use source06. Clippy90287
found two style issues in the new test; source07 changes those assertions only.
All 19 transfer-service tests pass again (88576; included in the 420 total), and
all-target Clippy -D clippy::all passes (62930). Three existing unused-helper
warnings remain under the full workspace gate. Final source08 changes decision
and fixture documentation only. All 529 final inputs and the exact executable
are preserved and verified. Both review axes have no unresolved scoped findings.
See [verification](iced-question-focus-verification.json),
[current work](ICED-QUESTION-FOCUS-WIP.md) and
[ADR0070](../../docs/files/adr/0070-iced-transfer-questions-and-cut-completion.md).
Remaining mixed-type and Move-specific native questions, retained-source leases,
safe Retry/Continue and source/journal/descriptor/original-media/restart authority,
real DnD, activation, Trash/Recovery/devices/media/Downloads, full accessibility/
performance/shared-stack/workspace/release and final review/commit remain active.
No product checkboxes are marked complete.

Previous common-conflict checkpoint follows. ADR0070 now has scoped native acceptance for common folder Copy conflicts and
batch choices in Iced. Transfer and Undo facts share the result inspector's six
scrolling keys, without moving action focus. Reserved scrollbar space and glyph
wrapping keep paths/dates readable at 320×240 above a narrow Browser. Native72184
first reproduced the unreachable metadata; the implementation fixes that actual
failure rather than changing the test expectation.

Both themes verify nested Keep both and Undo, folder Cancel/Skip, and three
two-folder Merge batches with separately applied file Keep both/Replace/Skip.
Actual payloads, original folder identities, source bytes, attention counts and
Undo restoration/removal are checked. A fixture initially treated a legitimate
retained directory as a copied file; its corrected payload oracle makes no
retained-container ownership or cleanup claim. No production change was made
for that fixture mistake.

Candidate58b1a80f passes 48 declared native checks across six cases. Runner24325
contains the independently passed 12-check operations-dark case; its overall1
is due only to the invalid earlier conflicts oracle. Final runner21371 exits0
with both transfer and conflict themes plus operations-light,36 checks. All six
application logs are empty, and compact metadata/batch-choice captures were
inspected. All 51 Iced library tests (98334), all-target Clippy -D clippy::all
(53233), formatting, vendor integrity,103-case matrix parsing, AST and whitespace
checks pass. Three existing unused-helper warnings remain under the full
workspace gate. Both independent review axes have no unresolved scoped findings.

Production matches build84230/source04; source05 changes only the native payload
oracle, and final source06 updates decision/fixture documentation only. All 528
final inputs and the exact executable are preserved and verified. The prior
419-test core proof is reused against 108 unchanged inputs, not claimed as rerun.
See [verification](iced-conflicts-verification.json),
[current work](ICED-CONFLICTS-WIP.md) and
[ADR0070](../../docs/files/adr/0070-iced-transfer-questions-and-cut-completion.md).
Remaining typed questions, source leases, safe Retry/Continue, native DnD,
activation, Trash/Recovery/devices/media/Downloads, full accessibility/performance/
shared-stack/workspace/release and final review/commit remain active. No product
checkboxes are marked complete.

Prior ADR0071 inspector checkpoint: Iced Operations shows the current path, confirmed
item count, bytes and verification stage, with an explicit floating job-bound
inspector. Final native66279 passes six scenarios/eighteen declared checks across
dark/100% and light/125%. The full workflows include a real partial copy beside
360px Operations, cancellation and recorded categories, Undo of only the confirmed
prefix, 25 new copies with the original inspector still bound, keyboard scrolling
and twenty-entry pages, section reset, and explicit older/newer job selection in
one native inspector with page reset.

Separate parent-close workflows verify normal Close→Closed with an inspector
and unanswered question open. Both children retire; reopening Operations reviews
the retained question before explicit cancellation and confirmed-prefix Undo.
Compact workflows reproduce and fix clipped facts at 320×240 and section focus
loss across the responsive breakpoint. The stable widget tree, compact margins
and reserved section scrollbar space keep facts scrollable, controls visible and
section activation/keyboard Close stable through shrink→expand→shrink. Both-theme
captures were inspected. All 51 Iced library tests (90867), all-target Clippy with
-D clippy::all (18439), formatting and vendor integrity pass; three existing
unused-helper warnings remain under the full workspace gate.

The shared Winit focus implementation passes normal and overdue-reply rejection
in both themes. Native16145 reproduced the reviewed deadline race with a1.2second
delay; the handler now checks the same deadline used by its timer. Both independent
review axes have no unresolved scoped findings. Final transfer native47958 exits0
on source06; earlier startup/capture failures remain recorded separately.

At that inspector checkpoint, production and test inputs match iced-inspector-source-05, with final
decision/fixture documentation changes only in source06. Both independent review
axes have no unresolved scoped findings. See
[verification](iced-inspector-verification.json),
[current work](ICED-INSPECTOR-LIFETIME-WIP.md), and
[focus diagnosis](ICED-PROGRESS-FOCUS-DIAGNOSIS.md). Remaining native typed transfer
questions, Clipboard retained-source leases, safe Retry/Continue and source/
journal/descriptor/media ownership, activation, real DnD, Trash/Recovery/devices/
media/Downloads, full accessibility/performance/shared-stack/workspace/release
and final review/commit gates remain active. No product checkboxes are completed.

Iced Copy/Cut/Paste now use the shared transfer service with captured source and
destination placement. Typed floating transfer questions support actual Keep
both/Replace and shared Undo above a340px Browser. Known Move results update only
the original Cut token; overlapping partial results cannot reintroduce consumed
files. Engine/service reports distinguish original source folder remainders from
unrelated new folders at the same pathname. Background questions notify once per
generation without taking focus.

Candidate fc469284 passes10 actual clipboard checks in each maintained theme and
11 existing dark name/Undo/Operations checks. Six independent public clipboard
protocol parents pass, including monotonic Cut remainders. All419 core tests and
51 Iced controller/library tests pass. Clippy found a single unnecessary clone in
a new test assertion; after that test-only correction, all18 transfer-service
tests and all-target desktop Clippy pass. Formatting,571-package inventory,
matrix/AST and whitespace checks pass. Both independent review axes have no
unresolved scoped findings after fixing the missing attention notification.

Final source04 binds518 unchanged inputs; production matches the source03 native
build and full suites. The executable and source inputs are preserved in verified
archives. A combined native runner was interrupted after its two passing clipboard
cases; its private processes were cleaned by guarded pidfds, and the third case
passed when rerun alone. See [verification](iced-clipboard-verification.json),
[checkpoint](ICED-CLIPBOARD-WIP.md) and
[ADR0070](../../docs/files/adr/0070-iced-transfer-questions-and-cut-completion.md).

Operations progress/result inspection, remaining native transfer questions,
Clipboard source leases, activation, real DnD, ownership/restart authority and the
rest of the PRD remain open. No product checkboxes are marked complete.

ADR0068 adds durable reservations for known retained sources to the shared Iced
transfer service. Expiry and consuming Undo preserve those sources; finish,
dismissal, handoff and physical-stage migration update protection atomically.
Seven new retention tests and a stopped-transfer service regression cover actual
private bytes, restart, quota refusal, aliases and live Undo review.

The core suite passes 413 tests before the review refactor; final focused checks
pass 50 tests, Iced controller/library checks pass 51, and all-target Clippy and
formatting pass. Both review axes have zero unresolved scoped findings. The
default build passes, with resolved libraries and no GTK dependency. All 503
recorded inputs remained unchanged. See [verification](retained-source-verification.json),
[the checkpoint](RETAINED-SOURCE-WIP.md) and
[decisions](../../docs/files/adr/0068-durable-retained-source-ownership.md).
Full source/journal/media/descriptor ownership and restored execution remain
unfinished, along with the entire remaining PRD and Next work list.

The prior shared Iced transfer foundation under ADR0067 provides:
typed questions, coalesced progress, independent original source/destination
placement, actual stopped reports and opaque exact continuations. Fatal errors
preserve confirmed outcomes and require reconciliation; known cancellation or
rejection before execution stays distinct. Malformed records retain their saved
bytes and files. Snapshot reads share immutable reports and paths.

Candidate `5996a732…` binds 500 unchanged inputs and passes 405 core tests plus
51 Iced controller/library tests. The 16 public transfer tests are included in
that core count; 22 private child helpers run through their parent tests. Two
sequential native operation regressions pass 22 checks in dark/100% and
light/125%, with empty application logs and eight inspected captures. All-target
Clippy lints, normal build, formatting, whitespace, the 571-package inventory and
matrix parsing pass. Both review axes report zero unresolved findings.

See [verification](iced-transfer-verification.json),
[checkpoint](ICED-TRANSFER-WIP.md) and
[ADR0067](../../docs/files/adr/0067-iced-transfer-service.md).
Iced transfer invocation and controls, durable continuation/source ownership/
journal gates and native transfer acceptance remain unfinished. Earlier
executable proofs below are historical; this checkpoint verifies their existing
name/Undo/Operations interface on the current binary, not full transfer UI.

The Iced operation worker is connected to floating New Folder/Rename drafts,
typed Undo questions and shared Operations. Production shares live GIO volume
observations with admission/placement through a private helper, without GTK
widgets. Names retain raw paths and captured source bindings until durable
acceptance. Unanswered Undo and queued work survive source closure; Operations
can reopen the question. Simulated unplug and exact helper loss refuse queued
work while preserving actual files.

Candidate `4f62a086…` binds 496 unchanged inputs and passes 50 controller/library
tests plus 12 public worker tests. Five native workflows pass 52 declared checks:
11 operation checks in each theme, 15 custom-action checks, 13 browsing/tiling
checks and two focused Preferences checks. Both operation logs are empty; three
regression logs contain only isolated-bus GVfs service notices. Ten selected
captures were inspected. All-target Clippy lints, formatting, whitespace,
571-package attribution inventory, matrix parsing and packaged documentation
checks pass. The three existing unused-helper warning groups remain.

Two-axis follow-up review confirms fixes for navigation cancellation, alias
refresh, failed-start tracking, overlapping notification holds and pidfd ordering.
No scoped findings remain. This does not establish full operation/device/restart
or release acceptance. See [evidence](iced-operations-ui-verification.json),
[checkpoint](ICED-NAME-UI-WIP.md) and
[ADR0066](../../docs/files/adr/0066-iced-name-dialogs-and-volume-observation.md).

The preceding renderer-independent worker checkpoint provided durable Create Folder,
Rename and shared Undo admission, serial execution, generation-bound questions,
cancellation, result-save Retry, coalesced notices and safe idle shutdown. Saved
Queued/Running rows reopen for review without replay; completed results drop
executable requests and placement snapshots. That checkpoint preceded Iced name
dialogs, filesystem Operations rows and live volume observation; exact journal/
source ownership remains required before full operation acceptance.

On 487 unchanged inputs, 51 focused public tests, desktop all-target Clippy lints,
normal Iced build, workspace formatting, tracked whitespace and the 558-package
dependency inventory pass. The executable is `99c423636c7c3bd7…`. Two independent
review axes report no unresolved findings after two P3 maintainability fixes.
Three existing transfer/recovery helper warning groups remain unsuppressed. No
new native behavior is claimed for this unwired service. See
[worker evidence](iced-operations-worker-verification.json),
[checkpoint](ICED-OPERATIONS-WIP.md) and
[ADR0065](../../docs/files/adr/0065-iced-filesystem-operation-interfaces.md).

The supported GUI uses Iced and `canopa-ui`, without GTK widgets. The global
Preferences foundation has a transactional Views/Toolbar/Locations/Shortcuts
draft, stale-safe saves, recording and raw bookmarks. The icon grid now embeds
its scrollbar and uses matching layout/navigation geometry, retaining the entire
rightmost focus border while resizing. That defect has a native failing-before/
passing-after probe and scoped five-workflow/53-check verification, including a
twenty-cycle reflow stress case. See [gutter evidence](iced-gutter-verification.json).

Desktop, Documents, Downloads and Pictures now resolve from the user's XDG
definitions through one byte-preserving startup snapshot. A bounded shared reader
runs on a worker before the first Browser opens; the sidebar and narrow Locations
menu consume the same paths. Missing/invalid definitions keep conventional Home
subfolders. Configured missing destinations remain explicit navigation errors.
The snapshot refreshes on the next application launch. No new dependency was
added. [ADR0064](../../docs/files/adr/0064-xdg-personal-folder-locations.md) records
the decisions. An exact-byte regression also reproduces and fixes loss of the
trailing separator in an explicit `$HOME/` mapping.

The preceding Locations executable `8b949f9d…` binds 485 unchanged inputs. All 47 controller and
20 public tests, Files all-target Clippy lints, build, formatting and tracked
whitespace pass. The final separator follow-up passes both native themes with
16 actual sidebar/menu launch-target checks, two empty application logs and four
selected reviewed captures. The preceding `48ddfc36…` candidate passed eight
Locations/Preferences/browsing/views workflows with 106 declared checks and ten
empty application logs. These are staged scopes, not a combined single-candidate
matrix. See [final evidence](iced-locations-verification.json),
[preceding matrix](iced-locations-verification-before-separator-fix.json) and
[checkpoint](ICED-LOCATIONS-WIP.md).

One early fixture setup inherited host HOME and created three test files and two
folders there. Birth-time/content/identity audit established all five were new;
only those files and empty folders were removed. No preexisting regular file was
involved. The driver now sets private HOME before setup callbacks, and the fixture
asserts every generated root is private. All five paths remain absent after the
corrected final runs. The [cleanup record](iced-locations-host-fixture-cleanup.json)
preserves the evidence.

Full preference focus/accessibility/source-authority/large-catalog acceptance,
remaining Iced operations/devices/media/file-exchange interfaces, durable native
hydration/source ownership/exact filesystem continuations, and full shared-stack/
workspace/performance/release/review gates remain required. The Locations
checkpoint had four unused operation-helper warning groups; the worker now uses
the Undo placement reader, leaving three unsuppressed. Every product checkbox stays
unchecked and the full goal remains active. Never replay arbitrary custom commands.

### Prior verified folder-view checkpoint

The Iced folder-view port has scoped verification under ADR0062. Columns/List/Icons now
use effective sparse folder settings, with shared layout/navigation geometry,
virtualized icon rows, metadata and a keyboard-accessible View options popup.
Folder changes publish to matching tabs/windows and save through one admitted
worker with one coalesced pending snapshot. Failed writes retain choices and
offer Retry. Empty Search preserves the underlying folder scroll; a real native
82 px displacement and an independent controller regression were reproduced and
fixed before final acceptance.

On 479 unchanged inputs, 37 adapter tests, 17 focused public tests, all-target
Clippy lints, the build, workspace formatting and tracked whitespace pass. Twelve
final sequential native runs pass 182 declared checks in dark at 100% and light
at 125% on executable
`c3dda02002a383de2263e9fb00d9341e827797bd509e048a006c43af65eec131`.
All 14 application logs are empty; 16 selected final captures were inspected,
not the exhaustive visual/accessibility matrix. The supported default dependency
graph uses Iced and `canopa-ui` without GTK widgets. Four existing operation-helper
warning groups remain. The global preferences editor, remaining Iced interfaces,
durability and all full PRD/workspace/release/review gates remain required. Every
completion checkbox stays unchecked. See
[scoped evidence](iced-views-verification.json),
[checkpoint](ICED-VIEWS-WIP.md) and
[ADR0062](../../docs/files/adr/0062-iced-folder-views-and-preferences.md).

### Prior verified interface checkpoint

The Iced toolbar now retains the actual location input's focus, selection and
caret while adapting between wide and narrow tiles. A native red reproduced
focus loss and unintended folder navigation; the fixed layout preserves one
input child and changes geometry. Editor source-change/error notices also keep
the editable form in a stable slot, preserving unsaved text and native selection
while the writer refuses stale saves.

Optional custom-action icons now use shared bounded XDG lookup/raster preparation
and the workspace's resvg on an admitted worker, with one replaceable pending
catalog. Views clone prepared handles. Superseded results cannot replace current
appearance/catalog pixels. Native checks cover actual raster toolbar/menu pixels,
live SVG symbolic foregrounds, and an executable text fallback for missing icons.
The supported GUI depends on Iced/canopa-ui and has no GTK widget dependency.

On 475 frozen inputs, 33 adapter tests, all-target Clippy lints, the desktop build,
workspace formatting and tracked whitespace pass. Ten final sequential native
runs pass 136 declared checks in dark at 100% and light at 125% scaling on executable
581524070e2666d51a50e7234578ef906335cc797f876f823ce0284b7259baf4.
All application logs are empty; 14 selected final captures were inspected, not
the exhaustive visual/accessibility matrix. Four existing operation-helper
warning groups remain. The full PRD and every completion checkbox remain open;
the remaining Iced interfaces, source-volume/durable runtime and full workspace,
release and review gates are still required. See
[scoped evidence](iced-interface-verification.json),
[checkpoint](ICED-INTERFACE-WIP.md) and
[ADR0061](../../docs/files/adr/0061-iced-editing-state-and-action-icons.md).

### Previous Iced checkpoints

The Iced custom-action foundation now includes a graphical draft editor, guarded
context/toolbar/shortcut invocation, configured toolbar order with narrow command
strips, serialized stale-safe saves and live configuration publication. Command
jobs use shared Operations, bounded paged/read-only/copyable output, process-group
cancellation, completion notifications and independent Browser lifetime. Buttons,
checkboxes and native fields traverse synchronously so batched Tab/Ctrl+A cannot
lose an edit; the discard prompt defaults safely to Keep editing. Menu opening
preserves the selected row, and active search/location inputs suppress toolbar
tooltips. Decisions and diagnoses are recorded in ADR0060 and the checkpoint.

On472 frozen inputs,29 adapter tests,17 focused public action/config/preferences
tests, all-target Clippy lints, the build and workspace formatting pass. Actions
16/17 plus browsing05/06, search05/06 and previews05/06 pass118 declared native
checks in dark100/light125 on executable eeee87ee2e31a898fb5a8999ab57f6e4615e96a990cebafda26801a6f44f9828. Application logs are empty;12 selected final captures were
inspected, not the exhaustive visual/accessibility matrix. Theme icons, the full
action/focus/reload/source-authority matrix, durable runtime, other Iced interfaces
and full workspace/release/review gates remain open. Four existing operation-helper
warning groups remain. The entire product checklist remains unchecked. See
[scoped evidence](iced-actions-verification.json),
[checkpoint](ICED-ACTIONS-WIP.md) and
[ADR0060](../../docs/files/adr/0060-iced-custom-actions.md).


Iced floating Quick Look now follows its originating browser's active tab and
selection, including filename-search results, with independent text/image/PDF
windows. A shared typed Wayland parent attribute sets the relationship before
initial configuration, preserving an independent 760×635 outer preview beside
a 360px tile. Read-only text selection/copy, source keyboard navigation, focus
return, image zoom/Fit, PDF pages, other-window isolation and owner-close behavior
pass native acceptance. Fit outside manual zoom bounds cannot reverse zoom
direction; a deterministic red/green regression covers tiny and large images.
Nineteen adapter tests, ten public preview tests, all-target Clippy lints, build
and formatting pass on 458 frozen inputs. Final native20/21 previews,22/23
browsing and24/25 search pass88 checks in dark/100% and light/125%; all60 captures
were inspected and application logs are clean. Exact shared-fork published
archives, attribution and reviewable patches verify. Workspace all-target
compilation and four fork library tests pass. Strict workspace Clippy still
rejects the four known unused-operation-helper warning groups; no warnings were
suppressed. The complete core entry point passes outside the sandbox with serial
test execution, including all Files tests. Earlier parallel private PipeWire
startup timeouts remain recorded; their cause is not established by the serial
success. See [core proof](iced-preview-core-verification.json),
[scoped proof](iced-preview-verification.json),
[native proof](iced-preview-native-verification.json),
[ADR0059](../../docs/files/adr/0059-iced-source-bound-previews.md),
[shared decision](../../docs/adr/0019-iced-transient-toplevels.md) and
[checkpoint](ICED-PREVIEW-WIP.md). Media playback, syntax colors, full focus and
accessibility/source-authority acceptance, and the remaining Iced interfaces
are still open. The complete Quick Look and product requirements stay unchecked.
Final executable: f1bbba93361f2dd95a080cc7000ffccd439e022e61e6620898b0b5a2a3f4a087.

The Iced filename-search interface is verified on 134 frozen inputs. Ctrl+F and
the visible Search button open tab-owned current-folder search, with explicit
subfolders, cancellation, containing-location labels, Show in folder, inspectable
read failures and additional result budgets. Closing search restores folder
selection and scroll; stale worker/control/list events cannot replace another
query or tab. Fourteen adapter and 22 focused public tests pass, together with
all-target Clippy lints, the default build and formatting. Native runs 10/11 for search
and 12/13 for browsing pass 58 checks in dark at 100% and light at 125%; all 38 final
captures were inspected. The case with 50,005 actual files proves the initial 50,000
limit, Stop preserving prior results and More reaching the last file. Screenshot
review found and fixed location text painting over Search, now guarded by a native
pixel assertion. Binary: b71a4a178dc9cd51e827f86fe83b03fff3d18edcb856fc104633e5b84182a190.
See [search proof](iced-search-verification.json),
[ADR0058](../../docs/files/adr/0058-iced-filename-search.md) and
[checkpoint](ICED-SEARCH-WIP.md). Search-to-preview integration now has the scoped
Iced evidence above; the complete PRD search requirement is not marked done. The entire product goal,
remaining GUI port, durability and final acceptance gates stay active.

The user corrected the GUI direction:Canopa Files must use the workspace's Iced
stack and `canopa-ui` for consistency with Shell and Notify. [ADR0018](../../docs/adr/0018-files-iced-and-shared-controls.md)
supersedes the GTK choice. The supported desktop source now points to an initial
Iced adapter and no longer depends on GTK. The initial rebuilt executable passed
seven adapter tests, nine core browsing/search regressions, Clippy lints and
formatting. Scoped real Niri browsing passed in dark at 100% and light at 125%, including
pixel assertions for visible selection/scroll at 360px and two visible browser
tiles. Screenshot review caught and repaired an initial scroll reset and an
offscreen explicitly selected file. See [browsing migration proof](iced-migration-browsing-verification.json).
Directory loading now delivers the requested folder independently, reads only
visible ancestors after compositor sizing, and cancels reads on navigation,
column removal or tab hiding. Only nearby row widgets are built. Native watches
prove a hidden parent with 5,000 files stays unread until widening; the selected
file remains usable. A widget-state panic found in that native run was reproduced
with a deterministic tree reconciliation test and fixed with path-keyed row
state. Both themes pass the complete browsing/large-ancestor scenario. See
[loading verification](iced-loading-verification.json) and
[ADR 0057](../../docs/files/adr/0057-iced-directory-loading.md).
Five operation helpers still report four dead-code warnings until their Iced
adapter is ported; full warning-free workspace validation remains required. Migration
and equivalent full native acceptance are required, preserving the entire
product brief. Earlier completed GUI checks below are historical GTK evidence,
including preferences and the Downloads walkthrough; their Iced gates are open.
Existing core code and filesystem tests remain reusable. Address
Wayland drag-and-drop through the shared stack/platform integration rather than
dropping the requirement or retaining a second GUI toolkit.

The durable registry now has atomic Retry/Continue metadata handoff:consume only
the selected plan and create its Queued successor in one transaction. A public
filesystem test reopens between Retry and Continue, preserves completed inode,
and verifies separate Undo; quota refusal preserves both original plans.
23 focused core tests pass, followed by settled handoff tests, core all-target
Clippy and formatting on112 inputs. See [handoff proof](job-handoff-verification.json).
This does not yet transfer source leases or connect native scheduling. The last
desktop reference is superseded by the verified Iced browsing binary; the
durability and complete GUI porting gates stay open. See
[Iced migration checkpoint](ICED-MIGRATION-WIP.md).

Durable unfinished operations are in progress; see
[ADR0056](../../docs/files/adr/0056-durable-unfinished-operations.md),
[diagnosis evidence](job-lifetime-diagnosis.json) and
[checkpoint](DURABLE-JOBS-WIP.md). Native149/151 independently reproduce clean
process exit followed by empty Operations and missing Retry. Same-process
control150 retries the repaired child successfully and preserves completed work.
The core now has a versioned, raw-path-safe representation of exact continuations.
Six focused tests pass: raw names/modes/separate Undo, source/destination identity
guards, format rejection, chosen Keep Both mapping, and actual cross-filesystem
saved-stage ownership/Recovery context through decode and reopen. Foundation
validation passes349 core tests,21 helper definitions, Clippy, desktop build and
formatting on105 frozen inputs. Portable task payloads subsequently pass three
new tests, reopened Recovery continuation and default all-target Clippy. Original
storage requirements now survive serialization independently of current media;
five focused tests pass. The schema20 core scheduling registry is implemented;
24 focused registry/checkpoint/storage/history-upgrade checks pass, including
request persistence during a live conflict and process exit without destructors.
The full core passes 368 tests with21 ignored child-fixture definitions across39
targets; all-target Clippy, desktop build and formatting also pass on111 frozen
inputs. A first build ran out of disk space; regenerable Files test outputs and
the failed compiler's working cache were removed before a successful unchanged
rebuild. See [foundation proof](job-registry-foundation-verification.json).
The application queue
still does not save or reload these plans;
source leases and filesystem-step crash reconciliation remain open. The
user-visible restart bug is not fixed yet. This is
progress toward the entire goal, not a reduced completion claim.

Application-owned Copy/Cut now retains the source media and browsing binding
from selection through Paste and Retry; destinations are captured independently.
Two native reds reproduced moving replacement media's bytes at the same path.
The fix passes final native141-146: 76 checks and 41 inspected captures, including
actual cross-filesystem Move/Retry/Undo in both themes, source alias retargeting,
existing destination safety, mixed-source isolation, directory Retry with newer
clipboard selections and Downloads. Clippy/default build/format and static/docs
checks pass on 103 frozen inputs. The 345-test core result is reused only for
unchanged core/test/build inputs; no new core run is claimed. See
[ADR0055](../../docs/files/adr/0055-clipboard-source-placement.md),
[proof](clipboard-placement-verification.json) and
[checkpoint](CLIPBOARD-PLACEMENT-WIP.md).

The [two-axis review](CODE-REVIEW-2026-10-17.md) records the source-placement
finding and the remaining natural-exit loss of unfinished requests, ignored
shared UI control styles and an optional acceptance-CLI smell. Durable jobs,
ordinary saved-source leases, broader descriptor/media authority, disposal,
Trash/Restore and full workspace/distribution acceptance remain open. Current
goal continuation is progress; the whole product brief stays active.

The PRD Downloads walkthrough now passes native128 dark and129 light1.25 on the
schema19 candidate:44 final checks, all18 captures inspected. It combines default
columns at360 pixels, source-bound floating text/image/code previews, a second
640-pixel destination, selection through resize, actual Cut/Paste, shared Undo
and destination keyboard context, then a graphically configured image action
that creates a real32x24 thumbnail with exact selection/cwd. Source bytes and
existing destination work survive; Operations is readable at360. See
[proof](downloads-verification.json) and [checkpoint](DOWNLOADS-WORKFLOW-WIP.md).
Initial127 remains recorded separately with its earlier21-check fixture. This
closes the explicit first design test, not the broader product requirements.

Retained-replacement cleanup is verified for the recorded scope;
see [ADR0054](../../docs/files/adr/0054-retained-replacement-cleanup.md) and
[proof](retained-cleanup-verification.json), with the diagnostic history in the
[checkpoint](RETAINED-CLEANUP-WIP.md). Public tests reproduce expiry unlinking
saved data but failing to remove its empty stage under a read-only copied parent.
Cleanup now records its intent before unlink, preserves captured physical parent
authority through same/cross-filesystem movement and Undo, and keeps existing
Merge parents under their own permission policy. Prepared restart states recheck
write/search before deletion. The settled schema19 candidate passes 41 focused
permission/expiry tests, 345 core tests with 21 helper definitions, all-target
Clippy, the default desktop build and formatting. Its 103 compiled inputs remain
unchanged through final native128-135: 162 checks and 80 inspected captures in
dark and light1.25. Those runs include the separately recorded Downloads proof,
Recovery tiling, legacy recorded-location inputs and ordinary cross-device Retry.
The full product goal remains active.

Copied-folder permissions are verified for the recorded scope; see
[ADR0053](../../docs/files/adr/0053-folder-permissions.md),
[proof](directory-permissions-verification.json) and
[checkpoint](DIRECTORY-PERMISSIONS-WIP.md). Copies preserve normal Unix modes,
including private/read-only roots and nested folders, through Continue, Recovery
and separate Undo. Durable temporary access restores modes after process death
and retains blocked reconciliation authority. Existing Merge folders retain
their own modes. Root traversal uses the created-stage or approved Merge identity,
refusing foreign replacements; a postinstall refusal retains the exact creation
owner and counts the completed root once. Initial unreadable sources make no
empty output or Undo. Actual public regressions cover umasks022/077, modes700/
555/500/750, raw names, real cross-device Move, external chmod/replacement,
100-entry history expiry and before/after/relocated final-mode recovery.

Settled focused22/1 helper, core326/21 helpers, all-target Clippy/default build/fmt
pass on102 frozen compiled inputs. Binary26cd7347… passes native123-126:90
checks across dark and light1.25 Recovery/Retry, with all46 captures inspected
and clean GTK logs. Private secondary filesystems are removed after shutdown.
Static inventory/fixture syntax/whitespace and packaged docs24/39 links pass.
The earlier322/21 full-core result remains explicitly intermediate before
root-epoch changes. Current goal continuation is **progress**; no blocking
condition or completion claim. Readonly-stage cleanup, source/request lifetimes,
full descriptor/media authority, durable unfinished jobs, disposal, Trash/Restore,
Downloads integration, FileManager1, release/workspace validation and review
remain active parts of the entire PRD.

Operations now fits narrow tiles with long filenames and simultaneous actions;
see [ADR0052](../../docs/files/adr/0052-operations-narrow-tiles.md) and
[proof](operations-tiling-verification.json). Native feedback isolated the
987→456→360-pixel minimum: character wrapping fixes long words, and flowing
buttons fixes the remaining action-row minimum. Keyboard behavior stays stable;
Alt+V reviews deferred decisions. Recovery identifies folders truthfully, and
selected error text uses readable shared colors. Native 116/117 each pass 20
layout/Recovery checks before the final selection-color rule; final 119/122 each
pass 22 Retry/Undo/clipboard checks on the final binary. All 46 successful-workflow
captures were inspected; 120/121 SIGTERM interruptions remain recorded failures.
Final Clippy/build/fmt and static/documentation checks pass. The prior 304 core
result is reused only for 81 byte-identical non-desktop/test/build inputs; no new
core run is claimed for these 3 desktop-only changes. Source/request lifetime,
directory metadata, descriptor authority, durable jobs, integration, release and
the full PRD remain active. Current checkpoint:
[OPERATIONS-TILING-WIP.md](OPERATIONS-TILING-WIP.md).

Recovery continuation ownership is verified for its recorded scope; see
[ADR0051](../../docs/files/adr/0051-recovery-continuation-ownership.md) and
[proof](recovery-continuation-verification.json). Temporary output refusal keeps
Continue; selected protected bytes resolve through indexed current owners after
active-record expiry and known containing-folder Rename. Operations retains
unfinished work alongside100 finished rows and keeps stable retry titles.
Final core304/20 helpers, Clippy/build/fmt and native110-113 (70 checks) pass on
one frozen candidate, with all38 captures inspected and clean GTK logs.
Root-install phases, source/request lifetimes, directory metadata, persistence,
disposal and the entire broader PRD remain active. Current checkpoint:
[RECOVERY-CONTINUATION-WIP.md](RECOVERY-CONTINUATION-WIP.md).

Directory Recovery copy-out now has native progress, cancellation, typed
Continue/Retry and separate Undo, with no Replace of newer output. See
[ADR0050](../../docs/files/adr/0050-directory-recovery-copies.md) and
[its checkpoint](DIRECTORY-RECOVERY-WIP.md). Final core checks pass:300 tests/20 helpers;
Clippy/build/format pass on the same final source, native105 dark and106
light1.25 pass13 checks each. Existing ordinary directory Cut/Retry107 passes
20 functional checks plus private-volume cleanup (47 native checks overall).
All27 captures were inspected and GTK logs are clean. Rust/build100 hashes
remain unchanged; precise proof is in recovery-copyout-verification.json. The whole PRD and broader recovery,
original-media authority, durable jobs, integration, release and review stay open.

- Workspace and product requirements inspected; no existing file-manager
  implementation was present.
- Name, ownership, path handling, keyboard defaults, state model and public
  test seams recorded in `docs/files/adr/0001-application-and-state.md`.
- Real Directory/Browser implementation now covers immutable filesystem
  snapshots, parent/forward/back navigation, range/toggle selection, hidden-file
  visibility, type-to-select, per-folder selection/scroll restoration, refresh,
  sorting and column width projection. Five browsing tests pass, including the
  native pointer/keyboard anchor handoff.
- Filename search traverses real directories, supports cancellation and result
  limits/progress, avoids symlink/mount cycles, and separates search selection
  from browsing state. The recursive search/restore workflow test passes.
- Folder preference persistence uses bounded, atomic private JSON state and
  preserves non-UTF-8 Unix paths. Its save/reload/default-reset test passes.
- Custom actions parse argv and expand path placeholders without shell
  interpretation. Three actual subprocess tests pass: raw unusual filenames,
  bounded output and cancellation of an ignoring descendant. The latter first
  reproduced and then fixed a parent-exit cancellation bug.
- `cargo test --offline -p canopa-files` passed all nine tests and focused
  Clippy passed before the desktop dependency was added.
- Backend inspection found Iced 0.14's Wayland file drag-and-drop events
  explicitly unimplemented. `docs/adr/0015-files-native-widgets.md` records the
  evidence and selects GTK4 with a private Canopa theme adapter. Core models
  remain toolkit-independent. GTK 4.22/GIO/GStreamer/Poppler are available;
  GTK Rust dependencies downloaded and `cargo check -p canopa-files` passed.
- The executable now creates real GTK Browser Windows/Tabs, responsive
  virtualized columns/list/icons, a location field, filename search, live
  directory monitoring, folder view controls and floating source-bound Quick
  Look. Native appearance uses Canopa's shared palette and typography. GTK
  button backgrounds were corrected after reviewing dark/light screenshots.
- Preview workers prepare bounded images, PDFs and text/source highlighting;
  image zoom and PDF paging are wired. Special files reject immediately and
  cancellation is tested. Unicode syntax offsets are covered. Native media
  controls exist, but actual audio/video playback acceptance remains pending.
- `scripts/verify-files.py` is registered in the native acceptance inventory.
  `native-acceptance-07/summary.json` records passing dark and light/1.25-scale
  cases: 360-pixel tiling, parent/child navigation, source-bound floating preview,
  actual image/PDF pixels, independent tab locations, view/selection retention,
  search restoration, live arrivals, private folder preferences and actual GIO
  default-application launching to an isolated fixture, real create/rename/Undo
  prompts, remote invocation forwarding and Undo from another Browser Window.
  Earlier failing cases
  exposed missing keyboard focus after rebuilding a virtual list; the adapter
  now focuses the widget and row together.
- The operation store now records individual changes transactionally in SQLite,
  using the workspace's existing native SQLite dependency. Legacy JSON history
  migrates once. Five operation tests cover raw filenames/restart, migration,
  occupied paths, newer contents/cancellation, unavailable storage and one writer.
  The native menu/name prompts/Ctrl+Z queue create/rename/Undo
  operations off the main thread and preserve application lifetime until done.
  Native regression cases pass against the new store.
- The core transfer seam now copies trees/links/raw names, moves on the same
  filesystem, forces cross-filesystem moves with source verification, handles
  Merge/Keep both/Skip/Replace and batch choices, retains replaced destinations,
  and records completed portions as one persistent Undo. Eighteen transfer tests
  pass, including partial cancellation, isolated failures, newer work, folder
  merges, forced moves across actual distinct filesystems, restoring older
  history after an inode-changing round trip, ordinary retention expiry, in-place
  destination edits during a conflict prompt, and newer work saved through an
  open handle to a retained file.
  An additional ignored child fixture is explicitly run and killed by its
  passing parent crash test; it is not missing coverage.
- The killed-copy test proves startup cleanup of its partial staged file and
  retention of completed Undo. Destination replacement pauses subsequent work.
  Unexpected retained work is protected from automatic expiry and remains
  catalogued for explicit recovery. Native forward conflict dialogs and Operations
  are now wired as described below. Graphical Undo conflict choices, retained-data
  recovery, DND and Trash remain unfinished. Broad requirement checkboxes remain
  open until their whole scope is verified.
- Native file clipboard now publishes GNOME, URI-list and KDE cut formats, reads
  external local URIs asynchronously with bounds, and respects ordinary text
  clipboard use in entries. Copy/cut/paste feed a shared worker queue with
  cancellation, forward file/folder conflict dialogs, unavailable-destination
  Retry, an Operations window (Ctrl+J/header), exact Retry failed items, remaining
  work continuations and targeted Undo. Jobs and clipboard ownership hold the
  application independently. Completed jobs and concrete notification Undo
  actions cannot reverse newer work. ADR 0007 records these behavior decisions.
- `native-transfers-06/summary.json` passes the private native transfer slice:
  actual copy and cut/move between windows, shared Undo, conflict cancellation
  and Keep both, hiding a decision then closing every window after clearing the
  file clipboard, reopening the same process and resuming, external URI-list
  input, isolated FIFO failure followed by a successful item, exact Retry failed,
  stale notification rejection, fresh notification Undo, and an unavailable
  destination restored before Retry. A private notification service receives the
  actual D-Bus messages and activates their real action signals.
  `native-acceptance-08/summary.json` (dark, 1×) and
  `native-acceptance-10/summary.json` (light, 1.25×) pass the entire browsing,
  preview and transfer workflow against the current implementation. Notification
  tests explicitly wait for the newly presented Operations window to take focus
  before sending browser input; filesystem completion can precede native mapping.
- The exact continuation mapping has an additional real filesystem regression:
  cancel inside a Keep both folder, resume into the chosen name, then Undo each
  chronological part. Another test proves that a targeted older Undo cannot
  reverse a newer folder creation. Search selection now remembers Down pressed
  before its results arrive. Native failures also exposed focus ordering when
  reopening a prompt without its original browser; the prompt is now transient
  for Operations. Clipboard acceptance uses captures that leave the clipboard
  untouched (Niri screenshot actions replace it with an image).
- Focused checks pass: all thirty-seven browsing/action/preference/search/
  preview/operation tests with
  `cargo test --offline --locked -p canopa-files --no-default-features`, desktop
  default-feature `cargo clippy --offline --locked -p canopa-files --all-targets
  -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check`.
  Dependency inventory regenerated successfully for 590 release dependencies.
  Headless validation now includes Files and attribution collection includes
  its dependency graph. Full workspace checks
  and independent code review have not yet run.

## Trash progress

- `operations/trash.rs` implements the desktop Trash layout with reserved metadata,
  raw path escaping, short opaque storage names, real private directories,
  sticky shared-volume checks and private fallback. Mounted filesystem identifiers
  choose the store; per-volume entries use relative paths. Discovery reads existing
  stores and does not create directories on unrelated mounts.
- Dedicated Trashed/Restored journal changes integrate metadata and payloads with
  shared Undo and recovery. Restore preserves newer destinations, and restoring
  to an explicit alternate path can be undone back into Trash. History expiry
  keeps Trash payloads. A whole batch uses one history entry.
- Public filesystem tests cover raw nested names/links and restart Undo,
  newer destination protection and Undo Restore, a symlinked shared Trash rejected
  in favor of private volume storage with a long non-UTF-8 filename, and automatic
  store selection preserving a symlink-parent original path with one batch Undo.
  Additional cases cover independent failure/cancellation, malformed or missing
  metadata, killed-process prefix recovery, and restoration after Undo expiry.
  A new regression exposed a missing cached volume-root check on Restore; it
  now refuses namespace replacement before mutating the selected item.
- The auxiliary native Trash window provides list selection, Restore, another
  folder chooser, Copy for manual recovery, shared Ctrl+Z/Operations, live refresh,
  and All locations/Main Trash scope controls. Scope changes clear old rows before
  loading. Native automation always chooses its private Main Trash before restoring.
- `native-trash-01/summary.json` passes native Delete/Trash/Restore/Undo and GIO
  interoperability against the initial home-store implementation.
  `native-trash-02/summary.json` adds mount discovery, scope controls and Trash
  Quick Look; `native-trash-chooser-02/summary.json` proves the native folder
  chooser. `native-acceptance-12/summary.json` passes the complete dark workflow
  including a 360-pixel Trash tile and independent floating preview. The initial
  narrow test exposed a 444-pixel title-bar minimum; moving actions into the body
  toolbar fixed it. `native-acceptance-13/summary.json` also passes the full
  workflow under light appearance at 1.25× scale, including the 360-pixel Trash
  tile and native folder chooser. The preview adapter now accepts a
  source interface and original-name hint. Its additional public preview test
  passes: audio classification uses the hint while the read path stays opaque.
- ADR 0008 records behavior, storage, isolation and current limitations. A public
  concurrent-request regression reproduced duplicate Restore history; item and
  volume validation now happens inside the operation lock after recovery. The
  focused repair passes. All 48 core tests, Files Clippy with warnings denied,
  formatting, attribution checks and the native build pass. The copy and Trash crash helpers
  are intentionally ignored as standalone tests and are actually launched/killed
  by their passing parent tests.
- Custom actions now have a native graphical editor: add/delete/reorder,
  name/icon/command, filename patterns, file/folder kinds, selection counts,
  recorded or typed shortcuts and toolbar placement. Drafts survive selection
  changes and ask before discard. Saving updates all Browser Windows.
- Actions use the shared `files` KDL namespace, with an owner-only headless
  validator and common immutable-snapshot checking in `canopa config check`.
  Graphical saves preserve other namespaces/comments and configuration symlinks,
  reject stale source bytes/targets, and use a private atomic write off the UI
  thread. Two public action/configuration regressions cover round-trip editing,
  other-owner preservation, invalid catalogs and stale-save refusal. All 50
  Files core tests and all 15 shared-configuration/CLI tests pass.
- Applicable actions appear in row context menus (right-click or Shift+F10),
  shortcuts and configurable toolbar buttons. Cut/Copy/Paste/Rename/Trash are
  also in the context menu. Popover/control focus retains its keyboard input;
  Quick Look explicitly routes browser navigation independently of source
  control focus. The action toolbar scrolls to preserve narrow layouts.
- Action invocations capture actual paths and working folder, recheck metadata
  off-thread, and appear in Operations with output, exit status and cancellation.
  They run independently of serialized file operations and unavailable Undo
  storage. Their output does not manufacture an Undo entry.
- `native-actions-03/summary.json` (dark) and `native-actions-05/summary.json`
  (light at 1.25×) pass real graphical creation, shared persistence, image
  conditions, shortcut/context-menu/toolbar execution, actual argument delivery
  and output presentation. The light pointer test first exposed a fixture
  coordinate assumption; it now derives pointer coordinates from actual logical
  window dimensions. Native logs contain no GTK warnings. The full dark
  regression workflow also passes in `native-acceptance-14/summary.json`.
  Files/CLI/config Clippy, runner prerequisites and dependency attribution checks
  pass. ADRs 0009 (Files) and 0016 (integration) record these decisions.
- Real native drag and drop now connects file rows, folder targets, column
  backgrounds and sidebar locations to the shared transfer queue. Same mounted
  filesystem defaults to Move, other/mixed/unknown sources to Copy; Ctrl/Shift
  override the choice. A target outline and overlay hint identify the action,
  count and folder without moving rows under the pointer. Selection is captured
  before the native press handler so multiple selections survive drag startup.
- Native failures reproduced empty modifier state at the destination and a late
  drop after source-side Escape cancellation. Local drags now retain source
  modifier state and declared capabilities, and cancelled gestures are rejected
  before queueing. Rejected offers are explicitly finished. Temporary tagged
  diagnostics are removed. Folder-row feedback takes precedence over its column.
- Three public drop-policy/transfer regressions pass, bringing Files to 53 core
  tests. They cover real Copy/Move/Undo across filesystems, forced actions,
  symlink classification, cancellation and per-item missing-source isolation.
  The second-filesystem fixtures use private writable locations and can fall
  back to `/dev/shm` when `/tmp` shares the checkout filesystem.
- `native-drops-15/summary.json` (dark) and `native-drops-16/summary.json` (light,
  1.25×) pass tiled-window drag transfers in all three views, folder targets,
  Ctrl/Shift, Escape, foreground filename conflict/Keep both/Undo, actual
  cross-filesystem defaults and moves, multiple selection/raw names, and actual
  file-URI transfer to and from an independent native process. The icon fixture
  now targets the rendered icon rather than the grid's selection padding. The
  workflows are registered in desktop acceptance; ADR 0010 records ownership,
  lifecycle and remaining acceptance. Focused Clippy and runner checks pass.
  `native-acceptance-17/summary.json` also passes the complete existing dark
  browsing/preview/clipboard/operation/Trash workflow against the new adapter.
  Formatting and Python compilation checks pass. Dark/light drag logs are free
  of GTK and peer warnings. Rendered checks confirm that the folder-row hint
  names that folder and that column browsing remains readable at fractional scale.

- Shared interface preferences now have a native Views/Toolbar/Locations/Shortcuts
  editor, explicit atomic Save/Reload, canonical navigation/custom-action shortcut
  validation, ordered primary commands and action positions, and organized local
  bookmarks. The sidebar uses XDG personal locations and a scrolling layout;
  a Locations page in the folder menu retains access in narrow tiles. Column
  projection accounts for the visible sidebar. Editor pages scroll at 360 pixels.
- Folder state version 2 stores only deliberately changed fields. Changing global
  defaults updates inherited fields and all open tabs; Use defaults clears folder
  choices. Legacy version 1 snapshots retain their previous defaults as full
  overrides. Public regressions cover sparse inheritance/reset, raw bookmark
  paths, shared action/settings preservation, canonical shortcut conflicts and
  invalid typing/selection keys, legacy migration, and synchronized action-toolbar
  switches/positions. All 58 Files core tests pass with the final synchronized
  toolbar state model, including six preference tests. The headless Files
  validator also accepts the actual graphically saved shared configuration.
- Native verification reproduced and fixed parent/dialog activation ordering,
  committing an in-progress spin-field value on Ctrl+S, notebook tab minimum
  width, focus-out warnings while disabling the editor, missing string expressions
  in searchable dropdowns, and focus loss between nested Locations popups. The
  latter now uses two pages of one popup. Temporary diagnostic logging and the
  reduced chooser-probe code are removed; probe artifacts remain clearly named
  under ignored native-preferences-probe directories.
- `native-preferences-11/summary.json` passes dark graphical bookmark persistence,
  toolbar add/remove/reorder, view defaults, remapped filename search, sparse
  folder choices, actual 360-pixel editor/browser geometry, parent restoration,
  bookmark navigation through the narrow menu, Use defaults, and persistence in
  a restarted process. `native-preferences-14/summary.json` passes the same
  workflow in light appearance at 1.25×. Its prior Add/Save failure was a fixture
  race with native button activation; the fixture now waits for activation before
  Save. Rendered inspection confirms that the editor fits 360 logical pixels and
  that resetting the folder changes Icons to the inherited List view. Native
  preference logs, including the restarted process, contain no GTK warnings.
  `native-actions-06/summary.json` (dark), `native-actions-07/summary.json` (light,
  1.25×), `native-drops-17/summary.json` and the full
  `native-acceptance-18/summary.json` pass the existing native regressions against
  the shared preference/keyboard adapter. Files Clippy, formatting, Python
  compilation, acceptance inventory, runner prerequisites and attribution checks
  pass. ADR 0011 records the settings contract. This completes the interface
  preference requirement; the other composite requirements remain open.

- Interactive Undo now offers Keep both, Skip and Cancel for occupied paths when
  reversing a same-filesystem rename/move or Trash. Skip retains that change for
  retry and continues independent changes in the same batch; Operations reports
  partly undone work and keeps the remaining history available. Core callers
  that do not supply a resolver keep the earlier conflict-refusal behavior.
- Keep both restores the incoming item with a new name and preserves the newer
  occupant. Its durable Undo intent carries a path relocation. The final SQLite
  transaction updates older/remaining history, retained-stage paths, protected
  cleanup records and inode bindings together with the completed reversal.
  Startup recovery and retained-only remainders apply the same relocation.
  Directory-link descendants are included even if the link is currently broken.
- Six new operation regressions cover raw names and older history after restart,
  nested retained replacements, directory symlinks, partial batch Skip/retry,
  Trash restoration, and an actual process kill after the filesystem rename but
  before the Undo API returns. After restart, the older 4,000-file copy and folder
  creation both undo successfully while the newer blocker remains untouched.
  The existing actual cross-filesystem round-trip test now additionally verifies
  inode bindings after a Keep both folder restoration. All 64 core tests pass;
  the three ignored crash-helper cases are executed and killed by their parents.
- Native dialogs show both versions' sizes and modification times. Trash sources
  show their original name rather than their storage basename. Jobs weakly
  capture the invoking window, so questions appear when Undo starts from focused
  Quick Look or Trash. The native test first reproduced a missing dialog from
  Quick Look; that source ownership is now explicit. Keep both is the safe Return
  default. The metadata label stays selectable without starting fully selected.
  `native-undo-06/summary.json` (dark) and `native-undo-07/summary.json` (light,
  1.25×) pass metadata/Quick Look focus, Skip, cancellation, Return/Keep both and
  Trash-origin restoration. Rendered inspection confirms readable unselected
  metadata and original Trash names. Both native logs are free of GTK warnings.
  `native-acceptance-19/summary.json` passes the full existing browsing, preview,
  clipboard, background transfer/conflict/notification and Trash workflow against
  this adapter. Clippy, formatting, Python compilation, runner inventory and
  prerequisites and dependency attribution checks pass. ADR 0012 records the
  contract and explicitly incomplete Undo cases; its broad requirement stays open.

- Recovery now lists active replacement backups and protected versions retained
  after Undo expiry, with original filename/location, operation context, metadata
  and unavailable states. Its native window is available from the folder menu,
  Operations, a configurable command and optional toolbar position. Space opens
  source-named Quick Look; Ctrl+R chooses a recovery copy's name/location. Workers
  load 256-version pages with cancellation and late-result checks. Jobs refresh
  an open catalog; F5 rechecks external changes.
- A recovery copy uses the existing staged copy/Undo transaction without
  consuming the retained version. The engine checks the private catalog key,
  stage identity and observed file version, refuses stale/unavailable snapshots,
  occupied destinations and the source's own retained-data folder, and preserves
  symlink bytes. Unexpected directory/special payloads remain visible without a
  copy action. Normal history expiry still owns ordinary backup release.
- Active catalog reads resolve subsequent namespace moves without changing
  historical Undo paths. Expiry archives that physical location and filename
  context. Later forward/reverse namespace moves and Keep both relocations
  update archived data in the operation's transaction. Renaming/trashing the
  replacement itself keeps its sibling backup's original name rather than
  adopting the replacement's new or opaque Trash name.
- Six public recovery regressions cover open-descriptor edits, expiry/restart,
  active backup preservation, stale/occupied/cancelled destinations, raw names,
  symlink copy-out, unavailable stages, folder moves spanning expiry, archived
  Keep both relocation, Trash naming and a 257-version batch. All 70 core tests
  pass, including the existing killed-process and actual cross-filesystem cases.
  `native-recovery-06/summary.json` (dark) and
  `native-recovery-08/summary.json` (light, 1.25×) pass actual replacement,
  metadata/preview, native save chooser copy-out, Undo of the copy, edits through
  a retained open document, 360-pixel layout, process restart and original-name
  preview after Trash. Rendered inspection confirms readable narrow rows and
  the changed retained text after restart/Trash. Both application logs have no
  GTK warnings. The full `native-acceptance-20/summary.json` regression passes
  browsing/preview/clipboard/background operations/notifications/Trash. Clippy,
  formatting, Python compilation, acceptance inventory, runner prerequisites,
  dependency attribution and whitespace checks pass. ADR 0013 records decisions
  and limitations. Fixture waits now distinguish empty clipboard startup,
  deliberate exit and Operations mapping from an application failure. An
  interrupted light run produced no result; its repeat passed.

- Undo now offers Replace for file/symlink restoration-path conflicts when
  reversing same-filesystem rename/move or Trash. The dialog explains that the
  newer occupant is retained in Recovery and keeps Keep both as Return's default.
  Folder and folder/file collisions continue to offer Keep both/Skip; Undo Merge
  remains outstanding. Noninteractive Undo retains its conflict-refusal behavior.
- A durable Undo intent records displaced data before extraction. Version and
  identity checks refuse occupants changed during the question. On failure the
  newer occupant returns without overwriting another arrival, or stays protected
  in Recovery. Completing reversal archives its recovery record atomically with
  deleting the history step and clearing the intent. Startup recovery and the
  retained-only remainder of a replacement move use the same transaction.
  Same-filesystem rename/Trash reversal now checks identity after the actual
  rename and attempts a no-overwrite rollback on a mismatch.
- Six new public Undo Replace regressions cover raw names, expiry/restart,
  changes during the question, Trash with a broken link, folder/file collision
  refusal, a move with an older destination backup, and writes through an open
  handle after displacement. A child is killed after an actual filesystem
  restoration and before the Undo API returns; reopening makes its newer
  occupant recoverable and the rest of the 500-file move undoable. All 76 core
  tests pass; all four ignored crash helpers are executed by their parents.
- `native-undo-09/summary.json` (dark) and `native-undo-10/summary.json` (light,
  1.25×) pass the earlier metadata/Quick Look prompt/Skip/Cancel/default Keep
  both/Trash cases, plus actual Replace, preview of the protected newer file,
  and native save-chooser recovery copy with both versions preserved. Rendered
  checks confirm the explanatory text, metadata, visible choices and retained
  newer content. Both application logs have no GTK warnings. A fixture failure
  came from typing a space in a filename prefix, which correctly opened Quick
  Look early; the prefix now uses only `Replace`. Tagged probes confirmed that
  focus remained in the list and are removed from source. Probe artifacts are
  clearly named `native-undo-focus-01`.
- The acceptance runner now routes SIGTERM through its normal failure/cleanup
  path. `native-interrupt-01/interruption-check.json` records a deliberate
  interruption during a native conflict and confirms that its private runtime
  was removed. The underlying run correctly records an interrupted failure.
  `native-acceptance-21/summary.json` passes the full existing native regression
  for browsing, previews, clipboard, background operations, notifications and
  Trash. Focused typechecking/Clippy, formatting, Python compilation, runner
  inventory/prerequisites, dependency attribution and whitespace checks pass.
  ADR 0014 records the Undo Replace contract and remaining cases.

- Undo now offers Keep both/Replace/Skip/Cancel when a copied replacement has
  newer edits or another file at its path, and when a retained-only remainder's
  restoration path is occupied. The incoming version is explicitly labelled
  previous destination with its logical name and metadata. Keep both updates
  older history across restart; Replace archives newer work in Recovery. Skip
  and Cancel leave the original history retryable. Metadata and Replace treat
  directory links as links and preserve their targets.
- Five new public regressions bring the core total to 81 passing tests. A
  killed-process fixture restores a previous destination from two 500-file copy
  operations, then verifies all newer work survives, older history still works,
  and no consumed stage folders remain. It exposed missing startup stage cleanup.
  Another regression exposed history beneath an older directory link hidden
  behind regular-file versions; Keep both now relocates that whole historical
  namespace. Completion requires the retained file to have left its owned stage.
  All five ignored crash helpers are executed by their parent tests.
- `native-undo-12/summary.json` (dark) and `native-undo-14/summary.json` (light,
  1.25×) pass all 16 Undo checks, including actual changed-copy choices,
  Recovery preview/copy-out and replacing/restoring a directory link. Rendered
  inspection confirms the newest preserved content and link metadata. The
  light fixture now explicitly acquires compositor focus after reopening
  Recovery; its earlier timeout was keyboard input reaching the browser.
  Application logs have no GTK warnings. `native-acceptance-22/summary.json`
  passes the full existing native regression. Final desktop build, core tests,
  Clippy, formatting, Python compilation, runner inventory/prerequisites,
  attribution and whitespace checks pass. ADR 0015 records these decisions.
- The 500-file Keep both/restart test exposed a 48.5-second reversal because
  every relocation decoded unrelated older history. Schema version 2 now indexes
  affected namespaces for history, archived Recovery and identity bindings, with
  atomic migration and bounded 256-record pages. The same test passes in about
  3.5 seconds including setup and the actual killed process. Its 20-second bound
  failed before the fix. A supported-v1 input fixture verifies older Undo through
  an inode binding, non-UTF-8 names, equivalent component spellings, distinct
  similarly named siblings and archived Recovery after Keep both and restart.
  ADR 0016 records the format and path semantics. Large active Recovery catalogs'
  later-namespace traversal remains separate follow-up work.
  All 82 core tests, the desktop build and all-target Clippy pass after this
  change; the same crash fixture takes 3.9 seconds in the full suite.
  `native-undo-15/summary.json` passes all 16 native Undo checks on the rebuilt
  application; `native-acceptance-23/summary.json` passes the full existing
  desktop regression in light appearance at 1.25×. Rendered inspection confirms
  preserved newer copy edits in Recovery; neither app log has GTK warnings.
  Formatting, Python compilation and whitespace checks also pass.

- Unresolved pending work now leaves the engine open for history/Recovery
  inspection while normal mutations remain paused. Recovery exposes the
  operation/error and owned quarantines/stages with original-name Quick Look,
  non-consuming copy-out and explicit retry. Schema version 3 gives rescue copies
  their own intent slot; successful completion and cleanup cannot erase the
  original pending Undo. Expiry waits while either slot is pending. Destination
  guards resolve parent locations to reject aliases into journal-owned folders.
  A public real-process regression first reproduced the inaccessible engine,
  then verified copy-out/restart, alias rejection, retry after manual path
  resolution and independent Undo. It also kills a rescue copy after actual
  bytes are written, proving startup cleans that partial copy and retains the
  original journal. ADR 0017 records decisions and remaining scope.
  All 83 core tests pass, including both new killed-process phases; all seven
  ignored helpers are executed/killed by their parent tests. All-target Clippy,
  typechecking, runner inventory/prerequisites and whitespace checks pass.
  `native-interrupted-08/summary.json` (dark) and
  `native-interrupted-09/summary.json` (light, 1.25×) pass eight native checks:
  actual process termination, startup inspection, rendered original-name
  preview, chooser copy-out, another restart, visible controls at 360 pixels,
  Retry and both Undo entries. Rendered inspection confirms saved contents and
  readable narrow controls. `native-recovery-09/summary.json` passes the ordinary
  retained-file workflow after the journal change. App logs have no GTK warnings.
  The fixture now distinguishes expected exits from its normal process monitor
  and checks rendered pixels before capture. A visual reading incorrectly
  called the original dark toolbar blank; pixel comparison corrected the review
  record. Tagged layout probes were removed without a product repaint change.
  `native-acceptance-24/summary.json` passes the full existing desktop regression
  in light appearance at 1.25×. Formatting, Python compilation and final
  whitespace checks pass. No native verification processes remain running.

- Changed copies without a previous destination now offer Keep item, Move to
  Recovery, Skip and Cancel. Keep item is the Return default and finishes the
  history step without touching its current contents; Skip keeps it retryable.
  Removal preserves the selected file/link in protected Recovery, atomically
  with completing that Undo step. Restart reconciles the protected extraction
  without overwriting another arrival. Folders/special objects may be kept but
  are not offered removal through the file-only recovery command.
- Keeping a child atomically keeps copy-created ancestor containers from the
  same batch, including across cancellation/restart. Schema version 4 adds an
  exact created-folder index so this decision uses bounded ancestor lookups.
  Skipped children leave their copy-created containers retryable rather than
  failing with a nonempty-directory error. Outcomes and native Operations report
  kept items separately from actual reversed changes.
- Nine new public regressions bring the core suite to 92 passing tests. Cases
  cover Skip/Cancel/restart, partial-batch Keep, an older parent operation,
  open-handle edits through expiry, changes while deciding, a new folder at the
  copy path, raw-name links/target protection, containing-folder dependency and
  cancellation after a Keep. A child is killed after an actual changed-copy
  extraction among 500 files; restart finishes Undo with all 500 newer versions
  in paginated Recovery and all original sources intact. All eight ignored
  crash helpers are executed/killed by their parent tests. The full core result
  is retained in `core-92.log`. Desktop build, all-target Clippy, formatting,
  Python compilation and whitespace checks pass. ADR 0018 records the decisions.
  `native-undo-16/summary.json` (dark) and `native-undo-17/summary.json` (light,
  1.25×) pass all 22 checks, including new current-only metadata, Skip/Escape,
  Return Keep item with separate kept reporting, protected removal and preview/
  copy-out. Rendered inspection confirms current metadata, explanatory labels
  and preserved newer contents. `native-acceptance-25/summary.json` passes the
  full existing desktop regression.
  The dark Undo run has 1,305 GTK TreeStore removal assertions in one short burst
  around the second native save chooser in its earlier replacement-recovery
  steps. Functional checks still pass; do not describe this as a clean-log run.
  Light Undo and the full regression have no GTK warnings. Replaying all 22
  checks serially under GDB/fatal criticals (`native-undo-chooser-debug-01`) was
  clean. Clearly named debug wrappers/artifacts retain the bounded repeated-
  chooser investigation. Its first Recovery-only attempt hit a fixture wrapper-
  PID assumption at the exit/restart step; it is not a product recovery failure.
  `native-chooser-stress-debug-02/summary.json` subsequently passes the 22 Undo
  checks and 30 serial native recovery saves under GDB/fatal criticals, with
  every output's exact contents checked and no GTK assertion. The initial burst
  remains unexplained; this does not establish that it is fixed.
  `native-chooser-parallel-debug-01` (dark) and
  `native-chooser-parallel-debug-02` (light, 1.25×) then each passed the 22 Undo
  checks and 30 repeated native recovery saves concurrently under GDB/fatal
  criticals. Exact saved contents matched; neither run produced an assertion or
  trapped stack. Parallel load did not reproduce the original burst. No
  application log probes or speculative workaround were added.
- Files now reloads saved shared configuration and its external theme dependency
  automatically. One captured main input supplies Files settings, actions and
  appearance together. Serialized worker preparation polls every 250 ms and
  admits a candidate after two equal source observations. Invalid candidates
  retain the complete last-good state and show a separate browser banner;
  repairs clear it. Discovery survives graphical saves, explicit sources remain
  explicit and `--defaults` never starts a watch loop. Canonical target stamps
  detect a symlink retargeted to an existing hard link despite identical inode
  metadata. Desktop installation revisions reject worker results that predate
  a graphical save/reload, then force publication of the latest settled source.
  Source-only edits preserve browser input/views; effective Files changes retain
  location drafts too. Appearance updates reuse one CSS provider and existing
  code-color tags, retaining preview objects, focus and PDF page. Editor fields
  remain untouched by external updates, including an uncommitted spin value.
  Stale main-file saves refuse; explicit Ctrl+R reload asks before discarding a
  draft in both editors. Native acceptance exposed and fixed the previously
  missing Ctrl+R action-editor binding. ADR 0019 records these decisions.
- Five public source/theme regressions bring the full core suite to 97 passing
  tests (`core-97.log`; eight crash helpers remain executed by their parent
  tests). Invalidation and same-inode symlink retargeting each reproduced a
  failure before their fixes; the final five configuration tests pass in
  `reload-green.log`. Tests also cover initially missing discovery, discovery
  after a graphical save, directory replacement, explicit missing files,
  dangling-link repair, isolated defaults, missing/modified/invalid themes,
  complete candidate rejection/repair and content-free errors.
  `native-reload-red-01` proved the old binary did not repaint automatically.
  `native-reload-05` and `native-reload-06` pass all nine native reload checks
  in dark and light at 1.25×, with clean GTK logs. `native-reload-08` (dark) and
  `native-reload-09` (light, 1.25×) pass the final ten checks, adding effective
  Files changes during location entry; both final logs are clean. Rendered
  inspection confirms the held second PDF page, stale-editor draft fields,
  explicit reloaded action and separate invalid-candidate banner.
  The expanded fixture waits for an actual rendered PDF before changing pages;
  its parent-return case respects deliberate child-folder selection.
  `native-acceptance-26` passes all 27 existing desktop regression checks with
  a clean GTK log, including zero-window transfer lifetime, clipboard, Undo and
  Trash. Build, all-target Clippy, formatting, Python compilation, maintained
  desktop inventory and dependency attribution checks pass. Actual playback,
  removable-drive behavior and broader lifecycle/control acceptance stay open.
- Files now discovers local GIO volumes and standalone local mounts in a dynamic
  Devices section shared by the sidebar and narrow Locations page. Mounted roots
  use normal asynchronous browsing. Unmounted local volumes remain visible for
  an explicit mount request; unmount uses the backend asynchronously without a
  force flag. Monitor signals coalesce updates without rebuilding directory
  views or configured locations. Credentials/errors use the native mount
  operation. Network/non-local and shadowed entries are excluded, and a volume's
  associated mount is not listed twice. Device requests do not enter file Undo.
  Active/queued Files jobs, including arbitrary custom actions, prevent removal.
  New transfers/actions wait during removal and are released afterward, keeping
  actions' ordinary ability to run alongside transfers. A pending device request
  holds the application; monitor subscriptions alone do not. Mount completion
  checks its original tab's navigation intent and current backend presence.
  ADR 0020 records the decisions and explicitly remains in progress.
- A separately compiled GIO extension module provides the external storage
  boundary only inside the private desktop's environment. It exposes real private
  files, hotplug signals, permission/busy failures and deliberately held requests;
  it never accesses a block device or changes a real mount. Initial mounted
  discovery, unmount and remount cases failed before implementation. Native
  acceptance then caught/fixed expanding device buttons widening the sidebar,
  and removal-menu focus preventing subsequent file paste. Both source URI
  clipboard data and actual queued copy/action outputs are independently checked.
  `native-devices-13` (dark) and `native-devices-14` (light, 1.25×) pass ten checks
  with clean GTK logs: mounted browsing/preview, normal unmount, explicit remount,
  live disconnect/reconnect in an open narrow Locations page, 360-pixel keyboard
  use and floating preview, active-action removal refusal, failure/retry, delayed
  mount preserving newer navigation, zero-window lifetime/closed-origin safety,
  and transfer/action queue release. Their public completion notifications prove
  the resumed jobs finished. Earlier run 09 hit a fixture read before its first
  notification file existed; 11 exposed the focus defect. Run 12 was terminated
  with SIGTERM while waiting at that same case. Final reruns include the focus
  fix and exact clipboard URI assertion. `native-acceptance-27` passes all 27
  existing desktop checks with a clean GTK log. Build, all-target Clippy,
  formatting, Python compilation and maintained desktop inventory pass. The
  previous 97-test core checkpoint is unchanged; no core behavior changed here.
  `native-devices-15` waits for both resumed jobs' completion notifications and
  captures their settled Operations labels, including the copy's Undo control.
  Visual inspection also caught narrow Locations shortening the fixture's short
  device name. The popup now propagates natural width within a 200–280 logical
  pixel bound. `native-devices-16` passes all ten checks at light/1.25× after
  that change; rendered inspection confirms the complete name within the same
  360-pixel browser and both completed jobs. Final logs remain clean.
  Removal capability selection (drive stop/eject), disconnected-root/identity
  guards, mounted-root drop targets, authentication/cancellation and actual
  kernel mount/physical unplug verification remain open. A simulated backend
  disconnect is not evidence for those requirements.

- Queued requests now capture storage placement and mount epochs. GIO removal
  invalidates the matching root before releasing the queue; worker preparation
  durably records known roots, original Unix path bytes and device identities in
  bounded private state. Loaded roots begin unavailable. Directory snapshots keep
  their worker-observed resolved location so a waiting request cannot follow a
  retargeted browsing alias. Retry retains the original device/browsing intent;
  no-UUID media requires the same mount session, while a fresh reviewed request
  can use the current mount. ADR 0021 records these choices and their limits.
  Native red cases reproduced backing-folder writes (safety-red-01), alias
  redirection (retarget-red-04), replacement-media Retry (replacement-red-07),
  and no-UUID replacement Retry (identityless-red-10). Final safety-08 (dark),
  safety-09 and safety-12 (light, 1.25×) pass five checks: queued alias refusal,
  replacement refusal, original-device Retry, alias-retarget refusal and offline
  restart protection. Identityless-11 passes refusal plus fresh-request success.
  These names are under `.scratch/file-manager/native-device-*`. Main and restart
  logs are clean; rendered inspection confirms the unstarted-request explanation,
  Retry request control and completed original-device retry with Undo. Existing
  device workflow `native-devices-17` passes ten checks. Existing desktop workflow
  `native-acceptance-29` passes all 27 checks after fixing a stale-Undo status
  regression caught in run 28. Storage refusals remain retryable; unrelated task
  validation failures retain Failed. Core checkpoint `core-storage-97.log`
  passes 97 tests with eight parent-invoked crash helpers ignored at top level.
  Build and all-target Clippy pass for the queued-request changes.
  Startup reconciliation now has a placement check before either journal lane
  replays. `storage-startup-red.log` reproduced cleanup of unfinished real data
  despite a refused check; `storage-startup-green.log` passes after the fix,
  preserving both interrupted records and bytes and allowing later reconciliation.
  Full core `core-storage-startup.log` passes 97 tests with eight parent-invoked
  crash helpers ignored at top level. Native `native-device-recovery-red-14`
  independently reproduced removal of the offline backing-folder file on startup.
  `native-device-recovery-15` (dark) and `native-device-recovery-16` (light,
  1.25×) each pass guarded startup, remount/Retry and subsequent Undo. Both main
  and restarted Files logs are clean; visual inspection confirms the offline
  explanation/Retry control and cleared Recovery state after reconciliation. `native-interrupted-storage-30` passes the existing eight recovery checks
  at light/1.25×, including Quick Look, copy-out, second restart, 360-pixel controls
  and separate Undo. Latest build and all-target Clippy pass. Format, Python
  syntax, acceptance inventory and whitespace checks pass.
  In-flight guards, original-device affinity for persisted history, clipboard and
  drag-source bindings, chooser bindings, pinned-directory races, per-file
  unavailable-source continuation and physical/kernel unplug remain open.

- Running transfers now accept worker-side source-entry/destination-folder
  placement checks through the public transfer seam. Desktop checks run per item,
  during 64 KiB copy steps, after conflict replies and before commits/source
  removal. Destination loss pauses with an explanation and exact continuation;
  an unavailable source fails individually while independent files continue.
  Registered copy stages now belong to their durable journal instead of unchecked
  TempDir cleanup. Continue checks all pending locations before reconciliation.
  A typed placement refusal after source quarantine leaves its intent/payload
  intact instead of attempting rollback on unavailable storage. ADR 0022 records
  behavior and remaining authority/phase limitations. Operations also recognizes
  its configured Undo shortcut through shared latest-Undo/busy checks.
  `storage-transfer-red.log` reproduced ignored mid-copy placement refusal;
  `storage-transfer-resume-red.log` reproduced Continue cleaning offline data;
  `storage-move-unlink-red.log` reproduced the absent post-quarantine check.
  Final full core `core-storage-inflight.log` passes 99 tests, zero failures,
  with eight parent-invoked crash helpers ignored at top level. Native build and
  all-target Clippy pass. `native-device-inflight-red-17` independently reproduced
  an active copy committing into the unavailable backing folder.
  `native-device-sources-red-18` reproduced one offline source blocking the local
  file. `native-device-sources-20` passes all three source isolation/backing-folder
  refusal/exact-Retry checks with clean Files logs. Inflight run 19 paused and
  resumed correctly but exposed a missing Operations Undo shortcut. Run 24 proves
  the shortcut now starts Undo; its 96 MiB reversal exceeded the fixture's ordinary
  15-second completion deadline while still running. Final inflight-25 (dark) and
  inflight-28 (light, 1.25×) each pass all four active-copy pause, preserved
  prefix/sources, exact continuation and separate-Undo checks. The resumed
  96 MiB Undo measured 34.529 and 35.924 seconds in this debug build; its fixture
  uses a 60-second large-file deadline. Inflight-26 was interrupted by SIGTERM
  during Undo and repeated. Four concurrent runs (inflight-21/22, sources-23 and
  baseline-31) timed out before Files launched while private notification helpers
  became ready just after 15 seconds; isolated run 24's helper was ready in
  0.36 seconds. Final `native-acceptance-32` passes all 27 existing desktop checks.
  Source isolation also passes three checks at light/1.25× in sources-27. Final
  logs are clean. Visual inspection confirms pause/Continue/Undo controls and the
  completed continuation's Undo control. Sources-29 repeats all three light
  checks after adding a next-frame wait; rendered inspection confirms the settled
  Completed status and Undo control for the exact failed-source retry. Format,
  Python syntax, maintained inventory and whitespace checks pass.
  Pinned-directory/syscall races, non-transfer in-flight checks, persistent media
  associations, phase-aware partial moves and persisted jobs remain open.

- Quick Look now gives native controls their activation/navigation keys before
  forwarding unhandled keys to the originating browser. Its preview area receives
  initial focus, and replacing focused content restores that default focus.
  Native red-02 reproduced Space dismissing a focused Play button; seek-red-06
  reproduced Right being diverted from the position slider. Source focus-red-15
  reproduced removing a media control moving focus to Open, so Space opened the
  next file instead of dismissing Quick Look. The corrected behavior preserves
  Left/Right parent/child navigation beside a 360-pixel browser. Video now uses
  GTK's media paintable in a Picture with the same always-visible native controls
  as audio. Video-08/09 reproduced the paused GtkVideo overlay Play button being
  nonfocusable, including a check of all duplicate Play controls. Corrupt media
  now shows a concise explanation; backend diagnostics remain in the log. Current
  errors are handled at creation and on later changes with weak widget callbacks.
  ADR 0023 records these decisions and remaining source/codec limits.
  The private media fixture generates actual WAV and VP8/Vorbis WebM, captures
  calibrated PCM from a private null sink, and checks distinct video audio plus
  real red/green frames. It uses native accessibility only for visible control
  observations and actual Tab/Space/arrow input, without application test hooks.
  PipeWire/Pulse, policy-only WirePlumber and accessibility buses are isolated;
  no hardware monitor is loaded, ALSA fallback is null and SDL targets private
  Pulse. Capture latency is 20 ms. The fixture corrected unsupported direct AT-SPI
  focus placement, duplicate Play lookup, an assumed Pause label, and a too-high
  PCM threshold at low volume; raw slider/PCM evidence is retained. Volume Home/
  Up, popover Escape, paused video seeking, no autoplay, stop on selection change,
  source ownership across another browser and owner-close shutdown are verified.
  `native-media-16` (dark) and `native-media-17` (light, 1.25×) pass sixteen checks.
  Final `native-media-owner-18` (dark) and `native-media-20` (light, 1.25×) each pass
  all seventeen, including owner-close audio shutdown with another browser alive.
  Run 19 received SIGTERM during initial playback and was repeated. The final
  Files logs have no GTK warnings/assertions; intentional corrupt-media diagnostics
  retain their application prefix. Rendered inspection confirms the transport bar
  and readable error message. `native-acceptance-33` (dark) and final
  `native-acceptance-34` (light, 1.25×) pass all 27 existing checks. Build, all-target
  Clippy, format, Python syntax, media runner prerequisites, maintained inventory
  and whitespace checks pass. The prior 99-test core checkpoint remains valid;
  core behavior did not change. The following entry completes prepared media's
  pathname binding; broader codec/runtime packaging, remaining preview metadata/
  control cases and the full broader scope remain open.
- Preview preparation now opens each regular file once. Classification, metadata,
  image/PDF snapshot reads and media preparation use that opened descriptor.
  Audio/video carry a retained `MediaSource` into GTK. Independent positional
  readers survive source-handle drop, rename and replacement, reject invalid
  positions, and stop at the length captured during preparation. The initial
  public regression returned a replacement file's bytes; the descriptor source
  fixes it. A replacement FIFO never blocks the prepared reader. Six focused
  preview tests pass, including file growth. This pins the opened file instance;
  it does not freeze in-place writes or solve browsing intent before open.
  ADR 0024 records the decision and limits.
  Native input-stream playback reproduced paused video rewind remaining at 0:07
  in runs 21 and 23. Targeted GStreamer/reader tracing showed GIO Rust 0.22.10's
  seekable adapter reporting position -1 and failing to restore it after a size
  probe. The application now supplies a complete read-only GInputStream/GSeekable
  adapter with accurate positions, cancellation and serialized access. Final
  `native-media-bound-25` (dark) and `native-media-bound-26` (light, 1.25×) each pass
  all seventeen media checks, including actual paused rewind frames and owner
  shutdown PCM. Run 22 timed out generating its fixture video before app launch;
  run 24 missed its initial preview-opening input and did not test decoding.
  `native-acceptance-35` passes the existing 27-check native regression, including
  actual image/PDF pixels. Build and all-target Clippy pass; temporary tagged
  instrumentation is removed and final native logs have no GTK assertions.
  The final core run passes 102 tests, with eight explicit child-process helpers
  ignored by the parent runner (`core-media-bound-retry.log`). The preceding final
  rerun received SIGTERM during compilation (`core-media-bound-final.log`).
  Formatting, Python syntax and whitespace checks pass.
- Common media format acceptance now exercises generated MP3, FLAC, Opus,
  AAC/M4A and H.264/AAC MP4 through actual controls, recorded PCM and rendered
  frames. Two-tone audio distinguishes real seeking and rewind from a moving
  slider. The first dark run passed 26 checks; a packaged light/1.25× run passed
  27 with a real folder-handler cold launch. A later dark run exposed a GStreamer
  abort while preparing tagged MP3 (`native-media-packaged-29`), correcting an
  initial interpretation as missed opening input. Stock GTK reproduces the
  same collection/stream-ID assertion with both filename and GIO input in run 32.
  The documented legacy selector has no effect in this installed implementation
  (run 33); no application environment override was added. The same encoded audio
  without its leading ID3 tag passes 100 preparations through each route in run 34.
  Decoder input now omits validated leading ID3 envelopes through a bounded view
  of the pinned file. Logical seek positions, original file bytes and full file
  information remain correct. The worker validates v2.2/v2.3/v2.4 sizes, flags,
  footer and a 32-tag limit. Ten public preview tests pass, including invalid
  envelopes, a sparse 128 MiB tag, filename hints and replacement after preparation.
  The final full core run passes 106 tests with eight explicit child helpers
  ignored (`core-media-formats-final.log`). ADR 0025 records the diagnosis and
  input decision.
  The freshly assembled debug-binary archive passes the expanded release smoke
  check: all eight executables, Files assets, a real Files namespace checked by
  all three packaged validators and existing provider protocols. Files, its
  licenses, runtime guide and desktop entry are included. SHA-256 evidence proves
  all eight packaged executables match the current build artifacts. Native
  `native-media-id3-35` (dark) and `native-media-id3-36` (light, 1.25×) each pass
  28 checks against that extracted package, including nine preparations of the
  original tagged MP3, actual codec playback/seek/rewind, and a folder-handler
  cold launch with spaces and Unicode. Final native logs have no decoder aborts
  or GTK assertions. Rendered inspection confirms paused MP4 rewind at 0:00.
  The original playback/control/owner-lifetime regression passes all seventeen
  checks against this package in `native-media-controls-37`.
  The companion updater now builds/stages Files with the other configuration
  owners. Its new regression first reproduced the missing staged validator, then
  passed replacement/backup with an old process still running. All nineteen
  updater process tests pass. The public configuration workflow now checks all
  three owners, namespace isolation and privacy; it passes after rebuilding stale
  local Shell/Notify binaries. The first owner build ran out of disk space;
  its completed retry is retained. Only generated build artifacts/caches were
  removed to make room; application executables, source and verification evidence
  were retained. Root ADR 0017 records archive, runtime, launcher and updater
  decisions. Native build, Files/CLI all-target Clippy, formatting, Python/bash
  syntax, maintained inventory, runner preflight and whitespace checks pass.
  The preflight needed an isolated bus outside the socket-restricted sandbox.
  This verifies local debug-binary assembly; optimized/hosted release checks,
  FileManager1 integration, original browsing intent, remaining preview cases and
  the rest of the product brief remain open.

- Interactive Undo now reviews an available changed saved previous destination
  before reversing any part of its change. The dialog shows its current
  metadata and offers Restore this version, Skip and Cancel; Skip is Return's
  safe default. Separate destination conflicts still require their own decision.
  Reviewed physical identities remain bound to the original logical items, so
  older Undo works after a saved file is replaced and after process restart.
  Eight public regressions cover edited/replaced versions, Skip/Cancel/restart,
  changes during review, separate Keep both/Replace conflicts, raw-target links
  during move reversal, a replaced saved-data folder, a killed 500-file reversal,
  and partial-move history expiry. The last case first reproduced silent disposal
  of an approved edited backup after the incoming source had returned. Expiry
  now protects every retained-only remainder for Recovery; approval authorizes
  restoration rather than disposal. No schema change is needed.
  A light native run exposed a hidden follow-up question during compositor focus
  handoff (`native-undo-saved-19`). Immediate follow-ups now wait briefly for
  actual owner/Operations activation, without presenting over another app.
  Final `native-undo-saved-20` (dark) and `native-undo-saved-21` (light, 1.25×)
  each pass all 29 Undo checks. Rendered inspection confirms the review, safe
  default, restored contents and subsequent destination conflict. Both logs
  have no GTK warnings/assertions. All 114 core tests pass, with nine explicit
  child-process helpers exercised by parents. Build, all-target Clippy,
  formatting, Python syntax, native inventory and whitespace checks pass.
  ADR 0026 records the decisions; `undo-saved-verification.json` records final
  logs, native checks and binary hash. Missing/unavailable saved data, directory
  recovery, responsive catalog reads during questions and the broader Undo
  requirement remain open.

- Recovery pages, Undo summaries, interrupted-operation summaries and storage
  preflight now use independent read-only WAL snapshots. They do not wait for
  an operation's live conflict callback. An idle snapshot is anchored with a
  nonblocking mutation-lock check; pages then read without holding that lock.
  Active preparation without a failure is excluded from the recovery catalog,
  while journals left by stopped or failed workers remain inspectable. The
  first concurrent-reader regression reproduced blocking during an Undo
  question. Further coverage exposed a healthy copy incorrectly presented as
  interrupted work, then full-suite coverage exposed a paused unavailable-
  destination transfer incorrectly hidden after its worker returned. Both
  classification cases are fixed. Three public tests cover concurrent catalog
  and summary reads, real copy preparation, and missing/replaced history paths
  without private database queries. The full 117-test core run passes, including
  destination loss/reconnect, exact continuation, and process-kill recovery;
  nine explicit child helpers are exercised by parents. Build and all-target
  Clippy pass. Native `native-history-interrupted-24` passes all eight restart/
  recovery checks and `native-history-readers-25` passes all 31 Undo checks,
  including refresh and actual saved-text preview before answering Undo.
  `native-history-readers-27` (light, 1.25×) also passes all 31 checks. Rendered
  inspection confirms the actual saved contents before the question is answered;
  all three successful native workflows have clean GTK logs. Two initial parallel native starts
  timed out before Niri IPC and before Files launch while host I/O pressure was
  high; serial verification after compilation succeeds. A later light run
  received SIGTERM during earlier changed-copy steps; its standalone rerun
  passes. The incomplete attempts are preserved. ADR 0027
  records the read and journal-classification decisions. File mutations and
  recovery copies remain serialized, and catalog observations are revalidated
  before copying. Formatting, Python syntax, native inventory and whitespace
  checks pass. `history-readers-verification.json` records the final binary hash,
  results and incomplete attempts. Only generated Files incremental caches were
  removed to make room for verification; source, executables and evidence remain.
  Namespace scan bounds and broader recovery work remain open.

- Undo now reviews the actual replacement of a moved or renamed item before
  returning it. Return this item explicitly adopts that version; Keep item leaves
  it in place and protects any saved previous destination in Recovery. Skip is
  the Return-key default and leaves the history retryable. The original location
  gets a separate conflict decision if occupied. Same-object edits return with
  the item. Folders, raw-target links and FIFOs use atomic rename without opening
  a special stream. The reviewed version survives journaled restart recovery;
  partial returns sync and bind the chosen source before keeping their saved-
  destination remainder. Successive-adoption regressions exposed identity cycles,
  old aliases overriding new forward moves, stale aliases after saved-destination
  restoration, and Keep both relocating an old binding over a new one. Binding
  revisions, terminal physical bindings and the retained-only incoming identity
  fix these cases. Schema 6 migrates supported histories transactionally and
  refuses future input without rewriting its file. Keeping an unchanged saved
  destination now describes its protection accurately in the Recovery footer.
  All 133 core tests pass, including ten explicit child helpers exercised through
  their parent fixtures; build and all-target Clippy pass. Native
  `native-undo-moved-29` (dark) and `native-undo-moved-30` (light, 1.25×) each
  pass all 12 reviewed-move checks; `native-undo-moved-regression-31` passes the
  31 existing Undo checks. Rendered inspection confirms the light review and
  dark protected-version preview/footer. All three logs have no GTK warnings,
  criticals or panics. Formatting, Python syntax, maintained inventory and
  whitespace checks pass. `undo-moved-verification.json` records the binary hash,
  actual scope and the pre-fix regressions. The earlier dark attempt 28 passes
  its 12 checks but predates the final schema/binding/footer work and is excluded
  from current acceptance. Only generated Files incremental caches were removed
  for disk space; source, executable and evidence remain. ADR 0028 records the
  behavior and migration. Missing/unavailable
  moved items, directory recovery and identity-catalog bounds remain open.

- Interactive Undo now represents an available item or an unavailable path and
  lookup error. Same-filesystem moved/renamed items can be skipped, cancelled,
  explicitly finished without return, or have only their shown saved previous
  destination restored. Skip remains the Return default. Finishing protects the
  saved destination through history expiry; restored-only steps report the
  incoming item not returned separately from actual reversals. Reappearance or
  changed saved data invalidates the review. Permission failures never authorize
  restoration into an assumed-empty path. An edited saved version can be
  approved directly in its shown review. Batch Skip continues other items, and
  normal Undo works after the original item reappears. Public tests reproduced
  two further history bugs: saved-only restoration relocated archived children
  as though a missing directory link had returned, and finishing an older
  missing move let Copy Undo silently adopt an independent restored destination
  with matching bytes/mode/mtime. Reversal bookkeeping now follows the actual
  intent and Finish resets the original identity expectation transactionally.
  Older Undo still requires its own decision about a different object there.
  A real 500-file process kill verifies saved-only restoration after restart and
  older Copy Undo while preserving every incoming file moved elsewhere. History
  stays at schema 6. ADR 0029 records the choices. The last complete core run
  passes 144 tests with eleven parent-invoked child helpers; build and all-target
  Clippy pass. Final native runs 35 (dark) and 36 (light, 1.25×) each pass 21
  checks; regression 37 passes all 31 existing checks, with clean GTK logs and
  rendered inspection of the unavailable review/protected preview. These results
  precede a newly added failing external-move regression: without an earlier
  tracked Move entry, restoring a previous destination can still make older Copy
  Undo accept that independent file silently. This is reproduced and unfixed.
  `UNDO-BINDING-CONTEXT.md` records the required causal expectations and physical
  journal identity follow-up; `undo-unavailable-verification.json` distinguishes
  passing scope from the known failure. Only generated Files incremental caches
  were removed for space. Do not claim this slice or full Undo complete.

### Changed/unavailable cross-filesystem destinations — verified scope

ADR 0033 adds changed destination Return/Keep/Skip/Cancel and unavailable
Finish/saved-only restoration. The reviewed version travels in a schema-8 Undo
intent. Successful return updates only its adjacent paired copy's cleanup
baseline; older Copy fingerprints retain their protection. Keep/Finish consume
both rows while protecting the predecessor. Saved-only replay uses the copy's
destination owners and never adopts an unrelated original-location occupant.
Skipped, kept and dismissed paths block earlier in-batch namespace mutations.

The focused cross-filesystem suite passes nineteen parent tests and one ignored
child helper exercised by its parents (`cross-current-focused.log`). Ten new
parents cover edits and replacement nodes, Keep/Finish, missing retry, saved-only
restoration, stale reviews and later cleanup edits, raw links/folders/FIFOs,
v7 upgrade and 100-item process-kill/restart in both reviewed and saved-only
modes. Individual red/green logs remain in the scratch directory. Full core passes
176 tests with thirteen ignored parent-invoked child helpers. Build and all-target
Clippy with warnings denied pass. Native 52 dark and 53 light at 1.25 scale each
pass seventeen checks; original-location regression 54 passes twelve, totaling
46 on the same schema-8 binary. All have clean GTK logs, use device IDs 35/2050
and remove the private secondary directory after session shutdown. Three rendered
reviews were inspected. Exact evidence and binary hash are recorded in
`cross-current-verification.json`. First Clippy's permission-literal lint was
fixed with equivalent octal 0o0 in the fixture. Only generated Files incremental
caches were removed after terminal compilers; both cleanup records remain. The
whole product goal remains active.

### Hidden-folder navigation context — verified scope

ADR 0034 fixes the hidden-ancestor mismatch: Directory visibility includes the
actual navigation child; Browser parent navigation reveals that child before
selecting it. Other hidden siblings stay hidden and the global hidden-file
preference stays off. Refresh/history retain temporary context, while explicit
Hide hidden clears current and remembered exceptions. Public tests cover ordinary
folders and a directory link with a raw non-UTF-8 name. Fourteen focused tests
pass (seven Browser, six preferences, one search), along with build and all-target
Clippy with warnings denied. Native 55b dark and 56 light at 1.25 scale each pass
twelve checks with clean GTK logs. Rendered wide/narrow selection and explicit
hiding were inspected. Native 55 also passed twelve checks, but its narrow row
was obscured by a toolbar tooltip; the existing pointer helper gained move-only
positioning for the final captures, with no app change. Exact scope and binary
hash are in `hidden-navigation-verification.json`. The earlier 176-test full
core/46-check cross-filesystem proof predates this navigation change and keeps
its own hash; it is not a rerun of the current binary.

### Nonempty created-folder Undo — verified scope

ADR 0035 adds Keep folder, Skip (Return default) and Cancel for owned nonempty
created folders. Keep uses the existing atomic ancestor-container transaction
without removing any files. Stale node/directory versions and cancellation refuse
Keep. Five public tests pass, including nested copied containers, retry after
moving added data out, and protected Recovery through reopen/expiry/copy-out.
The first nested test fixture incorrectly cancelled progress callbacks; corrected
to Continue in `created-folder-focused-v2.log`, with five final focused passes in
`created-folder-focused-final.log`. Full core passes 183 tests with thirteen
parent-invoked child helpers. Desktop build and all-target Clippy with warnings
denied pass. Native 57b dark and 58 light at 1.25 scale each pass fourteen checks
with clean GTK logs, totaling 28. Three rendered review/preview images were
inspected. Native 57 stopped before its first folder operation in fixture
notification handling; events() now handles the initially empty stream. Final
proof, exact hashes and diagnostic corrections are recorded separately in
`created-folder-verification.json`. Changed/replaced or unavailable created-folder
locations remain open.

### Changed/unavailable created-folder locations — verified scope

ADR 0036 adds changed-location Keep/Skip/Cancel and unavailable Skip/Cancel/Finish.
Captured identity/version classification avoids opening special streams. Keep
never adopts the node for older history. Finish dismisses only the selected
folder step, preserves its created ancestor rows and reports zero reversed
changes with a separate folder-not-removed outcome. In-batch barriers protect
those ancestors. Automatic preflight fixes an error that silently consumed a
missing-folder step during generic journal recovery.

Twelve focused folder tests pass, including seven new parents for replacement
nodes, missing/unreadable/read-denied-parent locations, older ownership, stale
versions/failures/cancellation, ancestor retry and protected Recovery through
expiry/copy-out. Full core passes 190 tests with thirteen parent-invoked child
helpers. Build and all-target Clippy with warnings denied pass. Native 59 dark
and 60 light at 1.25 scale each pass nineteen checks; owned nonempty-folder
regression 61 passes fourteen, totaling 52 on the same binary with clean GTK
logs. Four rendered dialogs were inspected. Formatting, Python syntax, inventory
and whitespace checks pass. Exact hashes, logs and scope are recorded in
`created-location-verification.json`. WIP is closed; the full product remains
active.

### Directory-return Recovery namespaces — verified scope

ADR 0037 fixes archived Recovery paths during directory Keep both returns.
Historical logical paths still follow the returned name; archived live paths
follow only the actual directory movement. Newer destination records stay in
place. Source-only returns use the actual relocated previous change before
creating their saved-only remainder. Schema remains8.

Three new public regressions pass, including incoming and destination archives,
reopen/history expiry/copy-out, and completing a partial return's saved remainder.
The original newer-destination failure is retained as a red log. Full core
passes193 with13 parent-invoked child helpers. Build, all-target Clippy with
warnings denied, formatting, Python syntax, inventory and whitespace checks pass.
Native62 dark and63 light at1.25 each pass9 checks, totaling18 on the same binary
with clean GTK logs. Four rendered conflict/Recovery previews were inspected.
Exact proof is in `directory-recovery-verification.json`.

### Persistent directory Merge — first verified slice

ADR0038 adds directory Merge to Undo, keeping Keep both as the Return default.
A schema9 parent marker wraps the original step; journalled child returns stay
in that same operation. Nested Merge, file Replace/Keep both/Skip/Cancel and
partial reopen/retry preserve completed children. Parent identity guards precede
staging after an answer. Root completion removes only the owned empty source;
older primary root ownership never adopts the newer destination folder.

Active Recovery resolves pending child namespaces. Incoming archived versions
have a separate association for child Keep both; newer destination records stay
in place. Reserved stage containers keep their own identities and cannot Merge.
Unavailable source-only Merge refuses before expansion, retaining its backup;
once it returns and is reviewed, Merge can finish and restore that destination.
Ordinary history expiry excludes unfinished Merge operations, with direct expiry
and killed-process acceptance still required.

Full core200/13 helpers and the10-test directory suite pass, including7 new Merge
parents and real v4..v8 migration inputs. All-target Clippy with warnings denied,
build, formatting, Python syntax, inventory and whitespace pass. Native64 dark
and65 light at1.25 each pass17 checks, totaling34, with clean GTK logs. All10
rendered captures were inspected. No native fixture repair was needed. Exact
source/binary hashes, scope, interim failures and final evidence are recorded in
`directory-merge-verification.json`. Full Merge and the product remain active.

### Merge interruption and remaining work — verified scope

ADR0039 adds Keep remaining items, Skip (Return default) and Cancel for source
contents left after a Merge's child steps finish. Keep consumes only the marker,
updates parent references without adopting newer primary root ownership, and
protects any unrestored previous destination in Recovery. It reports the folder
kept and zero reversed changes for this decision. Directory version/identity
changes and cancellation refuse stale Keep. Noninteractive errors retain history.
Trash metadata remains if its source folder remains; empty completion removes it.
No wire shape changed; schema stays9.

Seven new public parents prove actual child-install SIGKILL/reopen without
repeated returns, unfinished-Merge history retention, the initial remainder
failure, review choices/stale guards, protected previous data through expiry,
Trash catalog lifetime and correct older history when new source names are
reused. Full core207/14 helpers, Clippy/build, formatting, Python syntax,
inventory and whitespace pass. Native66 dark and67 light at1.25 each pass25
checks, totaling50, with clean GTK logs and four new rendered captures inspected.
No native repair was needed. Exact proof and source/binary hashes are recorded
in `merge-remainder-verification.json`. Full Merge and the product remain active.

### Additional Merge snapshots and owned destinations — verified scope

ADR0040 adds explicit Merge remaining items while retaining the safe Skip
Return default and the existing Keep/Cancel choices. Each new snapshot has its
own persisted scope and stable child identifiers in schema10. Additional work
cannot adopt older history or Recovery versions from already completed returns.
Nested snapshots preserve scope and bind incoming Recovery to their actual
merged destination container. Typed relocation protects pending filename slots
from unrelated later file returns and follows proven whole/nested destination
container moves instead. Migration preserves supported old histories, including
pending flat and nested schema9 Merge sessions.

Seven new public parents cover reused names, old/new Recovery copy-out,
unrelated file aliases, destination container movement, repeated nested passes,
and an actual observed-return SIGKILL/reopen without duplicate returns. One new
migration parent tests flat/nested v9 continuation; remainder guards now also
cover stale/cancelled Merge. Full core215/15 helpers, all-target Clippy/build,
formatting, Python syntax, inventory and whitespace pass. Native68/69 each pass32
checks, totaling64 with clean GTK logs; six new dialog/preview captures were
inspected. No native repair was needed. Exact proof, source hashes and the
original failures are in `merge-additions-verification.json`.

### Partial Merge folder review — verified scope

ADR0041 adds an explicit review when an incoming or merged folder is missing,
replaced, unreadable, or its Trash information cannot be verified. Review occurs
once per affected branch before its pending children. Skip/Cancel retain the
branch; restoring the original folder permits retry. Finish this Merge protects
saved versions and atomically removes only that branch's unfinished steps,
leaving filesystem data in place and reporting the incoming item not returned.
It refuses stale/cancelled answers and preserves older Copy ownership only
through still-owned destination ancestry. Nested Finish leaves its ancestor
remainder pending and continues a disjoint sibling. Schema remains10.

Nine new public parents pass, including source/destination replacements,
permissions, stale/cancelled Finish, older Copy ownership, nested/sibling
continuation, saved versions through expiry/copy-out, and missing Trash metadata
with a listed payload. Full core224/15 helpers, all-target Clippy, desktop build,
formatting, syntax, inventory and whitespace pass. Native70/71 each pass44 checks
(88 total), with clean GTK logs and eight inspected new captures. No native
repair was needed. One initial build failed on incremental-cache ENOSPC; only
generated Files caches were removed after terminal compilers, and the final
build passed without incremental caching. Exact evidence is recorded in
`merge-roots-verification.json` and `merge-roots-source.json`.

### Merge with unavailable saved versions — verified scope

ADR0042 permits directory Merge after Return moved item only when the saved
previous destination is unavailable. A persistent schema11 contents-only mode
returns children, removes only the owned empty incoming root and converts the
marker into one retryable retained-only step. If the saved version disappears
during an active Merge, a dedicated review offers Continue without saved
version, Finish this Merge, Skip and Cancel. Skip is the Return default. Continue
retains saved restoration in Undo; Finish leaves files in place and protects the
saved record in Recovery. Root and saved-container observations reject stale
answers. A changed available saved file keeps its explicit version review.

Schema11 indexes pending saved destinations. The slot and its captured older
history owners stay independent of unrelated later occupants and their aliases;
they follow actual containing-folder movement and the saved restoration's own
chosen name. Backfill preserves supported legacy history. Ordinary-file
contents-only returns share the same reservation rule. Public parents cover
active loss, stale/cancelled decisions, protected Finish/copy-out after expiry,
whole-container disappearance/reappearance, aliases/older Copy ownership,
actual parent moves, migration and changed saved files. An actual child-return
SIGKILL/reopen test proves no repeated approval or completed children before
later restoring the saved version.

Full core234/16 helpers, all-target Clippy, desktop build, formatting, Python AST,
maintained inventory and whitespace pass. Native72 dark and73 light1.25 each
pass57 checks on one binary; all10 new captures inspected and GTK logs clean.
Source manifest95 files is unchanged through verification. Build7625/49136
failed archive ENOSPC; after hash-verified preservation/relocation of generated
Files artifacts into private tmpfs, default incremental build21534 passed.
Current sources, executable, test evidence and artifact bytes are preserved;
exact cache mappings and both failure logs remain in
`merge-saved-verification.json`. No native fixture repair or subagents.

### Ordinary cross-filesystem Merge children — verified scope

ADR0043/schema12 returns ordinary files and raw-byte links through the existing
verified SourceRemoved/Copied pipeline in the same operation and directory scope.
Captured causal bindings survive conversion, so older Copy Undo owns the new
returned identity. Disjoint nested/empty folders use a small destination journal
committed atomically with their marker and per-child snapshot rows before folder
installation. Snapshot size no longer depends on one serialized record. Folder
Keep both persists its chosen alias and moves older history intent before child
bindings are captured. Typed source/destination member roles keep cleanup and
return paths distinct. Fresh parent authority avoids resolving a historical
parent role that a Rename member never had.

Six new public parents cover flat files/raw links, nested/empty folders,
Skip/reopen/retry, folder Keep both/older Copy, long valid filenames, actual128-
child SIGKILL/reconciliation without duplicates, and protected Recovery
availability/copy-out when its container return is deferred. A guard prevents
ordinary copying of private saved containers, which had stranded their catalog
references. This guard preserves their physical source location; dedicated
container/history migration and active-backup visibility remain unfinished.

Full core240/17 helpers, focused54/4 helpers, all-target Clippy, desktop build,
formatting, syntax/inventory and whitespace pass. Native75 dark and76 light1.25
pass17 checks each on device IDs35/2050, including private destination cleanup;
all12 final captures inspected and clean GTK logs. Failed74 is preserved: the
fixture typed Space inside a filename, then toggled the preview closed. Arrow
selection plus one Space fixes the fixture without application changes. Source
manifest98 files matches final runs. Generated Files libraries/common alias were
hash-verified and preserved in private tmpfs before linking; no build ENOSPC in
this scope. Exact evidence and remaining limits: `merge-cross-verification.json`.
Full Merge, Undo and the product remain active.

## Decomposed directory-move Undo (ADR 0044)

Schema13 now records decomposed directory source/destination ancestry and adopts
its existing child journal into a directory return. Root Merge preserves newer
folders; Keep both and automatic recreation retain older owned ancestry. Forward
Merge returns only moved work and preserves unrelated destination files. Shared
batch destinations retain references until all pending returns finish. Root Skip
covers its whole traversal, independent branches continue, Cancel preserves work,
and stale original-folder choices are rejected. Owned folder cleanup contributes
to the public completed count.

Eight new public filesystem parents include actual cross-device files, nested/
empty folders, links and mode, older file/directory Copy Undo, shared-source
Skip/reopen/retry, cancellation and stale review. A parent-invoked SIGKILL helper
observes the installed owned return folder with source children remaining, then
reopening completes128 recorded children without duplicates and preserves older
Copy ownership. Five upgrade parents now include authentic schema12 constraints.
Final core248/18 helpers, Clippy, desktop build, format, syntax, inventory and
whitespace pass. Native78 dark and79 light1.25 each pass21 checks using device
IDs35/2050, including cleanup. All16 final captures inspected. Run77 is retained
as a fixture checkpoint-order failure; no application changes were needed.

The apparent ancestor-heading finding was withdrawn after dedicated native
runs80/81/82: Quick Look covered the labels that had been misread. Visible
headings are correct and match the fresh-window reference exactly after
navigation/preview closure. No rendering change was needed; see
`column-heading-verification.json` and `COLUMN-HEADING-FOLLOWUP.md`. Native logs
have no panic/GTK critical/warning markers. Current binary SHA-256:
`3efaeb87625ae18dc1260c5fa4e64359e389ee229342c1443b36809cb55f29b3`.
Details and limits: `directory-move-verification.json`, ADR0044 and the scope WIP.
The full product goal remains active.

## Directory retry and clipboard intent (ADR 0045)

Failed-item retries now retain the engine's captured directory completion plan,
chosen destinations and physical source/destination identities. Move retries
finish owned empty source folders, and resumed entries record separate subtree
ordinals for Undo. Copy captures the same container identities without source
cleanup. Retry and Continue work in either order; failed rollback leaves a leaf
only in Retry. Native Operations uses these opaque plans directly.

Cut intent now survives the application's own partial clipboard updates and
Retry/Continue. New Copy/Cut or foreign clipboard changes supersede it, and
unavailable unfinished roots remain selected. Core259 tests/18 parent-invoked helpers, all-target Clippy, desktop build, format,
syntax, inventory and whitespace pass. The directory suite passes19 parents/1
helper. Native88 dark and92 light1.25 pass21 checks each; transfer regression90
passes14. All33 final captures were inspected. Decisions, exact sources, binary
and limitations are in ADR0045 and directory-retry-verification.json.

The packaged documentation audit found that flattened README/guide locations
broke their source-relative links. An explicit release map now stages24 files,
including theme examples, with39 working offline links. Development references
name their source-checkout paths without shipping scratch evidence. Archive
assembly and extracted smoke pass. All8 archived executables match the current
build, and both shipped examples validate through all3 owners with their relative
Nord dependency. Root ADR0017 and package-docs-final-evidence.json record proof.
Optimized and hosted release verification remain open.

## Next work

1. ADR 0030 fixes the external-move ownership regression and stabilizes physical
   expectations for restart recovery (verified below). ADR 0031 adds verified
   unavailable saved-version choices and partial source return (evidence below).
   ADR 0032 verifies original-location cross-filesystem conflicts and skipped
   namespace dependencies. ADR 0033 verifies changed/unavailable cross-filesystem
   destination reviews, protected Keep/Finish, saved-only restoration and restart.
   ADR 0035 verifies owned nonempty created-folder Keep/Skip/Cancel and protected
   container preservation. ADR 0036 verifies changed/replaced and unavailable
   created-folder choices, truthful Finish counts and ancestor continuation.
   ADR 0037 verifies incoming/newer-destination Recovery namespaces through
   directory Keep both and source-only saved-remainder returns.
   ADR0038 adds the first working directory Merge return with persistent child
   progress, nested conflicts and stable Recovery/root ownership. Finish its
   cross-filesystem children, root/remainder reviews and interruption/expiry
   acceptance before claiming full Merge. ADR0039 adds remaining-source review
   and child-install process-kill/history-expiry proof. ADR0040 adds additional-item
   Merge with persisted scopes, owned destination relocation and observed-return
   process-kill/reopen proof. ADR0041 adds changed/unavailable folder review and
   guarded branch Finish without filesystem mutation. ADR0042 adds contents-only
   Merge, unavailable saved-version decisions during Merge and independent saved
   destination ownership. ADR0043 verifies ordinary cross-filesystem child
   returns, new nested/empty folders and folder Keep both. Protected container
   migration is now verified for identified file/link backups under ADR0046.
   ADR0047 verifies known file/link ownership during forward folder Copy/Move,
   including live continuation receipts and actual handoff interruption. ADR0048
   verifies context ownership across unrelated later moves, recreated source
   folders, equal path pairs, legacy hints and expiry. ADR0049 verifies shared
   expiry across active/protected/edited owners, live and missing parent aliases,
   and a guard that preserves the previous file when extra saved work is present.
   Directory/special payloads, recovery of extra saved contents, explicit disposal,
   and already-decomposed legacy returns, remaining normal cross-device directory-
   move ancestry and availability cases, other cleanup
   checkpoints and changed saved directory payloads remain open. Next complete remaining
   directory conflict/Merge choices, cross-filesystem reversal
   conflicts, dependent-step Skip and remaining batch choices. Preserve displaced
   newer work for explicit recovery; copying from Recovery must not consume data
   still needed by Undo. Complete phase-specific interrupted-operation choices,
   directory recovery.
   Audit orphan cleanup failures after an intent has committed/cleared.
   Add fault coverage for a replacement reversal's
   retained-only remainder. Add protected-data disposal for archived entries
   without permitting deletion of active Undo backups; support unexpected
   directory payloads and retained stages with additional unexpected contents.
   Bound catalog namespace resolution work for very large histories. Complete
   accurate partial-progress presentation,
   persisted unfinished jobs and exact continuations after restart. Finish
   Trash/Restore conflict choices, batch restoration and restore across
   filesystems; verify mounted-store discovery against isolated mounted-volume
   and unplug fixtures. Complete preview control-focus acceptance and remaining
   selector/menu edge cases. Audit pinned-directory traversal and race/recovery
   paths (including quarantine/verification of rename sources), finish directory
   metadata policy and Keep both naming at filesystem name-length limits before
   claiming file operations complete. ADR0045 verifies
   captured destination identity across in-memory Copy/Move continuations and
   postorder cleanup after retrying failed leaves. Exact persisted continuations
   and directory-descriptor authority remain open. Actual
   removable-drive unplug testing remains distinct from the private unavailable
   directory test.
2. ADR 0034 fixes the observed hidden-ancestor selection mismatch with verified
   wide/narrow native navigation (`HIDDEN-ANCESTOR-FOLLOWUP.md`). Complete
   browsing-intent binding before preview open and remaining preview
   control/metadata acceptance. Prepared regular media now retains its descriptor
   under ADR 0024, with the bounded ID3 view and additional format acceptance in
   ADR 0025. Runtime/archive/launcher delivery is recorded in root ADR 0017;
   FileManager1 and optimized/hosted release verification remain open. WAV/WebM playback,
   seeking, volume, no autoplay, source/owner lifetime and narrow navigation now
   pass native dark/light acceptance in ADR 0023. Continue removable drives:
   drive stop/eject,
   authentication/cancellation, mounted-volume row drops, disconnected-root
   atomic placement guards and in-flight checks beyond transfers, persistent
   history/device
   associations, clipboard/drag/chooser bindings, unavailable browsing/preview,
   reconnect and physical/kernel mount verification. Queued browsing-alias
   requests, original-device Retry and unavailable startup reconciliation now
   have focused passing acceptance; their remaining in-flight and original-media
   limitations are recorded in ADR 0021. Do not allow a queued
   request intended for removed media to act on its underlying mount-point
   directory. The native GIO provider proves UI/backend contracts only.
   Its unmount deliberately leaves the fixture root directory present, so the
   underlying-directory failure can be reproduced without a real mount.
   Finish native custom
   action cancellation/background lifetime, error/condition changes, shortcut
   recording and concurrent transfer/action presentation. Finish empty-area context menus
   and complete control/preview focus acceptance, including remote Recovery
   activation after modal return (native98's focus assumption remains unproven).
   Clipboard interop still
   needs KDE/external-cut and unusual-name acceptance beyond the current native
   GNOME-copy/cut and external URI-list cases.
   Reproduce and trace the intermittent GTK native-save-chooser TreeStore
   assertion observed in `native-undo-16`; avoid an unproven application workaround.
   Follow up DND with modifier changes during the gesture, more external source
   conventions and direct background lifetime acceptance. Verify the combined
   narrow Downloads workflow with preview, transfer, Undo and custom action.
3. Complete remaining desktop integration including FileManager1, verify an optimized release and hosted gates,
   finish native Downloads acceptance, full workspace checks and independent review.

Do not mark requirements complete from compilation alone. Record concrete test,
runtime and artifact evidence against their actual scope.

## Goal continuation
Current goal turn classification: progress. ADR0052 Operations tiling is verified
for its recorded scope; the full PRD remains active. The prior ADR0049 checkpoint
below is historical.

### Historical ADR0049 checkpoint

ADR0049 shared Recovery expiry was verified for its recorded scope. Its ownership tracer
54074 reproduces loss of active/protected shared bytes;56208 confirms actual
ENOENT in current16 too, not only old input.32013 reproduces expiry removing the
previous file before rmdir refuses extra saved work. Five public parents now
observe Recovery/Undo/copy-out and real files, with SQL only preparing input.
First fix separates eligibility from unlink and checks other identified owners
before retiring eligible archive metadata; additional work/changed/protected
versions remain. Alias probes2841 reproduce loss under a strict lexical-path
guard; streamed actual-container lookup now preserves live/missing aliases and
the available catalog record. Seven ownership parents pass; focused80603 passes
all41 parents across ownership, Recovery, upgrades and saved-stage transfers.
Full core88150 passes289 parents/19 helper definitions;99 Rust/build source paths
snapshotted and unchanged. Pipeline11298 passes Clippy/default build/format;
desktop3bdcd3ce11a4611f577df16889160091df6e5140d36a6d06f60ebe3b934f03e4.
Native101 dark/83970 and102 light1.25/20828 each pass14 checks; actual devices
35/2050, all16 captures inspected, GTK logs clean and private secondaries removed.
Proof/source: shared-recovery-verification.json, shared-recovery-source.json and
shared-recovery-compiled-source.json. SHARED-RECOVERY-WIP.md records actual
red/green feedback and the next directory-Recovery tracer. All sessions terminal;
no agents or external blocker. Full goal and the entire Next work list stay active.
Full goal stays active; ADR0048's prior snapshot below is historical proof.

Prior verified checkpoint: ADR0048/schema16 strengthens archived
Recovery context with exact saved-container/owner steps and identified source
parents. Legacy15 path maps remain qualified Recorded location hints; data stays
previewable/copyable and the chooser avoids unproven initial parents. New public
regressions reproduce unrelated-file Undo and recreated-folder context hijacks;
exact keys and scoped parent/PRIMARY ownership fix them. Repeated equal path
pairs and normal history expiry pass focused tests. Renamed-folder alias tracing
also found stale older Copy ownership after crossFS return; captured committed
namespace provenance fixes it without weakening fingerprints. An indexed
ancestor-movement lookup replaces a scan of unrelated steps. See
CONTEXT-EPOCH-WIP.md for actual red/green feedback and the next shared-owner tracer.
Full core90660 passes282 parents/19 helper definitions. Pipeline74003 passes
all-target Clippy/default build/format with98 compiled source paths unchanged;
desktop SHA2ae680c7f0229f85f0d051bcd685b27c067d2598d10cb09528908c4af4659c59.
Native99 light1.25/78852 and100 dark/25771 each pass13 checks, actual devices
35/2050; all16 final captures inspected, GTK logs clean, private destinations
removed. Native97 initial dark also passes;98 light times out on remote Recovery
focus after a successful return. The fixture now explicitly selects the tile;
the application is unchanged, and remote activation focus stays separate work.
Proof/source: context-epoch-verification.json, context-epoch-source.json and
context-epoch-compiled-source.json. All current sessions terminal; no agents or
external blocker.
The full product goal and the entire Next work list stay active.

Prior goal turn classification: progress. ADR0047's identified file/link folder
Copy/Move integration passes276 core tests/19 parent-invoked helper definitions,
all-target Clippy, desktop build, format, syntax, inventory and whitespace.
Final pipeline38913 terminal 0;97 Rust/build paths unchanged. Desktop SHA256
f8b65103794d6ab8e8195f52d1aaa058617f90404cf7b63a9efb373102b59e07.
Native95 dark/95336 and96 light1.25/38868 each pass29 checks; all20 captures
inspected, logs clean, both private destinations removed. Proof/source:
saved-transfer-verification.json, saved-transfer-source.json and
saved-transfer-compiled-source.json. SAVED-TRANSFER-WIP.md records actual red/green
feedback, same/crossFS Merge, partial/retry, active/archived/expiry, raw/edited,
prior schemas, observed handoff kills and real destination reconnect. Root Keep
both now relocates saved context by the correct namespace role; receipts prevent
false missing-source failures after actual reconciliation. Full goal stays active.

Prior goal turn classification: progress. ADR0046's identified file/link saved-
container return scope passes268 core tests/19 parent-invoked helper definitions,
all-target Clippy, desktop build, format, syntax, inventory and whitespace.
Final pipeline62955 terminal 0;97 Rust/build source paths unchanged. Binary
e322f0b72d318941737d6bf6c295e372ffb3252a2d59e1ee122c9eb5617336a7.
Native93 dark/52811 and94 light1.25/86206 each pass25 checks; all18 captures
inspected, logs clean, private destinations removed. Source/evidence manifests:
stage-migration-source.json, stage-migration-compiled-source.json and
stage-migration-verification.json. STAGE-MIGRATION-DESIGN.md records red/green
feedback and journal design. Active/archived backups, changed-version review,
expiry, raw links, authentic v13 input, malformed-journal copy-out and observed
pre/post-catalog process kills are verified through public interfaces.
Occupied saved destinations now reach Skip/Keep both review and preserve the
unrelated occupant. The entire product goal and Next work remain active.


Prior goal turn classification: progress. ADR0045's recorded retry scope passes
core259 tests/18 parent-invoked helpers, all-target Clippy, desktop build, format,
syntax, inventory and whitespace. Native88 dark/3995 and92 light1.25/93858 each
pass21 checks; transfer regression90/99580 passes14. All33 final captures were
inspected. Binary7178de319160b4acb3289adf08dd243f36c90105688ed7b3eb3930f63825c846;
Rust/build snapshot94 unchanged. Proof: directory-retry-verification.json.
Light89/22638 received SIGTERM before completion;92 is the isolated repeat on the
same binary. No application fix or unsupported signal-cause claim. No extra91
run solely for singular wording:90 already exercises and renders1 change undone.

The packaged-link audit is fixed, with actual archive/extracted smoke proof,
24 shipped documentation files/39 offline links, all8 binaries matching, and both
examples validated by all3 owners with the relative Nord dependency. Proof:
package-docs-final-evidence.json; root ADR0017. This uses debug binaries and is
not optimized/hosted release evidence. Source documentation keeps repository
links; staged copies link to shipped guides and identify development references.

ADR0048 verifies archived-context ownership across unrelated file moves whose
parent paths coincide with an earlier saved-container movement, recreated source
folders, equal path pairs and expiry. ADR0049 then verifies identified shared
automatic expiry and extra-content preservation, with current/prior inputs,
parent aliases, copy-out and older Undo. Read SHARED-RECOVERY-WIP.md for the next
directory-Recovery tracer. Do not weaken physical ownership checks. Finish
unexpected stage contents and directory/special payloads, explicit disposal and
broader GC interruption/orphan handling, bounded long-
lived contexts, and already-decomposed legacy private-container returns. Source
Copy omission and forward handoff/Undo are now scoped proof under ADR0047, not
proof of those broader cases or persisted unfinished jobs. Read saved_stage.rs,
store/stages.rs, recovery.rs and store/directory/relocation.rs. Preserve journaled
progress and Recovery access; never adopt unrelated path lookalikes. Unowned
prefix-lookalike Undo Merge and understandable internal-collision wording remain.

Finish the entire Next work list: incoming/root/legacy availability and movement;
staged rollback/cleanup interruption; remaining batch/dependency/Trash and
directory Recovery/disposal/orphan/catalog work; durable jobs/progress/exact
restart; directory-FD/original-media/device authority; preview/action/clipboard/
drop/focus; combined narrow Downloads; FileManager1, optimized/hosted release;
full workspace checks, independent review and final commit. Remaining singular
labels in listing/Trash/remaining counts can be polished with the next UI work.
Do not declare full Undo or the product complete. Baseline remains
65e7cc245dc52607196bae6330f4a92da49918e2 on main; .git is read-only. Unrelated shell
research is untouched. The false heading finding remains closed.

All current sessions terminal. No agents or external blocker. Prior ADR0045
validation had two ENOSPC attempts, not behavior failures; current pipeline26438 passed after obsolete
Shell incremental cleanup and generated dependency preservation. Receipts are in
the directory-retry cache maps and incremental-cleanup.json. Preserve needed
symlink cache targets and compressed package maps; obsolete generated caches can
be regenerated from source. Native runs should be sequential until the isolated
SIGTERM's external cause is understood. Default incremental -j1 builds remain
appropriate; no Rust changes after final validation.
