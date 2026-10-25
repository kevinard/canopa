# 0102: Backend-owned live transfer Retry, Continue and abandonment

Status: live workflow scoped backend/Iced verification passed, 2026-10-20.
Extends ADR0056, ADR0074,
ADR0100 and ADR0101. The complete product brief remains active.

The operation module owns executable failed and remaining plans privately.
Saved reports remain descriptive and release process pins. A typed
`resume_transfer(id, part)` selects one retained plan; the adapter renders only
actions authorized by the current backend snapshot. It never constructs work
from displayed paths. Original source and destination observations, placement,
exact targets, directory completions and cleanup receipts travel with the plan.

Commit old-part consumption and successor admission against the exact durable
revision in one existing `handoff_request` transaction before moving ownership.
Failure leaves the original plan usable. Preserve historical failures and counts;
record each part's handoff or abandonment explicitly. The other part remains
independently resumable. Every execution has its own Undo entry. Queued
cancellation and refusal before execution return the exact unexecuted plan.
Save retry persists the actual result and cannot replay its original request.

The successor also persists the exact opaque descriptive checkpoint, with process
observations explicitly stripped. A regression first reproduced the handoff
discarding its Keep both targets and traversal/cleanup data. Keep those facts in
the successor's scheduling payload, validate them against its descriptive request,
and restore them without creating a private execution owner. Merely storing the
selected paths would make later reconciliation unable to recover the exact plan.
This does not implement restart authorization or journal reconciliation.

Live executable stopped work holds the owning application and its accepted
activity until resumed or explicitly abandoned. Closing every window and explicit
Quit supply no answer and release no plan. This initially also blocks managed
device removal while these owners remain; Operations explains the outstanding
work and provides abandonment. Abandoning saves a nonexecutable historical result
before releasing pins/holds. It does not remove files or undo completed work.
Unsaved results keep their final ownership and block conflicting dispatch.

Cut continuation tracks outstanding roots across both parts and their successors.
Only confirmed, saved results may shrink the original still-current Cut. Skipped
roots and work owned by another part remain. A newer paste revision, Copy/Cut or
external clipboard intent cannot be consumed by a stale completion. Clipboard
transport remains in the adapter; the backend owns settlement and claim ordering.

A new public cross-filesystem regression reproduced a temporary remainder from
one directory part leaving Cut populated after its sibling completed. Such a
remainder belongs to the still-executable sibling, whose final outcome determines
whether unowned children remain. Explicitly skipped children remain in Cut.
Track private paste generations independently of the public clipboard selection
revision, so a newer paste without a selection change also expires old settlement.

These are live-process rights. Loading a saved report creates no execution owner.
Restart execution still requires journal association, identity proof and the
review/new-authority paths of ADR0056. Descriptor-relative mutation and the other
handoff phases remain separate gates. This decision does not claim full product
or restart completion.

The private full backend suite on source40 passes 564 parent tests across 66
standard targets plus the custom device-runtime harness. Its 37 ignored entries
are not all claimed executed. After the final successor-checkpoint correction,
87 tests across all nine affected application targets pass on source45, including
the reproduced checkpoint loss and subsequent owner-loss refusal. Filesystem
engine sources are unchanged from the full run. This is not a claim that the
entire final-source backend suite was rerun after that last serialization change.

Final default library tests pass 78 tests (one ignored helper), formatting and
all-target Clippy pass with warnings denied, and the default build passes.
Archived/decoded-verified executable51f509f7f26f passes four current native cases
and ten grouped checks: unavailable-destination Retry and actual application
Retry/Continue in both appearances, including zero windows/explicit Quit,
external clipboard preservation, exact targets and independent Undo. Native
presentation covers Copy; Move and Cut branching have public backend coverage.
The preceding80575eab052f candidate additionally passes eight cases/28 checks.
Keep those executable scopes distinct. Critical captures were inspected and
primary standards/spec review has no unresolved findings in this scoped change.
See [the evidence and preserved attempts](../../../.scratch/file-manager/application-continuation-verification.json).
