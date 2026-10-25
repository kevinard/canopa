# Saved-destination Copy Undo separates return from payload disposal

Status: scoped schema56 verification on saved-copy-undo-inputs-15.json (898 captured
inputs). Fifteen focused public parents pass. Final backend/default validation
below uses this same unchanged source.
Continues ADR0147; the full five-area Files goal remains active.

Two real public-worker deaths after Copy Replace Undo restored the previous file
and unlinked its copied payload reproduced a restarted second unlink. Reintroducing
the same copied inode through an existing hardlink satisfied the old identity and
fingerprint checks. Reconciliation removed that new namespace entry and its work
folder. The alias and later edits to the restored destination remained intact.
The generic Undo journal had no durable record that unlink rights were consumed.

Extend UndoCopied's independent original-history/canonical-task binding with a
selected saved entry and a separate return claim. The saved entry must match its
canonical original parent/root, selected generation (Ownership.previous or original
replacement facts) and strict saved version. Hold those actual descriptors before
registration. A separately created copy work folder keeps uncertain copied bytes
independent from the saved version still owned by canonical Undo history.

Taking consumes the current filename before its one live no-replace rename.
Detached records its original post-rename snapshot. Restoring consumes the saved
previous name before its one live no-replace return. Restored durably records that
return. Consumed independently retires copied-payload unlink permission before its
one live unlink. Each phase and its binding save atomically or leave the exact old
owner. Reads of file contents remain outside metadata transactions. Root and entry
snapshots are verified around native operations.

An explicit Use Saved Version review may select edited bytes or a new physical
file within the original saved root. Canonical comparison permits only the
reviewed version and Recovery flag to differ. During that guarded live decision,
capture the actual selected generation and bind the complete selected task
separately from unchanged canonical history. Restart requires that recorded
selection; it never recaptures a current path as approval. Without fresh review,
the original saved-entry generation remains mandatory. Existing edited-version
and replacement-version acceptance tests exposed both compatibility cases and
now pass, including process death after the approved replacement is selected.

Restart never repeats a Taking rename, Restoring return or Consumed unlink.
Prepared can finish or compensate before consuming either namespace. An uncertain
Taking or Restoring outcome preserves work for Recovery Copy/Keep and a new Undo.
Restoring may recognize its already-returned original object, including later edits,
without moving it again. Restored never reopens or verifies its completed target;
only the still-original copied payload can be consumed, otherwise it is preserved.
The absent-copy case registers the same saved return without inventing a copy
reader or granting unlink rights. Staging verifies the actual empty-root creator
through its held descriptors.

Commit completed reversal plus empty-only cleanup for both original work roots.
Unexpected contents or replacement roots preserve the receipt. All roots remain
observable for placement checks. Source reservations block Undo admission and each
consumption transition; an existing journal also blocks new retained-source leases.
Schema55's bound ordinary Copy Undo wire shape and binding remain compatible:
new saved fields are omitted when absent, and existing selected objects deserialize
as optional selections without changing their serialized shape.

A separate real Keep regression reproduced twice: after a recorded saved return,
Keep archived the copy but cleared its only pending owner while the old canonical
Undo row survived. Fresh private Keep now archives the exact copy root, commits
that recorded return and retains empty-only cleanup for the original saved root in
one transaction. The archive never receives deletion rights. Ignored/refused archive,
canonical deletion, binding retirement and receipt writes roll the whole handoff
back. If return is still uncompleted, Keep preserves the canonical saved version
and its Undo as before. Earlier Copy Undo follows the returned file after this
metadata handoff. Serialized/stale catalog rows grant no private Keep authority.

Legacy generic saved-copy journals also reproduced the repeated unlink after upgrade.
They cannot receive new payload deletion or rollback-rename rights. Exact canonical
history and original saved-root proof may establish a previously completed return:
the old previous name must be absent and its original object must occupy the returned
name. Finish only metadata with an empty-root cleanup receipt; a reintroduced payload
stays protected in whole-folder Recovery. Unknown or unfinished legacy claims require
review instead of being adopted from current paths.

A history-clone test then twice showed that valid root proof alone did not bind this
legacy metadata handoff to its original history file. Verify the canonical change's
saved ForwardProof history identity and generation before installing any receipt.
A replacement database cannot acquire another history's original-root cleanup owner.
Remove the old unsafe legacy unlink/rollback block and its now-unused store wrappers.

Focused acceptance covers two restarts, eight phase boundaries, missing copies,
returned-target edits and replacements, root/entry replacement, refused/ignored
phase/completion/Keep writes, Copy/Keep/export and older Undo, raw filenames and
symlink targets, source reservations, schema55 bound-journal upgrade, ambiguous legacy
payloads, replacement history and guarded replacement saved versions. The unfiltered
scoped backend suite passes612 parents across22 targets (53 ignored helpers), and
the normal isolated desktop-feature library passes341 (31 ignored). Both required
active/stopped batch restart gates, default all-target Clippy with warnings denied,
Files formatting and schema56 fixture syntax pass. The default receipt is
continuation-core-1792847076272108370-receipt.json. All898 captured inputs remain
unchanged; every validation process terminated successfully. Source/attempt evidence is under
`.scratch/file-manager/saved-copy-undo-*`; exact live status and next work are in
[SAVED-COPY-UNDO-NEXT.md](../../../.scratch/file-manager/SAVED-COPY-UNDO-NEXT.md).

This does not finish changed-copy preservation, unproved legacy return/disposition,
other generic Forward/Undo/cleanup, Trash/Restore, broader Recovery/expiry/queue/report
acceptance, backend navigation/search/preview separation, product/physical acceptance,
startup/focus/performance work, or native/workspace/Shell/Notify/Niri/package/release
integration and GTK retirement.
