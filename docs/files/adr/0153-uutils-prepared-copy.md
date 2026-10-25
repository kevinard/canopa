# 0153: uutils content transfer in the shared prepared-copy lifecycle

Date: 2026-10-25. Scope: Copy and consumers of the prepared-copy seam. The full
Files product and restart-recovery goal remain unfinished.

## Decision and bounded proof

Use the published **uucore 0.12.0**, MIT, revision
`dc1efd89948a9ca4c78c3a4b9a6ac891019a8c69`, with one root vendor patch adding
`buf_copy::copy_with_progress`. Rust 1.88 is required upstream; Canopa uses 1.95.
The selected features are `buf-copy` and the upstream default `signals` feature.
`buf-copy` enables `pipes`/`fs`. Disabling `signals` fails compilation because
this release has unconditional references to nix signal types. We never invoke
uucore's CLI initialization, signal setup, locale setup, or global error status.

`scripts/prove-uutils-copy.py` builds a disposable headless proof with a separately
locked uu_cp 0.12.0 dependency. It does not become another production backend.
It proves the default path API copies a `/proc/self/fd/N` operand as a symlink,
rather than copying the selected inode's bytes. Following the operand alone does
not resolve its metadata/canonicalization contract for held links and replaced
parents. The path API also lacks supplied progress/cancellation callbacks.
Consequently we reject `uu_cp::copy` at this seam, without claiming all possible
cp configurations are unsafe. No complete Move is delegated to uutils.

The proof exercises raw names, selected regular data after parent replacement and
permission restriction, raw link-target transport, in-file progress/cancellation,
a failed output followed by another copy, and a killed/reaped copying process.
Actual dangling/raw-target symlink **copying** is verified by the composite public
`uutils_copy::shared_copy_retains_the_documented_metadata_and_link_policy` test,
not by the standalone fixture's create/read-link probe.
The API uses open descriptors, never a user pathname. Its callback runs before
I/O and after at most 64 KiB has been transferred. The patch reuses upstream
`splice`, `drain_pipe`, `RawWriter`, and Rust's bounded `io::copy` fallback.
A new local pipe belongs to each call. Existing upstream APIs are unchanged;
there is no application data-copy loop or alternative engine.

Select **synchronous direct linkage**, permitted by the plan, instead of a child
worker. This function touches no shared pipe cache, CLI status, or process-global
options. Calls on different descriptors can run concurrently. Callback failure
unwinds the copy on its owning operation thread; it cannot leave a background
writer. Process death kills that writer as part of the owning process. Existing
history writer locking excludes restart reconciliation until the owner exits.
There is no wire protocol, executable lookup, subprocess startup, acknowledgement
loss, or packaged worker to maintain. Native regular-file I/O can still block
inside a syscall; cancellation is cooperative between transfers, as with the old
reader, not a bound on kernel/device response time.

## Shared lifecycle and receipts

`prepared_copy` owns preparation for ordinary/recursive Copy, Merge, retry,
cross-filesystem Move, Recovery export, saved-container migration, and copied Undo
returns. It registers the empty owned stage and its actual payload before data
writes, validates held identities through callbacks, copies content, applies
metadata after verification reads, syncs, and produces the existing prepared receipt. Source validation
and SHA-256 remain in `fingerprint`; publication/history/source removal stay in
the engine. Existing native rename/unlink primitives remain unchanged.

The previous application read/write-and-hash loop is removed. Transfer and hashing
are now separate: after successful content transfer the host hashes both the still-held source
and the readable held output, requires equal digests, and checks the original
source snapshot around both reads plus current source/stage bindings. This catches
mapped writes that need not produce another timestamp transition and same-inode
staged edits before the receipt. Positioned output reads work even under a
mode 0000 stage and leave the writer offset alone. Both full extra reads are
accounted for in measurements; final mode/timestamps are applied afterward to
preserve atime. The fingerprint
format and meaning remain size, ordinary mode, mtime, link flag, SHA-256 of bytes
(or raw symlink target). No journal/schema change or uutils-specific recovery
state is introduced. A completed native copy without a host receipt is still
Preparing; it may never publish merely because its bytes happen to be complete.
Existing schema60 evidence governs restart and old partial data.

Metadata policy deliberately preserves the baseline: ordinary mode bits (0777)
and available access/modification times on regular files; ownership belongs to
the creating user; setuid/setgid/sticky are not copied. Symlinks retain raw targets
without dereferencing (including dangling links), with the existing symlink time
behavior. Required mode/time/sync failures fail preparation. No ACLs or xattrs are
copied, no hard-link groups are preserved, sparse input produces dense output,
and no reflink is attempted. These are existing limitations, now explicitly
verified; `copy_with_progress` is not advertised as all of cp. Special files are
rejected before readable opens. Optional metadata preservation and optimized
extent cloning need a separate upstream seam and acceptance work.

## Relationship to prior decisions

This supersedes the handwritten byte transfer within ADR0116's prepared-copy
implementation. Its descriptor/source/parent checks and ADR0121's publication,
Undo, durability and recovery rules remain in force. ADR0152/schema60 and the
known incoming-Move, Undo, Trash/Restore, legacy/cleanup and 500-item/20-second
Undo limitations are unchanged. The private schema61 experiment is not imported.

The exact proof, performance measurements and validation results are recorded in
`.scratch/file-manager/UUTILS-IMPLEMENTATION.md`. Packaging links the selected
library into Files and retains its MIT notice through the normal dependency
attribution collector. Full native and release acceptance remain separate gates.
