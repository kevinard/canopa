# Superseded checkpoint pointer

ADR0050 scoped proof remains historical at recovery-copyout-verification.json.
Its continuation follow-ups are now verified in ADR0051 and
recovery-continuation-verification.json. Read RECOVERY-CONTINUATION-WIP.md first
for current terminal state, next tracers and the still-active entire PRD goal.

---

# Latest checkpoint — verified scoped progress, entire goal active

All handles are TERMINAL; no agents or blocker. Do not duplicate old builds or
mark/pause/narrow the entire PRD. Baseline main65e7cc245dc52607196bae6330f4a92da49918e2.
Root and Files instructions/context already read; diagnose/codebase-design skills
already applied. .git read-only; user data never mutated by automated tests.

Final proof: recovery-copyout-verification.json (schema17),156 source paths,
100 Rust/build hashes unchanged, binary23d671d2c65f4059283d54e0418c0a8baea9b0aaa97a91f15bec2c9883ebb8b8.
Core75754 terminal0:300 parents/20 parent-invoked helper definitions. Previous
core68938 also passed before mechanical lint cleanup. Clippy/build/fmt60230
terminal0. Native105 dark24081 and106 light1.25/82890 each13 functional checks;
ordinary Cut/Retry107 dark10026 passes20 functional checks plus private-volume
cleanup on actual devices35/2050. Total47 native checks. All27 captures inspected
(contact sheets plus full dark04/06, light07, retry09); all GTK logs clean.
Packaged docs24/39 links, Python syntax, inventory and final whitespace pass.
ADR0050 and current implementation ledger record scoped verification. Entire
file-manager goal remains active; no optimized release/workspace/final review
claim. Do not reopen the closed ancestor-heading finding (preview occlusion).

NEXT: concrete public/native tracers for (1) a refused Continue after output-root
replacement retaining its native task/button, then (2) selected Active Recovery
key expiring to Archived during or between partial-copy attempts. Detailed
source-review hypotheses and likely seams are at the end of the historical notes
below. Reproduce before fixing; preserve exact plans, source observation and
truthful completed-work accounting. Root-install/read-error interruption gap
also remains unproven; no speculative fix yet. All full PRD ledger work remains.
New artifacts use recovery-copyout prefix; do not overwrite older ADR0037
`directory-recovery-*` evidence. Its historical core log was accidentally lost,
explicitly marked unavailable in directory-recovery-verification.json. Current
proof is independent and exact for its recorded source.

---

# Directory Recovery copy-out — active checkpoint

Entire unbounded file-manager goal remains active with delegated decisions.
Do not narrow it, mark complete or pause. Root/main baseline
65e7cc245dc52607196bae6330f4a92da49918e2; .git read-only. No agents/blocker.
Skills diagnose/codebase-design already applied. All user data tests private.

## Current implementation

ADR0050, history schema17. Recovery reuses recursive transfer execution through
an opaque RecoveryCopyContinuation, carrying original record/version and exact
remaining targets, directory IDs and destination-parent ID. Cannot use ordinary
Primary resume. Separate Recovery journal admits created-folder intents; copy-out
does not replay the original interrupted journal. Protect all original pending
paths/aliases plus source stage. New output root required; never Replace. Only
captured owned roots can auto-Merge on resume. Children KeepBoth/Skip/Merge.
Root observation changed => Refresh. Original known ancestor rename remaps source
plan only after selected record and observed version proof. No atomic deep-tree
snapshot, pinned directory/media authority or persisted exact job claim.

Desktop jobs now share progress, questions, partial failure status, Undo,
Continue/Retry via generic TransferOutcome<Task>. Recovery tasks/continuations
stay typed; clipboard/drop normal transfer behavior preserved. Directory Recovery
controls enabled. Recovery source preflight re-fetches record after known ancestor
movement. Display source hints never enter continuations/checks. Recovery conflict
buttons omit Replace; core independently rejects it.

## Evidence and live state

Red97973 terminal101: file-only adapter rejects protected saved directory.
recovery-copyout-red.log. First fix85160 terminal0: initial directory1, Recovery6,
interrupted1 with2 invoked helper definitions (recovery-copyout-first-fix.log).
55175 terminal0: directory8 and history upgrades5 (v4 through authentic16 inputs),
recovery-copyout-continuations.log. Desktop cargo check3630 terminal0,19.66s,
recovery-copyout-desktop-check.log.

Interruption87843 terminal101 only from new collision test mistakenly rejecting
Progress as well as Conflict; fixed callback now rejects only Conflict. Same run
passed2 interrupted parents and9 directory parents, including actual process kill
with partial bytes and completed prefix Undo. recovery-copyout-interruption.log.
Full core68938 TERMINAL0: cargo test --offline -p canopa-files
--no-default-features -j1; recovery-copyout-core.log. It passed300 tests and20 parent-invoked helper definitions. All other handles terminal,
including doc-writing11051. Rust/source snapshots recovery-copyout-compiled-source
and recovery-copyout-source created; compare hashes after validation.

New maintained native case --recovery-directories-only plus dark/light inventory
entries uses Copy Replace, private saved file→directory, exclusive new output
child arrival, conflict Escape, Operations Continue, safe KeepBoth, nested bytes,
360px browser/Recovery and floating preview, separate Undo preserving new work.
Python compile/inventory list pass. Native not run; old default binary is still
3bdcd3ce11a4611f577df16889160091df6e5140d36a6d06f60ebe3b934f03e4.
Next native numbers103/104; output prefix native-recovery-copyout. Source idle
until core terminal. Then Clippy --all-targets -Dwarnings, default build, format;
fix any failures sequentially and revalidate affected checks before native.

## Remaining immediate work

Finish full core, Clippy/build/static proof; run new native dark+light sequentially,
inspect all captures/logs and update ADR/ledger/evidence. Consider actual child
read-permission retry. Known unproven root-install window: a read/sync error after
created root commits but before directory-map capture might leave continuation
unable to recognize its root. Needs public observed-boundary tracer before a
speculative fix; do not claim all reconnect/interruption boundaries are solved.

Broader entire PRD ledger still open: explicit protected disposal, additional saved
contents/private selection, legacy decomposed returns, cleanup/orphans/catalog
bounds, durable jobs, pinned original-directory/media authority, Trash/Restore
batches/crossFS, preview/actions/clipboard/drop/focus, combined narrow Downloads,
FileManager1, optimized/hosted release, full workspace check, review and commit.
False ancestor-heading finding stays closed (QuickLook occlusion).

## Previous verification and artifact integrity

ADR0048+0049 shared recovery proof is historical once these new edits begin:
shared-recovery-verification.json,153 paths/99 Rust-build paths, core289/19 helper
definitions, Clippy/default build/fmt, native101 dark and102 light1.25 each14.
All16 captures inspected, logs/cleanup clean, actual devices35/2050. No current
completion claim derives from that old binary.

The older ADR0037 owns directory-recovery-verification.json and its build/focused
artifacts. New full-core command accidentally reused directory-recovery-core.log;
its still-live inode was renamed to recovery-copyout-core.log without restarting.
Historical schema8 core log was overwritten and is no longer available. Older
verification JSON now marks that loss explicitly; recorded counts193 remain
historical but cannot be independently substantiated from that missing log. New
inventory log similarly uses recovery-copyout prefix. All other old artifacts
remain. Do not overwrite them or infer historical results from current logs.

Follow-up hypothesis from source review, not yet reproduced: if an active saved
record is the oldest of100 history entries, a partial Recovery copy adds entry101
and trim may archive its selected Active key. An in-memory continuation currently
retains that old key, so its next lookup may fail despite identical protected
bytes being available under an Archived key. Reproduce via99 public CreateFolder
operations before partial copy, and also history expiry between attempts. Any fix
must rebind only the same identified saved container/observed version through
catalog ownership, never adopt by matching filename. Existing streaming stage-owner
index may provide that seam. No false promise of continuation across owner expiry
until tested. Current source is unchanged while68938 runs.

Full core68938 confirmed terminal0. Validation script recovery-copyout-validate.py
now runs Clippy/default build/format sequentially; check build-state JSON and live
handle before any Rust edit.

Clippy47957 terminal101: ten redundant reborrows left by transfer extraction
and one identical conflict-choice branch. Corrected these mechanically and
formatted the Recovery check closure for readability. Core300 snapshot preserved
in recovery-copyout-pre-clippy-compiled-source.json; final compiled manifest
updated after these changes. New validation60230 RUNNING; final full-core rerun
should follow for exact final-source proof. No behavioral code was changed by
the lint cleanup. First Clippy log/state preserved with -first suffix.

Validation60230 terminal0: Clippy52.45s, default build64.27s, format6.23s.
Binary23d671d2c65f4059283d54e0418c0a8baea9b0aaa97a91f15bec2c9883ebb8b8.
Native103 dark now starting with private-only --recovery-directories-only.
Do not overlap native sessions. After both native runs, repeat final-source core
and inspect captures while that compiler is active (no Rust edits).

Native103 dark68357 terminal1 after successful conflict Cancel/Continue/KeepBoth.
Fixture typed a filename containing Space, which invokes QuickLook; final Space
then closed it. Failure capture has the expected b-note(2) row selected, no GTK
warnings/panics, and tab.rs dispatches commands before typeahead. Fixture now
uses Home/Down/Down/Space for that directory-first row. No app change/rebuild.
Next native104 dark and105 light1.25, sequential.

Native104 dark75471 terminal1 after preview and first Undo pass. Final review's
Return correctly chose safe Skip and reported Partly undone, while fixture wrongly
waited for Completed. notifications.json and existing default-widget code confirm
this; fixture now explicitly selects Alt+K Keep folder. No app change/rebuild.
Next105 dark/106 light1.25. Both failed runs retained in full.

Native105 dark24081 terminal0 and106 light1.25/82890 terminal0 each13 checks.
All16 captures inspected via contact sheets; dark04/06 also inspected full size.
Source filename hints, no Replace, KeepBoth default, actual recovered text,
nonempty review and preserved newer output are visible. GTK logs clean.
Final-source full core75754 RUNNING in recovery-copyout-final-core.log.
No Rust edits during it. README now explains folder copies/cancel/continue and
separate Undo. ADR/ledger record native scope; finalize evidence after core.

recovery-copyout-verification.json records current native/build/static proof and
marks final core75754 running. Packaged docs check passes24 files/39 links;
compiled100 source hashes and binary23d671d... unchanged. One worthwhile native
regression after final core terminal: --undo-cross-only --undo-cross-case retry
(native107 dark) exercises ordinary Cut/Retry clipboard ownership through the
now-shared generic UI outcome mapping. Do not overlap it with another native
session or compiler. Then finalize ADR/ledger/proof/source manifest.

Final core75754 TERMINAL0:300 parents/20 invoked-helper definitions, no failures.
Native107 ordinary directory Cut/Retry regression is now RUNNING, log/output
native-recovery-copyout-regression-107-dark. No other live handles. All compile
and directory-native verification completed on final compiled source. After107
terminal, inspect captures and finalize ADR/ledger/JSON/manifest; no Rust edits
needed for this scoped result. Full goal remains active.

Additional source-review follow-up (not yet reproduced natively): output-root
replacement is correctly refused by the core with WouldBlock, and its public
test can reuse its privately held continuation after restoring the folder. The
UI consumes Task::ResumeRecover when enqueuing; a later Err goes to generic Failed
without retaining that task. So a refused Continue may lose its Continue button.
Reproduce this through the native cancel/conflict fixture before changing code.
A likely deep fix is returning a paused RecoveryCopyOutcome with the exact typed
continuation for a foreign output root, instead of Err, since nothing started.
Changed-source observation must still require Refresh. Destination-parent changes
already return paused outcomes through protected_check. Do not blindly wrap all
late engine errors as PreparationFailed/request-not-started: some can occur after
completed writes. Keep truthful partial-work accounting. Prioritize this and
selection-key expiry as next concrete Recovery tracers after current scope proof.
