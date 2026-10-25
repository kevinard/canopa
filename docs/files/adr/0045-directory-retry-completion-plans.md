# 0045: Preserve directory completion intent when retrying failed work

Status: accepted and verified for the recorded retry/continuation scope. The full
product goal remains active.

## Decisions

Retry failed items must finish the original directory move when the repaired
leaves succeed. Reconstructing a retry from failure paths loses the original
directory cleanup work. Guessing parents from those paths would also allow a
single-file retry to remove an unselected user folder.

The transfer module returns an opaque failed-item continuation alongside its
remaining-item continuation. Both retain directory completion intent captured
during the original traversal: source and destination paths, physical identities,
mode, traversal ownership and the original destination cleanup owner. Operations
executes these engine plans directly. The independent path-only retry helper
remains suitable for supplied paths; it cannot infer directory ownership.

A retry interleaves captured directory completions between sibling branches in
postorder. Each resumed directory records the ordinal of its first actual step
in the new operation, rather than assigning every directory ordinal zero. This
keeps later Undo membership local to the correct subtree. Each execution creates
a new Undo entry, preserving chronology when unrelated work happened meanwhile.
Earlier destination-container cleanup continues to belong to its original entry.

Captured source and destination folder identities are checked at transfer
checkpoints, including before leaf work and after copying. Checking only during
final source-folder removal is too late: a replacement container could already
have received data or lost a source file. A retry waits for the original folders
to return rather than adopting newer folders at the same paths.

Copy continuations retain the same container identities and chosen Keep both
names. Capturing a directory for identity checks grants no source-removal work:
only Move plans schedule postorder source completion. A failed Copy cannot
remove a source folder that became empty before retry.

Retry and Continue can be executed in either order. Both may retain the same
postorder completion. An already absent source folder requires no second removal
or new Undo change, while existing replacements and changed destinations retain
their identity guards. Cancellation that cannot roll a changed copy back places
that file in Retry, not also in Continue. Newer destination data stays available
for an explicit conflict choice.

A cut selection has a stable intent identifier, distinct from clipboard content
revision. Reading formats still rejects changes between asynchronous reads.
Publishing the remaining part of that same selection preserves its intent;
Retry and Continue carry it forward. A newer Copy/Cut or any external clipboard
change invalidates the old intent. A worker retains selected roots when their
failed, skipped or remaining descendants are unavailable, even if a path lookup
cannot currently see the root. Completion clears the cut selection only when
the same intent remains current and its work is complete.

## Evidence and remaining work

Public filesystem tests reproduce the missing parent cleanup, replacement-source
and replacement-destination mistakes, and a duplicated failed/remaining item after
cancellation. Focused checks now cover repaired leaves, sibling isolation through
Skip/reopen, returning the original containers, both retry/continue orders, a
single-file retry's unselected parent, and container changes after a copy even
when the file node is unchanged. The original paused-move chronology tracer also
preserves older Copy ownership after root Keep both. A paused Copy test replaces
each original container in turn, restores the actual object, and verifies that
retry uses the chosen Keep both destination. A failed Copy whose last source
child disappears before retry leaves its empty source folder intact.

Native failures reproduced a stale cut selection after retry and premature cut
clearing while the original source folder was unavailable. A content-revision
probe identified the application's own partial update as the first cause. After
separating cut intent and retaining unavailable work, final runs88 and92 pass21
checks each in dark and light/1.25: retry/Undo across two filesystems,
unavailable-source retry, preservation of newer Copy/Cut and external clipboard
ownership, and floating Quick Look at narrow width. Transfer regression90
passes14 checks, including native Copy/Cut/Paste, Continue, failed-item retry,
background completion, notification Undo and Trash/Restore. All33 final captures
were inspected, with no panic or GTK critical/warning markers in the runtime logs.
The first light run89 received SIGTERM before completion; the isolated repeat92
passed on the same unchanged executable. No application fix followed that signal.

Full Files core259 tests/18 invoked helpers, all-target Clippy, desktop build,
format, syntax, inventory and whitespace checks pass. The directory suite has19
passing parents/1 invoked helper. `directory-retry-verification.json` records the
source hashes, exact binary and logs. The plans remain in memory and are not an
exact restart contract.

The plans are currently in memory. Durable jobs, exact restart continuation,
directory-descriptor authority, broader original-media handling and automatic
source/destination relocation remain unfinished. Checkpoint identity comparisons
do not establish atomic directory-descriptor protection. Private Undo/Recovery
container migration and the other product requirements remain in scope.
