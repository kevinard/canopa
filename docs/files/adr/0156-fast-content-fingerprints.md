# 0156: XXH3-128 content fingerprints during development

Date: 2026-10-26. Supersedes ADR0155's SHA-256/history-compatibility constraint.
The full Files goal remains open.

## Decision

The user explicitly removed history compatibility as a requirement during
development. Use XXH3-128 for file-content and raw symlink-target fingerprints,
through the published `xxhash-rust` crate, locked at 0.8.19 with only `xxh3`
enabled. This adds one BSL-1.0 registry dependency with no transitive dependencies;
existing locked versions and checksums remain unchanged.

Use the same default-seed algorithm for ordinary Copy's actual output, path and
held-descriptor fingerprint reads, Undo, Recovery and source-removal comparisons.
Streaming reads retain 64 KiB buffers, complete EOF coverage, cancellation and
snapshot checks. Symlink targets use the matching one-shot function. Store the
128-bit result as 16 canonical big-endian bytes. No native-CPU flags, checksum
cache, sampling, extra worker or custom hashing implementation is introduced.

This is a non-cryptographic content identity check for the explicitly accepted
non-adversarial use case. Keep SHA-256 for unrelated small journal/object-binding
records; those checks are not the measured large-file content-hashing cost.

## Development receipts

`Fingerprint` requires the algorithm-specific field `xxh3_128: [u8; 16]`.
The earlier `digest: [u8; 32]` field is unsupported. Existing strict deserialization
rejects old, mixed-algorithm and wrong-width fingerprints instead of silently
interpreting a SHA value as XXH3. There is no legacy hash fallback, dual-hash
version dispatch, history conversion or automatic history/data reset.

SQL tables and their schema version remain unchanged: the incompatible format is
identified in each required fingerprint field. Earlier readers reject the new
field, and current readers reject the old field. Old histories containing SHA
fingerprints may no longer open or support Undo; their saved files are not deleted
to make them load. Structural migration code for unrelated records is untouched.

## Preserved operation boundaries

Copy still trusts uutils and fingerprints the actual output once after transfer.
Keep descriptor/namespace and metadata checks, stage registration, replacement
retention, metadata restoration after reads, sync, publication and history rules.
Destructive actions still read the full contents they compare before source
removal or disposal, now using XXH3 consistently. A mismatch must retain the source.
Interrupted native success without a completed host receipt still cannot publish.
The concurrent-write policy from ADR0155 is unchanged.

Evidence, benchmarks and remaining limits:
[COPY-XXH3-PERFORMANCE.md](../../../.scratch/file-manager/COPY-XXH3-PERFORMANCE.md).

Primary algorithm/API references: [xxHash](https://xxhash.com/),
[xxhash-rust](https://github.com/DoumanAsh/xxhash-rust),
[streaming API](https://docs.rs/xxhash-rust/0.8.19/xxhash_rust/xxh3/struct.Xxh3.html).
