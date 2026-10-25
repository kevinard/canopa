# 0110: Record exact ordinary mutation completion before application result save

Status: scoped backend/desktop/native verification, 2026-10-21. Continues
ADR0104–0109 and the original Phase2 contract; the full brief remains unfinished.

Filesystem success and the application result checkpoint are separate events.
The current execution association records confirmed history but explicitly cannot
prove the request ended. After commit/result-save death, a restored Rename remains
Interrupted even when its exact filesystem step is in shared Undo. Never repair
this by repeating the original Rename or assuming an absent journal means success.

Establish a typed, bounded completion receipt for ordinary Create Folder/Rename,
tied to the exact accepted Running revision/payload and actual confirmed step.
Only live successful execution after its work/cleanup can establish the end.
Metadata completion on restart may use that receipt without observing current
files as new execution authority. Newer edits, occupants, Undo, other source owners
and current-process clipboard intent must survive.

Reserve receipt storage under existing request limits before mutation. A failed
receipt/result save must keep the actual outcome protected for metadata-only Retry
save. Missing, incompatible, stale, corrupt, partial or unreceipted evidence remains
review-only; upgrading must not invent completion for older records. This does not
establish general transfer completion or durable replay of unfinished plans.

The first public regression reuses the real killed-child result-save fixture.
Current attempts, design refinement and verification limits are recorded in
`.scratch/file-manager/MUTATION-RECEIPTS-WIP.md`.

The red test confirms an exact current revision/digest and confirmed Rename step,
with no outstanding journal, still restores Interrupted. Schema26 therefore
reserves a typed mutation intent/digest and fixed end bit in the same start
transaction and existing budgets. Only a successful live engine can set the end
after cleanup and history maintenance; generic history append/recovery cannot.
If that write fails, an unserializable engine-scoped witness protects the actual
outcome and permits metadata-only retry. Startup accepts only an exact valid
ended record; the final result/source-release transaction validates it again.
An unsaved restored completion retains an activity hold. History expiry does
not erase this bounded historical completion fact or authorize any file replay.

Implemented for ordinary Create Folder/Rename. Corrupt completion evidence keeps
the row inspectable with an error; unended, stale, changed and legacy records
remain reviewable. Saving the end or result again retains the actual outcome,
activity/removal/shutdown hold and durable source references until metadata commits.
It never repeats filesystem work or adopts new process clipboard intent.

Complete backend coverage passes618 parents across67 standard targets plus the
custom device harness in eight sequential Cargo batches on source11. Forty
ignored entries are not all independently run. A subsequent constructor-only
grouping of startup Work/stop signal passes37 queue/crash parents and105 Iced
library tests (2 ignored), formatting, default all-target Clippy and desktop build
on source16. The full618 result remains explicitly bound to source11.

ELF26c804b48f55 (134894840B) passes6 native cases/24 check groups across both
appearances: failed Rename dismissal, actual completed Rename/result-save death
with newer bytes/inode and shared Undo, interrupted Undo and Keep in Recovery.
Four final captures were inspected. The fixture-only schema25→26 assertion
correction leaves all591 compiled crate/vendor/Cargo inputs equal to source16.
The first native attempt preserves its4 passing and2 early fixture failures.
Exact artifacts, failed attempts and limits are in mutation-receipts-verification.json.

This closes only ordinary mutation completion reconciliation. Other families'
end/step reconciliation, original/media/traversal/stage/cleanup proof, safe
cross-process continuation and the broader interruption/corruption matrix remain
Phase2 work. The original product brief and subsequent phases remain active.
