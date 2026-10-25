# Review changed saved versions before restoring them

Status: accepted under delegated implementation authority, 2026-10-16.

A previous destination retained by a replacement can change through an editor's
open descriptor, or another program can replace the saved object. Interactive
Undo now reviews that available version before reversing any part of its change.
The question shows the original destination, the current saved object's type,
size and modification time, and these choices:

- Restore this version explicitly continues Undo using the reviewed file or
  symbolic link. A link is restored as a link without following its target.
- Skip leaves that history step retryable and continues the remaining batch.
  Skip is the safe Return default for this question.
- Cancel preserves the current files and unfinished history. Escape cancels.

There is one saved object to review, so the public question has no fictional
existing item. Ordinary destination collisions retain their two-item metadata.
If newer work occupies a restoration path after approval, Undo separately asks
Replace, Keep both or Skip under ADRs 0012–0015. Approval of a saved version does
not grant permission to overwrite that occupant.

The engine captures the observed version before asking, verifies that its owned
saved-data folder still has the expected identity, then checks both again after
approval. A changed observation or replaced containing folder refuses the
reversal. Skip and Cancel do not update the stored change. Noninteractive Undo
continues to refuse an unreviewed changed saved version. An unavailable folder
or missing saved item still needs recovery; no file is invented for its review.
Unexpected directory and special-file payloads can be skipped or cancelled but
cannot use Restore this version yet.

The retained record keeps the original logical identity for older history and
uses the reviewed version's physical identity for restoration. Its existing
serialized version field already carries that identity, so no store schema
change is needed. The durable Undo intent contains the selected version before
mutation. Successful reversal binds the restored physical object to the older
logical item. Startup reconciliation applies the same binding after a completed
filesystem restoration and retains the selected version in an unfinished
retained-only remainder. Older operations can therefore still refer to the
restored item when the saved object was replaced rather than merely edited.

An unfinished retained-only restoration is protected when its history expires.
Reviewing its contents authorizes restoration, not disposal. The expiry
transaction marks that remainder for Recovery before removing its active history.
This also protects ordinary partial restorations, whose sole remaining result
is the saved file. A permission-denied regression first reproduced disposal of
an edited, reviewed version after a move's source had already returned. The
passing case restarts, expires history and copies the still-preserved version
out through public Recovery.

An immediate follow-up question can arrive before the compositor has returned
focus from the answered dialog to its owner. The job now keeps that follow-up
eligible for one second and presents it when its owner or Operations actually
becomes active. It leaves the question in Operations if focus remains elsewhere.
Manual review clears this eligibility. Decide later does not re-arm it.

Public filesystem regressions cover Skip, Cancel and restart; a replaced saved
object followed by older rename Undo; a change while the question is open;
separate destination conflicts with Keep both and Replace; move restoration of
a replacement symlink with raw target bytes; and replacement of the containing
saved-data folder during review. A child process is killed after an actual
selected-object restoration in a 500-file batch, before the API returns.
Reopening, finishing the batch and reversing its older copies preserves both
source trees and all externally moved saved originals without abandoned stages.
This tests process interruption during a real batch, not a deterministic
instruction-level crash point.

All 114 core tests pass, with nine explicit child-process helpers ignored by the
normal runner and exercised by their parent cases (`core-undo-saved-final.log`).
Build, all-target Clippy, formatting, Python syntax, maintained native inventory
and whitespace checks pass. The expanded Undo acceptance passes all 29 checks
in dark (`native-undo-saved-20`) and light at 1.25× (`native-undo-saved-21`).
Rendered inspection confirms the saved-only metadata, focused Skip default,
restored text and subsequent two-version destination conflict. Both application
logs have no GTK warnings or assertions. The first light run reached the second
question but left it hidden during owner focus handoff (`native-undo-saved-19`);
the final runs include the bounded foreground-handoff fix. The earlier dark run
also passed 29 checks (`native-undo-saved-18`). Public/native outcomes and the
verified binary hash are recorded in `undo-saved-verification.json` under the
file-manager scratch evidence directory.

Broader Undo and recovery work remains open: missing/unavailable move destinations,
folder reversals, Merge, cross-filesystem conflicts, dependency and batch
decisions, directory recovery,
further retained-only fault coverage and remaining filesystem authority/cleanup races.
[ADR 0027](0027-history-readers-and-live-recovery.md) adds catalog reads and
previews while a live review waits for an answer.
[ADR 0028](0028-undo-replaced-moved-items.md) adds reviews for available
replacements at moved or renamed paths.
See the implementation ledger for the full remaining scope and verification.
