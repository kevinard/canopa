# Trash queue checkpoint

Status: backend scope verified and preserved, 2026-10-19. Full product goal
active; Iced Trash/Restore interface pending. All known handles terminal.
Source03/584 inputs and archive members verified; archive SHA256
01e716220f26ce6c96911da3487cde6d730cfefa741ced320f10405383031832.
Core04 passes66 parents; final Trash target12 parents passes after equivalent
assertion cleanup; final Clippy has only3 existing library warning groups;
fmt and C strict syntax pass. Both final review axes0 unresolved/12 hashes.
Application remains resident binary8acb44cc; no new native proof claimed.
Authoritative next step/constraints: NEXT-EXECUTION-NOTES.md.
Exact attempts and checks: trash-queue-verification.json.

## Historical execution log

The following entries preserve the sequence of attempts. Their LIVE/pending
labels are historical and superseded by the closed checkpoint above.

# Trash queue work

2026-10-19. Previous goal turn: progress (resident host verified and preserved).
Full product goal active. Starting root ADR0084's backend Trash/Restore workflow,
then the required Iced views. Established public queue/filesystem seams apply.
Baseline resident-source-04/580 and binary8acb44cc remain preserved. No compiler,
native or archive handle live at start. No implementation delegation authorized.

Initial tracer: actual raw-byte file and final symlink Trash through OperationJobs,
typed completed result, actual Trash listing and shared Undo. Test authored before
implementation. New report/requests do not authorize replay or crash continuation.
See ADR0084 for intended ownership. Use CARGO_INCREMENTAL=0, --locked --offline -j1.

Update: core93540 terminal0 (27 parents + existing subprocess fixtures), Clippy63534 terminal0 (3 existing warnings). source01/583 and 9-path diff/scope recorded. Both read-only axes reviewed all9 hashes. Spec found P1 recovered completed move misreported as failure/invalid record; P2 outer/finalization error loses typed outcome and retains original Trash plan. Standards independently found dispatch-boundary Trash cancellation loses typed report. Must fix and re-review.

Additional19923 terminal101: attention3 pass, existing1024-capacity test fails101 rows vs100. request_source_retention target not executed because Cargo stops. Diagnosing publication gap: ranked hypotheses (1) Finished visible before trim (2) nonfinished extra row (3) duplicate IDs. New public snapshot regression reads concurrently and records only Finished IDs; temporary [DEBUG-trash-publication]25ms sleep widens the exact gap between update_job and trim. LIVE handle62284, trash-queue-publication-red-01.log. Do not edit inputs/start other compiler/native/archive/cache mutations until exact terminal. Remove probe before final acceptance. Source01 hashes now historical.

Next: atomic finished-row trimming with phase+attention publication; original capacity test and minimized regression green. Then preserve TrashOutcome through recovered/finalization failures, typed pre-execution cancellation, no executable stopped plan. Need actual system-boundary fault proof (consider private LD_PRELOAD fsync/SQLite fault fixture) rather than untested static fix. Final core/Clippy/fmt, renewed read-only reviews, source archive/receipt and authoritative NEXT notes still pending. Current executable remains8acb44cc resident build; no native proof for new backend claimed.

Publication62284 terminal101 with temporary gap probe: new concurrent public snapshot reader captured Finished IDs1..101, all unique. Confirms hypothesis1, excludes extra nonfinished/duplicate IDs. Removed [DEBUG-trash-publication] probe. update_job now publishes phase, attention and <=100 Finished rows atomically under snapshot lock; Worker.trim removes internal jobs absent from retained view. LIVE49181 runs full operation_attention (minimized test already green, original1024-capacity still running), log trash-queue-publication-green-01.log. No product edits until terminal.

Planned real fault regressions at system boundary: add tests/fixtures/trash_faults.c, built with cc into private /tmp/canopa-files-trash-fault-* directory, preload only isolated helper child. fsync wrapper fails once only on exact private source-parent directory after Trash rename; marker proves actual injected EIO. This reproduces P1 recovery commits then report omits move. C pthread_create wrapper can pause next worker only after private dispatch-arm marker; helper waits Running+started marker, sends Cancel then another durable admission (proves coordinator processed preceding Cancel), releases worker; deterministic P2 dispatch boundary. Never preload parent/normal application, guard /tmp prefix and raw exact paths. No production test hooks.

For finalization P2, use public queue to create100 folders, then external private SQLite connection adds BEFORE DELETE ON operations WHEN OLD.id=1 RAISE(FAIL,'private trim failure'). Queue next Trash: actual move/append succeeds, final history trim fails. Typed report must retain confirmed prefix/attention, no original executable plan, reopen after DROP TRIGGER. Queue starts before trigger so initial engine.trim unaffected. rusqlite already regular dependency.

Implementation direction after actual reds: richer InterruptedTrash owns outcome+ioError so outer failures preserve known prefix; error branch after successful recover inspects last_change(id) matching current source to count recovered committed move. Unstarted paths remain in result until settled. Queue stores typed interrupted/nonexecutable TrashResult, blocks new ordinary mutations when unreconciled, handles pre-dispatch Interrupted with cancelled report. Must retain old public engine error compatibility. Details still to implement; do not claim fixed.

Update:49181 terminal0 all5 attention tests, including original1024 and new concurrent snapshot regression. P1 real system-boundary regression90455 terminal101: private LD_PRELOAD source-parent fsync EIO fired after payload rename; recovery committed but reportStopped/failure. Matching recovered last_change counts committed move;71168 terminal0 actual regression+reopen/Undo. Cfixture root guard now uses sizeof literal prefix (30 bytes).

Finalization actual SQLite-trigger red34075 terminal101: after100 creates, triggerfails deleteold operation1 at finaltrim after Trash move; oldqueuePhaseFailed loses report. Added InterruptedTrash owning outcome+error, retained pending queue through outer failures, typed report.interrupted, nonexecutable fallback on Trash failures/panic and uncertainty gate.65890 terminal0 all7 Trash parents+ignored actual helper, including trigger finalization/reopen proof. No default binary update or native Iced acceptance.

CURRENT LIVE9129 -> trash-queue-dispatch-red-01.log. New private C pthread_create wrapper pauses exact next mutation worker after private dispatch-arm marker. Helper waits Running+dispatch-started, sendsCancel then durable next admission (serialized coordinator proof), releases worker. TEMPORARILY restored transfer-only cancellation guard in operation_jobs.rs interrupted_operations match to isolate genuine dispatch red; MUST restore (is_transfer || is_trash) after9129 terminal. Do not edit/compiler/native/archive/cache until terminal. Cfixture and Rust helpernewcode under tests; parentcc compiles preload, loaded only into private helper; no production hooks. After red restore, all Trash tests, appropriate regressions/Clippy/fmt, fresh reviews still required. Source01 manifests/diffs now historical; don't claim all findings closed.

Update:9129 terminal101 genuine dispatch-boundary red: cancellation acknowledged by coordinator before worker release, but typed report.cancelled false. RESTORED (is_transfer || is_trash) guard immediately after terminal. Snapshot/reopen characterization53823 terminal0; file name red-01 is not a failing result—existing durable request trimming already keeps reopen100. No extra load policy changed. Added RAII stop flag to concurrent reader test. Source mutation no longer has temporary probes or guard. Cfixture comment updated, finalization regression now also proves ordinary mutation blocked pending explicit reconciliation. fmt50966 terminal0. Current full five-target core command started below; exact handle recorded in next update. No new default executable or native acceptance.

LIVE51375: full five-target no-default core run -> trash-queue-core-02.log. No edits or competing compiler/native/archive/cache operations until exact terminal.

Current checkpoint:51375 terminal0 all62 parents/five targets;37289 terminal0 all-target Clippy3 existing; C -Wall -Wextra -Werror syntax0. Source02/584 archive1,640,805B SHA86df93c32d3d9192b5f32b16e246c2ebbc5ab2eadd45758d5bac7e06d2b7b340 and12-path diff/scope preserved. Standards02 zero/all12 hashes verified. Spec02 priorTrash andsnapshotfindingsclosed butnewP2 Restore mirrorsoldreportloss: coreRestore successfulrecovery returnsoriginalerror; finaltrimerror discardsknownsummary. Mustfix beforefinalcheckpoint. No compiler/native/archive/cache handle currentlylive. Source02 product inputs nowunfrozen forRestorefix; source02 docs/receipt stillhistoricalpendingfinal review.

Next new real tests: Cfsyncfixture add private fsync-arm marker so setupTrashdoesn'tfaultwhen testingRestore. Generalize helper action env (trash/restore) with same recover/dispatch modes. RestoreRecover mustactualrename thenEIOsourceParent beforecore.recovery confirms; existingqueuewillPhaseFailed despitefileexists. RestoreDispatch usesexistingprivatepausedworker; pre-execCancelthenlaterAdmission; oldguarddoesnotincludeRestore andwillFailed. Add publicSQLite trimtriggerRestore counterpart after99folders+setupTrash =>100engineentries, Restore101 triggersfinaldeleteid1. TestexpectedInterrupted+knownjob.paths restoredtarget.

Implementation direction: core InterruptedRestore owns Option<OperationSummary> +ioError, private restore_observed preserves actualsummary through recovery/trim while publicrestore ioResultcompatibilitykept. Queue typed RestoreReport/SavedRestoreResult(no executable stopped plan) with source/raw restoredpath/operation/error/cancelled/interrupted; snapshotrestoreoptional; preservefirstexecutioncancelreport anduncertainty gate. Need actual red/green, newfinalall62+Restoretests/Clippy/fmt, source03archive andrenewedreadonlyreviews. Do notclaim scopedcompleteyet.

Restore actual reds:48164 terminal101 private EIO after actual restoredrename ->oldPhaseFailed notFinished.75998 terminal101 filteredRestore1pass/3fail: recovery afterEIO, pauseddispatchcancel, privateSQLite finaltrim after99creates+Trash setup allfailasexpected. Core nowrestore_with_outcome +InterruptedRestore keeps OptionSummary andneeds_review; successfulrecovermatches exactChangeRestored original/current, finalerrorsretain knownsummary. Publicrestore keeps ioResult interface. Queue RestoreReport/SavedRestoreResult stripsstopped executableplan, distinguishesordinaryrefusal/uncertainty/cancel, extendsuncertaintygate, exposesrawsource/restoredpath.4083 terminal0 all11 operation_trash parents + actualprivatehelpermodes. New typed-reportassertions added; fmt73585 terminal0. Full five-target no-default core command nowrunning (exact handle in next update), log trash-queue-core-03.log. Productcode formatted/frozen whilelive; no othercompiler/native/archive/cache mutation untilterminal. FinalClippy/fmt/Csyntax, docs65testupdate, source03archive/receipt andrenewedreviewsstillpending. Currentdefaultapplicationbinaryunchanged8acb44cc; no newIcednativeacceptance.

CURRENT LIVE67436 full five-target core03. Code-only Restore delta03 scope6paths vsimmutable source02 captured for parallelread-onlyreview. No product edits untilterminal.

67436 terminal0:65 parents/five targets. Standards03 zero/six hashes; Spec03 new P2 known intent refusal incorrectly becomes permanent interruption. Added public leased retained-container Restore refusal followed by unrelated operation/fresh Restore. Actual79174 terminal101 reproduced Interrupted vsFailed. Intent errors now inspect pending journal: proven absent clears review/discards empty entry; remaining or unreadable journal preserves uncertainty.55827 terminal0 all12 Trash/Restore parents.66015 fmt terminal0; current Csyntax -Wall -Wextra -Werror terminal0. Code-only scope04 six hashes captured; both read-only axes reviewing. LIVE84087 full five-target core04. No product edits/compiler/native/archive/cache mutations until exact terminal; no new app/native acceptance.

84087 terminal0:66 parents (attention5, jobs12, Trash/Restore12, retention27, engineTrash10). Both scope04 axes zero/all6 hashes before/after. Current all-targetClippy75973 LIVE ->trash-queue-clippy-03.log; no product/archive/cache/native mutations until exactterminal. Docs/receipt/source03/final NEXT checkpoint remain. No updated app binary or native Iced proof.
