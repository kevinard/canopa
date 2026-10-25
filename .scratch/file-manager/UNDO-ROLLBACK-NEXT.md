# Displaced Undo compensation checkpoint

## Authoritative state

The full five-area Files goal remains active and unchanged. This is a completed
restart-recovery slice, not completion of Copy/Move, Undo or the whole product.

Source: undo-rollback-inputs-09.json, 891 unchanged inputs, schema53.
- Focused green05: 26 parents pass, 2 ignored child helpers, zero failures.
- Backend driver01: 545 pass across 20 targets, 49 ignored, zero failures or filters
  in the parent runs. Lib01 and batches10–19 all passed. Both required active and
  stopped batch restart gates passed in core12.
- Normal isolated desktop-feature lib01: 311 pass, 29 ignored, zero failures or
  filters. Receipt: continuation-core-1792825465-receipt.json. Private HOME/XDG and
  private D-Bus; no live desktop/session activation.
- Default all-target Clippy03 with -D warnings, Files format01 and fixture-check01
  schema53/syntax passed. No broader native/workspace/release result is claimed.

Backend exec3401 and desktop exec38785 terminated0. Focused24789, Clippy95410,
format/fixture37350 and final verification35068 also terminated0. No current run
remains live. undo-rollback-verification-01.json records the exact source relation,
attempts, counts, gates, receipt and protected binary hashes.

Source/evidence bundle: undo-rollback-source-01.tar.gz. Its verification JSON records
all member hashes; undo-rollback-source-final-live-01.json independently verifies
the decoded archive, live members and protected binaries after archiver completion.
Consult these records for the actual final archive hash. All failure originals and
older archives remain preserved.

## Implemented behavior

SourceRemoved Undo Replace now keeps its original Undo owner through verified copy
disposal and a separate one-shot displaced-item return. CopyCleared precedes return;
Skipped/Reserved/Claimed/Consumed/Returned states bind original work roots and actual
selected entries. Save Consumed before the live NOREPLACE return. Restarted Consumed
never repeats it. Returned receipts permit only empty-root cleanup and do not touch
later target occupants. Failed or ignored phase, receipt and retirement saves keep
the exact earlier owner. Original Undo history remains available; retry gets a new
conflict answer.

Empty or unallocated copies use explicit empty-copy compensation with no file unlink
rights. A missing copy stage uses the displaced stage as a placeholder and skips
copy cleanup of that root until the return receipt. This covers initial preparation,
a removed payload, a moved-away copy and disappearance between guard and root open.
It removes the SourceRemoved missing-payload path through legacy repeated return.

Recovery Copy/Keep snapshots cover four original work roots, available leaves and
whole folders including extra children. Existing Pending slots0–7 keep their meaning;
extra primary parts4–7 use slots8–11, secondary extras use12–15. Public extra-slot
Recovery Copy, Keep, export Undo and original Undo retry are tested.

Production entry: operations/staging/disposal/undo_return.rs. Focused tests:
operations/undo/abort_boundary/disposal_boundary/rollback_checks.rs. ADR0145 and the
ledger describe this checkpoint. Main application and Niri were not rebuilt.

## Preserved attempts

- Source01/red01 and red02: unchanged public Undo Replace required manual Keep
  despite original owner/root proofs and a matching partial prefix.
- Source02/green01: first return protocol, 18 parents pass.
- Source03/green02 and collision-red02: foreign-root test incorrectly expected
  replacement bytes to appear as owned Recovery data. Actual original and foreign
  bytes were intact and root inodes differed. This was a harness assertion error.
- Source04/green03: corrected refusal/access assertion plus previous-name reuse,
  24 parents pass.
- Source05/Clippy01: one question_mark lint. Source06/Clippy02 passes after equivalent
  propagation. Source05 also adds accurate Undo/all-work-folders Recovery messages.
- Source07/empty-red01 and empty-red02: a real repeated namespace consumption. After
  a missing-payload legacy return, restarting moved a reintroduced previous-name
  hardlink again and cleared both owner rows. empty-red-probe01 records the actual
  pending direction, absent previous name, same target/held inode and zero owners.
- Source08/green04: empty-only compensation fixes that regression, 25 parents pass.
- Source09/green05: adds first empty-copy registration death, whole copy moved aside
  and post-guard copy disappearance; final 26 parents pass. Clippy03 and all final
  broader/static checks use this exact source.

Failure bundle undo-rollback-failure-fixtures-01.tar.gz has 12 roots and 74 entries,
SHA256 051c473d5c56d2cb03f8054311e2211c6d017de8d448524a8b4627d642d96a6a.
Decoded verification and separate live verification confirm original bytes, modes,
mtime and object identities, with no permission borrows. The manifest lists every
preserved root. Private output roots live under crates/canopa-files, not repo root.

Predecessor: undo-displaced-inputs-04.json, schema52, 889 inputs, 536 backend and
302 normal desktop-feature tests. Its source archive hash is
b003a3131d0a62cb9edc9ede67f7fcc6012394ec7b7c9b269d4342c3cd499413.

## Next work

Next priority is durable cleanup after completed reversals. Inspect the public Undo
completion transaction and clean_completed_stage call sites, then establish an
actual public-worker crash test at the commit/cleanup boundary before changing
production code. The remaining generic/legacy Forward/Undo abort and directory-return
paths are not covered by this displaced SourceRemoved compensation protocol.

Other remaining requirements include broader Undo/expiry/queued lineage/large reports,
Trash/Restore, nav/search/preview backend separation, batch and cross-filesystem
Restore, Recovery retry/expiry, media/drag/device/accessibility/physical acceptance,
first-window/focus reliability, optimized startup/idle/large-directory/search
measurements, full native/workspace/Shell/Notify checks, compatible Niri distribution,
release/package smoke tests, documentation and GTK reference retirement.

## Working constraints

Read root/Files AGENTS, CONTEXT-MAP, Files context and relevant domain/ADRs. Diagnose
skill is in use: reproduce twice, show ranked falsifiable hypotheses before probes,
fix the real public pattern and preserve failed fixtures. No subagents were spawned.
Preserve unrelated dirty work, especially Notify; no commits, vendor changes or
worktrees. Do not narrow the goal or mark it complete for a slice.

Use CARGO_INCREMENTAL=0 TMPDIR=/tmp, -j1 --locked --offline, without Rust wrappers.
No overlapping Cargo mutations or replacing test ELFs while current_exe children
live. Reclaim only exact completed own test ELFs using reclaim-leaf-test-cache.py
under both Cargo locks and preserve its audit. Keep the required operation_transfer_resume
gate, all evidence and application/Niri binaries. Application SHA256 is
3f638343c9cc5ab73fea245013112edbd0e4e8d5189c55dac58b5e1e2ee95cf8;
Niri is 63e39bae347346d7ac5aa90fb974ee8fe09ee59d9bd69238546e990368232da7.
There was no auto-review rejection in this slice. All mutation tests use private
fixtures; never use real user folders. Preserve raw Unix paths and cross-filesystem
sources until commit. Do not invent further race findings without an actual repro.
