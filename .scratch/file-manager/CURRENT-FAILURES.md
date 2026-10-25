# Current failure reproductions retained after cleanup

Updated 2026-10-25. The pre-migration implementation is checkpointed in Git at
`88d78d3`; the scoped uutils Copy migration is recorded in UUTILS-IMPLEMENTATION.md
and ADR0153. It does not resolve the wider failures below. All paths below are relative to
this directory unless absolute. These are failure/research records, not full
product acceptance.

## Incoming Move/Undo name recreation

`incoming-return-private-red-01.log` records two failures and one passing control:
after a real incoming return and worker death, recreating the consumed name can
leave the completed Move eligible for another return, or fail to reduce a saved
Move to its saved-version remainder. The control without name recreation passes.

Keep `incoming-return-boundary-draft.rs`, `build-incoming-return-repro.py`, and
`incoming-return-private-build-01.json` as the reproducer source/build recipe.
The historical recipe uses existing Cargo fingerprints and exclusive output
names; use a fresh disposable build/output label when repeating it.

`incoming-return-fixtures-01.tar.gz` and its matching JSON manifest preserve the
two failing roots, 24 entries, SHA-256
`7018e529ddcecdf64ceb8188e59e26e6fcfaa0acc87669bd99e47bcd1a011ffc`.
Their original locations are `/tmp/canopa-incoming-return-FDSp7E` and
`/tmp/canopa-incoming-return-povQ2X`; this cleanup does not change `/tmp`.

The later three-test private green log and final private build log are retained
to distinguish the experimental candidate from production. Its location is in
`incoming-return-candidate-location.json`. It has known design gaps and must not
be installed merely because those three tests pass. The uutils plan supersedes
that experiment as the proposed next implementation direction.

## 500-item Undo performance

The existing public test
`restarting_a_killed_keep_both_destination_restore_preserves_newer_files_and_cleans_stages`
in `crates/canopa-files/tests/undo_destinations.rs` retains its original 20-second
limit. The uutils core run still fails at 24.998704253 seconds and retains
`/tmp/canopa-destination-undo-crash-bqjl27`; its log is
`/tmp/canopa-uutils-core-rest.log`. The limit is unchanged.
The optimized-hashing follow-up's fresh default-feature target fails at
28.052798044 seconds, retaining `/tmp/canopa-destination-undo-crash-MEl5sU`;
see `/tmp/canopa-copy-affected-tests.log` and COPY-HASH-PERFORMANCE.md. This
hashing change does not claim to solve the batch Undo timing gate.
`direct-displacement-related-02.log` records the production 29.84-second
failure. The retained timing/canonical profiling logs are private diagnosis,
not newer production acceptance or permission to change the limit.

`direct-displacement-timing-fixtures-01.tar.gz` and its matching JSON manifest
preserve the failed `/tmp/canopa-destination-undo-crash-Y5Xc23` root, 1,507 entries,
SHA-256 `f49351e708ef22a205e5f7adf984d8312acb401f169301066a534c7ba69b441b`.

## Additional schema60 Undo baseline failures

The uutils validation ran the complete public core target set and reproduced seven
failures in undo_unavailable_moves and one in undo_unavailable_saved. Direct
compilation of those same public test files against the preserved pre-migration
library reproduces identical failure names, errors and counts (5 pass/7 fail/1 ignored
and 8 pass/1 fail/1 ignored). These saved-only/source-only canonical history binding
gaps are inherited; no proof was weakened. See UUTILS-IMPLEMENTATION.md and
`/tmp/canopa-uutils-baseline-unavailable-{moves,saved}.log`.

## Recovery catalog baseline timeout

The uutils full default run and final 15-target run time out after 10 seconds in
`pages_remain_loaded_and_saved_identity_survives_active_undo_expiry`. Backtraces
locate the timeout at the initial Replace question, before Reader::start or byte
copying. Independently compiling the exact pre-migration HEAD source with matching
default dependencies reproduces the same timeout. Its cause remains open; see
`/tmp/canopa-uutils-baseline-catalog.log` and UUTILS-IMPLEMENTATION.md.
The optimized-hashing follow-up's normal workspace run reproduces the same
failure (three pass/one fail in that target); log:
`/tmp/canopa-copy-workspace.log`. The full Files library passes 370 cases and
affected follow-up targets are recorded in COPY-HASH-PERFORMANCE.md.

## Updater timeout check

The checkpoint's `scripts/test-update.py` run passed 18 of 19 tests. The
`test_timeout_reports_command_and_decoded_partial_output` case lacked its expected
stderr after the 0.2-second timeout. The permanent test is the reproducer; it
does not require an archived executable or retained native-test directory.
The uutils full validation passed all 19 updater tests; the older observation is
retained as history, not a current failure in this migration run.

## Latest verification

`refused-return-verification-01.json` indexes the latest schema60 results:
365 default-feature library parents, 102 integration parents across eight targets,
formatting/default all-target Clippy, and the explicitly bridged Undo crash group.
Keep its logs, original input manifests, bridge, and runner receipt. The embedded
source-archive and old protected-executable hashes are historical: those obsolete
scratch artifacts were deliberately deleted after the Git checkpoint.

Intermittent first-window/focus/media issues and the broader goal remain open as
documented in the ledger. No new successful native acceptance or stable native
failure reproduction is claimed by this cleanup. Maintained native fixtures are
already in Git under `scripts/`.
