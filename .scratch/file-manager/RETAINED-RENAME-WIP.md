# Queued Rename retained-source ownership

## Current terminal checkpoint — retained Rename and reviewed ordinary requests

Owner remains chat01a1587c-5f38-7f83-9113-c6ac3fcb8815, primary only. Original
chat notLoaded on final recheck; host audit shows no owned compiler/test/native
runner or competing writer. Historical private Files1804484 and its bus/Niri
remain untouched. No delegation, Git writes or other-chat messages.

Application code is frozen at source29/730 inputs. Prebuild31/49530 passes in
491.065s. Fullcore32/receipt83/8631 passes584 parents across67 standard targets
plus custom device_runtime in517.419s;39 ignored entries are not all independently
executed. Desktop35/receipt84/77213 passes84 tests/1 ignored,122.713s including
compile. Fmt36, Clippy37/28084 and defaultbuild39/49200 pass. All frozen inputs
match through those gates. Source46's native fixture correction and standalone
signal source leave all583 compiled crate/vendor/Cargo inputs unchanged.

Current ELF73cc8dfa2c9de4bd2aa463ce434b23bea4ed169f5e1f50564896fbaac6312a13,
134508008B. Protected gzip31653138B, SHA256
976fd5a81a145fa5135653f6ea6dd362e0c7d765726155b05972a87c4ec21c59; decoded
bytes equal the ELF (archive40/78434 terminal0). Prior ELF613871... and its gzip
were reverified before overwrite. Archive33/50338 verifies all249 inherited
archives. Archive52/12578 verifies exact members/hashes of all15 intermediate
source archives and protects/decodes diagnostic44's binary. Cleanup34/46237
reclaimed only20 precisely identified regenerated Cargo test executables,
726050720B, after the full backend pass; hashes/paths are retained. No proof,
application, compositor/probe, source or unrelated shared output was removed.

Clean native batches47/14357,49/66281,50/24270,51/1662 all terminate0 on the same
ELF and pinned Niri63e39bae3473: dismissal2/6, operations2/24, continuation2/6,
interruptedRecovery2/10. Total8 cases/46 groups in both appearances; source and
application hashes stay unchanged before/after each. Four critical clean-run
captures were inspected, plus two earlier diagnostic captures. Inventory182.

Native41/32298 terminates1: expected app SIGKILL was still a required process in
the fixture health list. Source42 fixes only deregistration/cleanup ordering.
Native43/39232 terminates143 after both children/root summary pass; no outer
receipt. Manual post-audit verifies every source42 input and the ELF unchanged.
Native48/98864 terminates143 during light acceptance with no root summary. Probe44
captures SIGTERM sender PID1670066, UID1000, comm codex, executable
/usr/lib/chatgpt/resources/codex. Why it sent the signal is unknown; do not
attribute older Native31/43 signals retroactively. All five exact orphan services
of /tmp/canopa-files-native-vgj39v8t were retired with runtime/UID/start/pidfd
checks; their files and the historical fixture remain. Clean49–51 use tty:true;
nonrecurrence is not a proven PTY fix. Both inner/outer diagnostics preserve
normal termination; fixture/app processes get no handler. Selftest45 exits143
with its own sender as expected. Official tool guidance documents long-lived
PTY sessions but does not explain this trigger:
https://developers.openai.com/cookbook/examples/gpt-5/codex_prompting_guide

Decisions: ADR0105. Review/scope/diff54 and retained-rename-verification.json
record this slice. Source56 is the documentation-reconciled final snapshot;
confirm external preservation58 and its terminal archive result before treating
it as sealed. Earlier chronological pending statements below are superseded.

## Next Phase2 work, not completed features

1. Reproduce the Rename prompt's authority lifetime before metadata admission,
   then design a backend-owned prepared capability and atomic lease handoff.
   Current object/parent pins start at admission. No prompt-period adoption bug
   is claimed without a real public/native regression.
2. Reproduce and fix the remaining check/use gap with original opened parents,
   entry-relative mutation, safe claim/quarantine, identity verification and
   rollback/recovery. A second pathname check is insufficient. Current Rename
   source reservations do not make syscalls atomic.
3. Extend request/execution ownership to other families, exact engine end/step
   receipts and filesystem-success/result-save reconciliation. request_effects
   counts/paths and absent journals do not establish completion. Undo needs its
   own reverse-execution semantics; do not blanket-exempt matching operation IDs.
4. Complete durable original-object/media proof, traversal/stage/cleanup ownership
   and explicit review/new-authority paths. Restored plans remain nonexecutable.
   Other stopped/restored Transfer/Trash/Undo/Recovery review/dismissal and general
   retained Move/Cut ownership remain separate from the new ordinary Mutation
   control. Independent clipboard/request owners must retain protection.
5. Complete actual interruption coverage at preparation, partial file copy,
   quarantine, commit/result save and continuation handoff; then phases3–6 of
   the original handoff/ledger. Full accessibility, hardware, optimized startup/
   focus/performance, all182 native scenarios, workspace/release/package checks,
   compatible Niri distribution and GTK retirement remain outstanding.

Primary-source research only: Linux name_to_handle_at can compare opaque object
handles; AT_HANDLE_MNT_ID_UNIQUE returns a64-bit unique mount ID. statx's unique
mount ID is not reused during one running kernel. Boot identity and filesystem
support still need private probes; no implementation or restart guarantee is
claimed. Ordinary mount IDs are reusable; opening by handle requires capability.
Sources: https://man7.org/linux/man-pages/man2/name_to_handle_at.2.html and
https://man7.org/linux/man-pages/man2/statx.2.html . Same-boot proof is a candidate,
not a replacement for supported media-return/reboot authority or explicit review.

## Earlier chronological evidence (superseded where marked pending)


Sole implementation owner remains chat01a1587c-5f38-7f83-9113-c6ac3fcb8815.
Original chat notLoaded; host audit found no active compiler/test/native/archive
runner. Historical private Files1804484 and its services remain untouched.
All725 finalsource36 input hashes match before new work. All earlier proofs are
sealed and preserved. No Git writes, delegation or other-chat messages.

Next: public queue reproduction of retained-source expiry before Rename dispatch.
The new test blocks dispatch on a real nonempty-folder Undo question, admits100
older folder creations, then admits Rename of a catalogued retained version.
This is a hypothesis until the test has actually run.

Attempt02/receipt62/handle3165 is terminal101: test compile failed because
OperationJobs::start takes PathBuf by value. No behavior result. Corrected the
fixture argument; source03 records the correction before another attempt.

Core04/receipt63/handle1010 and unchanged core05/receipt64/handle38222 are
terminal101. Both reproduce accepted Rename becoming Failed after100 queued
folder jobs expire its saved-source parent (17 other parents pass,2 ignored).
Ranked hypotheses shown before probes: missing admission reservation; expiry
ignores valid reservations; intact source rejected by observation validation.
Probe06 adds only Rename reservation admission plus diagnostic source existence
in the assertion. Prediction: expiry preserves the source, but existing generic
mutation guard will then refuse this request's own reservation. No owner bypass
has been added, and no fix/pass is claimed yet.

Probe06/core07/receipt65/handle77598 terminal101. The same queue now retains its
actual source, but Rename fails with the existing retained-owner WouldBlock.
This distinguishes missing admission retention from expiry ignoring reservations
and from false folder-observation rejection. Source08 adds an exact Forward
Change::Rename request-execution owner exception inside journal admission; every
other request/lease remains blocking. Undo, expiry, preparation, other mutation
families and reverse stage migration receive no exception. Scheduling phase is
not used to infer a new owner: the original durable link survives stopped rows
for an already-owned forward journal. No new replay authority is introduced.

Tests add admission rollback, queued cancellation, successful ownership release
and actual Undo after history expiry. Core execution coverage adds foreign
standalone refusal, independent clipboard/request refusal, nonreplayable failed
execution and Undo retaining the forward owner's guard until result settlement.
A scratch runner copy adds private --core-lib selection without changing existing
runner receipts or overwriting the original runner.

Source08/core09/receipt66/handle3954 terminal0:48 integration parents across
operation_jobs19, request_execution2 and request_source_retention27; four ignored
entries include parent-invoked helpers, not all independent executed parents.
Core10/receipt67/handle77113 terminal0:16 unit tests including independent owners
and own-associated Undo refusal pass. No existing protection was dropped.

Source11 adds ordinary Mutation request dismissal, persisted descriptive result
with original error/review notes and retained execution evidence. Both journal
lanes and exact revision are checked atomically with source release. No request
becomes executable. UI consumes backend dismissibility; active/queued/unsaved
rows refuse dismissal. New public tests cover failed retained Rename restart,
DB refusal rollback, independent owners, explicit fresh selection, actual commit
journal refusal/reconciliation and post-SIGKILL result review with newer bytes
and original execution digest/Undo preserved. These dismissal tests are pending.

Core12/receipt68/handle98482 terminal101:20 integration parents pass, one
real-journal fixture times out waiting for reconciliation Finished;2 ignored.
The dismissal refusal itself and failed-retained-Rename restart test pass.
No request_execution/source_retention target execution is claimed from this run.
Unchanged source11/core13/receipt69/handle43364 repeats the failure, backtrace
locating the reconciliation wait. Ranked hypotheses before new probes: failed
Rename discards history needed by recovery; self-retention blocks reconciliation;
placement validation rejects a moved journal path. Source14 only captures the
actual terminal reconciliation snapshot instead of waiting blindly for Finished.

Source14/core15/receipt70/handle88912 terminal101 reproduces an exact FOREIGN
KEY constraint failure during reconciliation. The syscall succeeded and journal
blocked dismissal correctly; discard_empty had deleted the parent operations row
because its first confirmed-change insert was refused. Source16 preserves an
otherwise-empty operation while either journal names it. Other empty operations
remain discardable. This is the first ranked cause, not a retention bypass.
The same public test must now reconcile before dismissal; all attempts retained.

Source16/core17/receipt71/handle21003 terminal101: the actual-journal
reconciliation/dismissal regression now passes. A separate failed-retained-Rename
restart test times out (20 other parents pass,2 ignored; later targets skipped).
Unchanged core18/receipt72/handle32250 repeats it; backtrace identifies the wait
for the restored Failed row, after worker initialization has ended. Ranked causes:
final lease-release acknowledgement precedes old history-lock retirement; saved
row validation fails; startup database error. Source19 only captures initialization
error before waiting for a missing row. Do not hide this by sleeping/retrying start.

Diagnostic source19/core20/receipt73/handle42441 passes21 queue parents.
Unchanged diagnostic repetition handle54301 stops at first failure: receipts74,75,76
pass; receipt77 fails20 pass/1 fail/2 ignored. Log retained-rename-restart-probe-77
captures initialization ending with no jobs and Resource temporarily unavailable
(os error11), consistent with the history flock still being held. Receipt78 was
not started by this loop. A focused public engine/lease test (source21) now loops
64 immediate explicit-final-release/reopen boundaries without retries or sleeps,
to discriminate history-owner retirement directly from application row decoding.

Source21/core22/receipt78/handle24445 terminal101: direct explicit final lease
release/reopen fails at iteration0 with EAGAIN (27 other parents pass,1 ignored).
This confirms lock-retirement ordering independently of row loading. Source23
adds a completion barrier after final explicit release once the engine has closed.
An unwind-safe worker exit notification follows duplicated-lock drop. Ordinary
Drop remains asynchronous and other live owners still protect history.

Maintained native dismissal cases are added for dark100/light125: colliding
Rename review; actual SIGKILL after confirmed Rename with result saving refused;
newer post-crash bytes/inode, execution digest and subsequent native Undo. They
are pending against a rebuilt app; no prior binary supplies this acceptance.

Source23/core24/receipt79/handle53541 terminal0:52 parents (queue21,
request_execution3, source_retention28), with4 ignored entries and separately
invoked helpers. The immediate64-iteration lease reopen regression and full
original restart/dismissal chain pass. Real-journal reconciliation, atomic save
refusal and SIGKILL lost-result review preserve current bytes/Undo/digest.

Primary review identifies one further lifetime question: immediate Undo may pass
only because no later operation has yet retried expired-stage cleanup. Source25
extends the public expiry regression with a later unrelated folder creation,
then targets the original Rename Undo. This is an unverified hypothesis until run.
No further production change accompanies that extra real-work boundary.

Source25/core26/receipt80/handle29605 terminal101: later real cleanup causes the
original retained-Rename Undo wait to fail (20 other parents pass,2 ignored).
Ranked causes shown before probes: expiry removes the private return container
needed by confirmed Undo; older Undo targeting is wrong; a finished request's
reservation incorrectly remains. Source27 only captures Undo's terminal/question
snapshot plus original-parent existence. Preserve this failure; do not claim the
retained-Rename feature complete from source23's immediate-Undo pass. No proposed
Undo-stage retention/promotion/schema design has been implemented or accepted.

Source27/core28/receipt81/handle50885 terminal101 captures Undo Failed with
'a more recent operation must be undone first'; the original retained parent
exists. This disproves the proposed missing-container explanation for this
attempt. The fixture requested an older Undo before undoing the newer folder.
Source29 corrects only the fixture sequence: reverse the newer folder first,
then original Rename. No engine/source-stage/schema/promotion change is needed
or implemented for this result. Native inventory --list29 passes with182 cases.

Source29 is frozen for application code: prebuild31/49530 terminal0 in491.065s;
fullcore32/receipt83/handle8631 terminal0 in517.419s,584 parents/67 standard targets
plus custom device_runtime,39 ignored entries not all independently executed.
Desktop35/receipt84/77213 terminal0:84 pass/1 ignored in122.713s including compile.
Fmt36 and Clippy37/28084 terminal0; build39/49200 terminal0 in122.066s. All730
inputs unchanged. Prior archive33/50338 verifies every249 inherited archive.
Cleanup34/46237 reclaims20 precisely listed regenerated Cargo test executables
only (726050720B), with hashes and fresh prebuild paths; no proof/archive/source/
application/compositor/probe/shared unrelated output removed.

Prior-app38 reverified ELF613871... and its protected gzip before overwrite.
Current ELF73cc8dfa2c9de4bd2aa463ce434b23bea4ed169f5e1f50564896fbaac6312a13,
134508008B. Archive40/78434 terminal0:31653138B, gzipSHA
976fd5a81a145fa5135653f6ea6dd362e0c7d765726155b05972a87c4ec21c59; decoded
bytes equal the executable. Native41/32298 terminal1: both new cases complete
failed-request dismissal and reach deliberate SIGKILL, then Session.wait_for
rejects that expected exit because the app was still a required live process.
The stack and both appearances establish a fixture registry error, not product
behavior. Dark narrow dismissal screenshot inspected: controls/text readable.
Source42 applies the established interrupted-fixture pattern: deregister before
kill/wait and re-enrol only if still live for cleanup. Exact signal/status, file,
identity, digest, phase and Undo assertions remain unchanged. No native complete
pass is claimed from41. The runner-only signal diagnostic records only arming.

Native43/39232 terminal143 after BOTH case children returned0 and root summary
recorded passed with unchanged pinned Niri. Its outer inline Python wrapper
ended before writing its receipt; the inner-runner-only probe records arming,
no SIGTERM. Post-audit43 manually verifies all730 source42 inputs and app73cc...
unchanged, without asserting a clean wrapper exit or attributing the signal.
No private native/build/test/archive children remain; old Files1804484 untouched.
Dark restarted narrow review screenshot inspected: facts/controls readable.

Ranked next probes: termination of the uninstrumented outer wrapper; a cleanup
process-group error; external tool/session termination. Probe44 is a standalone
copy extending exact command-line filtering to --canopa-native-parent-trace, so
both outer wrapper and original runner record SIGTERM sender PID/UID/PGID/comm
and preserve default termination. Fixture/app processes receive no handler.
Build44 terminal0. Selftest45 terminal143 as expected, records its own sender
comm python3 and default termination. No product code/assertion/timeout change.
