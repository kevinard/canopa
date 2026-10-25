# Opened Copy leaves and private work, in progress

2026-10-22. Follows RESTART-DIRECTORY-WIP.md. The full five-area Files goal
remains active; the required unignored restarted Retry/Continue test is the gate.

restart-leaf-red-01.log reproduces staging an unselected link target when stage
registration replaces the source. The helper eventually reports failure but
has already observed/staged the replacement. Opened source leaves now retain
their actual parent and leaf descriptors, type before readable I/O, preserve raw
link targets and check the final source checkpoint. The first green attempt
exposed two test/compatibility issues: renaming a pinned regular leaf changes its
ctime, so a new readable reopen correctly refuses that old snapshot; the
nonregular error must retain InvalidInput. The tests and error classification
were corrected without weakening the no-replacement assertions. Core02 passes
58 library parents, 1 ignored.

restart-leaf-stage-red-01.log then reproduces adopting an unrelated staged
payload during progress as successful copied data. Opened stages now create and
write through the held destination parent and private directory. PreparedCopy
keeps the actual payload alive through publication. Schema31 persists available
stage/parent/payload facts; the new payload is registered before its first byte
write. Proven-stage cleanup refuses unknown or replaced payloads and validates
the original stage and parent. Generic legacy/unsupported stages and journalled
cleanup claims remain separate unfinished work. Core03 passes59 library parents,
1 ignored before the extra registration-boundary regression.

Core04 is running in unified-exec session68137. It selects the library, directory
permissions, application transfers and a new public transfer_stage_recovery
target. Its two parents kill real Copy/cross-filesystem Move children after the
first staged byte write. They check exact owned cleanup and refusal of replaced
files, links, FIFOs, directories, stages and destination parents across two
restarts, retaining original source and partial-copy bytes. The ignored helper
is explicitly invoked. The library additionally tests death-equivalent loss of
payload registration: an actually created but unrecorded leaf must survive two
opens. No result is claimed until the running handle reaches terminal status.

Core04 subsequently completed successfully:131 parents /4 targets, 4 ignored.
Source review preserved known stage roots if a leaf handle is unavailable,
instead of downgrading that record into legacy cleanup. Two additional units
then exercise Forward publication/refusal and unavailable leaf proof. Frozen
inputs01 has840 entries. Core05 finds a real recovery-order failure in the new
publication test: the journal was cleared before cleanup could refuse a
replacement. Clippy01 also finds that inline ownership facts enlarge a legacy
enum. Both failed attempts are retained.

Ownership facts now use one boxed value without changing their JSON. Recovery
cleans uncommitted work before clearing its intent. For committed changes with
stage facts, append_with_cleanup atomically records the change and a typed
Cleanup intent. Cleanup validates the operation/ordinal/path and never replays
the already-completed change. A refusal keeps its durable record. Core06 is
running as unified-exec session5875; inputs01 predates these further changes.

Core06 completed:62 library parents and2 public killed-copy parents pass, with2
ignored helper inventory entries. Source02 then adds real SIGKILL after the
append/Cleanup transaction in both journal lanes and a failed cleanup-insert
rollback test. Core07 passes135 parents across4 targets, 5 ignored inventory.
Core08 passes the Transfer and cross-filesystem/changed/unavailable Undo targets.
Its exact count is76 parents across4 targets, 4 ignored helper entries.
All840 other inputs stay identical in source03; only the cleanup helper's
unnecessary mutable closure borrow changes after Clippy02 reports it. Clippy01's
large enum was fixed by boxing ownership without changing JSON. No warning
suppression or weakened filesystem assertion was introduced.

Current frozen inputs03 has841 entries. Core09 is running in session85817
(library, saved-stage returns, Recovery directories, saved/directory/cross-
filesystem and changed-copy Undo); Clippy03 is running in session61178. Poll
those actual handles rather than restarting after observation silence. Audited
cache reclamation01 removed8 exact completed test ELFs/293360704 bytes under both
Cargo locks; source, app/Niri, archives and the required gate remain intact.
Final queue/schema/required-gate and isolated desktop checks remain outstanding.

Known next ownership boundaries remain explicit: the source pin currently lasts
through copy preparation, not the later cross-filesystem removal; source removal
must receive that original capability. Destination replacement still uses the
older pathname helper around opened publication. Legacy/unsupported stages,
first-registration failure cleanup, private cleanup claims and ordinary
same-filesystem Move/folder removal still need their full lifecycle. These facts
must not be bypassed by restoring scheduler execution ownership early.

## Final source03 checkpoint

All work handles reached terminal status. Core09 passes145 parents across7
targets (including64 library parents, superseding Core07's library result),
8 ignored inventory entries. Clippy03 passes with warnings denied. Core10 runs
all four queue/schema targets with --no-fail-fast and the required gate included:
63 parents pass, and only the required killed-process Retry/Continue parent
fails because its restored actions are still false. Its actual binary SHA and
retained private paths are recorded in restart-leaf-core-10-receipt.json.

Combined latest-per-target coverage is355 passing parents /18 targets, 1 required
failure, 19 ignored inventory entries. Child helper result lines are not counted
again. Source03 differs from02 only in the private postappend helper's closure
borrow style; the old helper is preserved with its exact source02 hash, and the
full library reruns on03. All841 current Cargo/crate/vendor inputs stay unchanged.
The isolated private-bus default library passes132 /3 ignored on03 (receipt
1792639501). Format02 and schema31 fixture syntax pass. Both Clippy failures and
all intermediate runtime failures remain preserved; no warning was suppressed.

restart-leaf-verification-01.json records the exact scope. The source03 archive
and failing required-gate fixture are retained with decoded member/hash audits.
The protected b379372fb3f1 application remains unchanged; no new native/full-
backend/workspace/release claim is made. The actual scheduler continuation owner,
remaining mutation/cleanup boundaries, other operation families and the complete
five-area goal remain active.

Next required work remains journalled cleanup/source/replacement claims, exact
step and handoff/abandon lineage, then real restored ownership and both part
orders of the required Copy/Move restart gate. Full backend/native/workspace
acceptance, later operation families and all remaining goal areas stay active.
