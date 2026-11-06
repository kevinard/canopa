# Reuse pure journal computations while checking current authority

Status: computation equivalence and functional regressions verified;
the 500-item Undo performance budget remains unmet.

After the completion codec correction, the 270-item queued Copy gets its conflict
question but completes only eleven steps in nine seconds. Its CPU profile puts
85% in `transfer_cursor::stage_task`, including 44% in SHA-256 and 43% in decoding
the full original selection. The independent 500-item Undo profile also records
4.56 seconds of repeated SQL preparation. The dedicated 20-second Undo budget
and two-second Recovery page-latency check remain unchanged. The pagination
fixture's asynchronous batch setup has a separate budget (ADR0158).

Cache only pure computations keyed by complete source bytes. Each fixed decoder
keeps its most recent body and immutable parsed data, up to 4 MiB of input per
thread. SHA-256 memoization keeps at most four inputs and 4 MiB combined per
thread. A same-length edit is a miss; malformed new input still reaches the full
decoder. Oversized inputs are computed without retention. Thread exit retires
these caches. Original SHA-256 preimages, domain tags and output bytes remain
unchanged, including the existing journal format.

Candidate5 passes both pure-cache tests, all eight functional Undo regressions
and sixteen selected history boundary parents. Copy advances121 steps in9s
versus11 before, but its270-item setup outlasts the generic10s catalog wait.
The500-item Undo remains over budget:24.317s in the regression run and21.528s
under CPU profiling. The new profile records serialization as the largest cost,
with1.4s also spent decoding Change records.

Candidate6 extends exact-byte decoding reuse to the current journal and Change
data. Callers receive owned clones; current validation still runs after decoding.
These are serialized descriptions, without opened handles or live review leases.
Fixed-size proof and digest arrays use Serde's byte serialization path, which
serde_json emits as the identical integer arrays. A compatibility check covers
every byte value in compact/pretty JSON and decoding through the previous shape.
No schema, field names, cryptographic inputs or output digests change.

Current journal rows and their expected digests are still read and compared on
each verification. Cached parsing never substitutes for filesystem identity,
opened-descriptor, source reservation, permission, request revision or current
phase checks. No cached filesystem authorization or result is introduced.

Reuse SQLite prepared statements in the repeated store paths, with 128 compiled
statements in the mutating connection. Every execution binds its current values
and every read runs in the current transaction. The cache contains SQL bytecode,
not query results; SQLite/rusqlite retains schema invalidation and reset behavior.

The original queued Copy/Recovery test and 500-item Undo timing test remain the
regression seams. Changed-history, changed-body, consumed-name, source-retention
and restart cases must pass before claiming this candidate verified. The small
pure-cache cases specifically reject stale same-length input and check SHA-256
equivalence under cache churn. Full workspace acceptance remains separate.

Candidate06 passes all three pure computation/exact-wire regressions, sixteen
selected history boundary parents and the full375-case Files library run
(34 ignored helpers). The pagination case and all eight original functional Undo
regressions pass. In the full workspace run, Files integration finishes605passed,
onefailed and41ignored; Keep Both500 takes22.417508467s against20s. One earlier
fresh-process run passed that assertion, but does not override the workspace
failure. Keep the performance gate open. Strict workspace Clippy and all Portal
tests pass. Receipts: `target/portal-audit-83/workspace-final-results-06.json` and
`files-keep-both-isolated-06.log` in the same directory.
