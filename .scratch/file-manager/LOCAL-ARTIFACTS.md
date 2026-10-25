# Local Files artifacts and retention policy

The implementation was committed on 2026-10-25 before the uutils migration:
`88d78d3` records Files and its integration, and `4e7fd47` records shared windowing
support. Source snapshots can now be recovered from Git.

The user subsequently requested deletion of obsolete binaries, source snapshots,
and test captures, retaining current failure reproductions and the latest
verification records. This replaces the blanket artifact-retention instructions
in older handoffs. Historical references to removed archives or executables are
records of past checks, not a requirement to recreate those artifacts.

The cleanup preserves all committed plans, product documents, ADRs and Markdown
notes, the latest schema60 verification records and source manifests, and the
small current-failure bundles indexed in `CURRENT-FAILURES.md`. Local generated
records remain ignored. `artifact-cleanup-2026-10-25.json` records the measured
size change, retained-file hashes, and deletion counts.

The 142 test roots previously moved into `retained-fixtures/` were historical
diagnostic outputs and are removed by this cleanup. The current Move/Undo and
performance failure bundles are retained separately. Historical fixture-path
references may therefore no longer exist. Private `/tmp` experiments, root
`target/`, Git history, and installed executables are outside this cleanup.

The checkpoint does not establish complete Files acceptance. The 903-input
schema60 source manifest and its scoped results remain valid for their recorded
source. Known timing/recovery gaps remain described in `IMPLEMENTATION.md`,
`MOVE-RETURN-NEXT.md`, and ADR0152. The uutils implementation has not begun; see
`UUTILS-BACKEND-PLAN.md` for the next task.

Checkpoint checks: vendor provenance, workspace formatting, syntax for 98 Python
scripts, acceptance JSON, locked workspace metadata, service ordering, and the
staged whitespace check pass. The updater suite passed 18 of 19 tests; its
partial-output timeout test failed because the expected stderr was absent after
the 0.2-second timeout. That failure remains open. Full workspace/native/release
acceptance was not rerun for this source checkpoint.
