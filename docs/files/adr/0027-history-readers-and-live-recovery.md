# Inspect Recovery while an operation waits for a decision

Status: accepted under delegated implementation authority, 2026-10-16.

Recovery must remain useful while Undo or a transfer waits for a decision. The
user can refresh the catalog, inspect a saved version's metadata and preview its
contents before deciding how to continue. Waiting for an interactive answer must
not hold up these reads or the Undo label and interrupted-operation summary.
File mutations remain serialized. A recovery copy still uses the shared queue;
this read change does not run a second mutation alongside a waiting operation.

The engine now keeps an independent history read source, captured after startup
migration and reconciliation. Each catalog page opens a read-only SQLite
connection and a deferred transaction against the existing WAL database. The
page's history, namespace resolution and pending rows share one committed read
view. It does not wait for the mutation mutex, migrate storage, replay journal
entries or collect retained data. A nonblocking mutex check anchors an idle
snapshot before another writer can start; a busy writer is bypassed. Undo labels,
storage preflight and interrupted summaries use the same read source. Callers
use the existing public methods; no database migration is needed.

Reader opens disable creation and symlink following, require the supported
schema, and compare the known history identity before and after opening. A
missing or observably replaced history path is refused rather than recreated or
interpreted as another database. Filesystem observations and recovery error text
are captured separately from the SQLite view; they can become stale as work
continues. Existing recover-copy mutation checks re-read the catalog record and
selected file version under the writer. Refresh obtains a new page, and separate
pages may observe subsequent commits. This is not an atomic snapshot of the
filesystem, nor a substitute for remaining directory-descriptor race work.

Recovery failure messages are shared independently of the long-lived mutation
mutex, then copied for each read snapshot. Live preparation journals without a
reconciliation failure are excluded from its interrupted list and pending-file
catalog while the writer is busy. When the writer is idle, every unfinished
journal remains inspectable, including a transfer that stopped on unavailable
storage without first attempting reconciliation. An unfinished copy currently
being written is not an
interrupted operation or a recovery version. Actual unresolved primary and
recovery-copy journals retain their specific failure messages and files. Writer
reconciliation continues to inspect all journal entries, including those for
which no failure has yet been recorded.

Public filesystem tests first reproduced a Recovery reader blocked until an
Undo question was answered, then the same problem in history summaries. A
bounded observation thread checks the public readers while the writer waits;
the test releases the writer before asserting a timeout so a failed regression
cannot deadlock the test process. A separate real-copy test first exposed the
new reader incorrectly listing live preparation as interrupted work. Full-suite
coverage then exposed a paused transfer's journal incorrectly hidden after its
worker returned; idle snapshots now retain it. Missing
and replaced history paths are checked without querying private database rows.

All 117 core tests pass (`core-history-readers-final.log`), with nine explicit
child-process helpers exercised by parent cases. Build, all-target Clippy,
formatting, Python syntax, maintained native inventory and whitespace checks pass.
Native `native-history-readers-25` (dark) and `native-history-readers-27` (light,
1.25×) each pass all 31 Undo checks, including refreshing Recovery and previewing
the edited saved text before answering Undo. Rendered inspection confirms its
actual contents. `native-history-interrupted-24` passes all eight interruption,
copy-out, restart and reconciliation checks; its rendered attention view retains
the specific failure reason. All three runs have clean GTK logs. The verified
binary hash and results are in `history-readers-verification.json` under the
file-manager scratch evidence directory.

Two initial parallel attempts timed out waiting for Niri IPC before Files launch
while the host had high I/O pressure. The serial interrupted and dark workflows
passed after compilation. A subsequent light run received SIGTERM during earlier
changed-copy checks; its standalone rerun passed. Those incomplete attempts do
not establish application acceptance and are retained separately.

Broader recovery work
remains open, including bounded namespace resolution for very large histories,
phase-specific interrupted-operation choices, directory recovery, protected-data
disposal, orphan cleanup and remaining filesystem authority races.
