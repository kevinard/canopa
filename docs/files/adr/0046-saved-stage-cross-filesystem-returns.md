# 0046: Return saved-data containers with their catalog ownership

Status: identified file/link return implementation verified; broader recovery
and transfer handling in progress. The full product goal remains active.

Undo can return a folder to another filesystem while it contains versions still
needed by Undo or protected by Recovery. Copying those private folders as user
data would recreate their inodes without updating the catalog. The interim guard
in ADR0043 prevented that loss but left the return unfinished.

Schema14 indexes saved-container ownership separately from namespace paths.
Ownership comes from identified active and archived retained versions. An ordinary
user folder with a similar private-looking name remains ordinary data.

A saved-container return copies and verifies its known file or link in a new
private stage, keeping the original available. Preparation uses the existing
partial-copy journal. A dedicated migration journal then installs the container
and records an atomic catalog cutover: physical saved identities change in the
same transaction that consumes the derived return step. Historical paths retain
their logical namespace; deleting the derived step makes existing namespace
resolution refer to the returned location. Archived locations follow the actual
return in that transaction.

Retained versions keep the logical identity used by older history bindings and
separately record their recreated original physical identity. A reviewed or
unexpected replacement remains distinct from that original. Migration must not
authorize edits: unchanged expected versions adopt the installed snapshot;
changed versions retain their normal review requirement.

After cutover, the journal owns source cleanup. It verifies the canonical copy,
quarantines the old saved file in its owned container, verifies that file again,
and removes only that file and the empty container. An interruption can expose
the prepared copy before cutover or the remaining source after it. Unexpected
contents, changed containers and changed copies stay preserved. There is no
recursive removal of an unexpected payload.

The stronger existing public Recovery tracer now requires complete cross-device
Undo, removal of the old source tree, and successful Recovery inspection/copy-out
after reopening. Its initial failure is the interim guard; the first implementation
passes that tracer. Two method-visibility errors were corrected before that run.
Public tests now cover active-backup visibility during partial returns,
successive older Undo, unchanged expiry, edits through an open handle, raw link
bytes and modification times, private-looking ordinary folders, and authentic
schema13 input with both active and archived owners. Recreating a link initially
changed its modification time and caused an invented older-Undo conflict;
restoring the source fingerprint's metadata before installation fixes that case.

Actual process kills exercise two observed boundaries: the destination is
installed while Recovery still points to the source, and Recovery points to the
destination while the source still exists. Opening with a refused placement
check freezes reconciliation for public inspection; retry then completes the
journal and preserves successive older Undo. The fixture uses an 8 MiB payload
and direct SIGKILL, and verifies the observed catalog path again before replay.
Its original 64 MiB payload exhausted the watcher's deadline before observation;
that failed fixture run is not application-failure evidence.

Malformed migration input whose change has no return path is refused for
execution, while Recovery can inspect and copy its prepared file. Record
construction must not assume validation succeeded and unwrap that absent path.

An occupied saved-container destination goes through the normal conflict review
before migration: Skip preserves the pending return, and Keep both chooses a
new name for the whole identified container. The private container's contents
are never merged with an unrelated occupant. Preparation retains its own final
occupied-path guard for a race after review.

Verification passes 268 core tests with 19 parent-invoked helper definitions,
all-target Clippy, the desktop build, formatting, syntax, maintained inventory
and whitespace checks. Native runs 93 (dark) and 94 (light at 1.25 scale) each
pass 25 checks, including private destination cleanup, on the same executable.
All 18 captures were inspected and both GTK logs are clean. The native cases
use keyboard/clipboard Copy and Move, Skip, Quick Look for the pending backup,
Keep both, Recovery refresh/preview/copy, and successive older Undo. Exact
source hashes, logs and scope are recorded in
`.scratch/file-manager/stage-migration-verification.json` in the source checkout.

Unexpected contents,
directory/special saved payloads, forward Copy/Move of containing folders and
already-decomposed legacy private-container returns still need full handling.
Broader directory recovery, durable jobs, directory-FD
authority, original-media handling and the rest of the product brief remain open.
