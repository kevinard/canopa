# 0073: Retained payload mutation ordering

Status: scoped mutation-ordering implementation verified. Continues ADR0068/0072
and the full PRD; complete execution ownership remains in progress.

An owned clipboard selection or unfinished request must not lose a retained
payload through an unrelated file-manager mutation. Extend the existing journal
admission transaction to refuse steps that change paths inside a reserved saved
container. Check the same paths when acquiring ownership against an already
committed journal. Acquisition and destruction must share one ordering.

Resolve each affected path through its parent-directory identities. Follow
ancestor aliases while preserving the final symbolic link. A link outside a
saved container does not become its target's data. Moving a whole identified
saved container remains a physical catalog relocation; do not conflate that
with consuming its payload. Existing reservation migration must continue to work.
Read-only Copy from a retained version remains allowed. A refused unrelated
mutation preserves its bytes, destination and existing Undo, with an inspectable
failure; releasing its final owner permits the normal operation again.

Use the established public OperationEngine, request/lease, Trash and transfer
seams with real private files. Cover Rename, Trash, replacement writes, same- and
cross-filesystem source removal, aliases/links/lookalikes and physical container
relocation. Native acceptance uses only the private Iced/Niri harness.

The shared predicate covers forward source removal, replacement preparation,
saved-container migration and both endpoints of reverse directory moves. Resolve
the parent before walking its physical ancestors, failing closed when it cannot
be inspected. Whole-container Rename records the same parent metadata as saved
transfers; validate its reserved container format before creating history or
mutating files. Ownership still comes from the catalog, never a similar name.

Twelve added public regressions reproduce and verify refusal, unchanged data and
history, successful operation after release, and Undo of any committed output.
They include nested parent aliases, empty cross-filesystem folder removal and
Undo, nested saved-container migration, final links and ordinary lookalike folders.
The candidate `c2b9e792` passes all 442 core tests, 51 Iced library tests and
all-target Clippy. Both scoped review axes have no unresolved findings.

Private native acceptance passes ten checks across dark100 and light125. Rename
of an owned retained version reports an actionable refusal; 100 actual folder
operations expire its original Undo entry while Copy still protects its bytes.
Paste copies those bytes, and superseding clipboard ownership permits cleanup.
Both application logs are empty; narrow captures were inspected. The preserved
`bf3f9f2f` baseline genuinely consumed the retained version through Rename.
See the [verification record](../../../.scratch/file-manager/retained-forward-verification.json).

This adds retention protection, not authority to execute a saved plan or bypass
its own reservation. Exact request/journal association, descriptors, original
media, containers not yet catalogued and restart reconciliation remain open.
Do not expose Retry/Continue through an unverified execution path.
