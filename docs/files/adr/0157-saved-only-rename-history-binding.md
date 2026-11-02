# Saved-only Rename restoration keeps its original history binding

Status: accepted, 2026-11-02; focused verification recorded with session follow-ups.

The workspace gate exposed seven existing unavailable-move Undo failures. A user
can restore a moved item's previous destination while leaving its missing incoming
item alone. The selected return is represented as RetainedOnly, while its original
same-filesystem Move is a Rename row. Saved-return admission already normalized
Copied and paired cross-filesystem SourceRemoved rows for this operation; it did
not normalize Rename. Every such return was rejected as differing from its
original Undo history, including fresh version review and crash/restart cases.

Both saved-return and displaced-return admission now derive RetainedOnly from the original Rename's current path,
retained record and incoming identity. Its selected path, saved stage, ownership,
version and replaced identity must still match the exact derived change, subject
to the existing explicit fresh-review/relocation rules. The independent digest
still binds the complete original Rename row; it does not bind only the projected
fields or accept an arbitrary saved container. A related source-only Replace case omits the unavailable retained record from
its selected Rename task. Displaced-return admission now recognizes precisely
that omission while continuing to hash the complete original history row. The
existing live unavailable-version guard still authorizes the partial return.
Ordinary full Rename returns and the paired cross-filesystem binding remain
unchanged.

No schema or stored representation changes. The existing public unavailable-move
suite passes all twelve parent cases, including actual process death/restart,
changed saved-version review and preservation of older Copy/Recovery identities.
Related saved-destination/replacement/recovery tests and the workspace gate are
recorded in `.scratch/session-followups/PRD.md`. This does not claim completion of
the broader Files roadmap or relax saved-container admission.
