# Review a replacement before returning it during Undo

Status: accepted under delegated implementation authority, 2026-10-16;
implementation in progress.

A file or folder can take the place of a moved or renamed item. Interactive
Undo now shows that actual item's name, type, size and modification time, plus
the original location, before deciding what to do with it. The review offers:

- Return this item explicitly moves the reviewed replacement to the original
  location. It does not locate or move the original item if another program
  moved it elsewhere.
- Keep item leaves the replacement here and finishes this part of Undo. Any
  saved previous destination becomes protected Recovery data.
- Skip leaves the history step available for later and processes other items.
  Skip is the safe Return-key default. Cancel and Escape stop remaining work.

Edits to the same moved item do not require this replacement review: moving
that object back preserves its current contents. If the original location now
contains newer work, the usual two-item conflict still asks Replace, Keep both
or Skip. Returning a reviewed replacement does not authorize overwriting that
occupant. Replaced copies and changed saved destinations retain their separate
reviews in ADRs 0018 and 0026.

Return is an atomic same-filesystem rename of the selected node. Folders keep
their contents, symlinks keep raw target bytes, and special nodes such as FIFOs
are renamed without opening their streams. The captured version is checked
after the review and again after any later questions, before journaled mutation.
The final rename checks its identity and moved metadata. A displaced hard link
can legitimately change ctime during the operation's own extraction, so that
phase uses the moved-version comparison after the full preceding check.
These guards do not replace the remaining directory-descriptor/race work.

The logical identity in history remains the original item's identity. A chosen
physical version is recorded separately in the durable Undo intent. Completing
the return binds the restored object to the older logical item, so older Undo
continues to refer to it. Startup uses that chosen physical identity when
recognizing a completed return. If the source has returned but its previous
destination cannot yet be restored, both parents and the saved stage are synced
and the chosen identity bound before recording the retained-only remainder.
Its backup stays protected under ADR 0026's history-expiry policy.

Keeping is a database transaction rather than a filesystem deletion. It archives
the saved previous destination with its context and protection flag, consumes
the kept change and any same-operation created containers that must remain, and
clears the intent together. There is no adoption binding for the kept replacement.
Older history cannot silently treat it as the item that originally moved.

Identity bindings also carry a revision. A forward change captures the current
revision and follows only bindings made afterwards. This prevents an old
adoption from overriding a newly recorded move of the original physical item.
Each returned physical identity becomes a terminal binding at its new revision,
including when an earlier physical item is selected again. Keep both relocates
older bindings without overwriting newer bindings at the selected return path.
Restoring a saved previous destination rebinds both its own logical identity and
the replaced incoming identity to the restored version. A retained-only remainder
preserves that incoming identity so this also works after a partial return and
restart. These rules let older history follow a restored previous destination
after several replacement adoptions, without creating alias cycles.

Schema version 6 adds this binding provenance, an optional reviewed version to
Rename records, and an optional replaced identity to retained-only remainders.
Ordinary records omit the optional fields, retaining their prior wire format.
Versions 1–5 migrate in the existing transaction; version 4's derived indexes do
not need rebuilding. Existing changes precede existing bindings on migration.
Older binaries refuse version 6 before interpreting the new fields or tables.
Configuration schema and ownership are unchanged. Further bounds on the retained
identity catalog remain part of the outstanding history scalability work.

Public filesystem coverage includes Skip/Cancel/restart, older history after an
explicit return, separate Keep both/Replace collisions, changes during either
question, directories and raw-target symlinks, FIFO return, hard-link collisions,
protected Keep across expiry, and a permission-denied backup restoration after
the selected source has already returned. A process is killed after an actual
reviewed return in a 500-file replacement-move batch, before the API returns.
Restart finishes all returns and restores previous destinations, then older copy
Undo removes only the selected returned copies. Both original source trees and
externally moved originals remain intact, with no abandoned stages. This is
batch process-interruption coverage rather than an instruction-level crash hook.
Further regressions cover returning the original physical item after adoption,
fresh forward moves of that item, layered adoption followed by saved-destination
restoration (including a permission-denied partial return), and Keep both after
successive adoptions. Migration fixtures use supported v1/v4/v5 input and public
results; future schema input is refused without rewriting its database file.

All 133 core tests pass, including ten explicitly ignored child helpers exercised
by their parent process-interruption fixtures. Build and all-target Clippy pass.
Native dark and light/1.25× runs each pass 12 reviewed-move checks, and the broader
native Undo workflow passes its 31 existing checks. Rendered inspection confirms
the light review and the dark protected-version preview and footer. All three
application logs have no GTK warnings, criticals or panics. The final binary hash,
logs, migration inputs and reproduced regressions are recorded in
[the verification record](../../../.scratch/file-manager/undo-moved-verification.json).

Uninspectable same-filesystem moved items now use
[ADR 0029](0029-undo-unavailable-moved-items.md). Unavailable saved versions,
unavailable-device authority, folder Merge, cross-filesystem reversal conflicts,
broader dependency/batch decisions and accurate partial-
progress presentation remain open. Full Undo and the product brief remain in
progress; see the implementation ledger for verification and remaining scope.
