# 0048: Tie Recovery context to identified history movements

Status: accepted under delegated authority; verified for the scope below.
The full file-manager goal remains active.

Recovery's displayed original location and its saved physical data have separate
authority. A saved container can move before its associated user file. Its
location hint must follow only the file or source folder that actually owns that
context. Reusing parent names in a later operation does not establish ownership.

Two public regressions reproduce false context changes without damaged saved
bytes: Undo of an unrelated later file move rewinds an archived location, and a
recreated source folder claims context left by a partially moved older folder.
Path-pair markers from schema15 cannot distinguish these histories.

Schema16 records the exact operation and ordinal of the saved-container
movement, the subsequent owning movement, and the logical source-parent
identity. A forward source-folder completion must match that parent's scoped
physical expectation. An active file completion must belong to the Undo entry's
captured PRIMARY reference. Reverse context changes require the recorded owning
step; reversing the saved-container movement removes only its own marker.
Repeated equal path pairs cannot consume each other's markers. Directory Merge
wrappers retain their inner folder's ownership.

The containing folder may have undergone a committed Rename before a recursive
cross-filesystem Move. Historical reference paths remain attached to their Undo
step. Capturing the next source therefore follows committed namespace movements
from the older reference and also matches its expected physical identity at the
actual source. Finding the same inode somewhere else in history is insufficient.
The derived namespace index searches only committed moves of the reference's
path or its ancestors, in chronological order. Its source-path index and the
expected-identity index avoid scanning unrelated history for each captured file.
Existing records populate the new namespace index in 256-step pages at upgrade.
The public alias regression returns the moved and renamed file, then exercises
older replacement and Copy Undo without granting unrelated destination work any
ownership. Its first failure preserves the correct bytes and metadata but still
expects the obsolete pre-return inode.

Older schema15 path maps contain no step or parent provenance. Upgrade preserves
their last displayed location as a **Recorded location** hint. It does not infer
new ownership from matching paths. Their physical saved data still follows
identified namespace changes and remains previewable and copyable. The copy
dialog retains the original filename, but does not choose an unproven original
parent as its initial destination. At most one unproven marker is retained to
qualify that hint; older unproven path-pair chains are not authority.

An index associates archived context markers with their container and owner
steps. Deleting a step queues only affected archive entries. History trimming
and startup drain this queue in bounded pages, retiring markers once neither
step exists. A live owner in a newer continuation remains reversible after the
older container step expires. Catalog changes and index maintenance share their
existing history transaction. This cleanup performs no filesystem mutation.

Public regressions cover unrelated moves, recreated folders, equal path pairs,
partial/retried Merge, renamed-folder aliases, authentic older input and 112
successive folder moves through normal history expiry. Raw SQL only prepares
legacy fixtures. Native acceptance exercises archive creation, containing-folder
Cut/Paste, unrelated file Undo, Recovery previews and copies, stopped-app v15
input upgrade and Keep both. Core validation passes 282 tests with 19
parent-invoked helper definitions. All-target Clippy with warnings denied,
desktop build, format, Python syntax, maintained inventory and whitespace checks
pass. Native 99 light at 1.25 scale and 100 dark each pass 13 checks on the same
schema16 executable and actual device IDs 35/2050. All 16 final captures were
inspected; both private destinations were removed and GTK logs have no warnings,
criticals or panics. Source/evidence manifests are recorded in
`.scratch/file-manager/context-epoch-verification.json`.

Native 97 dark passes with the initial fixture. Native 98 light reaches a
successful Keep both return and passes the file checks, then times out expecting
a remote Recovery action to obtain compositor focus. Its existing Recovery
window remains open. The fixture now explicitly selects that tile before sending
keyboard input. Final 99/100 use this selection step; application code is unchanged
after core/build validation. Delayed modal focus restoration versus remote
activation policy is not distinguished by this evidence. Broader remote-action
focus acceptance remains separate work; the failed capture/logs are preserved.

This decision does not complete bounded catalog work, unusual saved payloads,
shared-owner disposal, durable unfinished jobs, directory/media authority or the
remaining implementation ledger.
