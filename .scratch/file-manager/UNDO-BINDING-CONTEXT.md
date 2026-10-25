# History identity ownership implementation

Status: implemented as schema 7 and verified for the recorded scope. The full
product goal remains active. [ADR 0030](../../docs/files/adr/0030-history-identity-ownership.md)
records the design and migration policy.

The original external-move regression is fixed: restoring independent B never
authorizes older Copy A to delete it. The initial scalar snapshot approach passed
that case but a source snapshot broke valid layered adoption. The final approach
stores expected physical identities per history reference, with stable operation,
ordinal and role keys. Each forward intent captures its older source and saved-
destination owners before filesystem mutation.

Source returns update only source owners; saved-destination returns update only
destination owners. Keep, Finish and saved-only returns do not grant incoming
ownership. Completion and partial-return conversion update owners atomically with
history, archives, relocation and intent clearing. Namespace relocation preserves
expected versions while updating paths. The active entry cannot own itself, so
its expectation remains stable during restart recovery. The earlier proposed
extra frozen Pending::Undo identity field is unnecessary under this invariant.
Trash/Restore recovery now resolves the actual adopted physical identity too.

Versions 1–6 migrate through public filesystem/history behavior. Foreign legacy
Copy aliases require review because their old path-wide alias could have meant
an unrelated restored destination. The legacy tables remain empty; live code
uses the per-entry references exclusively. Existing per-step storage limits still
apply; very large parent-owner contexts remain catalog-bound work.

Evidence so far:

- `undo-unavailable-external-copy-red.log`: original defect.
- `binding-context-recovery-red.log`: adopted Trash recovery defect.
- `identity-refs-focused.log`: initial final-model focused suite passed (3
  migrations, 2 context tests, 14 changed-move tests and 12 unavailable tests).
- `identity-refs-core.log`: initial full-model run passes 148 core tests, including v6 ambiguous-
  Copy migration and the already-adopted process-kill variant; 11 child helpers
  are explicitly ignored by the runner and exercised through their parent tests.
- `identity-refs-core-final.log`: rerun after validating migrated change records
  before indexing and correcting a needless borrow in the test fixture.

Final core passes 148 tests; build and all-target Clippy pass. Native 38/39
pass 24 checks each (dark/light at fractional scale), and regression 40 passes
31. All 79 native checks pass without GTK warnings, criticals or panics; two
rendered ownership reviews were inspected. `identity-ownership-verification.json`
records the exact binary hash and evidence. Schema-6 evidence remains historical.
No private database outputs are used as test assertions, and no application test
hooks were added. All launched processes are terminal.

This work does not complete unavailable saved versions, cross-filesystem Undo,
folder/dependency decisions, directory recovery, history bounds, filesystem/device
authority, FileManager1, optimized/hosted release or the other PRD requirements.
