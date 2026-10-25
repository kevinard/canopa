# Final-leaf restart completion

The full five-area goal remains active. The previous schema40 archive was verified
at the start of this turn: restart-copied-leaf-source-01.tar.gz, SHA256
62c106bd82132bc171df04b30fd44655813a4e611ea01174dfd385c00d4e078f,
1052 decoded members. Its captured inputs were unchanged except for the new test
before production edits. Previous source, proof and failure datasets remain.
No commits, worktrees, subagents or vendor edits were made in this slice.

## Actual failures and fixes

terminal-leaf-red-01.log and -red-02.log repeat a real public OperationJobs child
SIGKILL after its last Copy append. History has one committed step, the exact
scoped task receipt is intact, journals are clear, and no end is recorded.
Restart fails with “The active transfer has no exact unfinished part.” This
rules out completed-output inspection and application-result saving as the cause.

The cursor decoder now returns a typed completed or stopped result. Existing
StoppedTransfer invariants remain strict. Complete metadata requires valid rows,
counts, the exact independently bound committed suffix and no unresolved saved
source receipt. Completed sources, outputs and parents are never reopened.
Store.record_transfer_end and cursor recovery share one connection-level record
body; completion and cursor retirement commit or roll back together. The worker
then saves the existing CompletedTransfer application report. Pending journals
still require the public captured Reconcile action. A second crash during final
resumed cleanup consumes only its proved source removal, accounting zero new
copy bytes for that successor.

core-01.log is a compilation-only variable-name error while extracting the shared
transaction body; it was fixed before tests ran. core-02.log passes the new final
completion/review/save cases but catches a fixture ordering error in the folder
report check: normalized source order put the final leaf before the folder. After
using an earlier folder name, terminal-remainders-red-01.log reproduces the real
report gap: the original folder retains newly arrived files, but its restored
completed report omits that folder.

Schema41 adds normalized source-remainder rows, role7 and eight progress counters.
A confirmed nonempty original source-folder task records that descriptive fact
before later tasks. Normal returns retain their previous final report refresh;
restarted completion uses durable observations and does not inspect completed
paths. Row digests/counts/budget protect these facts without rewriting a whole
queue per file. Migration retains old task rows/digests, and the decoder accepts
six/seven-counter headers. Continuation checkpoints stay version3 normally and version4 for a
copied-source cleanup. These report facts grant no filesystem execution rights.

## Late hard-link regression

The initial source passes 99 selected library parents,412 broader backend parents
and 180 isolated default library parents. A further namespace check then failed
in terminal-alias-red-01.log and -red-02.log: after actual unlink, reintroducing
the original inode through a retained hard link made recovery infer a return,
clear the Claimed journal and fail to append its bound removal step. The durable
records show unchanged fingerprint, both step receipts present, and only Copy
committed. Inode equality cannot distinguish the reintroduced name.

Only Ready now permits the no-claim/returned inference. Claimed with no payload
consumes the proved removal metadata without inspecting the source name;
authorized returns already have a durable Returning phase. The regression covers
both original and resumed final cleanup, public review and two further opens,
preserving both hard links and the copy. lib-03.log passes 100 parents, 9 ignored.
Inputs02 differs from inputs01 only in source_removal.rs and copy_boundary.rs;
the exact earlier files remain as terminal-leaf-before-alias-source_removal.rs
and -copy_boundary.rs. All 413 selected backend parents across 20 targets pass on inputs02, 30 ignored
inventory helpers, with every input verified unchanged before/after each batch.
The final isolated desktop/static checks pass on that same source; do not label
the initial source's results as the final source.

## Scoped verification

terminal-leaf-lib-02.log passes all 99 no-default-feature library parents, 9 ignored
inventory helpers. Eight new unignored public-service parents cover edited and
replaced completed sources/outputs, replaced/missing output parents, raw names,
dangling links, explicit journal review, two opens, a second crash during resumed
final cleanup, earlier source-folder reports, missing/borrowed/corrupt evidence,
rollback at end recording/retirement, and application-result Retry Save/activity
holds. All seven earlier copied-leaf parents still pass.

terminal-leaf-inputs-01.json captures 864 Rust/Cargo/vendor/fixture paths. The ten
bounded backend batches verify them before/after every run. Together with lib-02,
core-10.log through core-19.log pass 412 selected parents across 20 no-default-feature
targets, 30 ignored inventory helpers. Both required stopped/active restart gates
remain unignored. Default all-target Clippy with warnings denied and formatting
pass on that same source; fixture syntax/hash also pass. The exact initial/final
profiles, summaries and receipts are in terminal-leaf-verification-01.json.
Inputs02 has 413 selected backend passes across 20 targets (including 100 library),
30 ignored helpers, and 181 isolated default desktop-library passes, 10 ignored.
Current default all-target Clippy with warnings denied (clippy-02.log), formatting
(format-02.log) and the captured schema41 fixture syntax/hash pass. Receipt
continuation-core-1792691001-receipt.json verifies all 864 inputs before/after the
private D-Bus/HOME/XDG desktop run, without a display or live system bus. All nine
new public-service parents and the seven preceding copied-leaf parents pass on
that source. Both required stopped/active gates are unignored and current.
terminal-leaf-verification-01.json records the exact parent summaries, source
relationships, gate binary and unchanged protected binaries. Inputs01 retains
its independent 412/180 results and the exact two earlier source files. No other
captured input changed. A passing media test does not resolve its prior flake.

All four red failure pairs remain. terminal-leaf-red-fixtures-01.tar.gz, SHA256
c2d910d1dc7373d58aca1cb7e5a78901b7e3568c310993d5b11243b706a8f482,
has 40 decoded verified entries. Its verification JSON records roots, modes,
identities, file hashes, links and originating logs. Original files remain. The two additional hard-link failure pairs have their
own terminal-alias-red-fixtures-01.tar.gz, SHA256
c9e3b0d010096ce0447992df8d8f9edf6246b4213847969408f3db5bd65d5378.
Its 20 decoded entries include two verified hard-link targets. Both inode/file
hash relationships are checked; original roots remain. terminal-alias-probe-01.json
records the repeated unchanged fingerprint and missing committed-removal evidence.

Final source, documentation and scoped proof records are preserved in
terminal-leaf-source-01.tar.gz; its actual hash and decoded member verification
are recorded in terminal-leaf-source-01-verification.json. Protected source27 app,
compatible Niri and earlier local-runtime gate must remain unchanged. Do not infer
native, full workspace, release, Shell/Notify or physical-device acceptance from
these scoped checks. The earlier intermittent paused-video/seek issue remains
undiagnosed, even if this isolated run passes.

## Important remaining work

Failure while recording the engine end currently preserves its original cursor
and can recover on a later open. The new rollback tests prove that behavior.
In-process Retry Save is tested for the independent application-result save after
the end was recorded. A proof-owned pending metadata witness is still needed to
expose in-process Retry Save when recording the engine end itself fails; do not
claim that boundary complete.

TERMINAL-LEAF-NEXT.md first tracks exact Ready/Returning source-task dispositions,
then that witness and directory publication/child queues,
then directory-removal, whole-entry Move, reviewed replacement and saved-container
phase advancement. Complete Preparing/Cleanup binding, queued lineage/admission,
legacy and large-report/paged-owner behavior, sanctioned Undo/relocation/expiry,
Trash/Restore, renderer separation, product/device/accessibility checks,
first-window/focus reliability and performance, native/workspace/release/Niri
integration and GTK retirement remain part of the active goal.
