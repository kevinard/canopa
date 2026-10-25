# Undo return preparation retention

The full five-area goal remains active and unchanged. The previous turn made
verified progress; this turn fixes reproduced Undo partial data loss. No agents,
commits, vendor edits or main application/Niri rebuilds. Preserve unrelated work.

Current source: undo-abort-inputs-09.json (884 inputs), schema50. The predecessor
nested-access-inputs-09.json (882 inputs) and source archive SHA
da668ee1afd288a871619d71292787cee2579e26d4b0881d80c23609db2d0dd4 were reverified
before editing. Root/Files instructions/context and relevant staging/disposal docs
were read. New production module: operations/store/undo_work.rs. New acceptance:
operations/undo/abort_boundary.rs, with a cfg(test) actual copy-write death hook.

Actual red01/red02 on inputs01 kill public Undo after a completed cross-filesystem
Move, edit its first64KiB partial, then Reconcile. Both delete the unique bytes,
remove the work folder, clear pending and report Finished with zero garbage rows.
The original moved destination remains128KiB x; source/item is absent. Failure
roots are retained under /tmp/canopa-undo-abort-* and private crate-working-directory
.canopa-undo-abort-output-* folders. Never mutate them before preserving evidence.

Binding protocol: register_undo_work accepts only actual staging::Registration and
the exact original Undo SourceRemoved history step. It saves history generation,
operation/ordinal, canonical history hash, exact pending hash and actual source
Reproduction descriptor before any data write. Payload registration must extend its
own empty stage without changing source proof. Generic Store::intent may preserve
only the exact bound body. Primary clear retires undo_work atomically; Recovery Copy
uses its own lane and cannot clear the primary binding. Recovery requires original
work binding before directory access. Schema50 guards downgrade interpretation.

Reconcile preserves payload-bearing Undo returns instead of clearing their journal
and calling identity-only cleanup. A purported returned file must match its original
payload generation and complete source fingerprint; moving a64KiB partial to the
source cannot falsely complete Undo. The whole work folder is separately catalogued.

Fresh nonserialized Keep review exists for bound, non-displaced SourceRemoved Undo
work. It binds exact pending/binding/access bodies plus root/payload versions. Keep
atomically archives leaf and full work root as protected nonexpiring records,
verifies archive/index writes, clears only exact journal/binding, checks retirement,
and restores access. It never unlinks/moves files or grants transfer execution.
Underlying Undo history is unchanged. Displaced versions remain explicitly outside
this Keep scope for now; their payloads are preserved and need complete handling.

Focused results: green01 inputs02 passes the original catalog symptom. Green02
inputs03 passes4 parents but main test undoes its Recovery Copy instead of the
original Move. Green03 inputs04 passes5 parents but main test violates existing
newest-first Undo ordering after two exports. Inputs05 green04 passes6 parents;
new Keep-both case exposes binding comparison against the unrelocated stored path.
relocation-red02 reproduces that actual regression. Inputs06 green05 passes all7
parents and1 ignored child after normalizing only the recorded relocation for owner
comparison. Inputs07 adds catalog wording only. Main public workflow respects LIFO:
Copy the partial, Keep its whole work, Undo that export, retry the original Move's
Undo ID, then copy the retained whole folder with extra work. Data remains protected.

Source07 broad checks passed211 library tests and batches10..16, then batch17
exposed six real regressions in reviewed cross-filesystem Undo and conflict
preflight. review-red02 reproduced the canonical failure. Source08 normalizes only
explicit reviewed fingerprints/version, verifies the actual opened reviewed source
at private registration, and keeps the canonical hash over the original history.
Its live and killed reviewed returns pass; an early conflict check regressed missing
parent error ordering, reproduced by conflict-red02. Source09 moves that preflight
inside restore_source after original-parent validation and before return payload
preparation. conflict-green01 passes both original-occupant and parent cases.

Final validation is complete on source09's 884 unchanged build inputs:
526 backend tests/48 ignored across 20 targets (211 library/27 ignored; batches20..29),
292 normal isolated desktop-feature library tests/28 ignored, zero failures/filters.
Both required active/stopped batch restart gates pass in22. Default all-target
Clippy02 with warnings denied, Files format02 and fixture-check02 schema50/syntax
pass. The normal private runner receipt is
continuation-core-1792815539-receipt.json. Backend driver77570, Clippy89255 and
default30948 all terminated 0; no current test children remain from those runs.

verify-undo-abort.py records final results, attempt/source relations, protected
application/Niri/restart-gate hashes, and the existing12-root67-entry failure
archive. Final source and evidence are tracked by
undo-abort-source-01-verification.json; independently decoded/live checked by
undo-abort-source-final-live-01.json after the archiver terminates. Do not change
included files after archiving without preserving a fresh source record.

Failure fixture archive01 has SHA
cebbb267d15f5c62caef526e0beb9aa1d8f2efe4e5c51a09ed033513a79953b6,
12 roots/67 decoded and separately live-verified entries. Its scope includes the
early actual data loss, LIFO/test-owner mistakes and Keep-both regression. Broad
integration failures cleaned their private roots; no retained fixture claim is made
for those. No main application/Niri rebuild, native/workspace/release or physical
acceptance is claimed. The complete five-area goal remains active.

Next: complete generic Forward/Undo cleanup and automatic verified Undo partial
disposal, displaced Undo retention/rollback, and durable empty cleanup after a
completed reversal. Current clean_stage still has legacy identity-only unlink paths;
some remaining Undo paths retire metadata before cleanup. Also audit parent/source
generation checks through legitimate Undo relocations. Build real public feedback
loops before changing these. New Reproduction binding supports future limited
disposal; this slice does not claim that disposal or all Undo recovery is finished.

All broader Undo/expiry/queued lineage/large reports, Trash/Restore, backend
navigation/search/preview extraction, product/media/drag/device/accessibility/
physical acceptance, startup/focus/performance, native/workspace/Shell/Notify/Niri,
release and GTK-reference retirement requirements remain open. Other unproven
acceptance ideas from the prior turn: lease release token reuse stress and active
permission-context relocation/copy. Do not claim findings without reproducing them.

Use CARGO_INCREMENTAL=0 TMPDIR=/tmp, -j1 --locked --offline. No Rust wrappers.
Never overlap Cargo mutations or replace a test ELF while current_exe children use
it. Reclaim only audited exact completed test ELFs under both Cargo locks. Preserve
app/Niri, the required operation_transfer_resume gate, source and prior evidence.
After terminal backend/default results, update docs, verify manifests/results,
archive failed private roots and final source/attempt evidence, decode archives and
verify live hashes after archiver termination. Use fresh artifact names.
