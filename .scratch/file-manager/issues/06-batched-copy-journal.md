# Batched Copy journal (ADR0180) — implementation plan and log

Status: done in the worktree, pending native acceptance and commit

Branch `files-batched-copy-journal`, worktree `.scratch/artifacts/batch-wt`
(off `main` at `ccd424a9`). Build with the root `target/`
(`CARGO_TARGET_DIR=/home/kevinard/projects/canopa/target bash scripts/cargo.sh ...`
from the worktree). Disk was at 25 GB free; do not add a second target dir.
Another session edits Shell/Settings in the main tree; never touch those.

## Decisions taken for the ADR0180 open questions

1. Batch limit: 64 items or 4 MiB of content, whichever first (fixed).
2. Move stays single-item.
3. `user_version` bump; older Files refuses the history.

## Why not "one lane per batch item"

The journal is lane-parametric (Primary/RecoveryCopy; 26 `match lane` sites,
4 `CHECK(lane IN (0,1))` tables), so N virtual lanes would reuse every
single-item check. Measured against the ADR0179 profile it only amortises
commits, WAL frames and shm locks (~20%), because the per-item
binding/digest/cursor verification (~30%) would still run per item. ADR0180's
single CopyBatch intent with one binding per batch is the design that reaches
~2x. Keep this note: the virtual-lane idea is the fallback if M3 (cursor)
proves intractable.

## Milestones

- M1 Types and schema: `Pending::CopyBatch(Box<CopyBatch>)`, phases
  Preparing/Forward/Cleanup{published}, batch binding (one digest over the
  encoded batch, history original, cursor task range), VERSION bump,
  `interrupted()`/Recovery listing for a CopyBatch, `has_pending`.
- M2 Engine without a cursor: gather eligible items (Copy, regular file or
  symlink, same source and destination folder, no name at destination, no
  reviewed replacement, <=64 items / 4 MiB); one batch stage; payloads named
  by index; copy; one Forward transaction (ordinals first..first+N-1);
  publish in order with NOREPLACE, stop at first failure; append prefix +
  Cleanup; remove the rest; clear. Unpublished tail goes back to the queue
  front. Library tests for happy path, mid-batch conflict, source change,
  cancellation before/after Forward.
- M3 Cursor: head range (first task position + count); step receipts for every
  ordinal of the batch; checkpoint before step 1 and after the append.
- M4 Recovery per phase + cfg(test) cut points at every boundary and between
  publications (copy_boundary style); restart has no duplicate conflicts and no
  lost/extra Undo steps.
- M5 Recovery review/Keep for a CopyBatch, measurements
  (scripts/measure-files-copy.py, syscall counter in the scratchpad), ADR0180
  -> accepted with results, native transfer/recovery acceptance cases.

## Log

- 2026-11-14: plan written; studying store write paths (register_preparing,
  intent, append_with_cleanup, clear_intent), stage_intent, context,
  transfer_cursor, staging recovery.
- 2026-11-14 M1+M2 done in the worktree (uncommitted):
  `operations/copy_batch.rs` (types, validate/transition, run, recover,
  gather; MIN_ITEMS 8, MAX 64 / 4 MiB), `store/copy_batch.rs` (binding table
  `copy_batch_intents`, begin/advance/complete, require), VERSION 61,
  `append_rows` split from `append_transaction`, `prepared_copy::fill_regular`
  shared with single copies, `Pending::CopyBatch` + Recovery listing (whole
  stage as one work folder), transfer.rs fast path at loop top (cursor None
  only). Failed file = failure like single step (pause-type errors go back to
  the queue); cancel publishes the finished prefix.
  Tests: `transfer::copy_batch_boundary::tests` (one Undo, name taken mid-batch,
  changed source, cancel, SIGKILL at registered/payloads/forward/published-1/
  published-5/appended) all pass; lib + integration pass apart from
  load-sensitive resume helpers (14 s deadline, pass at low load).
  Measured (release probe, tmpfs 1k x 4 KiB, CPU): 1.47 s -> 0.42 s; syscalls
  578 -> 213 per file; WAL pwrite 73 -> 6.6 per file.
- Next: M3 cursor (app transfers always have a cursor, so no app benefit yet).
- M3 cursor: receipts per copied file bound to consecutive queued tasks;
  `verified_steps` and `cursor::advance` accept a run of plain Copy steps;
  completion retires unpublished receipts. Job-service kill tests (payloads,
  forward, published-5, appended) resume every file exactly once.
- M5: Keep in Recovery for a stuck batch (`copy_batch_review`,
  `keep_copy_batch`); test with an unknown file in the stage.
- Bug found by measuring: a 64-file Forward intent with long paths exceeded
  the 128 KiB journal step budget, so batches failed and fell back on ext4.
  Gathering now estimates the encoded size (96 KiB) and Forward shrinks to fit;
  test `long_paths_still_batch_inside_the_journal_budget`.
- Results: ext4 10k CPU 19.1 -> 4.88 s, wall ~12 s vs gio+sync 2.4 s; tmpfs
  1k CPU 1.47 -> 0.42 s. Library 516, integration 651, uutils 7 pass.
- Remaining: native acceptance on a build of the branch; commit when asked.
- Native matrix on frozen4 (branch debug build): 97/109 stock first pass under
  load (another session building); the 12 failures pass serially at low load
  (claim-review needed `user_version` 61 in its fixture). Fork DnD 3/3.
  Receipts: `.scratch/artifacts/files-acceptance-batch/`.
