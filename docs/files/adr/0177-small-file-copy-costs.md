# 0177 Small-file Copy costs: cached journal statements and a profile

Status: accepted 2026-11-14. Follows
[0165](0165-batched-copy-durability-and-progress.md), whose barriers, guarantees
and proposals are unchanged.

## Profile

A release probe (`examples/copy_throughput.rs`, line tables, symbols kept) copied
10,000 × 4 KiB files on tmpfs, where syncs are free, so only per-item CPU is
left. `eu-stack` sampled the process 502 times (Yama allows it through
`PR_SET_PTRACER` in a wrapper; no `perf` is installed). Shares are inclusive:

| Share | Where |
| ---: | --- |
| 88% | inside `Store::deferred` spans (the per-item steps) |
| 25% | `append_forward` / `append_with_cleanup` (completion and cleanup rows) |
| 23% | `transfer::checkpoint`: re-proving the destination folder, the source entry and recorded folder originals (`openat`, `statx`, `name_to_handle_at`, `close`) |
| 20% | `register_preparing` (empty stage, then payload) |
| 17% | `sqlite3_step`, of which about half is `COMMIT` writing WAL frames |
| 10% | **SQL parsing** (`sqlite3Prepare`), almost all from uncached statements in `store::stage_intent` |
| 10% | the content copy itself (`copy_regular`, splice) |
| 7% | SHA-256 of journal bindings (`stage_intent::digest`, `context::forward_digest`) |

Leaf time is mostly syscalls: `statx` 13%, `close` 8%, `name_to_handle_at` 7%,
`pwrite` (WAL) 6%, `fcntl` (descriptor duplication) 5%, `openat` 6%.

## Decision: cache the hot journal statements

`store::stage_intent`, `store::stages` and `store::directory_access` now run
their connection-level `query_row`/`execute` calls through the existing
`CachedSql` helpers. The SQL text, parameters and transaction boundaries are
unchanged; SQLite only skips re-parsing. The statement cache holds 128
statements.

Switching the span pragmas (`synchronous`, `wal_autocheckpoint`) to cached
statements was also tried and measured. It made no difference and was
reverted: their cost was the `COMMIT` that shares `execute_batch`, not the
parsing.

## Measurements

`scripts/measure-files-copy.py`, release probes of the same tree with only the
store files differing, alternating in each round, three rounds, median wall
seconds. The SSD was shared with other work (IO pressure about 30%).

| Filesystem | Workload | Before | Cached | `gio`+sync |
| --- | --- | ---: | ---: | ---: |
| tmpfs | 100 × 16 KiB | 0.183 | **0.162** | 0.011 |
| tmpfs | 10,000 × 4 KiB | 16.69 | **14.72** | 0.43 |
| tmpfs | mixed, 829 items | 1.99 | **1.84** | 0.21 |
| ext4 | 100 × 16 KiB | 0.273 | **0.259** | 0.029 |
| ext4 | 10,000 × 4 KiB | 51.6 | **48.6** | 1.17 |
| ext4 | mixed, 829 items | 10.17 | 10.19 | 3.32 |

CPU per small item falls from about 1.67 ms to 1.47 ms (12%). ext4 gains less
because journal write volume, not parsing, dominates there. Receipts:
`target/copy-bench-2026-11-14/results.jsonl` and `summary.txt`.

## Large folders

`examples/folder_open.rs` times the browser's folder snapshot (read, per-entry
metadata and sort, off the UI thread) and a filename search of the same folder.
Release build, warm cache, three rounds:

| Filesystem | Entries | Snapshot | Search | `ls -l` |
| --- | ---: | ---: | ---: | ---: |
| tmpfs | 10,000 | 20 ms | 23 ms | |
| tmpfs | 100,000 | 203 ms | 222 ms | |
| ext4 | 10,000 | 26 ms | 29 ms | 55 ms |
| ext4 | 100,000 | 286 ms | 315 ms | 424 ms |

About 2–3 µs per entry, faster than `ls -l` on the same folder. Reading is not a
bottleneck for large folders. Cold-cache reads (which need dropping the page
cache as root) and the time until the first rows are drawn in a native session
are not measured. Receipts: `target/copy-bench-2026-11-14/folders/`.

## What remains

Small files are still about 34× `gio`+sync on tmpfs and 41× on ext4. The rest is
structural:

- **Five journal transactions per item**, each separated from the next by a
  filesystem step, so they cannot be merged without new recovery states. On
  ext4 their WAL volume is most of the 48 s.
- **Identity proofs at every checkpoint** (about nine per small file). They are
  the protection against a replaced destination or source folder, and are kept.
- **SHA-256 bindings** of pending intents. These digests are part of saved
  journals, so changing the algorithm breaks reading an interrupted journal from
  an older version.

Group commit, registering several items' stages, intents and completions per
transaction, is the only change that addresses the first and largest cost. It is
not attempted here. A later profile ([0179](0179-exact-checkpoint-costs.md))
found the cost to be mostly system calls in repeated checks, and removed a
quarter of it exactly. The batched protocol is proposed in
[0180](0180-batched-copy-journal.md).

## Verification

The library suite, the transfer, Recovery, resume, interrupted and Undo
integration cases, and the full `integration` executable.
