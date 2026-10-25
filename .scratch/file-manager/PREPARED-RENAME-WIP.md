# Current: Phase2 prepared Rename source, parent and device authority verified

Sole owner remains chat01a1587c-5f38-7f83-9113-c6ac3fcb8815, primary only. Original
chat notLoaded on final recheck. No owned compiler/test/native/helper or archive
runner remains. Host Niri1238 and historical private bus1804446/Niri1804450/
Files1804484 stay untouched. No Git writes, delegation or other-chat messages.

Backend preparation now owns Rename's original entry, followed parent, storage
requirements and retained-source protection before Iced opens the name draft.
Clones share one single-use capability; refusal preserves the same preparation,
while committed queue admission atomically transfers its source references.
Independent owners still block. A replacement device is refused even when its
actual directory/file inode stays the same; a fresh deliberate F2 selection works.
The real open-draft object and device adoption failures are preserved.

The lifetime test also exposed overlapping ordinary queue/journal writes failing
with database-is-locked. A plain100-folder public burst reproduces without any
prepared token or extra database reader. All19 mutating Store transactions now
acquire the writer before reading. No replay, retry loop, timeout, schema or budget
increase. Exact cause/probe distinctions are in ADR0106/0107 and the work log.

Frozen source26/739 inputs: prebuild28/48118 terminal0,465.868s; fullcore29/
receipt96/91079 terminal0,518.247s,591 parent tests across67 standard targets plus
custom device_runtime.39 ignored entries are not all independently executed.
Default Iced32/receipt97/71272 passes84/1 ignored in122.222s. Fmt33, Clippy34
and defaultbuild35/72124 pass, with every frozen input unchanged. ELF240396644fda,
134636048B, protected in decoded-verified gzip36/37859 (31997313B; archive SHA
676d8be6bf0f433f7a2b1e9376d95087f6d91ad4b15b4653601ecfdd55d40ea9).

Final-ELF native37/21477 passes2 expanded F2 cases/10 groups in89.911s; native38/
56278 passes4 Create/Rename/Undo/dismissal cases/30 groups in297.685s. Both
appearances, pinned Niri63e39bae3473, same source and ELF before/after. Four
critical final captures inspected; two earlier object-refusal captures also read.
Inventory184. Earlier ELF6fc9783ac705 separately passes native20/21,10 cases/
52 groups; those results are not counted as final-ELF results. Native02 and25
remain failed reproductions. Every attempt is retained.

Cleanup03/94332 removed exactly620 isolated Niri generated .rlib/.rmeta caches,
1419570811B, under its Cargo lock with executable hashes unchanged. Cleanup31/
76230 removed only20 freshly enumerated regenerable Files test executables after
the full pass,726860736B; hashes/.d ownership recorded, Cargo lock held, no live
compiler/test. Application/compositor/probe/source/proof and unrelated Shell
outputs remain. Those20 test binaries need relinking for a later full suite.

Decisions: docs/files/adr/0106-prepared-rename-authority.md and0107-history-write-
transaction-ordering.md. Evidence: prepared-rename-verification.json and primary
review/scope/diff40. Finalsource42 reconciles documentation/evidence; external
preservation43 must confirm archive completion. Full product remains open.

Next Phase2: deterministic real check/use race reproduction, then safe opened-
parent/claim/identity/rollback/recovery execution; exact end/step receipts and
filesystem-success/result-save reconciliation; durable original objects/media,
traversal/stages/cleanup and explicit reviewed authority; other-family ownership/
review and full actual interruption matrix. Saved plans remain nonexecutable.
Also audit other creation/queued parent boundaries before claiming all mutations.
Then phases3–6: remaining browsing/backend separation, full product/native/A11y/
hardware acceptance, optimized startup/focus/performance and workspace/release/
packaging, compatible Niri distribution and GTK retirement. Preserve all proofs,
check ownership/host state, and poll exact live handles before new work.

A possible next diagnostic is Linux seccomp user notification on an exact private
child's renameat2, followed by normal kernel continuation. Research only; no probe
or production implementation exists. This is a race test, not a sandbox policy.
https://man7.org/linux/man-pages/man2/seccomp_unotify.2.html . Earlier file-handle/
unique-mount-ID research also remains unprobed and supplies no restart guarantee.

## Earlier chronological evidence (pending flags below are superseded)

# Prepared Rename authority

Owner remains chat01a1587c-5f38-7f83-9113-c6ac3fcb8815, primary only. Original
chat notLoaded; no owned build/test/native runner or competing writer. Historical
private Files1804484 remains untouched. All733 sealed source56 inputs match before
new writes. No Git writes, delegation or other-chat messages.

Next loop: actual F2 prompt, replace selected leaf while open, allow normal watcher
refresh, submit typed name. Assert original parked object and replacement both
unchanged, no renamed target. Candidate gap unverified until the native case runs.
Current app73cc8dfa2c9d is the sealed previous build; no production change yet.
New maintained cases use both appearances. Initial setup command used wrong
manifest key cases instead of scenarios; failed before writing that manifest.
Corrected to the existing scenarios key. No behavior result from setup.

Native02/90503 terminal1 in53.257s, source01 all735 inputs and app unchanged.
Both real dark/light cases reproduce adoption: original selected object stays
parked, replacement bytes move to renamed.txt and Rename dialog closes. Both
observation JSON files and before/after captures remain. Not a startup failure.
Ranked hypotheses shown before changes: admission-only capture; stale watcher/
origin handling; dispatch ignoring original observations. Source review confirms
UI draft held only path, admission captured current entry, and Origin.current
was not consulted on submission. A UI epoch check alone would not cover missed
refreshes. Original queued-observation tests remain regression coverage.

Implementation pending verification: backend PreparedRename captures the final
entry and followed parent with existing pin limits before Iced opens the draft.
Its shared single-use token owns a private retained-source lease. Atomic admission
copies refs to the accepted request and removes only that lease's refs in one
transaction; empty lease row retires normally. Refusal preserves token and refs.
Current drafts and accepted queues share the original observation. Names keep
Unix bytes in backend; Iced owns draft text/focus. Ordinary low-level submit still
captures at its own admission boundary. No saved replay or atomic syscall claim.

Cleanup03/94332 terminal0: exactly620 isolated Niri generated .rlib/.rmeta files,
1419570811 bytes, recorded hashes under Cargo lock. Application and Niri executable
hashes unchanged. No .so, probe, executable, source, shared Shell or proof removed.

Core05/receipt85/3615 terminal101: compile80s succeeds. Queue24 parents pass,
2 ignored; new draft-retention expiry test times out waiting for last folder job
after90s. Later targets do not run. Source04/736 inputs remains exact. No fix
claimed. Ranked next probes: result/start-save failure blocks dispatch; Finished
row absent only from snapshot; real filesystem stall/Running journal. Source06
adds only public snapshot and SQLite phase diagnostics at the original deadline.

Diagnostic07/receipt86/33366 terminal101: before the old deadline, the
public queue shows many `operation history: database is locked` failures, then
Interrupted work needs Recovery review. Last row101 still queued; actual SQLite
phases0=29/2=61/5=11. Original observation/ref ownership did not cause a missing
Finished snapshot. New source08 minimises to100 overlapping plain CreateFolder
admissions, no prepared token or extra database connection. Ranked before probe:
deferred read-to-write snapshot upgrades; preparation's extra retained reader;
lease retirement contention. Official SQLite isolation/transaction docs explain
stale WAL read-snapshot upgrades and BEGIN IMMEDIATE; a cause/fix remains to be
verified through the minimal public workload.

Minimal09/receipt87/37259 terminal101: plain100 CreateFolder admissions reproduce
locked journal failures without prepared sources or any extra SQLite connection.
This disproves a required preparation/extra-reader/lease-retirement cause. Store
intent, append-context and trim/return transactions read ownership before writing
under deferred BEGIN. Source10 changes all19 existing mutating Store transactions
to BEGIN IMMEDIATE so the writer is acquired before those reads. Independent
scheduling writers were already Immediate. No added timeout/retry, no replay, no
schema change, and no writer held over user questions or filesystem operations.
This is the leading snapshot-upgrade hypothesis's probe, not yet a verified fix.

Core11/receipt88/87023 terminal0 in103.820s:57 parents across queue26, execution3,
retention28; four ignored entries include a parent-invoked helper. Unchanged-source
burst repetitions89/90/91 in handle63018 all terminal0. Source12 adds independent
prepared-owner coverage, native original-return/Undo assertions and ADR0106/0107.
All production code remains equal to source10; only one additional backend test
is unrun. Broad validation remains pending. All failed attempts preserved.

Source12 gates: new independent-owner core13/receipt92/12740 terminal0; default
Iced14/receipt93/88632 terminal0,84 pass/1 ignored in128.600s. Fmt16, Clippy17,
build18 in handle9882 all terminal0, every739 input unchanged. Clippy96.841s,
build128.075s. Prior app protection15 reverified decoded bytes before overwrite.
New ELF6fc9783ac7050286ace4e74dbee9f1556c34d12fdc61e17b6304aeb0fba0be9e,
134634072B. Archive19/73915 terminal0 protects decoded-equal gzip32012141B,
SHA256 f02fa7fbbbc9b8358e134e4811d06bfa8898e179fe078818d274987a99d0d4c3.

Native20/32549 terminal0 in64.303s,2 prepared cases/6 groups, both appearances.
Both real refusal captures inspected: readable error and controls. Returning the
original submits the same refused draft; shared Undo returns exact bytes/inode.
Native21/47796 terminal0 in507.278s:8 existing cases/46 groups, operations,
interruptedRecovery, continuation and dismissal in both appearances. Same app,
pinned Niri63e39bae3473, all739 inputs/ELF unchanged before/after. Full backend
has not yet run. Inventory184; full-product acceptance remains open.

Review candidate before freezing full backend: preparation captures source/parent
but currently selects storage requirements only at later submission. New source22
adds a public provider-replacement regression with the same actual entry and inode
while the draft token exists. No host mount or device changes; no adoption bug is
claimed until the new exact terminal-result assertion runs.

Core23/receipt94/19113 terminal101: prepared device intent becomes Finished after
private provider UUID replacement while its original source/parent remain the
same actual objects. This is real adoption, not timeout or synthetic serialization.
Ranked before probes: late snapshot capture; Storage ignores original requirements;
a different command accidentally prepared. Native fixture now extends the same
real F2 draft over the existing private GIO provider's disconnect/replacement/
reconnect markers, retaining exact original inode and bytes. No production fix
yet. All existing ordinary object-return/Undo assertions remain.

Native25/53982 terminal1 in67.218s on unchanged6fc9783ac705/source24: both
actual F2 cases reproduce device adoption. Both JSON records show phase5, source
absent and renamed target present after the private provider UUID changed while
actual file/parent inode stayed fixed. Ordinary object-return/Undo assertions pass
before reaching this new failure. All739 inputs and ELF unchanged.

Source26 captures Storage snapshot in backend preparation and uses that same
placement for admission; current availability is still checked against original
requirements. No UI device policy, new storage budget, schema, replay or timeout
change. Fresh deliberate preparation may use the reviewed replacement. This is
the late-capture hypothesis's focused fix; broader gates are pending again for
the changed application source. Previous gates remain exact to their old sources.
