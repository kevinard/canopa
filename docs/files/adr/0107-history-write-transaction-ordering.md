# 0107: Acquire the history writer before mutation checks

Status: scoped backend, Iced and native verification, 2026-10-20.

The prepared Rename lifetime test exposed ordinary journal failures while100
CreateFolder requests were being admitted and executed concurrently. Diagnostic
core07 reports `operation history: database is locked`, followed by requests
refused for interrupted work. The smaller core09 case reproduces without a
prepared Rename, extra SQLite connection or retained-source release worker.
This is ordinary backend admission overlapping journal writes.

The filesystem Store used deferred transactions that read ownership, context,
references or history before writing. The independent scheduling writer already
uses immediate transactions. A scheduling commit can invalidate the deferred
reader's WAL snapshot before its write upgrade. SQLite documents this failure
and BEGIN IMMEDIATE's writer acquisition before those reads:
[isolation](https://www.sqlite.org/isolation.html) and
[transactions](https://www.sqlite.org/lang_transaction.html).
The observed generic error does not retain SQLite's extended result code; the
snapshot-upgrade mechanism is inferred from this transaction order and the probe.

All19 existing mutating Store transaction sites now use BEGIN IMMEDIATE. This
includes intent, append/reversal, retention relocation, directory-return updates
and history expiry. Read-only snapshot connections keep their existing behavior.
Do not hold these transactions across user questions or filesystem mutations.
Use existing bounded SQLite busy handling; do not replay a filesystem operation,
add application retry loops or extend timeouts to hide a lock-upgrade failure.
No schema change or additional process-wide lock is needed.

Focused core11 passes57 parent tests (four ignored entries include helpers),
including the formerly failing preparation lifetime and ordinary burst cases.
Three unchanged-source burst repetitions89–91 pass as well. This evidence supports
the transaction-order change; it is not a claim that external writer starvation,
hardware failure or arbitrary lock contention cannot occur. The final source26
slice passes591 backend parents/67 standard targets plus custom device_runtime,
84 default Iced tests, formatting, all-target Clippy and a desktop build.39 backend
ignored entries are not all executed; one Iced helper is ignored. Six final-ELF
native cases/40 groups cover the affected Rename/Undo/dismissal workflows in both
appearances. Earlier intermediate-ELF continuation/Recovery results stay separate.
Exact sources, binary hashes, failed reproductions and limits are in the prepared
Rename verification record and work log.
