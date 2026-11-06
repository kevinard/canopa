# Phase71 source-only replay archive

These are exact source snapshots of the scoped diagnostics indexed by
`../paper-appearance-evidence.json`. They contain no executables, credentials or
private queues. Original receipts retain their source and runtime hashes.

The orchestrators expect repository-root execution and the existing stable
`target/portal-print-71/replay-source/` paths and checksum-addressed native stores.
Restore a needed source there with its recorded checksum before replay; do not
run one-time baseline builders or overwrite historical receipt/output paths.
The maintained annotation and metadata regressions live in root `scripts/`.
The prototype receiver/libppd investigations are diagnostic, not installed or
accepted dependencies. See `PHASE71-LIVE.md` before continuing validation.
