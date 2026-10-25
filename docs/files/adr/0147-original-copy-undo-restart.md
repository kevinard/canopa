# Original Copy Undo consumes each namespace once

Status: scoped schema55 verification. Final source: copy-undo-inputs-11.json
(896 captured inputs). Continues ADR0146. The full
five-area Files goal remains active.

Two actual public Copy Undo worker deaths after metadata completion left an
empty temporary folder with no journal or cleanup owner. Both subsequent public
reconciliations succeeded while the folder remained. Source edits and a new
copied-path occupant survived. The old temporary stage had only device/inode
identity, so restarted cleanup could not safely adopt its current pathname.

Unchanged Copied steps without a saved previous destination now open the selected
copy and create their work folder through held original descriptors. Before any
rename, save UndoCopied and its separate per-lane copy_undo binding together. That
binding includes the original history generation, exact canonical change and
pending-task digest. Refused or ignored registration leaves the copy intact and
cleans only the creator's original empty folder.

Prepared has unconsumed execution rights. Taking consumes the visible source
name before its single live no-replace rename. Detached records the selected
original and its exact post-rename version; verified fingerprint reads occur
outside metadata transactions. Consumed retires payload deletion rights before
the single live unlink. Cleared and Preserved have no payload mutation rights.
Each transition verifies its original parent/root, canonical history, pending
record and independent binding before and after saving.

Restart can finish a still-original Prepared step or compensate it without
changing history. Taking never repeats a rename. If its selected copy still
occupies the visible name, only an empty work folder can be retired as
compensation; uncertain contents require Recovery. Detached can consume its
still-original, unchanged recorded payload once. Consumed never repeats unlink,
even if the same inode reappears in that name. Uncertain work remains protected.

Completion derives an empty-only UndoCleanup receipt from that bound task and
atomically commits reversal plus receipt. Cleanup never reopens the completed
copied path. A new failed-write test twice reproduced a shared completion gap:
SQLite ignoring deletion of changes allowed the original owner to retire while
the history row survived. Receipt installation now requires that the exact
history step is absent inside the same transaction, rolling back the whole
handoff if deletion was ignored.

Interrupted Copy Undo exposes its whole original work folder. A fresh private
Recovery review can archive that folder and retire its exact pending record and
binding atomically, leaving uncompleted original Undo history available. Changed
roots or stale reviews refuse Keep; serialized catalog rows carry no review
authority. Recovery Copy remains independently journaled and its export is
undoable. Unix filename and symlink-target bytes survive these paths.

Nine focused public parent tests pass, with one ignored child helper. They cover
actual deaths at eight boundaries, two restarts, safe compensation and fresh Undo,
new and reintroduced names, replacement roots/parents/items, missing or edited
proofs, refused/ignored initial and phase saves, completion and Keep transaction
rollback, Recovery Copy/Keep, export and original Undo, and raw symlink bytes.
Default all-target Clippy with warnings denied, Files formatting and schema55
fixture syntax pass. Backend coverage passes 560 parents across 20 targets (50 ignored helpers).
Source07 passed the library and 18 integration targets. Only the final integration
target changed: its old payload-only/automatic-return acceptance was updated to
whole-folder Copy, refused reconciliation, fresh Keep and export Undo. The complete
updated target passes on source11; every production and other validated test input
is byte-identical, recorded in copy-undo-source-bridge-01.json. Temporary test
diagnostics are removed. The normal isolated desktop-feature library passes 326 parents (30 ignored),
with private HOME/XDG directories, private D-Bus, no display or host activation.
Its receipt is continuation-core-1792838551-receipt.json. Final default all-target
Clippy with warnings denied, Files formatting and schema55 fixture syntax pass.
Exact source relations, attempts and protected binaries are recorded in
[the verification record](../../../.scratch/file-manager/copy-undo-verification-01.json).

The original orphan and ignored-deletion fixtures, plus repeated workflow and
test-ordering failure fixtures, are retained in three decoded and separately
live-verified archives (eight roots, 90 entries).
The final source/evidence archive and its independent decoded/live check are
recorded under `.scratch/file-manager/copy-undo-source-*`; exact next work is in
[COPY-UNDO-NEXT.md](../../../.scratch/file-manager/COPY-UNDO-NEXT.md). This does not finish Copy Undo with a saved
previous destination, changed-copy preservation, legacy/generic Undo and cleanup,
Trash/Restore, or any of the broader five-area requirements.
