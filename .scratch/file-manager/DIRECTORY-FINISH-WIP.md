# Source-directory removal restart recovery

The full five-area goal remains active. This slice binds the exact durable
postorder task to its RemovedDirectory Forward intent before mutation, advances
only the independently proved committed suffix, and atomically retires an
uncommitted returned claim with its normalized empty-only Preparing intent.
Claimed with an absent payload never inspects the old source name.

The actual completed-removal crash reproduced twice before the fix: restart
reported an uncovered filesystem step. Logs directory-finish-red-01.log and
directory-finish-red-02.log retain their fixture roots. Initial green passed.
directory-finish-core-01.log subsequently passed all 13 public directory boundary
parents (two private crash helpers ignored), including Ready/Returning return,
Claimed completion, recreated source entries, and empty Merge completion counts.
The original exec handle 93859 expired before its terminal result was retrieved;
its complete log records 13 passed, zero failures. No process is being assumed
live or restarted from that observation failure.

Schema remains 42; no new wire authority or weaker leaf matcher is introduced.
directory-finish-core-04.log passes all 18 directory boundary parents, two private
helpers ignored. Added cases cover later batch work after replacement of the
completed folder, missing/changed task proofs, SQL refusal at receipt retirement
and Preparing save, a second crash after the atomic return transition with foreign
private payload preservation, and a crash after a resumed operation's removal.
core-02 was a fixture compilation error; core-03 passed the behavioral assertions
but two fixtures attempted shutdown without abandoning retained stopped work.
Those fixtures were corrected; their logs and retained roots remain intact.

directory-finish-inputs-01.json captures 867 paths. Exactly six Rust files differ
from directory-expansion-inputs-02.json; the remaining 861 are unchanged.
directory-finish-lib-01.log passes 124 backend library parents, 12 ignored;
Clippy/default/all-targets/-D warnings and workspace formatting both completed
with exit zero. Static receipt records the unchanged schema42 fixture hash and
successful AST parse. Broad integration runner 50003 completed with exit zero:
core20..29 plus lib01 cover 20 Cargo targets, 437 passed parents and 33 ignored
helpers. Summaries count the terminal result per Cargo target, excluding nested
fixture process summaries. Required stopped/active restart gates are unignored
and pass in core21. Every batch checked all 867 inputs before and after.

The isolated default-library runner 45083 completed with exit zero: label
1792715001, log directory-finish-desktop-lib-01.log, 205 passed parents, 13 ignored.
It used private HOME/XDG, private session bus and no display/system bus/activation
directories. continuation-core-1792715001-receipt.json confirms exit zero and no
changed inputs before/after. The previously intermittent paused-video gate passed
in this run; its broader reliability work remains open.

verify-directory-finish.py completed with exit zero. The verification record
includes all target summaries, nine added public-service parents, all eighteen
directory boundary parents, both required unignored restart gates and exact
protected binary hashes. The protected application, Niri and earlier local-runtime
restart gate are unchanged. directory-finish-review-01.patch presents the exact
six-file difference from the preceding archived checkpoint.

directory-finish-red-fixtures-01.tar.gz preserves two actual pre-fix failed
histories and outputs: SHA256
377810285e9050b124cbe6e96f418cb5691799b7dbbf2a2613c3fb32ab6d0d81,
28 decoded verified entries. Original fixture roots remain.
directory-finish-source-01.tar.gz and its verification record preserve the final
867 captured inputs, documentation, logs, failure archive, receipts and tools;
archive-directory-finish.py verifies every decoded member against its captured
hash. Predecessor archives and all original failure roots remain intact. No
native/workspace/release or physical acceptance is claimed. Whole-entry Move,
reviewed replacement and saved-container task recovery are next; the full goal
remains active, as detailed in DIRECTORY-FINISH-NEXT.md and the ledger.
