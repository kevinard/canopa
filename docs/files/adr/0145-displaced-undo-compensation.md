# Durable compensation of interrupted Undo Replace

Status: schema53 implemented and validated on 891 unchanged source inputs.
Continues ADR0142–0144. The full goal stays active.

Two public-worker crashes reproduced unchanged Undo Replace work remaining blocked
until manual Keep. The original source proofs and partial-prefix bytes were valid,
but the recovery guard excluded displaced work. A later missing-payload test twice
reproduced the legacy fallback moving a reintroduced previous-name hardlink again
after a completed return, then retiring both journal and binding. Object generation
did not prove that a directory entry could be consumed twice.

Extend verified copy disposal while keeping the independently bound original Undo.
Before disposal, prove the original displaced root/item and reproduce the return
copy from its captured source. The existing Reserved/Claimed/Consumed copy protocol
still consumes its copy namespace once. Save CopyCleared after its work roots retire.

Return displaced work through a separate original holding folder and explicit
Skipped, Reserved, Claimed, Consumed and Returned states. A private creator token
binds the new folder to the selected original item. Claim previous into that folder,
sync both directories and persist Claimed. Save Consumed before the live NOREPLACE
return through pinned parent/root descriptors. Keep strict source snapshots across
that save. A restarted Consumed record never repeats the rename. A verified target
permits a Returned receipt, whose only remaining rights are empty-root cleanup.
Later target edits or replacement occupants cannot be moved or refreshed by cleanup.

Missing or not-yet-created copy payloads use explicit empty-copy compensation.
This state grants no copy-file unlink rights and keeps the same original Undo
owner. With no copy stage, it references the existing displaced preparation and
skips treating that folder as disposable copy work. Save the return outcome before
retiring its root. This covers initial preparation, a removed payload and a payload
disappearing between the guard and folder open. It removes the SourceRemoved path
through legacy repeated return, without adopting current replacement files.

Every phase advances journal and independent binding atomically. Refused or ignored
saves retain their exact earlier owner, including failures after actual return and
at retirement. The underlying Undo history remains unchanged by compensation; a
new Undo request gets a fresh conflict answer. Completed earlier history steps and
the moved destination remain intact.

Recovery snapshots and Keep now cover up to four work folders. Existing serialized
Pending slots 0–7 retain their meanings; additional parts use slots 8–15. New entries
in consumed source names stay available for inspection, Copy and Keep. Retention
archives available leaves and whole original folders in one checked transaction,
preserving extra children. It grants no operation execution. Schema53 guards the
new return states and additional pending roles against older executables.

Focused public acceptance passes 26 parents, with actual deaths at return claim,
consumption, publication and cleanup boundaries; same-inode and foreign name reuse;
new entries in the consumed previous name; failed phase/receipt/retirement saves;
later target occupants; replacement roots; extra-slot Recovery Copy; Keep, export
Undo and original Undo retry; and empty/disappearing payloads. One failed case was
a harness assertion expecting a foreign replacement root to appear as owned data;
the corrected test proves refusal and access after the original root returns.
Final validation passes 545 backend tests across 20 targets (49 ignored helpers)
and 311 normal isolated desktop-feature library tests (29 ignored), with zero
failures or filters in the parent runs. Both required active/stopped batch restart
gates, default all-target Clippy with warnings denied, Files formatting and schema53
fixture syntax pass. Exact attempts, source relations and the isolated default
receipt are in
[the verification record](../../../.scratch/file-manager/undo-rollback-verification-01.json).
Twelve failed fixture roots retain 74 independently decoded/live-verified entries;
the original fixtures and application/Niri binaries remain preserved.

This does not finish completed-reversal cleanup or other generic/legacy Forward,
Undo and directory-return paths. Broader Undo/expiry/queued lineage/large reports,
Trash/Restore, backend separation, product/media/drag/device/accessibility/physical,
reliability/performance and native/workspace/release work remain open.
