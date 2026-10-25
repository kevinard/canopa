# Selected saved returns consume their original name once

Status: schema58 implementation, superseded by the schema59 preparation work in
[ADR0151](0151-original-undo-displacement-preparation.md). Six public crash parents
pass under Cargo on `move-return-inputs-01.json` (902 inputs), along with proper
default all-target Clippy and formatting. Its five related integration targets
finished with 56 passes and three failures: two generic Undo Replace creation
cases and the reviewed saved-return permission/Recovery flow. Follow-up fixes and
current-source checks are recorded in ADR0151. The full Files goal remains active.


A deterministic one-item Move/Replace worker is killed after its incoming item
and saved previous destination have actually returned. Recreating the consumed
previous name through an older hardlink makes the generic journal reclassify the
completed return as RetainedOnly. The no-recreated-name control retires it. All
bytes survive, but the old name receives another logical return opportunity.
The original public20-item probe and the new exact-boundary test reproduce this.

After the actual incoming return, prepare a selected saved-return task before
renaming its previous entry. Capture the actual opened parent, original work
folder, selected entry and strict snapshot. Its receiving parent must have the
same original generation as the recorded saved-folder parent. Registration binds
the original history-file generation, unchanged canonical Undo step and complete
selected task independently. Fresh reviewed version changes and Keep both names
have explicit canonical normalization; current paths do not replace old proof.

Prepared has not called rename. Returning is persisted before the one live native
rename. Restart may recognize the selected original entry at its destination and
record Returned, even if the consumed previous name has been recreated. It never
replays that name from Returning. Unproved completion preserves the task and its
files for Recovery. Returned commits metadata and hands original empty-folder
cleanup to the existing durable receipt. A reused name or unexpected extra child
is retained by that receipt, with fresh Keep available through Recovery.

A private nonserializable Keep review binds the exact task/binding and the current
versions of its original saved folder and any displaced folder. Refreshed Keep
archives both whole folders before retiring their journal and canonical step.
It records only the actual incoming return, without claiming an unproved saved
return. Stale, cancelled, serialized or replacement-root reviews cannot authorize
this handoff. Recovery Copy remains read-only with respect to these roots.

A further real worker-death regression first reproduced a completed-return metadata
handoff reopening the source name after its original item had been moved elsewhere.
An identical replacement acquired older Copy Undo ownership and was removed. This
occurred only in the private fixture. The task now carries the returned identity
resolved before the live incoming return. Completion and Keep apply the recorded
source-owner context directly; they do not read the completed source pathname.
Saved-destination context already follows the selected retained identity. Copy
Undo bypasses the new return-context lookup to avoid repeating its earlier batch
serialization regression.

Permanent public tests cover completed-return edits with and without a recreated
old name, interrupted consumed names, stale/cancelled/serialized Keep, replacement
saved roots, cloned history followed by original-history reappearance, and an
identical replacement at the completed source with older Copy Undo. They use real
public operations and actual SIGKILL, with retained failed fixtures and immutable
private candidate/source evidence under `.scratch/file-manager/move-return-*`.

This does not finish generic incoming Move continuation, legacy journal disposition,
or generic Undo displacement creation. In particular, the source57 additional-pass
failures are public Undo Replace roots still created through the legacy Stage
constructor without original generation proof. Upgrade those roots from their
actual reviewed descriptors and creator, together with independent task binding;
do not weaken saved-container admission. Remaining Copy/Move/Trash/Restore/Undo and
cleanup acceptance, backend separation, product/device/accessibility checks,
startup/focus/performance and complete release integration remain open.
