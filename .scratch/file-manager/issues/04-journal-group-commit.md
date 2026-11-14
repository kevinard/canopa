# Group commit for small-file Copy journals

Status: done (ADR0180, branch files-batched-copy-journal)

## Context

Raised 2026-11-14 by the competitor comparison: small-file Copy is the clearest
measured gap. After ADR0165 (batched barriers) and ADR0177 (cached statements),
10,000 × 4 KiB files take 14.7 s on tmpfs and 48.6 s on ext4. `gio copy` plus
`sync` takes 0.43 s and 1.17 s. The ADR0177 profile puts most of the
remainder in five journal transactions per item, each of which must reach the
kernel before the next filesystem step:

1. register the empty stage (`register_preparing`);
2. register the actual payload leaf before its first write;
3. write the Forward intent;
4. append the completion row with its cleanup (`append_with_cleanup`);
5. clear the intent after the stage folder is removed.

## Proposal

Register work for a batch of up to N small items (for example 64 items or 4 MiB)
per transaction instead of one item per transaction:

- one transaction registers N empty stages, then N payloads are created;
- one transaction registers the N payloads and their Forward intents;
- data is copied and published item by item;
- one transaction appends N completions and cleanups;
- stage folders are removed, then one transaction clears the N intents.

That is about five transactions per batch instead of per item. With the 32 MiB /
1 s barrier batching of ADR0165 they also share syncs.

## Proof obligations

This is a schema and recovery change. Each must be designed and tested before
implementation:

- `pending`/`recovery_pending` and `stage_intents` are singletons per lane.
  Several pending intents per lane need a keyed table, a migration, and an
  upgrade path for an interrupted journal written by the previous version.
- Restart reconciliation must handle every mixed state of a batch: some stages
  registered but not created, some payloads written, some published, some
  completions appended. The ADR0006 crash cases must be extended to kill the
  process at each of those points with several items in flight.
- Undo ordinals (`max(ordinal)+1` in `stage_intent::next`) assume one step in
  flight. A batch needs reserved ordinals that stay contiguous when only part of
  the batch completes.
- Conflicts, Replace, failures and cancellation must flush and close the batch
  first, so they still start from a durable single-item state (ADR0165's rule
  for Replace).
- Progress, the restart cursor and `deferred_item` accounting must count items
  inside a batch exactly once.
- Directory items stay unbatched at first: their creation and finish
  (`DirectoryFinish`) carry folder originals and access records.

## Acceptance

- 10,000 × 4 KiB on ext4 within 5× `gio`+sync (from about 41×), measured with
  `scripts/measure-files-copy.py` against a frozen baseline.
- Every existing transfer, Undo, Recovery, resume and interrupted case passes
  unchanged, plus new kill-point cases with partly completed batches.
- No behaviour change for Move, Replace, Recovery copies or Undo returns.

## Comments

### 2026-11-14: measured, exact speed-ups done, design proposed

- The profile shows Copy is CPU- and syscall-bound, not commit-bound: about 778
  system calls per small file, about 40% of them in `transfer::checkpoint`. With
  the journal on tmpfs, journal I/O was about 5 s of a 27 s ext4 run.
- The exact speed-ups (same journal states, same checks) are done in
  [ADR0179](../../../docs/files/adr/0179-exact-checkpoint-costs.md): −26% CPU,
  778 → 578 system calls per file.
- Group commit as proposed above does not fit the current model. The cursor
  binds steps to one active task, and the binding, digest and cursor checks are
  per transaction. A concrete design (a `CopyBatch` singleton intent, a cursor
  head range, five transactions per batch, recovery per phase, and the proof
  obligations) is in
  [ADR0180 (proposed)](../../../docs/files/adr/0180-batched-copy-journal.md),
  for review before implementation. Its open questions are at the end of that
  ADR.

### 2026-11-14: implemented

ADR0180 is implemented on branch `files-batched-copy-journal`. 10,000 × 4 KiB
on ext4 now takes 4.9 s of CPU and about 12 s wall, against 2.4 s for `gio`+sync
(about 5×, the acceptance target here). Kill-point cases cover every batch
phase, both without and with a restart cursor.
