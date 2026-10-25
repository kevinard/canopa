# Undo Replace preserves newer files in Recovery

Status: accepted under delegated implementation authority, 2026-10-16;
implementation in progress.

Undo offers Replace alongside Keep both, Skip and Cancel when returning a
renamed/moved or trashed file to an occupied original path. Replace retains the
newer occupant in protected Recovery before restoring the incoming item. Regular
files and symbolic links support this choice; links are moved as links. Folder
and folder/file collisions keep the existing Keep both/Skip choices while Undo
Merge remains outstanding. Keep both remains the safe Return default. The dialog
explains where the newer file will go and shows both items' metadata.

Protected data is separate from ordinary Undo backup expiry. The displaced file
keeps its original Unix filename/path and an operation label identifying the Undo
that preserved it. Its recovery flag prevents automatic deletion after the
100-entry history limit. Writes through a pre-existing open file descriptor keep
reaching that retained object; they do not affect the restored file. Recovery's
fresh-observation checks still apply when copying it out. Recovering a copy uses
a chosen name/location and leaves the protected version intact.

Before changing either path, the pending Undo intent records the restoration,
the displaced file's observed version, its owned stage and recovery context.
Extraction checks the occupant's identity and version again; changes while the
question was open are refused. A failed restoration rolls the newer occupant
back without overwriting an intervening object. Same-filesystem rename/Trash
reversal also verifies the restored object's identity after its actual rename
and attempts a no-overwrite rollback if it changed.

Completing reversal inserts the protected Recovery record in the same SQLite
transaction that removes the Undo change and clears its pending intent. A
replacement move can also have an older destination backup to restore. If its
source has returned while that backup remains outstanding, the transaction
archives the displaced newer source and converts the unresolved step to its
retained-only remainder. Directory synchronization precedes history completion.

Startup reconciliation uses the same rules. A completed filesystem restoration
commits its Recovery record with the history update. Before restoration has
completed, displaced work returns to the original path when it is free; a new
occupant at that path causes the displaced work to remain protected in Recovery.
No recovery step silently replaces that intervening occupant. A mismatched or
unavailable retained stage requires further recovery instead of deleting data.
Legacy pending Undo records default to having no displaced file.

Six public filesystem regressions cover raw names, expiry/restart, edits or
source replacement during the question, Trash with a broken symlink, folder/file
collision refusal, a move with an existing destination backup, and later writes
through an open handle to the displaced file. The crash case kills a child after
an actual restoration rename and before its Undo API returns. After reopening
the engine, the displaced newer file is recoverable and the rest of the
500-file move reverses successfully. All 76 core tests pass; the four ignored
crash-helper tests are explicitly executed and killed by their parent cases.

`native-undo-09` (dark) and `native-undo-10` (light, 1.25×) pass the existing
metadata, preview-origin prompt, Skip, cancellation, Return/Keep both and Trash
checks, followed by actual Replace, source-named preview of the displaced newer
file and a native save-chooser recovery copy. Rendered inspection confirms the
explanation, both versions' metadata, visible choices and the retained newer
text. Both application logs have no GTK warnings. Focused Clippy and formatting
pass. The runner also handles SIGTERM through its cleanup path; the private
`native-interrupt-01/interruption-check.json` records the deliberate interruption
and removal of its runtime.
The full existing desktop regression also passes against this build
(`native-acceptance-21`), covering browsing, previews, clipboard, background
operations, notifications and Trash.

Changed copied replacements and retained-only conflict choices are now covered by
[ADR 0015](0015-undo-previous-destinations.md). The broader Undo requirement remains
open. [ADR 0018](0018-undo-changed-copies.md) adds explicit choices for changed
copies without previous destinations. Next work includes changed retained
versions, changed/replaced move destinations,
folder Merge, cross-filesystem reversal conflicts, dependent-step Skip, remaining
batch choices and accurate partial-progress presentation. Further fault coverage
is needed for a replacement reversal's retained-only remainder. Pending history
that cannot reconcile automatically must remain inspectable. Protected-data
disposal, directory recovery, pinned-directory traversal and the remaining race
audit stay in the implementation ledger. See ADRs 0012 and 0013 for the related
interactive Undo and Recovery contracts.
