# 0179 Exact Copy speed-ups: fewer system calls for the same checks

Status: accepted 2026-11-14. Follows
[0177](0177-small-file-copy-costs.md). The journal protocol, its states and
its checks are unchanged. The batched protocol that would change them is
proposed separately in [0180](0180-batched-copy-journal.md).

## Where the time went

A ptrace syscall counter (no `strace` is installed) on 1,000 × 4 KiB files on
tmpfs, with the ADR0177 build:

| Call | Per file |
| --- | ---: |
| `statx` | 289 |
| `close` | 127 |
| `name_to_handle_at` | 95 |
| `openat` | 77 |
| `pwrite64` (WAL frames) | 70 |
| `fcntl` (WAL locks, descriptor copies) | 58 |
| `open` | 46 |
| all | **778** |

A plain copy needs about 15. Stack samples put about 40% of the calls in
`transfer::checkpoint`, which runs about eight times per file. Each checkpoint
proved the selected source folder three times: for the observed selection
(`lstat` and both handles), for the recorded folder original (both handles
again), and for its identity (`lstat` again). Each proof opened, captured and
closed the same two objects.

## Decision

Five changes make the same observations with fewer calls:

1. **One observation per path within a checkpoint.** `operations/proofs.rs`
   gives a checkpoint a scope. The first request for a path opens its parent
   (`O_PATH|O_DIRECTORY`) and entry (`O_PATH|O_NOFOLLOW`), as each check did,
   and captures identity, type and handles once. The observed-selection check,
   the folder-original check and the identity comparisons in that same call
   compare against those observations. A checkpoint only reads, so nothing Files
   does can change the objects between its checks. Outside a checkpoint, every
   request observes afresh. The scope is per thread, nests into the outer
   scope, and ends with the call. Errors keep their kind and code, and the checks
   still run in their original order.
2. **Folder-original proofs** open the parent and entry without the two
   `fstat` calls whose identities they never used.
3. **Stage, payload and publication identity reads** use one `fstatat` of the
   name in the held folder instead of `openat`, `fstat` and `close`. Both resolve
   the name without following a final link and report the same device and inode.
4. **The source snapshot's name check** compares the same fields (identity,
   size, mode, modification and change times) from one `fstatat` of the name in
   the held source folder, instead of opening it, statting it and closing it.
5. **The destination observation** takes its identity and its persistent proof
   from one `statx` (`Original::identify`) instead of an `fstat` and a separate
   `statx`.

## Measurements

The probe is `examples/copy_throughput.rs` in a release build, with the baseline
(ADR0177) and candidate run alternately and the history on tmpfs. The figures
are median CPU seconds (user + system) over three rounds. Other agents were
building and running native sessions on the machine meanwhile.

| Workload | Before | After | System calls per file |
| --- | ---: | ---: | ---: |
| tmpfs, 1,000 × 4 KiB | 1.79 | **1.33** (−26%) | 778 → **578** |
| ext4, 10,000 × 4 KiB | 21.3 | **16.6** (−22%) | |

Wall times on ext4 (37–38 s for both) were dominated by the machine's load
(load average 5–6 on 4 cores) and are not comparable. CPU time is.

## Considered and not done

- **Cached span pragmas.** ADR0177 measured no difference.
- **Skipping re-verification of intents this connection just wrote.** At most
  about 3% (the SHA-256 of the binding). Part of that check is a filesystem
  proof of the history file, which must stay.
- **`locking_mode=EXCLUSIVE`** would remove the WAL lock calls, but the requests
  store opens a second connection to the same history.
- **Merging adjacent commits.** In the plain-copy path every journal
  transaction is followed by a filesystem step before the next one. The only
  exception is clearing an intent followed by the cursor checkpoint. They live
  in separate store methods, each with its own `IMMEDIATE` transaction.
  ADR0180 instead gives a batch of files one intent and fewer transactions.
- **Fewer checkpoints per file.** That would detect outside changes later, so it
  is a change of policy, not an exact speed-up.

## Verification

- `proofs::tests` cover sharing inside a scope, fresh observations outside one,
  nested scopes, recorded folder and name proofs, and the `ENOTDIR` and
  replaced-object errors.
- The library suite, the full `integration` executable and the default Copy
  checks pass.
