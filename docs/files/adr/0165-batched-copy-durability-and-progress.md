# 0165: Batched Copy durability, write-behind and transfer estimates

Status: implemented 2026-11-11 (`875f445`, `9fc93b7` and the barrier-failure
follow-up for the engine; `7ef8021` progress; `5a8883b` benchmark). Verified
with the Files library, the transfer/Undo/Recovery integration cases and an
optimized headless benchmark. Not yet measured on removable media or in a native
session. Continues ADR0006 (full-sync transactions), ADR0153/0155/0156 (the
prepared-copy lifecycle) and ADR0158/0159 (journal costs).

## Where the time went

`scripts/measure-files-copy.py` drives the production engine through
`examples/copy_throughput.rs` (release profile, its own `target/copy-throughput`
directory) next to `cp -r` and `gio copy`, with and without a final `sync -f`.
An `LD_PRELOAD` probe (`scripts/fixtures/copy-sync-probe.c`) counts and times
`fsync`/`fdatasync`. Machine: i5-6500, Samsung 850 SATA SSD with ext4, and tmpfs.

A plain new-file Copy issued **9.3 `fsync` calls per file**: five journal
commits (`synchronous=FULL` WAL) for the empty stage, the payload, the Forward
intent, the completion with cleanup, and the cleared intent; the payload data;
the destination folder after publication; the stage folder; and the destination
folder again after the stage was removed. For 100 files that was 929 calls and
1.64–1.89 s of the 1.9–2.7 s total on an idle disk. On tmpfs, where `fsync` is
free, the same copy took 0.17–0.19 s: about **1.7–1.9 ms of CPU per item**,
spread over descriptor identity checks (`name_to_handle_at`, `statx`), journal
encoding with its digests, and SQLite. Hashing (XXH3) and data transfer are not
significant for small files. No single hotspot remains in that CPU cost.

## Decision: batch barriers for new-item Copy

Each Copy transfer opens a durability session (`operations/durability.rs`). The
plain new-item steps run inside `Store::deferred` spans:

- registering the empty stage and its payload, the Forward intent, the
  completion and cleanup, clearing the intent, and restart-cursor checkpoints;
- the payload data sync, the stage folder sync, the published parent's sync and
  the parent sync after the empty stage is removed;
- the same steps for a newly created folder.

Inside a span, journal commits use `synchronous=NORMAL`: each commit is still
written to the WAL before the next step starts, only its `fsync` is skipped.
The filesystem syncs record the destination filesystem instead of syncing.

A **batch barrier** runs `syncfs` once per destination filesystem and then syncs
the history database and its WAL. Filesystems come first, so a durable history
row never precedes its own data. It runs:

- when the batch is 1 s old or holds 32 MiB of new content;
- before a conflict or unavailable-destination question;
- before any Replace, and before failure handling;
- when the transfer returns, on every path. A finished Copy therefore means its
  data is on the device.

A failed barrier stays pending and is retried by the next one. If it still
fails, the transfer stops with attention ("Files could not confirm that the
copied data reached the device"), keeping its outcome and continuation, so an
unplugged destination still pauses the transfer instead of failing it.

Move (including cross-filesystem Move and its source removal), Replace,
Recovery copies, Undo, Trash and every other operation are unchanged: they run
outside spans with `synchronous=FULL` and their own per-step syncs.

`syncfs` replaces one `fsync` per file deliberately. On ext4 each file `fsync`
allocates that file's delayed blocks and commits its own journal transaction;
one `syncfs` allocates everything and commits once. It also writes back other
dirty data on the same filesystem, which can make a barrier slower on a busy
disk but never less correct. Writeback errors since the descriptor was opened
are reported by `syncfs` (Linux 5.8 and later) and fail the transfer.

### Why this keeps the documented guarantees

The crash tests in ADR0006 onward kill the copying process. A killed process
loses nothing that was already passed to the kernel, and every step still
reaches the kernel in the original order: WAL frames before the next filesystem
change, payload data before its publication rename, publication before the
completion row, the completion row before the stage is removed. Restart
reconciliation therefore sees exactly the states it sees today. The library's
child-process boundary tests and the integration crash cases pass unchanged.

`fsync` only matters for power loss or a kernel crash. ADR0108 already records
that the suite establishes process interruption, not hardware power-loss
acceptance. Within one unflushed batch a power loss can now leave:

- **a journal prefix.** Transfers, cursors and history share one SQLite
  database, and WAL recovery keeps a prefix of transactions, so the history is
  always a consistent earlier state.
- **new copies the history does not know.** For example a published file whose
  intent was lost. It stays as an ordinary file without an Undo entry, like a
  `cp` copy.
- **a published copy whose stage was also removed.** Ordinary copy recovery
  already completes a pending Forward from the published identity when the stage
  is missing (`staging/publication.rs`).
- **a published copy whose data did not reach the device.** Its recorded
  fingerprint no longer matches, so Undo treats it as a changed copy and
  preserves it instead of deleting it.

Sources are never touched by a deferred step, and no existing destination data
is moved, replaced or retained inside a span. The worst case is losing up to
about one second or 32 MiB of new copies from an interrupted, not-yet-finished
Copy, plus possibly a spurious Recovery item. This is the explicit trade-off.

## Write-behind for slow media

A large output syncs its data (`fdatasync`) after every 32 MiB written, so at
most one window is dirty. Without this, a USB stick accepts a whole file into
the page cache at memory speed, progress jumps to the end, and the final sync
then stalls for the real transfer. Together with the 32 MiB batch limit and the
final barrier, reported progress stays within about one window of the device and
"done" is on the device. No `O_DIRECT` is used. This applies to every prepared
copy, including Move and Undo returns, because it only adds syncs.

## Progress, rate and remaining time

`operation_jobs/transfer_progress.rs` adds:

- **A cancellable pre-scan.** It counts the selected entries and bytes on its
  own thread, without following links, and stops when the job ends. Its partial
  counts show as "N of M+ items · A · counting…" until it finishes.
- **Totals.** "X of Y items · A of B", with a progress bar.
- **Rate and remaining time.** Items and bytes share one work measure (one item
  counts as 256 KiB), smoothed exponentially (4 s for the rate). The remaining
  time counts down between samples and absorbs new estimates with a 3 s time
  constant, so a burst of small files does not make it jump. No estimate is
  shown in the first 2 s or while counting.
- **The current file name**, as before.

Progress is published at most every 250 ms (about 4 Hz, previously 80 ms), so
UI wakeups no longer follow files or 64 KiB chunks. A resumed transfer scans
only its remaining tasks.

## Measurements

Release build of `e0f5bd0` (baseline) and of this change (candidate), run
alternately in the same rounds, cache-warm sources, private folders. Median
wall seconds, with the number of runs; the Canopa time includes opening a fresh
history. Other agents were building on the same SSD throughout (Linux IO
pressure 30–80 %), so ext4 numbers are noisy and slower than an idle disk: an
earlier idle baseline took 1.9–2.2 s for 100 files.

| Filesystem | Workload | Baseline | Candidate | `cp -r` | `cp`+sync | `gio` | `gio`+sync |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| ext4 | 100 × 16 KiB (n=3) | 16.13 | **0.80** | 0.009 | 0.100 | 0.044 | 0.088 |
| ext4 | 10,000 × 4 KiB (n=1) | 1001.5 | **64.8** | | 1.26 | | 1.55 |
| ext4 | mixed, 829 items, 117 MiB (n=1) | 102.4 | **7.6** | | 1.87 | | 1.98 |
| ext4 | one 1 GiB file (n=1) | 28.4 | **32.0** | | 30.6 | | 35.2 |
| tmpfs | 100 × 16 KiB (n=3) | 0.19 | 0.22 | 0.003 | 0.004 | 0.021 | 0.023 |
| tmpfs | 10,000 × 4 KiB (n=3) | 19.4 | 19.2 | 0.23 | 0.18 | 0.53 | 0.46 |
| tmpfs | mixed (n=3) | 2.34 | 2.41 | 0.062 | 0.063 | 0.32 | 0.34 |
| tmpfs | one 1 GiB file (n=3) | 1.24 | 1.29 | 0.35 | 0.35 | 0.35 | 0.35 |

`fsync` calls seen by the probe (libc only; `syncfs` is a raw syscall):

| Workload | Baseline | Candidate |
| --- | ---: | ---: |
| 100 small files | 929 | 14 |
| mixed tree | 7,541 | 41 |
| 10,000 small files | 91,223 | 354 |

In an earlier interleaved round with the first candidate (before the larger
auto-checkpoint), the mixed tree took 160.1 s against 15.2 s (n=3), and the
1 GiB file 43.8–68.4 s against 49.3–70.8 s, with `cp`+sync at 41.0 s. The
auto-checkpoint change alone took the mixed tree from 17.3 s to 13.4 s (n=2) and
its `fsync` calls from 143 to 51.

Reading the table:

- On ext4 the barrier change is a 13–20× speed-up for small files and folders.
- tmpfs has no barrier cost, so it shows that the per-item CPU is unchanged.
- One large file now runs at the disk's speed, within noise of `cp`+sync.
  On tmpfs it costs about 0.9 s/GiB more than `cp`, from the output hash and the
  per-64 KiB checks.
- Small files stay far from `gio`: 9× `gio`+sync for 100 files and 42× for
  10,000 on this loaded disk.

Receipts: `target/copy-bench/results.jsonl` and the `run-*.log` files next to it.
The frozen probe executables were removed after the runs; their SHA-256 values
were baseline `43cf618069ca7b3c26d667cf94aafad10dce9eb069588e54103c22451153b178`
and candidate `eeb575e2e6374d64d958248432099de8567a9f7739cb07f8d0f1a3a8cdf64878`.

## Remaining cost and proposals

Two costs of the per-item guarantees now set the floor for small files:

- **CPU.** About 1.9 ms per item on this i5-6500 for the journal and identity
  proofs, so 10,000 small files take about 19 s even on tmpfs. `gio copy` takes
  0.5 s.
- **Journal write volume.** About 83 WAL page writes per item (about 330 KiB for
  a 4 KiB file), from five transactions over several indexed tables. With the
  barriers batched, this write traffic is what remains of the ext4 cost: 65 s
  against 19 s on tmpfs for 10,000 files.

The "within 2× of gio" target is therefore **not met** for small files. Meeting
it needs group commit at the journal level: registering many stages, intents
and completions per transaction. That means more than one pending intent per
lane, which is a schema and recovery change with proof obligations for every new
state. It is not attempted here; it is the proposed next step.

Further options, not implemented:

- `copy_file_range`, and reflink on btrfs/XFS, instead of `splice` through a
  pipe. ADR0153 deliberately keeps uutils as the only data mover. This would
  need a seam in the fork or a reviewed exception.
- An overlapped write-behind (start writeback of window N+1 while waiting for
  window N) to recover read/write overlap for cold sources.

## Not verified

There is no writable removable drive on the measurement machine (its USB stick
is a read-only ISO), so USB throughput and progress honesty are argued, not
measured. Native GUI acceptance of the new progress row is pending.
