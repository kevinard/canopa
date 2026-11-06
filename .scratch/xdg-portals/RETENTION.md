# Portal investigation evidence retention

On 2026-11-06, all 1,999 remaining untracked files were portal
investigation artifacts under this directory (36,140,738 bytes).
The application source, maintained fixtures and dependency provenance were already
committed.

Retained 200 original files: native result summaries (including failures),
validation and process-retirement receipts, storage receipts, and the current
Niri integration review's small inputs and results. Removed 1,799
files (35,125,750 bytes): superseded prototype/source copies, generated
protocol code, raw logs and screenshots, progress/registry dumps, and repeated
source and package inventories.

[The retention index](retention.json) records counts, grouped removal digests,
retained paths, and 19 compact receipts. Each compact receipt keeps
its original path, byte count and SHA-256, with outcomes and binary identities.
Large source inventories are represented by entry counts and canonical JSON
digests. These are summaries, not lossless archives or independent source proofs.
Historical ledgers and receipts retain their original paths and hashes; removed
local outputs are no longer expected to exist. Maintained reproductions live in
`../../scripts/` and dependency recipes in `../../vendor/`.

## Niri review

The [review plan](niri-integration-review/review.json), core/full routing diffs,
configuration templates, small metadata payload, validation inputs and results
remain available. [The install summary](niri-integration-review/install-review.json)
retains all non-license destinations and summarizes the license inventory.
Licenses must still be installed. Before an approved installation, regenerate the
complete file inventory and check current destination guards against the archive.
[Archive metadata](niri-integration-review/archive-metadata.json) retains the
accepted archive checksum and metadata without another full member listing.
No installation or live service action was performed; the portal goal remains
paused for review.

## Validation

This cleanup changes documentation and retained investigation evidence. JSON
parsing, receipt compaction, retained-file hashes, Python syntax, routing diff
replay and the final Git path inventory are checked without rebuilding Rust or
running native fixtures. Build outputs and ignored artifacts are outside this
cleanup. Future generated logs, screenshots and copied sources belong in ignored
`target/` directories; retain only concise receipts here.
