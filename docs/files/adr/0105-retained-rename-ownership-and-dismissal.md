# 0105: Retain queued Rename sources and explicitly dismiss reviewed requests

Status: scoped backend, Iced and native verification, 2026-10-20.
Continues Phase2 and ADR0103/0104. The complete durable-operation contract remains open.

## Evidence

A public queue regression admits100 folder creations and then Rename of a
catalogued previous version behind a real Undo question. Admission observes the
original file, but older work expires its Undo owner before Rename dispatch.
The previous backend removes its saved-source container and Rename fails.
Source03 reproduces this twice (core04/05, receipts63/64). Attempt02/receipt62
failed fixture compilation and supplies no behavior result.

Adding only admission retention preserves the actual source through expiry, but
the generic mutation guard refuses Rename's own reservation (source06/core07,
receipt65). This distinguishes the missing reservation from faulty expiry and
false observation rejection.

## Decision

Accept Rename's retained-source dependencies in the scheduling transaction, as
for ordinary transfer requests. Keep the original selected-entry and parent
observations separately. Reservations survive Undo expiry and application restart;
they neither persist those process observations nor authorize saved-plan replay.

Inside journal admission, a Forward Rename may use only its exact linked request's
reservation. The request_executions association is established by its already
admitted live execution; labels and paths supply no owner. Every other request or
clipboard lease still blocks mutation. Do not drop protection before the syscall.
Undo and expiry receive no exception, even when they carry the original operation
ID. Preparation, other change kinds and stage migration retain their current guards.
This deliberately narrow rule does not claim general cross-filesystem Move ownership.
An existing forward journal can retain its original association after scheduling
settlement; looking at the current phase would incorrectly discard that association.

Failed or restored Create Folder/Rename rows expose **Dismiss request** through
the backend, with explanatory Iced text. Active, queued and unsaved rows cannot
be dismissed. Dismissal records a descriptive Finished result and releases only
this request's reservations, preserving completed files, Undo and execution
evidence. It executes no saved operation. Existing result/error and restored
execution notes remain in the descriptive result. A new deliberate command
selects the intended files again.

Check both journal lanes and the exact scheduling revision in the same immediate
transaction that saves dismissal and releases reservations. An unresolved journal
requires Recovery review first. Database refusal leaves the old row and all its
protection intact, so the user can retry dismissal. Other independent owners
remain protected. Finished rows follow the existing100-row retention policy;
there is no new storage budget or unbounded dismissal history.

## Additional failures found through the public regression

Refusing the first confirmed-change insert after a real Rename exposed an older
cleanup error: discard_empty deleted the journal's parent operation row, so
explicit reconciliation failed with a foreign-key error. Preserve an otherwise
empty operation while either journal names it. This does not infer lost results
or manufacture missing historical rows. Source14/core15/receipt70 captures the
exact failure; source16's public journal reconciliation regression passes.

The failed-request restart fixture also exposed a lease-worker retirement race.
After engine drop, final explicit lease release acknowledged metadata deletion
before releasing its duplicated history lock. Restart could fail with EAGAIN.
Source19 repetition receipts74–77 capture the intermittent application failure;
source21/core22/receipt78 reproduces it directly on the first immediate engine
reopen. Final explicit release after engine shutdown must wait for worker/lock
retirement, with an unwind-safe completion signal. Ordinary UI-side Drop stays
nonblocking, live clones retain protection, and release while the engine remains
open does not wait for its long-lived worker to stop. No retry or sleep masks
startup failures. The public64-iteration immediate reopen test and application restart chain pass;
ordinary nonblocking Drop and concurrent release/error coverage also pass.

## Verification and limits

Frozen application source29 (730 inputs) passes584 backend parent tests across67
standard targets plus the custom device_runtime harness.39 ignored entries include
some parent-invoked helpers; not all execute independently. Default Iced library
verification passes84 tests, one ignored. Formatting, all-target Clippy with warnings
denied and the default build pass. No schema or storage budget is increased.

ELF73cc8dfa2c9de4bd2aa463ce434b23bea4ed169f5e1f50564896fbaac6312a13
(134508008 bytes) is preserved in a decoded-verified gzip. Source46 adds only the
corrected native fixture/diagnostics and evidence text, with all583 compiled
crate/vendor/Cargo inputs equal to source29. Eight selected native cases/46 grouped
checks pass in dark100% and light125%, using pinned Niri63e39bae3473: ordinary
request dismissal, Create/Rename/Undo, live continuation and interrupted Recovery.

The new native case performs real SIGKILL after confirmed Rename while result
saving is refused. Restart preserves newer output edits and its inode; explicit
Dismiss request retains the original execution digest and actual Undo. Subsequent
native Undo returns those edits. Failed-request dismissal preserves colliding files.
Backend tests also cover stopped retained-source restart, failed dismissal rollback,
independent owners, active/queued/unsaved refusal, a real unresolved Forward journal,
explicit reconciliation and correct Undo order after subsequent cleanup work.

Native41 failed because the fixture still registered its deliberately killed app
as a required live service. Source42 corrects that registration/cleanup sequence
without changing assertions. Native43's two cases/root summary passed, but its outer
wrapper ended143 before saving its receipt; a manual audit preserves exact inputs
and ELF without calling it a clean wrapper pass. Native48 was interrupted during
light acceptance: a sender diagnostic records SIGTERM from the Codex executable
at PID1670066. Its trigger is unknown; no fixture cleanup or product cause is
claimed. Five exact orphan services were retired with runtime/UID/start-time/pidfd
guards, leaving all files and the historical fixture intact. Native47/49/50/51 are
clean exit0 batches;49–51 use a persistent PTY. Nonrecurrence does not establish a
PTY fix. The standalone diagnostics preserve default termination and install no
fixture/app handler. All failed/interrupted attempts remain available.

Four clean-run critical captures were inspected for narrow failed/restored request
review, Undo and reconciled Recovery; additional diagnostic captures were examined.
All249 inherited proof archives and15 intermediate source archives were verified.
See [verification](../../../.scratch/file-manager/retained-rename-verification.json),
[work log](../../../.scratch/file-manager/RETAINED-RENAME-WIP.md), and the external
preservation58 record for final archive integrity and source56 sealing.

This does not provide atomic entry-relative mutation, durable original object or
media proof, exact end/step receipts, automatic lost-result reconciliation, prepared
Rename authority before name submission, or complete restart continuation. Other
operation families' review/ownership work and phases3–6 remain active. Native Move/
Cut continuation, complete accessibility/hardware/performance and release gates
remain open. Managed custom actions retain ADR0092's separate explicit contract.
