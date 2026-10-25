# 0087: Recovery catalog and Iced window

Status: retained-file and directory Recovery workflows scoped-verified; broader native
acceptance pending, 2026-10-19. Continues ADR0086 and the
renderer-independent workflow direction in ADR0074/0076.

Recovery has one auxiliary window per host. Opening it attaches a read-only,
coalescing catalog reader to the existing OperationJobs owner. Neither the
reader nor Iced opens another history writer. The backend owns paging,
deduplication, cancellation and observation of committed history and retained
payload metadata. Refresh preserves the number of pages the user has loaded;
Load more adds a bounded page. Observing a new location requires a fresh read
after installing its monitors so discovery cannot miss a change in that gap.
Recompute effective coverage and directory identity even when target paths are
unchanged: a missing ancestor can return, or a directory can be replaced at the
same name. Coalesce filesystem notifications across the reader for 150 ms;
manual Refresh remains immediate. The delay has one owned, cancelled-on-close
source on the reader's context and does not periodically poll the filesystem.

Recovery page preparation must not decode every unrelated later history step
for every row. When a record has no container-movement context, use the existing
derived namespace index to check whether its saved container or any ancestor has
a later namespace move. Without that first move no context chain can begin.
Otherwise retain the complete identity-aware traversal, including existing
movement contexts. This changes neither the journal nor database schema.

Expose an opaque saved-item identity separately from its observed version.
Selection follows the saved item when its owner moves from active Undo to the
archive or its identified container moves. Preview preparation follows the
current stored path and version, with the original raw filename as a format
hint. Neither identity nor preview version grants execution authority. Unavailable
items stay inspectable and become available again when their owned paths return.

Iced owns selection, viewport, destination drafts and presentation. Reuse the
shared row, control, preview and operation interfaces. Row construction remains
bounded in narrow tiles. Enter offers Recover a copy; Space opens the floating
preview; Escape closes the owning window and its preview. The existing
configurable Recovery command has no default shortcut. Expose it from Places
and Operations. Closing Recovery never cancels admitted work or answers a
question. Shared questions retain their explicit Cancel default; Recovery omits
Replace, while Keep both, Skip and eligible Merge require an explicit choice.

Recover a copy captures the selected item and destination draft. Accept a full
local path or file URI and preserve the original filename bytes when only its
folder changes. Refusal keeps the draft. Submit Recover, Reconcile and exact
in-memory Retry/Continue through OperationJobs. The view never rebuilds a plan
from descriptive failed/remaining paths, and reopening does not grant restart
execution authority.

Verify catalog behavior at the established public catalog/queue and Recovery
engine seams using actual private files. Native Iced acceptance must cover
retained bytes, source-bound previews, raw destinations, conflicts, Undo,
continuation and window closure in both themes. These decisions do not complete
general durable restart authority or the full product brief.

Scoped evidence: 67 focused core parents across eight targets, 48 unfiltered Iced
library tests, warning-free all-target Clippy with warnings denied, and the default
application build. Native retained-file Recovery passes in dark100/light125:
actual saved previews and external edits, source binding, copy/Undo, a retained
refused destination draft, raw original names and raw folder/full-path URIs,
and owner-preview closure from a narrow tile. Dark Trash and shared Operations
regressions pass as well. See the implementation ledger and
[verification receipt](../../../.scratch/file-manager/iced-recovery-verification.json).
Directory Recovery additionally passes five native check groups in each theme.
The fixture uses an unexpected retained directory containing read-only folders,
a large first child and a collision created independently during the copy.
Cancel preserves completed bytes, newer output and original modes. Continue
refuses a substituted output root and retains the exact plan for the original
directory. Every window can close while the resumed question remains unanswered;
activation opens another Browser in the same process. Read-only scheduling
receipts prove that the original request stays Running through closure,
reopening and Review, then finishes without a successor after explicit Keep both.
They inspect identity and phase only and grant no execution authority. Separate
Undo entries reverse the continuation and prefix while preserving newer files.
Actual 340-pixel Browser/Recovery geometry and a floating preview beside its
owner are checked, including fractional-scale resize settlement.

These runs use the same new executable. Its three presentation corrections
describe Recovery collisions accurately, distinguish submission from accepted
work, and use the singular status for one saved version. The earlier library and
Clippy checks were repeated; filesystem core inputs are unchanged. Both scoped
review axes are clear. See the
[directory verification](../../../.scratch/file-manager/recovery-directory-verification.json).
The first follow-up light run raced Browser focus against question retirement;
the fixture now waits for native retirement before subsequent input. This is no
application focus fix. Interrupted Undo reconciliation now has scoped native
coverage under ADR0088. Retry failures, expiry, long-catalog and general restart
acceptance remain pending. Startup had a pre-Browser timeout before a
later successful run; no startup fix or general reliability claim is made.
