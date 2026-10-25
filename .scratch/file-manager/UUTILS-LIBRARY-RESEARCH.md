# uutils as a linked transfer library

Inspected 2026-10-25. Primary baseline: upstream release tag **0.12.0**. Selected `main` files were also inspected and are explicitly distinguished below. No dependency changes, builds, or file-operation experiments were performed. This is a bounded API assessment, not a security audit.

## Verified release API

There really is a typed library API, beyond the command-line `uumain` entry point:

```rust
uu_cp::copy(sources: &[PathBuf], target: &Path, options: &Options)
    -> CopyResult<()>
uu_mv::mv(files: &[OsString], opts: &Options) -> UResult<()>
```

Both crates document their public `Options` as intended for downstream construction, naming Nushell. This is meaningful upstream recognition of library consumers, but the wording is a request to avoid unjustified changes, rather than a frozen API guarantee. `uumain` separately parses Clap arguments. Sources: [release cp API and options](https://github.com/uutils/coreutils/blob/0.12.0/src/uu/cp/src/cp.rs#L273), [release mv API and options](https://github.com/uutils/coreutils/blob/0.12.0/src/uu/mv/src/mv.rs#L56).

The complete parameters and option fields expose **paths**, no caller-supplied `File`/dirfd operands, cancellation token, conflict callback, or progress callback. Progress is a boolean that constructs internal Indicatif displays. Copy options include recursion, dereferencing, overwrite/backup policy, sparse/reflink modes, and preservation settings for ownership, mode, timestamps, links, xattrs and context. Copy collects nonfatal failures into `NotAllFilesCopied`, printing individual errors. Sources: [cp signature, aggregation and progress](https://github.com/uutils/coreutils/blob/0.12.0/src/uu/cp/src/cp.rs#L1338), [mv progress and error loop](https://github.com/uutils/coreutils/blob/0.12.0/src/uu/mv/src/mv.rs#L626).

Move's multi-source loop can print/set the exit code for failed items, continue, then return `Ok(())`; that return value alone is not a complete per-item outcome. `OverwriteMode::Default` can prompt for an unwritable target if process stdin is a terminal. Source: [release mv loop and overwrite handling](https://github.com/uutils/coreutils/blob/0.12.0/src/uu/mv/src/mv.rs#L670).

## Process-state caveat

The inspected `main` error module stores exit status in one static `AtomicI32`; `show!` sets that status. Atomics avoid a data race but do not isolate concurrent operation outcomes. This is evidence of process-level coupling, **not** a claim that all uutils APIs are memory-unsafe or universally unsuitable for threads. Source: [upstream error module](https://github.com/uutils/coreutils/blob/main/src/uucore/src/lib/mods/error.rs#L55). The 0.12.0 move loop demonstrably calls these status-reporting helpers, as cited above.

## Smaller reuse seam

The inspected `main` exposes feature-gated `uucore::safe_copy` (Unix), `buf_copy`, and `fsxattr` modules. [Module gates](https://github.com/uutils/coreutils/blob/main/src/uucore/src/lib/features.rs).

- `safe_copy::open_source` and `create_dest_restrictive` return open `File`s from paths. The latter creates a new inode at mode 0600; its nofollow option protects the final path component. `safe_copy_file` copies via `io::copy`, intentionally leaves final permissions to the caller, and leaves partial-destination cleanup to the caller. These remain path-based opens, not a supplied-dirfd traversal API. [Source and contract](https://github.com/uutils/coreutils/blob/main/src/uucore/src/lib/features/safe_copy.rs).
- The Linux copy backend calls `uucore::buf_copy::copy_fast` with already-open source/destination `File`s. Its reflink and sparse strategies live in crate-private functions, including `copy_on_write`; they are not an independently exported transfer engine. [Linux backend](https://github.com/uutils/coreutils/blob/main/src/uu/cp/src/platform/linux.rs#L63). The current `buf_copy` implementation could not be retrieved during this bounded inspection, so its exact public contract and cancellation behavior remain unverified.
- `fsxattr::copy_xattrs_fd(&File, &File)` and its unsupported-filesystem-tolerant variant provide a concrete metadata helper using pinned open inodes. `copy_acls` remains path-based in the inspected file. [Source](https://github.com/uutils/coreutils/blob/main/src/uucore/src/lib/features/fsxattr.rs).

These smaller seams were verified on `main`, not all independently reverified at tag 0.12.0. Versioned package documentation must be checked before selecting a dependency: the browser's `docs.rs/latest` results inconsistently showed older 0.5.0/0.9.0/0.10.0 pages, so they were not treated as proof of the latest release contract.

## Assessment / inference

Direct linkage is technically viable when an application wants complete cp/mv command semantics. It does not by itself provide the host-controlled identity, conflict decisions, structured per-item receipts, progress, cancellation, or recovery boundary needed by a richer file manager. Prefer evaluating a narrow content-copy/metadata primitive behind Canopa's own operation controller; adopting the whole command implementation would require adapting upstream interfaces or accepting its CLI-oriented state and reporting model. A handle-oriented primitive is a plausible way to reduce low-level duplication, but this investigation does not establish a ready-made uutils replacement for Canopa's operation engine.

## Subprocess option and proposed direction

Running a pinned uutils command in a child process is also technically viable. This isolates its process-level reporting and permits process cancellation. Its terminal progress/verbose text is not a structured per-item completion protocol; Files would still need durable records and to reconcile partial output. A child can be interrupted while writing, so a proposed adapter should copy into owned staging storage and let Files decide when to publish it. These are integration inferences, not upstream guarantees.

In release 0.12.0, `mv` performs the rename itself and owns its cross-device copy/remove fallback. It offers no host commit hook between the copy and source removal. That makes wholesale `mv` delegation a poor fit for Files' requirement to commit the destination before removing the selected source. [Release move fallback](https://github.com/uutils/coreutils/blob/0.12.0/src/uu/mv/src/mv.rs#L862).

Recommended evaluation: first prove uutils-based copying into a private destination, with metadata, source replacement, cancellation, and killed-worker/restart cases. Compare direct linkage with a small Files worker process linking the same crates. Keep atomic publication, original-source removal, Undo history, and recovery in the shared Files controller. Pin a verified release; do not resolve an arbitrary system `cp`/`mv` through PATH. This assessment does not implement a migration.
