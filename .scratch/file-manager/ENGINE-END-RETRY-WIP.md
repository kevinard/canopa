# Recovered transfer-end metadata retry

The full five-area goal remains active. This slice addresses failure recording a
proved Copy/Move engine end after restart; directory and other filesystem task
phases remain unfinished. No commits, worktrees, subagents or vendor changes.

The predecessor source-return-source-01.tar.gz was verified at turn start:
b1ac792bd4989d496380201c61b5085181e23237111db0cd99f39e6849da089e,
1046 decoded members, all 864 captured inputs unchanged. Previous data/proofs and
protected application/Niri/earlier local-runtime gate binaries remain preserved.

The final-Copy public-service regression failed twice after an actual SIGKILL:
engine-end-retry-red-03.log and -04.log. Refusing the engine-end write produced
Interrupted instead of SaveFailed despite a valid complete cursor. Two earlier
attempts (-01/-02) had test harness compilation errors, corrected before the
actual repeated failure. The private failure roots and output files are archived
in engine-end-retry-red-fixtures-01.tar.gz, SHA256
b379f655867bd070a55e674f2464106d7cb11d81f1d997000305c5d6da21d38a,
20 decoded verified entries, two failure pairs. The verification JSON records
metadata, member hashes and read-only SQL probes: clear journals, one cursor,
no stop and a null end. Original roots remain retained.

Recovery now validates the entire original cursor and typed end before metadata
writes. Only a proved end survives write refusal as a process-local pending
witness bound to the engine and exact accepted request. It grants metadata retry,
not filesystem execution. Startup and successful Reconcile carry the actual
confirmation, rather than inferring Recorded from the result variant. Invalid or
unproved cursors remain Interrupted. Completed Retry Save records the engine end
and retires its cursor atomically, then saves the application result. Stopped
Retry Save uses the existing atomic witnessed end/result save; independent object
reopening can provide unfinished owners only after that save. No schema/codec
change: schema41 and continuation3/4 remain. See ADR0128.

The first focused regression passes in green-01.log. Broader copied-leaf boundary
core-01.log passes the new gates but reveals an older assertion still expecting
Interrupted for the intentional metadata-only refusal. The corrected expectation
retains Interrupted for every corrupt/borrowed proof; core-02.log passes all 22
boundary parents, 3 ignored helpers. New gates cover stopped ends at startup and
review, Copy and cross-filesystem Move, end/stop recording and retirement refusal,
repeat failed Retry Save, continuation controls unavailable before save, activity
holds, edited completed paths/inode retention, later Abandon and reopen, and stale
accepted request bytes/revision/reservation. The existing active-cursor integration
assertion now distinguishes metadata refusal and verifies blocked clean shutdown.

engine-end-retry-inputs-01.json captures 864 build/fixture paths. Nine Rust paths
changed from the predecessor (worker/restoration, execution and its transfer end,
completion/cursor/stop stores, copied-leaf tests, active restart integration test).
The full no-default library passes 106 parents, 10 ignored helpers, lib-01.log.
The bounded selected-backend runner passes 419 parents across 20 targets, 31 ignored
inventory helpers (lib-01.log plus core-10.log through core-19.log), on inputs 01.
Every batch has terminal exit 0 and unchanged before/after inputs. Both required
stopped and active restart gates remain unignored and pass on that source.

The initial all-target Clippy run (-clippy-01.log) requests two collapsible if
forms in shared end validation. Only those two guards changed to equivalent
let chains; there are no altered conditions or write paths. inputs 02 captures the
final 864 paths. source-relation-01.json and exact before-clippy backups reconstruct
all inputs 01 and show the two-file change; all other 862 captured files match.
Final Clippy with warnings denied passes (-clippy-02.log), terminal exit 0. Final
formatting passes (-format-01.log). The two affected public restart/execution
integration targets pass36 parents, 4 ignored helpers (-core-20.log), terminal 0,
with unchanged inputs 02; final-checks-01.json records the command and result.
Both required stopped/active gates remain unignored on this final source.
The isolated default-feature library passes 187 parents, 11 ignored helpers on
inputs 02 (-desktop-lib-01.log). Receipt continuation-core-1792702001-receipt.json
has terminal exit 0 and all 864 inputs unchanged before/after the private D-Bus,
HOME and XDG run, with no display or live system bus. Both new public parents and
all copied/terminal/returned-source parents pass on the final library build.

engine-end-retry-verification-01.json records the earlier419-test source 01 basis,
two-file syntax-only relation, final36 restart/execution parents and 187 default
library parents, static checks and both final required gates. The protected
source27 app, compatible Niri and earlier local-runtime gate hashes are unchanged.
The final source/docs/proofs and exact old guard backups are preserved in
engine-end-retry-source-02.tar.gz; its verification JSON records the actual archive
hash and every decoded member hash. There are no outstanding test processes. No native/full-workspace/release, Shell/Notify or physical-device
acceptance is implied. The earlier intermittent paused-video/seek issue remains.

Next: directory publication/child-queue and accepted Merge expansion durability,
source-directory removal and other task phases, complete Preparing/Cleanup context
binding, legacy loose bindings, queued lineage/admission, large reports/paged
owners, Undo/relocation/expiry, Trash/Restore, renderer separation, broader product
acceptance, reliability/performance and release integration. The goal remains
active; these focused results do not complete it.
