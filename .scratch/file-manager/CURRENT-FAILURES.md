# Current failure reproductions retained after cleanup

Updated 2026-10-25. The production implementation is checkpointed in Git at
`88d78d3`; the uutils migration has not started. All paths below are relative to
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
limit. `direct-displacement-related-02.log` records the production 29.84-second
failure. The retained timing/canonical profiling logs are private diagnosis,
not newer production acceptance or permission to change the limit.

`direct-displacement-timing-fixtures-01.tar.gz` and its matching JSON manifest
preserve the failed `/tmp/canopa-destination-undo-crash-Y5Xc23` root, 1,507 entries,
SHA-256 `f49351e708ef22a205e5f7adf984d8312acb401f169301066a534c7ba69b441b`.

## Updater timeout check

The checkpoint's `scripts/test-update.py` run passed 18 of 19 tests. The
`test_timeout_reports_command_and_decoded_partial_output` case lacked its expected
stderr after the 0.2-second timeout. The permanent test is the reproducer; it
does not require an archived executable or retained native-test directory.

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
