# Undo when a moved or renamed item cannot be inspected

Status: accepted under delegated implementation authority, 2026-10-16;
implementation and acceptance in progress.

Interactive Undo of a same-filesystem move or rename now distinguishes an
available replacement from an item that cannot be inspected. Its question
contains the recorded location, original location and actual lookup error.
`UndoItem` carries either an actual directory entry or an unavailable path and
error; missing items never receive invented sizes or modification dates.

- Skip leaves the step available for later and continues other batch items.
  It is the safe Return-key default. Cancel and Escape stop remaining work.
- Finish this step explicitly removes this incoming item from Undo without
  returning it. Any saved previous destination becomes protected Recovery data.
  No filesystem object is moved or deleted by this decision.
- Restore previous destination is offered when the incoming lookup reports
  NotFound and an inspectable saved file or link is available. It restores the
  saved version shown in this review, while leaving the missing incoming item
  wherever it is. The original source location is not overwritten.

Restoration approves the captured current saved version, including an edited or
replaced backup. It rechecks the saved stage and exact captured version after
the answer; a stale review refuses mutation. Finishing also rechecks the lookup
error before changing history. If the incoming item reappears, the user reviews
Undo again. Skip followed by reappearance of the original object permits normal
Undo. A permission error is not treated as proof that the path is empty.

Finishing archives the protected saved destination and consumes the step in one
database transaction. Keeping an item, finishing an unavailable step, and
restoring only its saved destination do not grant older incoming-file history
ownership of another file. The original completion resets proved insufficient
when another program made an earlier move absent from history. [ADR 0030](0030-history-identity-ownership.md)
replaces those path-wide aliases with ownership captured before each operation.
Every later Undo still checks its expected physical object before acting.

Saved-only restoration uses the existing retained-only Undo intent. Namespace
and Recovery relocation follow the actual reversal recorded in that intent,
rather than assuming that the original move returned its incoming item. A
directory-link regression reproduced an archive being relocated back to an
original location where the link had not returned. Public Recovery now keeps
the archive's actual recorded namespace; restoring the link there makes its
saved children recoverable again. The original UI used schema 6; the subsequent
identity-ownership correction in ADR 0030 upgrades history to schema 7.

Operations reports unavailable incoming items not returned separately from
actual reversals and kept existing items. Restoring a saved previous destination
can therefore report a completed reversal and an incoming item not returned.
Finishing alone reports zero changes undone. The earlier operation is labelled
as finished with items not restored, rather than simply Undone.

Public filesystem coverage includes Skip/Cancel/restart/reappearance, protected
Finish through history expiry and recovery copy-out, permission errors, batch
continuation, stale incoming and saved-version reviews, explicit restoration of
an edited saved version, archive namespaces beneath a directory link, and the
older independent-copy regression. A 500-file process fixture is killed after an
actual saved destination returns, before the Undo call returns. Restart finishes
the saved-only restores and older Copy Undo removes only those returned previous
copies; incoming items moved elsewhere and the older source tree remain intact.
This is batch process-interruption coverage, not an instruction-level crash hook.

Desktop storage preflight can still pause an unavailable-device request before
the engine starts. These choices do not establish original-media identity or
replace the outstanding in-flight placement and directory-descriptor work.
Unavailable saved versions, cross-filesystem reversals, folder Merge, broader
dependency/batch decisions, directory recovery, bounded identity/catalog work
and the rest of the product brief remain open. Verification evidence is recorded
in the implementation ledger as acceptance completes.

The earlier schema-6 UI workflows passed 73 native checks and 144 core tests,
before adding the external-move ownership regression. That regression is now
fixed by ADR 0030 alongside the existing tracked-adoption cases. Current schema-7
verification, including a new native external-move workflow, is recorded separately
in the implementation ledger. Earlier binary evidence does not establish the
new implementation's acceptance.
