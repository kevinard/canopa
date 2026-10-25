# Incremental transfer transactions and retained replacements

Status: accepted under delegated implementation authority, 2026-10-16.

This supersedes the JSON storage part of ADR 0005. Operation history now uses
private SQLite state at `canopa-files/operations.sqlite3`, with one writer,
full-sync WAL transactions, and an independently recorded intent for each
filesystem change. Rewriting an ever-growing JSON batch for each completed file
would make large transfers perform quadratic storage work. SQLite stores one
change per row and groups those rows under a single Undo entry.

The adapter uses rusqlite 0.32, whose [upstream manifest](https://github.com/rusqlite/rusqlite/blob/v0.32.1/Cargo.toml)
selects the same libsqlite3-sys 0.30.1 already used by the workspace's SQLx.
There is one native SQLite dependency and no new application-local fork.
Existing version-one JSON history is imported once, preserving raw Unix paths,
pending intents and operation identifiers. Its previous source remains available
as a migration artifact; importing again cannot resurrect already-undone work.

The current core seam is `OperationEngine::transfer` with a Transfer Request,
cancellation and an interaction callback. Progress, verification, file/folder
conflicts and unavailable destinations are explicit events. Outcomes distinguish
completed work, isolated failures, skipped sources, cancellation and work needing
attention. Native conflict dialogs, Operations, clipboard and drag-and-drop
integration remain required; this ADR does not claim those workflows complete.

Copies prepare private sibling stages, sync their data and metadata, then
commit using atomic no-replace rename. Regular files and symbolic links retain
their path bytes; symbolic links are copied as links. File content fingerprints
include SHA-256, size, mode, type and modification time. A source changing during
copy fails that item and leaves its source available. Preparation is itself
journaled, so process death during copying does not orphan a large partial file.

Moves on the same filesystem use atomic rename. A forced move across filesystems
first commits and verifies its destination. The source is then renamed into an
owned quarantine on its own filesystem and checked against the copied snapshot
before removal. If that check fails, the source is restored. If its former name
is now occupied, the quarantined work is retained for explicit recovery.
Cancellation before source removal rolls back an unchanged unfinished copy;
completed earlier items stay at their destinations with one shared Undo entry.

Folder Merge traverses children and applies ordinary file-conflict choices.
Keep both uses an available numbered basename and a no-replace commit. File
choices can apply to remaining file conflicts independently of folder choices.
Destination versions are checked again after a conflict response, including
in-place edits that keep the same inode. A retained file's later changes are
also checked before expiry, so an editor saving through an already-open handle
cannot have that newer work silently collected as an old replacement.
Sources are normalized so selecting a folder and one of its descendants does
not transfer the same tree twice. Copying a folder into itself is rejected.
Empty source folders are removed after their children finish and recreated
before restoring those children during Undo.

Replace retains the previous destination in its identified private stage.
Reversing a copy removes the unchanged incoming copy and restores that previous
destination. Reversing a move also restores the incoming item at its original
location. Namespace races refuse overwrites, and partially completed reversals
keep the information needed for their remaining work. Unexpected newer work is
marked for recovery and is never removed merely because ordinary Undo history
expires. Expired ordinary replacement data is collected through a persistent
cleanup queue, without recursive deletion of unexpected contents.

Undo of created copies uses quarantine and verifies the actual extracted object
before deletion. Changed files and occupied original paths currently stop Undo
and preserve their history. [ADR 0012](0012-interactive-undo.md) adds initial
graphical Keep both/Skip choices for occupied rename/move and Trash restoration
paths; the other conflict and protected-data Recovery cases remain unfinished. Restoring a cross-filesystem move recreates inodes. Persistent identity
bindings associate those new objects with their older history entries, including
recreated parent directories. This makes a copy → rename → cross-filesystem
move → successive Undo sequence work without rejecting the application's own
restored files as unrelated replacements. Original source modes and modification
times are restored where needed for that history.

A destination's directory identity is checked between items, so replacing a
mount-point directory cannot silently redirect subsequent copies into a new
folder. User Retry can acknowledge an available destination. Actual mount and
unplug acceptance still belongs to the removable-drive/native workflow.

Real filesystem tests cover copying raw names and links, one persistent batch
Undo, cancellation during copying, partial moves, independent failures, merge
and batch conflict choices, replacing files and restoring them after reopening,
destination races, changed sources and copies, history expiry, and forced moves
between the repository filesystem and `/tmp`. Cross-filesystem tests explicitly
report when a runner has no second filesystem; the current development runner
has distinct filesystems and executes those branches. Another test kills its
own child during a real copy, reopens the engine, checks partial-file cleanup,
and undoes the completed prefix.

[ADR 0007](0007-native-clipboard-and-operations.md) records the native clipboard,
shared Operations view, background lifetime, notifications and exact-target retry
and continuation integration. Before full file-operation acceptance, remaining
work includes restoring unfinished jobs after restart, graphical Undo conflicts
and recovery, directory metadata policy, a traversal/race audit, actual removable
volume unplugging and drag-and-drop, Trash and restore, large-directory
measurements and independent review.
