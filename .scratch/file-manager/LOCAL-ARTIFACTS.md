# Local Files artifacts and the implementation checkpoint

The implementation is being committed on 2026-10-25 before the uutils migration.
Source, maintained tests/fixtures, integration scripts, product documents, ADRs,
and the Markdown investigation/handoff notes belong in Git. Generated logs,
screenshots, temporary probes, database files, source archives, and protected
executables remain local and are ignored. The existing evidence occupies about
8.4 GiB; it is not part of the source checkout or release package.

Crash tests left 142 private output directories directly under the repository
root and `crates/canopa-files/`. They are relocated, with their data and modes
preserved, to:

```text
.scratch/file-manager/retained-fixtures/repository-root/
.scratch/file-manager/retained-fixtures/canopa-files/
```

The local `repository-cleanup-2026-10-25.json` records each original path,
destination, original directory identity and the verification of the move. Empty
test roots are retained too. This is a relocation, not a deletion of failed-test
evidence. Original source archives, logs, protected binaries, and private `/tmp`
experiments have not been removed or installed.

Historical receipts and handoffs record the paths that existed when their checks
ran. Consult the relocation manifest before trying to inspect those fixtures at
their old locations. If a reproduction requires an original path, restore only
that exact directory after checking that its old name is unoccupied. Do not run a
bulk restore while tests or Files are using the same locations.

The checkpoint does not establish complete Files acceptance. The current source
matches the 903-input schema60 manifest before this documentation/cleanup change;
the scoped test results and known timing/recovery gaps remain described in
`IMPLEMENTATION.md`, `MOVE-RETURN-NEXT.md`, and ADR0152. The uutils implementation
has not begun; see `UUTILS-BACKEND-PLAN.md` for the next task.

Checkpoint checks: vendor provenance, workspace formatting, syntax for 98 Python
scripts, acceptance JSON, locked workspace metadata, service ordering, and the
staged whitespace check pass. The updater suite passed 18 of 19 tests; its
partial-output timeout test failed because the expected stderr was absent after
the 0.2-second timeout. That failure remains open. Full workspace/native/release
acceptance was not rerun for this source checkpoint.
