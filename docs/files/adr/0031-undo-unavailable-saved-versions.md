# 0031: Undo when a saved previous destination is unavailable

Status: accepted under delegated implementation authority; verified for the scope below.

A replacement's saved previous destination can be missing, on unavailable storage,
unreadable, or inside a containing folder that was replaced. Interactive Undo
must describe that unavailable version without inventing file metadata. The
review shows its original destination, recorded saved location and lookup reason,
with current-item metadata when it can actually be read.

Skip is the Return-key default and preserves the step for retry while processing
other batch items. Cancel and Escape stop remaining Undo work. Decide later leaves
the operation waiting in Operations. Noninteractive Undo still refuses to proceed.
A reappearing file receives ordinary saved-version checks; moving it away and
back can change its recorded version and require explicit approval under ADR 0026.

Finish this step explicitly leaves current files in place and removes that step
from Undo. It archives a protected Recovery record even if no saved payload can
currently be read. The record survives restart and history expiry, and becomes
copyable from Recovery when its original owned containing folder and saved data
become available again. Finishing never adopts a replacement containing folder
or deletes its contents. It does not count as a filesystem reversal.

For an available moved/renamed item, Return moved item only offers a useful partial
Undo: return the incoming item to its original location while leaving restoration
of its saved previous destination pending in Undo. Ordinary collision choices
still protect newer work at the return location. Copy entries do not offer partial
deletion while their previous version is unavailable; Skip preserves the copy,
and Finish leaves it in place with the protected saved recovery record.

The unavailable observation captures the containing folder's version or lookup
error and, only within the owned folder, the saved object's lookup error. Finish
and return-only choices recheck that observation. Reappearance, permission repair
or a changed containing folder invalidates the review. Return-only checks again
after any return-location conflict question, before recording its mutation intent.
Skip and Cancel need no mutation authorization and always leave history intact.

A source-only return uses an ordinary durable Rename Undo intent with no saved
restoration, while the history row retains its saved record. Completion recognizes
that deliberate difference and atomically converts the row to its retained-only
remainder. Restart applies the same conversion. Existing reviewed-item identities,
displaced newer-work recovery and relocation remain in the intent; no wire/schema
change is necessary. Ownership updates use the actual returned path after Keep
both, so older entries follow the returned item rather than the newer occupant
at its original name. ADR 0030's strictly older reference ownership still applies.

Outcomes report saved previous destinations not restored separately from actual
reversals, items kept and unavailable incoming items not returned. A return-only
step can therefore count one reversal while retaining one pending restoration;
Finish counts zero reversals. Operations exposes that distinction and retains the
appropriate Undo action for an unfinished remainder.

Public filesystem tests exercise the agreed operations/Undo/Recovery seams;
native acceptance uses isolated files and normal keyboard/UI interactions. Exact
coverage and evidence belong in `.scratch/file-manager/IMPLEMENTATION.md`. This
decision does not complete directory-payload recovery, cross-filesystem Undo,
folder/dependency decisions, bounded history work, device authority or the full PRD.

Final verification passes 157 core tests, with twelve child helpers invoked by
parent tests. A new 100-file batch is killed after an actual source-only return;
restart preserves every pending saved restoration and the older Copy Undo.
This exercises process interruption through public operations, not an application
crash hook. Build and all-target Clippy with warnings denied pass. Native 41/42
pass twelve new checks each in dark and light at 1.25 scale; regression 43 passes
31 existing Undo checks and moved-item regression 44 passes 24. All 79 checks
have clean GTK logs. Both new return-only dialogs were visually inspected for
readability and visible choices. Exact logs, binary hash and limits are recorded
in `unavailable-saved-verification.json` under the file-manager scratch directory.
