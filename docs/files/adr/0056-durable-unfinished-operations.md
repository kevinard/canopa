# 0056: Durable unfinished operations and exact restart intent

Status: implementation in progress under delegated authority, 2026-10-17. The
full product brief remains active; this is the intended contract, not acceptance.

## Evidence

Native149 closes every Browser during an actual directory Copy. The regular
child completes, the unsupported child fails, and Files exits cleanly after its
notification. A new process opens an empty Operations view; Retry cannot copy
the repaired child. Completed bytes remain intact. Native150 changes only the
process lifetime by keeping an unrelated Browser: a new destination/Operations
window in the original PID exposes Retry and copies only the repaired child,
preserving the completed file's inode and bytes. The queue currently retains
its exact plans only in memory. The diagnostic checkpoint retains both runs.

## Decisions

Unfinished requests belong to durable operation state. Closing Browsers or
ending the process must not discard failed or remaining work. Reopening exposes
the exact Retry/Continue actions and the actual completed portion. A failed
directory transfer keeps its captured completion plan and chosen destinations;
reconstructing a request from failure paths or repeating the original batch
would lose ownership and could overwrite newer work (ADR0045).

Persist original source and destination storage requirements separately, along
with their browsing bindings (ADR0055). Restart must not adopt replacement media
or a retargeted alias. Availability may refresh; original placement does not.
Backend observation counters alone cannot prove identity across process
lifetimes. A persisted request without a stable media identity must refuse an
unprovable return and require a fresh deliberate request.

Clipboard intent counters belong to one process. A restored request has no
authority over a new process's clipboard, even if a counter value is reused.
Its completion must preserve every newer Copy/Cut or external selection.

Startup loads unfinished state on a worker and presents it for review. It must
not automatically rerun commands or an uncertain partially committed request.
Queued, running, failed and remaining states stay distinct. Logical request
checkpoints and filesystem journals must preserve completed work through crash
boundaries; a stale original request must not repeat an already completed prefix.
An interruption that cannot yet be resolved remains visible with its owned data
and a truthful explanation. Visibility alone is not exact restart continuation.

Keep recent finished results as well as unfinished requests so a completion
notification opens a useful Operations summary after process exit. Preserve the
existing limit of 100 finished rows independently of unfinished work. Completed
rows no longer retain executable requests or source leases; their Undo controls
follow the actual available shared history.

Persistence must preserve original Unix path bytes, use a versioned bounded
format and durable writes, reject incompatible/corrupt records without deleting
them, and report write failure before releasing the last protection of unsaved
work. Queue updates and dismissal need explicit ownership and ordering. Discarding
a request does not delete its files or undo its completed changes. Retained source
data and cleanup ownership must outlive requests that still depend on them.

## Implementation and verification gates

Scheduling records live in the existing operation database (schema27), through
the core request interface. A short independent SQLite writer lets acceptance
and checkpointing finish while the mutation worker waits for a user decision.
Records have monotonic IDs, optimistic revisions and explicit queued, running,
failed, remaining, failed-with-remaining and finished phases. Stale updates and
dismissal cannot replace newer work. Finished records cannot become executable
again. Reading a Running record never executes it. The native adapter owns the
versioned payload's presentation and task meaning; core code owns transactions,
history location checks and scheduling metadata retention.

Retry/Continue consumes its selected saved plan and creates the Queued successor
in one transaction. The old row retains any unselected plan. Stale revisions,
active or finished source rows, and quota failure cannot produce a second
executable request or consume the old plan. This core metadata handoff is
implemented. Live source-owner transfer, exact successor checkpoints and Iced
controls are now connected in [ADR0102](0102-live-application-transfer-continuation.md).
[ADR0104](0104-request-filesystem-execution-evidence.md) now associates ordinary
Transfer/Mutation executions with the exact Running revision and confirmed work.
This is bounded descriptive evidence, not a completion receipt or replay authority.
[ADR0105](0105-retained-rename-ownership-and-dismissal.md) adds queued Rename
retention and explicit review/dismissal of ordinary Create Folder/Rename requests,
preserving execution evidence and completed work. Restored transfer/family review,
cross-process ownership transfer and filesystem-step reconciliation remain required.

[ADR0108](0108-opened-parent-rename-claims.md) carries ordinary Rename's prepared
entry/both-parent authority into opened-parent execution and journalled claims,
including readonly directories. Uncertain claims preserve their data for review
and Recovery Copy. [ADR0109](0109-keep-interrupted-rename-in-recovery.md) adds
fresh explicit review that transfers an available ordinary Rename claim to a
protected Recovery owner without replay, deletion or a fabricated completed
Rename. Other unresolved families, exact step/end reconciliation and durable
execution authority remain required.

[ADR0110](0110-recorded-mutation-completion.md) adds a separate typed completion
receipt for successful ordinary Create Folder/Rename execution, reserved against
the existing budgets before work and recorded only after actual success/cleanup.
An exact end permits metadata-only result restoration without replay; failed
end/result saves retain the actual outcome and application/source protection.
Legacy, unended, stale and corrupt records remain reviewable. This does not
complete other operation families or durable execution of unfinished plans.

[ADR0111](0111-recorded-transfer-completion.md) adds typed successful Copy/Move
ends and exact metadata-only result restoration. It preserves the historical
report through process death, failed saves and Undo expiry, without adopting
current paths or media. Unfinished transfer plans still need stable source/media,
traversal, stage, cleanup and cross-process execution authority.

Bound each payload to4 MiB, all request payloads to64 MiB and unfinished records
to1024. Refuse an over-budget transaction without changing previous records.
Keep100 finished records independently of unfinished ones. These are protective
storage bounds, not acceptance of silent eviction or lost save failures. Worker
code must surface refusal before starting new work or releasing unsaved plans.

The placement observer is now a core module shared with the native adapter.
Its opaque portable Requirements preserve raw paths, resolved browsing bindings
and original stable media IDs. Restoring captures fresh availability while
discarding old process epochs. Runtime Retry can use a still-current non-UUID
observation; restarted Retry cannot. Retrying or saving again preserves the same
original requirements, including a previously unprovable placement.

The scheduling table alone is not a lease on selected saved files or an atomic
filesystem-step checkpoint. Live retained-source dependencies and descriptive
handoff are connected; cross-process ownership links, hydration of proven work
and journal reconciliation remain implementation gates. Do not
treat a stored original Running task as safe to replay, or connect native
restoration by skipping those gates.

First establish a core-owned round-trip representation for opaque transfer
continuations, including raw names, exact targets, directory identities/modes,
saved-stage receipts and original cleanup owners. Verify decoded plans through
actual transfers and Undo after reopening the engine, including conflict choices
and edited/replaced containers. Serialization must not expose private fields to
the native adapter or replace their identity checks.

Then connect durable request state to the shared queue and verify natural exit,
explicit restart, queued and running interruption, source/destination media
changes, Retry/Continue ordering, distinct completed-prefix Undo, write failures,
corruption and dismissal through the public/core and native seams. Recovery,
Trash/Restore, ordinary mutations and custom actions retain their own semantics.
The full PRD's other requirements and the known source-lease/descriptor gaps
remain open. This decision does not claim that any of those gates already pass.
