# 0180 Batched Copy journal

Status: accepted and implemented 2026-11-14 (see [Implementation](#implementation)
and [Results](#results)); proposed the same day. Follows
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

## Implementation

`operations/copy_batch.rs` (the protocol, recovery and gathering) and
`operations/store/copy_batch.rs` (the binding and its transactions). The
review questions were settled as follows, and the implementation differs from
the proposal where noted.

- **Size.** At most 64 files, 4 MiB of content and about 96 KiB of encoded
  intent. Every journal value has a 128 KiB budget, and 64 long paths with
  their proofs exceed it, so gathering estimates each file's share. Before
  saving Forward, the batch drops copies from its end until the intent
  actually fits.
- **At least 8 files.** Fewer consecutive plain files keep single-item steps,
  so a small copy keeps its exact per-file visibility. Batched files appear
  together when their batch publishes.
- **Move, Replace, Merge, Recovery copies and Undo** stay single-item.
- **`user_version` 61.** Older Files refuses the history instead of misreading
  a batch.
- **Binding.** `copy_batch_intents` holds one digest over the history
  identity, the lane and the encoded intent, plus the history file's
  generation, checked before every transition and before recovery touches a
  file.
- **Restart cursor.** There is no new head structure. The Forward transaction
  writes one step receipt per copied file, bound in order to the consecutive
  queued tasks (the head and the tasks after it). Completion retires the
  receipts of unpublished files. Restart verification and `cursor::advance`
  accept a run of plain Copy steps, one per consecutive task. A step that is
  not a plain copy keeps the old limit of two steps for the head task.
- **Failures.** A file whose copy fails ends the batch and is reported as a
  failure, exactly as its single step would be. A failure that means
  cancellation or an unavailable destination instead returns the file to the
  queue, where the single-item step pauses or stops. Files after it return to
  the queue unpublished.
- **Cancellation** publishes the files whose copies had finished.
- **A batch that fails as a whole** (a journal error, for example) is retired
  by its own recovery. The transfer then continues with single-item steps.
- **Keep in Recovery.** A batch whose recovery refuses (an unknown entry in its
  stage, a changed copy) is listed as one work folder. Keep archives the whole
  stage as a Recovery record and retires the journal without moving, removing
  or appending anything.
- `prepared_copy::fill_regular` is shared by single and batched copies, so
  content, holes, attributes, metadata and fingerprints are produced
  identically.

## Verification

`transfer::copy_batch_boundary::tests` cover:

- one Undo for a batch;
- a name taken during a batch (that file is asked about, the rest publish);
- a source changed during a batch (it alone fails);
- cancellation;
- long paths still batching inside the journal budget;
- a SIGKILLed engine at registration, payloads, Forward, after 1 and 5
  publications and after the append: reopening leaves no pending journal and
  no stage, and exactly the published files have one Undo step each;
- the same kills through the job service with a restart cursor: after
  Reconcile and Continue, every file arrives exactly once, with no stage and no
  "(2)" copies;
- Keep in Recovery of a stuck batch.

The Files library (516), the full `integration` executable (651) and the
`uutils_copy` checks pass.

## Results

The release probe (`examples/copy_throughput.rs`) compares the ADR0179 build
with this one, alternating, three rounds each, as median seconds. The machine
was shared with other builds, so ext4 wall times are noisy; CPU time is the
reliable comparison.

| Workload | ADR0179 CPU | Batched CPU | Batched wall | `gio`+sync wall |
| --- | ---: | ---: | ---: | ---: |
| tmpfs, 1,000 × 4 KiB (journal on tmpfs) | 1.47 | **0.42** | 0.48 | |
| ext4, 10,000 × 4 KiB (journal on ext4) | 19.1 | **4.88** | **12.0** | 2.4 |

System calls per file (1,000 files) fall from 578 to 213 on tmpfs, and WAL
frame writes from 73 to 6.6. On ext4, 10,000 small files are now about 5×
`gio`+sync, down from about 40× (ADR0177). That meets the acceptance target
first set in `.scratch/file-manager/issues/04-journal-group-commit.md`.

The full native Files matrix passes against a debug build of this change
(`.scratch/artifacts/files-acceptance-batch/`), including the 25-file
transfer, which now runs as a batch in the app. Twelve cases first failed while
another session's builds loaded the machine; all passed when rerun serially on
a quiet machine. `files-iced-claim-review` now expects history version 61.

## Not verified

- Power loss: as ADR0165 says, the suite establishes process interruption, not
  power-loss acceptance.
