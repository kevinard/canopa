# Iced operation interfaces checkpoint

Status: worker foundation and normal desktop build verified. The whole
PRD remains active, with every completion checkbox open. Iced/`canopa-ui` is the
supported GUI; historical GTK files are migration reference only.

## Implemented in this slice

`src/operation_jobs.rs` is a renderer-independent service over the existing
operation engine. A metadata coordinator admits requests durably while a single
filesystem worker executes or waits for an Undo question. It currently accepts
Create Folder, Rename and concrete shared-history Undo requests.

The service publishes coalesced asynchronous notices and latest snapshots. Undo
questions carry a job ID and generation; stale replies cannot answer later
questions. Queued cancellation finishes without executing its original plan.
Running cancellation uses the engine's safe boundaries. Save failures retain the
actual stopped result, block new dispatch and hold the application; Retry Save
does not repeat filesystem work. Orderly shutdown refuses active/unsaved work.
Final handle drop sets a stop flag even if its bounded control queue is full.

Completed rows drop executable requests and placement snapshots. Startup loads
Queued as Awaiting review and Running as Interrupted, without replay. Original
Unix path bytes and portable browsing/media requirements survive serialization.
Unknown/corrupt records remain on disk with an error. Uncertain engine journals
are preserved and block new ordinary changes until proper Recovery review.

Decisions: [ADR0065](../../docs/files/adr/0065-iced-filesystem-operation-interfaces.md).
Review: [separate Standards and Spec reports](iced-operations-worker-review.md).

## Verification

Frozen input inventory: `iced-operations-worker-source-02.json`, 487 inputs.
Compared with the prior 485-input Locations checkpoint, only Cargo.lock,
Cargo.toml, Files Cargo.toml/lib.rs/operations.rs changed, plus the two new
worker/test files. Renderer source is unchanged.

`iced-operations-worker-public-02.log` passes 51 tests across operation jobs (8),
operation transactions (12), scheduling registry (11), storage requirements (5),
history readers (3), and created-folder Undo (12). Two ignored child helpers are
explicitly invoked by parent tests; the new worker child is actually killed with
queued work behind an Undo question. Assertions verify actual private files,
raw names, Undo, non-replay, alias refusal and save retry without execution.

Desktop all-target Clippy lints pass in
`iced-operations-worker-desktop-clippy-01.log`. Three existing unused helper
warning groups remain; no suppressions were added. Workspace format and tracked
whitespace checks pass. The 558-package dependency inventory is current; normal
Files dependencies contain Iced and `canopa-ui`, without GTK widgets. A stricter
core-only Clippy check also passed before the final review refactors; final
public tests and desktop lints cover those refactors.

The normal build passes in `iced-operations-worker-build-01.log`. Executable
`99c423636c7c3bd7ce5125d0e0c49f145bc84bcc03a6c57190fc7c7bca64b118`
is 103,513,440 bytes and all 487 frozen inputs remain unchanged. The scoped
receipt is [worker verification](iced-operations-worker-verification.json). No new
native acceptance is claimed: this service is not yet connected to the GUI.
The previous Locations native receipts remain historical evidence for their own
binary and source, not acceptance of this worker or a combined candidate matrix.

## Next required work

Connect the service through an Iced subscription using `take_notices`, retaining
admission drafts until their receiver confirms persistence. Add source-bound
floating New Folder/Rename dialogs, basename validation, unchanged raw-name
no-op, stable notice/input tree and keyboard focus. Enable only implemented
commands and refresh affected tabs without redirecting unrelated browsing.

Integrate filesystem rows, questions, Save Retry and holds into shared Operations,
including last-Browser closure, notifications and source-window/tab independence.
Preserve all Undo source explanations/capabilities and metadata; Cancel is the
safe default. A question must not steal focus from another application.

Wire live platform volume observation before relying on mutation placement for
actual removable media. The service currently loads portable volume state, but
does not yet receive the missing Iced platform observer's updates. Reuse the core
Storage authority; do not treat unavailable persisted media as local storage.

Transfers, Trash/restore, Recovery, devices, native clipboard/DnD/FileManager1,
media, exact durable continuations/source owners, pinned-descriptor authority,
accessibility/performance and the full native/workspace/release/review/commit
gates remain. Never infer safe Continue from a restored original Running request,
and never replay arbitrary custom commands.

No compiler or native fixture remains running from this slice; both reviewers
finished. Only inactive Files compiler caches were pruned; source, historical
evidence and unrelated work were preserved. Root storage has about 386 MiB free
and `/tmp` about 1.1 GiB, so check available space before another large build.
