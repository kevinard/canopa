# 0097: Drag destination observations survive refresh

Status: scoped backend and native regressions verified, 2026-10-20.
Extends ADR0094/0095 and the renderer-independent runtime direction. The complete
Files product remains in progress. Primary review only.

Two native attempts reproduced an unintended Move into a replaced destination.
The original hover was prepared and advertised Move. Renaming that folder,
creating a replacement at the same path and refreshing the target view caused
the adapter to cancel its old hover and prepare another one. The new preparation
captured the replacement, so the original directory pin was no longer consulted.
The existing backend test with an unchanged PreparedDrop passed; the defect was
repeated preparation within the same gesture, not a failure of the pin itself.

PreparedDrag now retains each pathname's first successful destination capture:
original storage/alias binding, opened directory proof and automatic action.
Preparing that pathname again reuses and rechecks the original capture on the
worker. Cancelling a hover invalidates that hover's choices without forgetting
the gesture's observation. A changed directory, alias or storage observation
cannot obtain replacement authority through a frontend refresh. A new gesture
can capture a newly reviewed destination. Other destinations remain usable.

The capture pins the folder object, not its contents. Unrelated arrivals and
refreshes remain compatible with ordinary Move and shared Undo. An initial
failed preparation has no successful capture to retain. No execution authority
is serialized, and native offer validation remains the frontend's responsibility.

Keep at most 256 successful distinct destination paths per gesture, also subject
to the existing shared selected-entry pin budget. Refuse further new paths;
never evict an old observation and allow it to be recaptured. Memory locks are
not held across filesystem inspection. Admission releases unused destination
captures on the worker while the chosen proof continues into the transfer.
Otherwise captures retire with their owning gesture handles.

The public regression reproduced the same unsafe Finished Move before the fix.
It now refuses preparation without changing either folder or the selected bytes;
a fresh gesture succeeds and shares Undo. A second regression verifies contents
arriving in the original directory still permit repeated preparation, Move and
Undo. The 14 drop, 27 transfer and 27 retained-source parent tests pass headless;
fresh-process helpers are not counted again. The existing device case also
re-prepares after a storage identity replacement at the same actual directory
inode, proving that the original storage observation survives too.

Five native checks pass in each appearance on the rebuilt application: destination
replacement refuses before any operation is admitted, a fresh reviewed drag can
Move/Undo into that folder, contents arriving in the original directory still
permit Move/Undo, replacing the selected source preserves both versions, and a
fresh drag captures that reviewed version and shares Undo. The fixture observes
exact native source termination and offer retirement, plus read-only scheduling
IDs/revisions/phases and actual private bytes. Wire object IDs may be reused, so
retirement observation begins after the exact prepared target's capture.

The earlier dark controls (nine checks) and light/125% cross-filesystem and
presentation choices (seven, including private cleanup) also pass on this
candidate. All-target Clippy with warnings denied, the default build and 77
library tests pass. One ignored library helper runs through its parent. No GUI,
shared dependency or serialized execution authority change is involved.
The replacement refusal capture and source failure facts were inspected.
[Exact evidence and archives](../../../.scratch/file-manager/dnd-stale-verification.json)
retain both native failures, the public regression and all successful receipts.

Mounted-device drop targets, other delayed/stale geometry, broader transport
coexistence, the full Iced Downloads walkthrough and the remaining product gates
stay open. In particular this does not resolve the independent peer's strict
no-Drop cancellation failure, crash continuation or production performance.
