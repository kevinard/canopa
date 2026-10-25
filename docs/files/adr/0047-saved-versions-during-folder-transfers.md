# 0047: Preserve saved-version ownership during folder transfers

Status: identified file/link transfer integration verified; broader recovery
handling in progress. The full product goal remains active.

A recursive Copy of a selected folder copies user data. Its identified internal
Undo/Recovery containers stay with the source history and must not be cloned into
the new folder. Recognition uses the saved-stage owner index and the resolved
physical directory identity, never a filename prefix. Ordinary folders with
similar names remain ordinary data. Initial omission handles known file/link
payloads with no extra entries; unexpected payloads or additional contents remain
preserved and require further handling. Explicitly selected private folders need
a separate policy from internal containers encountered during traversal.

A Move must carry saved data and its catalog ownership together. Moving regular
children recursively and recreating their private container strands older saved
references. A new public two-filesystem regression reproduces Recovery reporting
Unavailable after an apparently completed Move. A separate Copy regression
reproduces an unwanted cloned internal backup alongside the expected user files.

The Move implementation extends ADR0046's identified copy, installation,
atomic catalog cutover and journaled cleanup. A forward handoff appends a saved
container Rename namespace step in the same transaction that rebinds retained
physical identities. The step keeps its logical source identity, adopts the
installed physical directory expectation, captures the original parent identity
and optionally records the owning folder's Recovery context movement. Later Undo
must resolve a recreated original parent through the existing scoped identity
references, including when the containing directory was only partially moved.

Schema15 distinguishes this wire extension from schema14. Older records omit the
optional saved movement, forward-journal flag and empty recorded context moves.
Public prior-schema input now covers14, including an interrupted Undo handoff
whose wire fields are unchanged from14. Recreated physical expectations remain
separate from original logical ownership.

Moving the private container can finish before the owning user file. Recovery
keeps the source filename/location while that file remains there. Only a later
committed file or containing-directory movement updates the displayed context.
Archived records retain those scoped movements through reversal. Unrelated
versions in the destination must retain their own context. Internal
container name collisions choose a fresh private sibling rather than merging
with or replacing an unrelated occupant. Same-filesystem Merge moves the
identified container intact.

Keep both for a returned containing folder rewrites the saved movement's
receiving and incoming context paths with their corresponding directory roles.
Rewriting only Rename's primary paths leaves an invalid context relationship.
Validation continues to require the context parents to match those paths.

A continuation carries a receipt for a prepared saved handoff: operation, ordinal
and original container identity. Startup/resume reconciliation may finish the
handoff before the queued item runs. The receipt is recognized only while that
exact saved namespace step remains in history and its identified destination
still has its catalog owner. An arbitrary missing source is not completion.
This preserves chronological Undo entries for the reconciled prefix and later
resumed work. These receipts are currently part of the in-memory continuation;
persisting unfinished jobs and exact restart plans remains separate required work.
Internal nested saved-container movements do not inflate user item counts, while
an Undo entry remains available for actual recorded internal progress.

Focused evidence includes completed and cancelled folder moves; same-device and
cross-device Merge with Skip/retry and Keep both; private-name collisions; active
and archived versions through return and expiry; edited saved data and raw links;
root Keep both; prior schemas; and process kills at installed/source-owned and
catalogued/source-present boundaries. A private destination reconnect fixture
renames/restores its isolated destination parent at an observed installed handoff
and exercises the real placement callback, reconciliation, exact in-memory
continuation, separate Undo entries and older saved data. It initially reproduces
a false missing-source failure; receipts correct it. Detailed red/green logs and
source checkpoints live in SAVED-TRANSFER-WIP.md under .scratch/file-manager.

Verification passes 276 core tests with 19 parent-invoked helper definitions,
all-target Clippy, the desktop build, formatting, syntax, maintained inventory
and whitespace. Native runs 95 (dark) and 96 (light at 1.25 scale) each pass 29
checks on the same executable and actual distinct filesystem device IDs. All 20
captures were inspected, both GTK logs are clean and both private destinations
were removed. The native flow includes folder Cut/Paste, Recovery Quick Look and
copy-out before and after returning the moved folder, then older replacement and
Copy Undo. Source hashes, precise scope and logs are recorded in
`.scratch/file-manager/saved-transfer-verification.json` in the source checkout.

Context behavior is verified for the recorded folder moves, partial completion,
Skip/Keep both and returns above. ADR0048 subsequently supplies exact movement
provenance and source ownership for archived context through unrelated later
moves, recreated folders, equal path pairs and history expiry. Its schema16 proof
is separate from the schema15 evidence above. Unexpected contents, special/directory payloads,
ADR0049 subsequently verifies shared automatic expiry and preservation when
additional saved work is present. Explicit disposal, broader cleanup interruption,
already-decomposed legacy returns, bounded catalog and
context-history work, durable jobs, directory authority and the full
implementation ledger remain required.
