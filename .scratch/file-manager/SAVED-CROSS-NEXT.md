# Continue actual cross-filesystem saved-container restart recovery

The full five-area goal remains active. This turn made concrete production
progress: pinned forward copy, descriptor-only metadata restoration, and created
target-generation ownership preserved in catalog cutover. ADR0134 and
SAVED-CROSS-WIP.md record exact evidence, including two corrected fixture mistakes.
Do not repeat that investigation or re-derive the callback problem. The current
foundation is captured in foundation-inputs01 (871 paths). Metadata substitution
after the last check and the move/three-Undo integration now pass.

The actual public-worker completed-cross gate is STILL RED on the current source
(current-restart-red01): uncovered operation#2. It is unignored in source and was
explicitly filtered from the 152-pass/18-ignored library run. The whole saved
integration target passes 24/1; Clippy and fmt pass. Do not call this complete
restart recovery or ignore the gate permanently. No complete backend/native/
workspace/release acceptance. No schema change yet (43).

Next implement the strong migration publication/source-cleanup and exact task
binding together. Read SAVED-CROSS-DESIGN.md, saved_stage.rs, store/context.rs,
store/stages.rs, transfer/cursor.rs and store/transfer_cursor.rs. Current legacy
recover/cleanup_source still reopens names and uses scalar identities/fingerprints;
merely binding that code to a cursor would authorize unsafe restart mutation.
Preparation uses handles, but does not pass them into those mutation helpers;
do not assume File fields necessarily drop before the helper call.

Prefer a small safe explicit state machine. A promising simplification is to
claim the whole original source container to a private sibling after catalog
commit, prove it before RootClaimed, then claim previous->folder inside that held
root before FileClaimed permits unlink. Claimed absent names never inspect the
old public source namespace, even if the same original inode reappears there.
Only the original empty claimed root can be removed. Extra children/foreign
payloads/parents/devices refuse; no recursive removal. See design notes for
proof/phase obligations, preserving source through cutover and unresolved Preparing.

Generate full original source parent/container/previous and target parent/work/
created-payload evidence from actual held verifiers. Bind immutable recipe and
counted/bytes to the actual head before publication. Keep ordinary Move and saved
copied matching separate; canonical context/receipt cannot be minted from current
paths or legacy scalars. Context.prepare already special-cases StageMigration
forward through Forward(stage=None); require_forward ignores Migration and demands
next==ordinal for uncommitted Forward. After cutover its original context is stored
in the committed Change and pending bindings are cleared. Use that original
committed context, not fresh owner capture, for remaining cleanup phases.

Keep atomic active/archive/source-reservation cutover, metadata-save retry, nested
zero counts, actual copied bytes (links zero), Keep Both targets, chronological
Undo and older data. The new Ownership.as_saved_copy requires its existing payload
generation; rebind updates Some only, retaining an old strong proof if a legacy
target has no proof. Never downgrade to legacy scalar cleanup rights.

Use real public workers killed before/after publication, catalog cutover, source
claims/unlinks and task update, plus a second crash. Test new source occupants,
identical-inode namespace reuse, original parent/work/payload/device substitution,
missing/corrupt/borrowed receipts/context, metadata rollback, raw links, nested and
batch counts/part orders, older Undo and source reservations. Preserve failure roots.

Preserve all prior archives and protected app/Niri/local-runtime restart gate.
No commits or vendor edits. No subagents without explicit authorization. TMPDIR=/tmp,
CARGO_INCREMENTAL=0, -j1 --locked --offline. Disk is tight. Reclaim only completed
own test ELFs with the audited helper and fresh audit file; never the required
operation_transfer_resume executable. Finish rustfmt in a separate acknowledged
call before capture. Do not delete source/proofs/archives or unrelated dirty work.

Complete broader Preparing/Cleanup provenance, weak legacy/queued lineage,
large reports/paged owners, Undo/relocation/expiry, Trash/Restore, then the rest of
the original goal (backend separation, product/device/accessibility/physical
acceptance, reliability/performance, native/workspace/release/Shell/Notify/Niri
integration and GTK retirement). Do not redefine success around this foundation.
