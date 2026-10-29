# 0155: Trust upstream Copy and fingerprint its output once

Date: 2026-10-25. Scope: the shared prepared-copy lifecycle; the full Files goal
remains open. Supersedes ADR0154's source/output comparison policy.

Follow-up: [ADR0156](0156-fast-content-fingerprints.md) removes the development-
history compatibility constraint and replaces content SHA-256 with XXH3-128.
The transfer/lifecycle policy below remains; its SHA compatibility statements
describe this historical checkpoint.

## Decision

The user explicitly chose to trust successful uutils copying, hash the output
once for Undo, and retain safeguards needed for Recovery and destructive actions.
This revises the earlier migration requirement to independently verify source/
output content equality. It is not a claim that the copier was unreliable.

After uutils returns successfully and its reported count matches output length,
read the actual held output through EOF and compute its SHA-256 digest once.
Do not reopen or reread the source for a second content comparison. Positioned
reads preserve the writer offset and work through the already-readable descriptor
even at mode 0000. The output snapshot must remain stable across hashing;
cancellation and read errors prevent a completed receipt. Apply final metadata
after this read, then sync before normal publication and history completion.

Keep SHA-256 and the existing fingerprint representation for history compatibility.
No hash-algorithm or cryptographic threat-model migration is part of this change.
The improvement removes a source reread, comparison buffer and equality loop;
it does not remove the CPU cost of the remaining output hash.

## Authority and boundaries

Keep the selected source, output and parent descriptors and namespace checks,
original source metadata snapshots, regular-file/type restrictions, per-chunk
cancellation, durable stage registration, sync and publication ordering. A native
copy success without a completed host receipt still cannot publish after restart.
Uncertain work remains owned by its journal and available to Recovery.

The stored output fingerprint describes the bytes actually copied, rather than a
later read of a potentially changing source. The existing source-fingerprint field
uses that digest with the selected source metadata as the expected contents for
later destructive checks; preparation alone does not prove that the source still
matches it. Cross-filesystem Move, copied-leaf continuation and saved-container
cleanup still read and compare actual source/output fingerprints before source
deletion or disposal. Copied Undo returns still compare the prepared output with
the recorded expected fingerprint. Their identity, claim and journal rules remain.

Ordinary Copy no longer rejects a source/output mismatch solely because a mapped
source write did not advance metadata. It also fingerprints same-inode output
changes made before hashing as the actual copied result. This is the deliberate
policy change. It does not promise an atomic snapshot of a concurrently written
source or detection of every external edit. Undo still checks the recorded output
before removing it; later changed output receives the existing preservation/review
behavior. A Move with different source/output contents must retain the source.

Schema60 and old SHA-256 histories remain readable without reinterpretation or
migration. No dependency, upstream patch, worker, cache or new durable state is
introduced. Shared preparation remains one lifecycle for all existing consumers.

Evidence, optimized measurements and limitations are recorded in
[COPY-TRUST-PERFORMANCE.md](../../../.scratch/file-manager/COPY-TRUST-PERFORMANCE.md).
