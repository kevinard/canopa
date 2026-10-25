# 0154: Hash the fully compared Copy streams once

Date: 2026-10-25. Scope: prepared-copy verification; the wider Files goal remains
open. Refines the receipt implementation in ADR0153.

## Evidence and decision

The optimized migration baseline still spends about 1.1 seconds on each SHA-256
read of 128 MiB on the measured i5-6200U, which lacks SHA instructions. Complete
Copy takes 2.49–2.66 seconds; an optimized pre-migration reference takes
1.28–1.45 seconds. These measurements use the actual release profile, the same
private workload and locked dependencies. The debug regression was not sufficient
evidence on its own. Repeated final comparisons and limitations are recorded in
[the performance evidence](../../../.scratch/file-manager/COPY-HASH-PERFORMANCE.md).

Compare every selected-source byte with the actual owned output, then update
SHA-256 once for their verified common bytes. Require both streams to reach EOF
together. Equality is exact, rather than inferred from two digests; the returned
digest is still SHA-256 of the complete source and complete staged output.
Neither stream is sampled, cached, skipped, or accepted from metadata alone.
This removes a redundant compression pass while retaining both full reads.

The source reader is independently reopened through its held descriptor, as in
the previous verification, and checked against that descriptor's snapshot. The
output is read through its held readable descriptor with positioned reads, even
at mode 0000. Short reads are handled independently; an extra output suffix or
early output EOF refuses the receipt. Two 64 KiB buffers bound memory. Cancellation
is checked between chunks and before each positioned-read retry; there is no new
thread, worker, background read or writer lifetime.

Capture both descriptor snapshots before reading and validate both after the
entire comparison. Keep the caller's original pre-copy source snapshot checks
and current source/output/parent checks. This retains the mapped-write and
same-inode stage-edit refusal requirements from ADR0153. As before, these checks
do not claim an atomic filesystem snapshot against arbitrary concurrent writers.

## Unchanged authority and durable meaning

Metadata is restored after verification reads, including atime; the host still
syncs before producing its prepared receipt. Registration, publication, retained
replacements, source-removal checks, history completion, cleanup, Undo and
Recovery are unchanged. Failed verification cannot authorize publication; the
existing interrupted-preparation machinery still owns the staged data.

Fingerprint fields, SHA-256 values, serialization and schema60 do not change.
Existing Undo/Recovery fingerprint reads and post-publication Move checks still
perform their original content verification. Old histories need no migration or
reinterpretation. The uutils byte mover and its vendor patch are unchanged.

This supersedes ADR0153's two-compression receipt implementation only. Sparse/
reflink, ACL/xattr and hard-link limitations, generic Undo/Recovery gaps, the
unchanged 500-item/20-second gate and wider native/release acceptance remain open.
