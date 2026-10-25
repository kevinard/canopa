# Strong copied saved-container restart claims — implementation in progress

Continues ADR0134 and SAVED-CROSS-NEXT.md. The full five-area goal stays active.
Previous turn made verified foundation progress; current source was checked
against all 871 foundation input hashes before edits. Preserve all prior source,
failure and protected binary artifacts. No commits/vendor edits/subagents.

Current unverified implementation (schema44): forward copy captures original
source parent/container/previous and destination parent through the pre-review
Selection. Container version is recorded to refuse namespace reuse before claim.
A copied binding digests immutable source/target stages, source version/fingerprint,
created copy identity/fingerprint and operation/context data; counted/bytes remain
in the canonical Change. Original target payload proof is required before binding.
Source version and actual created payload are verified through their held FDs.

New saved_stage/forward.rs publishes folder->previous and the created root through
held verified parents with NOREPLACE. Source data stays until atomic catalog/
reservation cutover. Cleanup Ready claims the whole original container to a private
sibling, verifies original root/previous before RootClaimed, then quarantines
previous->folder and verifies before FileClaimed permits unlink. Claimed absent
names do not inspect the old public source name. Unknown children, substitutions
and changed copies refuse; only the original empty claimed root can be retired.

A distinct saved-copy task matcher consumes its exact receipt to pop one task,
restore actual copied bytes, and count/report only non-nested items. Ordinary Move
and saved same-filesystem matching remain separate. Context validation uses the
original uncommitted Forward before cutover and original committed Change/bindings
after it. A dedicated non-serializable Transition saves cleanup phases without
recapturing current owners or recreating receipts. Legacy unproved forward
migrations remain reviewable rather than execute through scalar mutation.

Generic Preparing/legacy reverse/Undo provenance is still unfinished and must not
be confused with this new path. Strong copied recipes/cleanup and phase refusal
need actual public-worker crashes, changed receipts/contexts, parent/work/payload
substitution, metadata rollback, same-inode namespace reuse, batch/nested/part
orders, raw links and older Undo verification. No completion claim yet.

First gate build: saved-claims-gate-01.log, unified handle2033 if live. Formatting
was completed as separate acknowledged calls before the build. Reclaim audit
saved-claims-cache-01.json deleted only the completed owned library/integration
ELFs from the preceding foundation runs. TMPDIR=/tmp, CARGO_INCREMENTAL=0,
-j1 --locked --offline. Never restart a live handle due only to observation timeout.

Gate01 now passes unignored (actual completed cross move, changed output and new
source occupant, two restarts). Before further tests, committed cleanup phases
were strengthened with an independent digest in pending_bindings. It is derived
from the original committed bindings, never newly captured owners, and is saved
atomically with the phase. A changed valid enum now refuses before directory
access. Uncommitted claims require Ready. Catalog cutover installs the first
phase digest in the same transaction as the canonical step and reservations.

Phase01 passes five public parents/one ignored helper: the original gate,
metadata race, all 11 publication/cleanup crash boundaries, original-directory
same-inode namespace reuse after RootClaimed/FileClaimed, and missing/corrupt
receipt/context/phase checks before and after catalog commit. New source names
survive; completed output changes survive two further opens; actual bytes/counts
are restored without recopy. Root-claim filenames are bounded ASCII derived from
operation/ordinal/source inode and cannot exceed NAME_MAX through a long source
name. NOREPLACE and original proof still refuse collisions.

Phase02 adds parent/work/payload substitution with restoration, extra private
payloads after unlink, metadata cutover rollback, raw saved links, Keep Both and
nested/later batch work. It is currently running (handle35983). Formatted current
source is inputs01, 872 paths; no source edits during that run. Generic Preparing,
legacy reverse/Undo and broader goal obligations remain open even if these pass.

Phase02: nine public parents passed; the batch/nested parent failed only because
its fixture still asserted a top-level target parent for a nested Merge. The
child's mutation completed but the test guard panicked before its SIGKILL marker.
This is not application-failure evidence. Its roots remain. All-target default
Clippy01 passed inputs01 with warnings denied before correcting that fixture.
The exact old test source is preserved in before-nested-guard.rs.

Inputs02 changes only that test module: correct the explicit nested parent guard,
add actual second-process cleanup crash and phase-save rollback tests. No
production source change after the five-parent Phase01 pass. Phase03 is running
on inputs02 (handle22012). Bounded broader checks are prepared but not started;
no complete recovery claim yet. Remaining generic Preparing/legacy reverse and
other full goal requirements are still open.

Phase03 passes all 12 public parents and two ignored helpers on inputs02. The
nested fixture guard is corrected; no production matcher or filesystem check was
relaxed. Includes a second actual process kill during Reconcile and atomic phase
binding/journal rollback. The full library passes 163/19 with zero filtered tests;
all-target default Clippy02 passes inputs02. Broader 20-target backend validation
is running (driver handle13093), starting with saved-container/Undo. ADR0135 records
current protocol; full restart/Preparing/reverse/other-goal acceptance stays open.

Broader batch10 was terminal101 on inputs02: saved-container integration18pass/
6fail/1ignored, unavailable moves12pass/1ignored. All six failures create new
MoveToRecovery archives through Stage::prepare, which explicitly has no original
proof. The repeated-move repro fails again immediately in archive-red-02.log at
move0 with the exact missing-proof error. This is a real modern archive creation
gap, not permission to adopt legacy paths. Ranked hypotheses were shown before
probing/fixing. Original changed-copy/replacement/Undo sources are preserved.

Inputs03 changes those three production files plus the saved-cross test module.
Reviewed::prepare_preservation creates its actual container under the held parent
and records the already-selected entry's Original before displacement. Preserve
renames through held descriptors and verifies the same selected object/version.
Snapshotting a strong archive verifies original roots/previous rather than minting
fresh proof. Other legacy Undo creation/cleanup paths remain unfinished. A new
public-worker parent covers newly archived versions at four real cross-copy kills,
new source data/output edits and two later opens. No original refusal was relaxed.
Archive suites are running as saved-claims-archives-01.log, handle52874. Broader
batch10 was terminal and not restarted under the old manifest.

Archive suites terminal0 on inputs03:24 saved-container parents +12 unavailable
Move parents,2 ignored. The modern-archive creation fix restores all six prior
failures, including112 repeated cross-filesystem moves/history expiry. Exact
terminal/source receipt is archive-receipt-01.json.

A further public-worker test exposes a real completion boundary: kill after
unlink, edit completed output, then Reconcile fails on its fingerprint instead of
retiring the empty original claim. final-cleanup-red-01/02 repeat it twice; both
raw pending records show FileClaimed. Inputs04 contains that test before the fix;
only saved_stage/forward.rs changes for inputs05, with its original preserved.
Ranked hypotheses were shown before fixing. Strong committed verified claim
absence now ends metadata without reopening target/source public namespaces;
FileClaimed+missing folder permits only verified original empty-root removal.
Data still present keeps original-copy verification mandatory. New tests cover
edit/replaced output root/replaced destination parent at unlink and root-removed,
new source occupants, preserved displaced originals and two additional opens.
Full no-default library running as lib02, handle19130, inputs05. No full goal or
full restart completion claim is established by these scoped checks.

Inputs05 full library passed165/19, all14 public saved-cross parents green. Broad
batches30..34 passed, including both required stopped/active batch restart gates.
Batch35 stopped terminal101: transfers32/1, changed-copy Undo8pass/1fail/1ignored.
The archive fix had applied Reviewed::open's file/link restriction before Keep
review, rejecting a directory that replaced a copied file. The existing public
folder Keep-item gate reproduces twice (keep-folder-red-02). No data was changed
by that refused observation. Ranked causes were shown and exact originals saved.

Inputs06 changes only replacement.rs and undo/changed_copy.rs: observe an actual
entry of any type for Keep/Skip, but original replacement open and preservation
still require the held file/link. No folder can be displaced to Recovery. The
full changed-copy/transfers gate is running in keep-folder01, handle1549. Next
library03 and driver04 must use inputs06. Earlier green checks are explicitly
associated with their older manifests, not silently attributed to inputs06.

The folder Keep-item correction is green: keep-folder01 terminal0, transfers32 +
changed-copy Undo9 =41passed/2ignored on inputs06, exact source receipt preserved.
Full no-default library03 is running (handle16300), default all-target Clippy03
also running/queued under Cargo locking (handle15696). Await BOTH terminal before
reclaiming any test ELF or starting driver04; the cache helper takes a nonblocking
Cargo lock. Driver03 is terminal101 and must not be resumed/relabelled. Driver04
is prepared with the green folder suite plus the nine remaining batches; it must
use inputs06 and library03. Workspace formatting02 and fixture02 already pass.

Full library03 terminal0 on inputs06:165passed/19ignored/zero filtered, all14
saved-cross public parents enabled and green. Library exact-source receipt03
written after terminal. Clippy03 handle15696 still running; do not reclaim/start
driver04 until it too returns terminal.

Current inputs06 broad driver04 is terminal0:479passed across20 targets including
library03,40ignored child/helper inventory. Both required active/stopped batch
restart gates are unignored and pass. Inputs remain unchanged after each batch.
Clippy03 all-target default-feature -Dwarnings is terminal0; workspace format02
and fixture02 pass. Static proof static-checks01 is tied to inputs06.

Isolated default-feature library now running at handle33446, label1792709717,
source inputs06, desktop-lib01.log. It uses private HOME/XDG/session bus, no display,
no system bus/activation; protected app/Niri are not rebuilt. No auto-review
rejection or user approval request occurred. Last completed own integration ELF
only was reclaimed under cache-before-desktop01. Final verifier/archive awaits
this terminal result. Goal remains active; all other listed work is still open.

Isolated default-feature library01 terminal0, inputs06:246passed/20ignored/zero
filtered. Exact receipt continuation-core-1792709717-receipt.json reports no
changed inputs before/after, private environment elapsed353s. All14 public saved-
cross parents pass in both libraries. Final source/static/backend verification
and source archive are the remaining checkpoint tasks; no code work remains in
this scoped slice. Full goal and generic Preparing/Cleanup audit stay active.

Final verifier terminal0: saved-claims-verification-01.json verifies all872 inputs,
479 selected backend tests/20targets/40ignored,246 default-feature library tests/
20ignored,14 public saved-cross parents in both libraries, both required enabled
restart gates, exact attempt/backup relations, static checks, failure archive and
unchanged protected app/Niri/local-runtime gate hashes. No source changes followed
inputs06. The final source archive is built only after all logs and docs close;
its external source01-verification proof records decoded member and archive hashes.
Generic Preparing/Cleanup is the next implementation priority. Full goal active.
