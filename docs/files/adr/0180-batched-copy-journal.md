# 0180 Batched Copy journal (proposed)

Status: **proposed 2026-11-14, for review before implementation.** Follows
[0165](0165-batched-copy-durability-and-progress.md),
[0177](0177-small-file-copy-costs.md) and
[0179](0179-exact-checkpoint-costs.md). Replaces the proposal in
`.scratch/file-manager/issues/04-journal-group-commit.md`.

## Problem

After ADR0179, a small new file still costs about 1.3 ms of CPU and about 580
system calls on tmpfs. `gio copy` needs about 15. The rest of the cost is the
per-file journal protocol, which is built for exactly one file in flight:

- **One pending intent per lane.** `pending` and `recovery_pending` hold one row
  (`singleton=1`), and every recovery path reads that one intent.
- **Five transactions per file:** the empty stage, the payload, the Forward
  intent, the completion with its cleanup, and the clear. Each one installs or
  verifies a `stage_intents` binding (an encoded copy and a SHA-256 digest),
  reads back the cursor task (`transfer_cursor::stage_task`) and the history
  file's identity, and writes about 15 WAL pages. That is about 70 WAL frames
  and 58 WAL lock calls per file.
- **One active cursor task.** The restart cursor binds a filesystem step to its
  single active task (`active_head`), and at most two step receipts
  (`ordinal - first <= 1`) belong to one task.

Every per-transaction cost scales with files. The only way to amortise them is
for one journal state to describe several files.

## Proposal

### Scope

Batching applies to consecutive queue items that are:

- part of a Copy (not Move, Replace, Merge, Recovery copies or Undo);
- regular files or symbolic links with no reviewed replacement;
- children of the same source folder, going to the same destination folder;
- free of a name at the destination when the batch is gathered.

A batch holds at most 64 items or 4 MiB of content, whichever comes first. Any
other item, and the first item that fails a gathering condition, ends the batch
and runs through today's single-item path.

### Journal: one intent describing the whole batch

A new pending variant keeps the singleton invariant, so every check of "is
work pending" keeps working:

```text
Pending::CopyBatch(Box<CopyBatch>)
CopyBatch {
  id, first: u64,            // ordinals first..first+N-1 reserved at Forward
  stage: Stage,              // ONE private stage folder in the destination folder
  phase: Preparing | Forward | Cleanup { published: usize },
  items: [ { task: cursor position, source, name, payload: "0".."N-1",
             payload_identity, ownership, reproduction, change: Option<Change> } ],
}
```

Its `stage_intents` binding has the same fields as today, with one digest over
the encoded batch. The binding's step is `first`. The database `user_version`
goes up by one, so an older Files refuses the history ("unsupported operation
database version") instead of misreading a batch.

### Phases: five transactions per batch instead of five per file

1. Create the batch stage, then register `CopyBatch{Preparing}` with the stage
   proof and the item list. Today's rule applies: create, then register what was
   created.
2. Create the N payloads (`O_EXCL`, named by index) and register their
   identities and ownership in one transaction.
3. Copy each payload's data, restore its metadata and fingerprint it. Syncs are
   deferred to the ADR0165 barrier as today.
4. One transaction for `CopyBatch{Forward}`: the N `Change::Copied` records with
   ordinals `first..first+N-1`, plus N cursor step receipts.
5. Publish each payload with `renameat2(NOREPLACE)`, checking the destination
   folder before each one. Stop at the first failure.
6. One transaction appends the published prefix's k changes (contiguous ordinals
   from `first`), sets `Cleanup{published: k}`, and advances the cursor past k
   tasks.
7. Remove the unpublished payloads (copies only), then the stage, then clear.

That is five transactions per batch. WAL pages shared by the batch's rows are
written once per transaction instead of once per file.

### Recovery

- **Preparing:** remove payloads whose recorded identity and ownership match,
  then the stage if nothing else is in it. Unknown entries keep the stage for
  Recovery review, as an unknown leaf does today. The cursor is unchanged, so a
  resumed transfer copies the batch again.
- **Forward:** for each item in order, the item is published if its destination
  name holds the payload's proven object (its `Original`). Publications form a
  prefix, because they run in order and stop at the first failure. Append the
  prefix with ordinals from `first`, advance the cursor past it, then continue
  as Cleanup. A non-prefix pattern stops with attention and keeps everything.
- **Cleanup:** finish removing the remaining payloads and the stage, then clear.

Undo is unchanged. The appended rows are ordinary `Change::Copied` steps, so
each file is undone, retained or Recovered as today.

### Restart cursor

The active head becomes a head range: the first task position and a count. A
step receipt binds `(task position, ordinal)` for every ordinal in
`first..first+N-1`, which generalises today's `ordinal - first <= 1`. The cursor
checkpoints once per batch, before step 1 and after step 6.

### What ends or splits a batch

- **A name appears** between gathering and publication: `NOREPLACE` fails, k
  items are appended, and the rest go back to the front of the queue. The
  single-item path then asks the usual conflict question.
- **A source changes or fails** before step 4: that item leaves the batch, and
  the single-item path reports it. Sources are never modified by Copy.
- **Cancellation** before step 4 discards the batch. After step 4 it stops
  publishing and appends the prefix.
- **A question or failure** flushes the ADR0165 barrier first, as today.

## Proof obligations

- Kill-point cases (the ADR0006 harness) with several items in flight, at each
  phase boundary and between two publications. Each checks that the restarted
  history and cursor have no duplicate conflicts, no lost or extra Undo steps,
  and no unknown entries outside the stage.
- The refusal of the new history by the previous version, and no migration of
  old journals (they hold no batch).
- Every existing transfer, Undo, Recovery, resume and interrupted case passes
  unchanged.
- Progress, cursor and `deferred_item` accounting count each item once.

## Expected effect

From the ADR0179 profile: the binding, digest, cursor and history reads, the
commits, the WAL frames and the WAL locks become per batch. That is roughly half
of today's per-file CPU. What stays per file is the copy, its fingerprint,
creating and publishing the payload, and the destination check before each
publication. Expect roughly 2× on tmpfs and somewhat more on ext4, where WAL
writes and checkpoints also shrink. Files would still be far slower than `gio`
for very small files. The remaining per-file proofs are the price of verifiable
Undo and Recovery.

## Questions for review

1. Is a fixed batch limit of 64 items or 4 MiB right, or should it follow the
   ADR0165 barrier (1 s or 32 MiB)?
2. Should Move join later? Its per-item source removal needs its own intent, so
   this proposal leaves it out.
3. Is refusing old Files versions (the `user_version` bump) acceptable, rather
   than keeping batches out of histories that older versions might open?
