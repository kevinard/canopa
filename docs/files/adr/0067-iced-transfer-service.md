# 0067: Transfers in the shared Iced operation service

Status: implementation in progress, 2026-10-18. Continues the full PRD and
ADR0056/0065/0066; this is not acceptance of completed transfer or restart support.

Extend the existing serial service around the core transfer engine. Ordinary
mutations, Undo and transfers share history ordering. Metadata admission stays
available while a transfer copies or waits for a generation-bound decision.
Use typed Undo/transfer questions and replies so a stale or wrong-kind response
cannot release a different question. Progress updates replace the latest bounded
snapshot and coalesce renderer notices, without queueing every copied chunk.
Share immutable path lists and reports between snapshots. Reading progress on
the Iced thread must not repeatedly copy retained batches under the service lock.

Keep source placement captured at Copy/Cut separate from destination placement
captured at Paste. Both are core Storage snapshots, with resolved browsing
bindings prepared earlier by directory workers; capturing them performs no new
filesystem work on the UI thread. External clipboard sources capture placement
at Paste. Source failures remain isolated; destination loss retains the exact
remaining work. Checks run at the transfer engine's existing access boundaries.

Persist the actual transfer report and the engine's opaque failed/remaining
continuations after stopping. Never save an original batch as the retry plan for
partly completed work. Finished rows release executable plans and placement.
Partial rows preserve exact plans and original source/destination requirements;
loading them never executes them. A failed result save retains its in-memory
outcome and application hold, as other filesystem work does.

A fatal engine or finalization error is an interrupted result requiring
reconciliation, never an ordinary failed original request. The core error
retains its confirmed outcome, including exact plans already assembled before
finalization failed. The service persists those facts and the interruption
reason in a nonexecutable Running record, displays Interrupted, and refuses
further changes until reconciliation. Errors before a confirmed report exists
retain no invented completed count or reconstructed plan. This interim review
state does not fulfill the remaining reconciliation/Recovery implementation.

Known cancellation or refusal before execution must remain distinct from that
uncertainty. Cancellation retains safe remaining intent; it must not create a
reconciliation blocker. A preflight refusal may retain its original unexecuted
plan only with an explicit before-execution marker. Fatal execution errors and
lost workers cannot use that marker. Record a transfer's operation ID at each
confirmed commit, including folder creation, so interruption before final
summary hydration still preserves a reopenable confirmed report.

Durable Retry/Continue must consume only the chosen exact plan through atomic
request handoff, preserve the other plan and retain the original placements.
Saved-source ownership and journal association must be established before a
restored continuation is executable. Until those gates are implemented, stopped
plans stay visible for review; persistence alone does not authorize replay.
The full objective still requires those gates, ordinary runtime Retry/Continue,
Iced conflict/progress controls, clipboard/DnD, notifications and native acceptance.

Verification uses the established public service and actual private files:
copy/move and shared Undo, typed conflicts, cancellation, independent source
failure, original placement, stopped-result reopening and failed writes. Native
verification follows when Iced invocation and question controls are connected.
