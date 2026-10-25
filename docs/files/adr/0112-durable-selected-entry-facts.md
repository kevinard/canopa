# 0112: Capture durable original entry facts before transfer admission

Status: implementation in progress, 2026-10-22. Continues ADR0056 and ADR0111.
The public restart continuation gate is still failing; no saved plan gains
execution authority from this work alone.

A real killed-process test saves a directory transfer with both failed and
remaining parts. Startup preserves its completed prefix and report, but exposes
neither Retry nor Continue. A pathname/inode check cannot safely recreate the
lost process pins: inode numbers and ordinary mount IDs may be reused.

Capture a bounded persistent kernel file handle through the original O_PATH
file descriptor, with its unique mount ID and boot identity. Hash the handle's
type, length and bytes, retaining an opaque fixed-size value. Open each selected entry relative to its already opened parent and capture both
descriptors. This binds the facts to the same original relationship without
adopting a later parent pathname. Preserve final symlinks and never read a FIFO or device.
Destination roots retain their first actually opened folder's facts. Missing or
unsupported observations stay unavailable; later fresh availability does not
rewrite them. In-memory ownership and descriptor budgets remain separate.

Checkpoint v2 carries these original selection facts through descriptive
failed/remaining plans. Ordinary admission saves the exact initial checkpoint
before acknowledging its request, within the existing request budgets. Decoding
v1 remains supported with no invented facts. The execution-start transaction also
seals the original snapshot with its digest and exact request revision before
publishing a filesystem journal. Replacing the initial application plan with a
stopped report does not erase that independent evidence. Schema28 prevents older
executables from interpreting the new saved bodies and transfer-start records. Completion receipts still mean
only actual completed work; old process clipboard intent is not restored.

These facts are not a replay capability. Before enabling restart execution,
prove exact request/journal/step association, original storage, traversal folders,
stages and cleanup ownership. Open and validate the actual objects used by each
mutation, with descriptor-relative access and claims where required. Current
paths must not substitute for original objects. An unprovable original or device
must refuse safely; a returned original may become usable only under the complete
execution contract. Cross-boot/device returns require stable media proof beyond
a same-boot mount identity.

The unignored public killed-process regression remains the acceptance target:
both Retry/Continue orders, exact Keep Both targets, repaired unselected children,
newer completed-prefix bytes/inodes and independent Undo. The complete crash
matrix and other operation families remain required. This decision does not
close the full restart, product or release gates.


Verification on2026-10-22:134 selected regression parents pass across8 targets,
including the descriptor-limit clipboard child, source retention, ordinary
completion recovery and live continuation. Seven entries are ignored in the
parent inventory; the required unfinished-restart acceptance test is explicitly
excluded from this regression command and run separately. It fails again on the
current schema28/checkpoint-v2 source. All-target default-feature Clippy and
formatting pass; all832 recorded Cargo/crate/vendor inputs match. No current
native or full-workspace pass is claimed for this unfinished implementation.

The second real failed fixture proves both parts retained exactly the sealed
original selection, with both selected sources and the root available. Its
stopped end receipt is absent, and startup correctly restores no execution owner.
The target remains actual safe Retry/Continue after restart, not a permanent
review-only compatibility state. Remaining work includes exact stopped/step
receipts, traversal/stage/cleanup proof and opened-object mutation boundaries.
