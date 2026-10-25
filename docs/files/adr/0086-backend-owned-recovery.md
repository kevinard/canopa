# 0086: Backend-owned Recovery workflow

Status: scoped queue, copy, continuation and reconciliation verified, 2026-10-19.
Coalescing catalog and Iced integration remain unfinished. Continues
ADR0013/0017/0050/0051 and the runtime ownership direction in ADR0074/0076.

Recovery inspection and copy-out belong to the shared application runtime.
Iced will own selection, scrolling, destination drafts and question presentation.
It must not open a second history writer, choose journal lanes, reconstruct
remaining work or turn displayed source paths into execution authority.

Expose bounded, cancellable Recovery pages through OperationJobs. Reads use the
engine's committed history-reader interface and remain available while a live
operation waits for a decision. Catalog rows keep the engine's observed saved
version and identified owner context. An original path can be a display hint;
copy-out resolves the actual retained source again before durable admission and
execution. The queue retains its source dependencies and original storage
requirements. Copying never consumes the retained version or grants permission
to overwrite a fresh destination, including a broken link.

Use the existing independently journaled Recovery-copy lane for files, links and
recursive directories. Publish confirmed completion, partial results, progress,
failures and cancellation through the shared operation interfaces. Preserve
confirmed results after engine/finalization errors. An ordinary refusal before
any intent must not become an unreconciled mutation that blocks unrelated work.
Recovery may copy protected data while the primary journal remains unresolved;
it must not reconcile or replay that primary journal as a side effect.

Recovery questions have their own semantic kind. The runtime refuses Replace;
Iced omits it. Keep both, Skip and eligible folder Merge retain their existing
engine semantics. Replies remain bound to the actual job and question generation.
No window or notification supplies an implicit answer.

Keep exact in-memory Retry/Continue plans with the runtime, alongside the original
storage observations. Handing remaining work to another request transfers source
ownership atomically through the existing request handoff. A refusal retains the
plan. A different output folder or parent must not become authorized output.
Persisted reports remain descriptive after restart and never reconstruct a plan
from failed or remaining path lists. Durable restart authority remains a separate
unfinished product requirement; do not silently claim it from serialization.

Retry interrupted work is an explicit reconciliation request. It rechecks the
engine's journal-owned paths and platform placement. It cannot authorize an
overwrite or mark an uncertain runtime report reviewed before reconciliation
succeeds. Inspecting or copying retained data does not acknowledge that review.
Capture both journal lanes and temporary directory-access records at admission;
compare their complete context under the engine lock before execution. A queued
review cannot adopt a different journal. Successful reconciliation wraps only
the uncertain reports captured with that request, preserving their historical
facts and source ownership. A failed result checkpoint remains SaveFailed and
holds later work until it is saved. This wrapper grants no execution authority.

Verify through the established public queue, Recovery engine and storage seams,
with actual private files and isolated child fixtures where a real worker must
be stopped at a filesystem boundary. Native Iced Recovery presentation needs
its own acceptance. Full Recovery, general restart/Retry authority and the entire
product brief remain incomplete.

## Scoped verification

The final focused suite passes 116 parent tests across the operation queue,
transfers, Trash, retained-source ownership and Recovery engine interfaces. Eleven
Recovery runtime/entry parents cover actual copy-out and Undo, raw destination
bytes, inspection during live questions, exact partial directory continuation,
missing or replaced output parents, queued cancellation, result-save failures,
explicit Keep both and rejected Replace/stale replies. Initial engine cancellation
returns the known unstarted source without filesystem callbacks or a journal.

A private child is killed during an actual Undo quarantine. Copy-out preserves
the primary interruption and a newer occupant; explicit reconciliation refuses
the occupied path, succeeds once it is resolved, and allows ordinary work again.
An actual finalization failure retains the confirmed copy and its historical
report across successful review and reopening. A queued review of a running
journal refuses after that journal changes. Private pause fixtures exercise real
filesystem boundaries; no production test hook or user folder is involved.

All 47 unfiltered Iced library tests pass, as do warning-free default all-target
Clippy and formatting. Both scoped review axes have no unresolved findings. These
checks cover compatibility adapters; the current application executable remains
the previously preserved Trash candidate. There is no native Iced Recovery
acceptance in this slice. The coalescing Recovery catalog, destination/preview
window integration and safe restart execution remain open. See
[the verification record](../../../.scratch/file-manager/recovery-runtime-verification.json).
