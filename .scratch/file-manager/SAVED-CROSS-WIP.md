# Cross-filesystem saved-container restart work — in progress

Continues the verified schema43 saved-container whole-Move checkpoint (ADR0133,
SAVED-TASK-NEXT.md). The full five-area goal remains active. Live descriptor copy, metadata restoration and target ownership now have
scoped fixes. Strong restart publication/cleanup/task binding is not yet
implemented; the final verification below retains the actual red gate.

Authoritative inspection confirms saved_stage::prepare_inner replaces its saved
source parent scalar from current path metadata, uses held handles while copying,
then calls legacy recover through the serialized Migration instead of passing
those handles into its mutation helper. recover publishes using scalar/path checks and
cleanup_source quarantines/unlinks by paths. destination_stage drops ownership.
Store.publish_saved_stage already performs atomic catalog/source-reservation
cutover with append; preserve that transaction and older Undo behavior.

Added cfg(test) transfer/saved_cross_boundary.rs and a finished hook after actual
prepare_forward, before task checkpoint. The public OperationJobs fixture creates
an actual retained replacement version, selects its catalog-owned container and
moves it between /tmp and a private repository output directory (device IDs must
differ). SIGKILL occurs only after migration cleanup returned. Parent creates a
new source occupant, edits completed saved data, and requires Finished/count1
across two opens without Retry/Remaining or data mutation. Failure source and
output roots are retained. This correct seam checks exact active task restart;
older undo_saved_stage_move handoff integration deliberately retries a fresh root.

saved-cross-red-01.log is the first run; handle must be polled if still live.
Reclaim audit saved-cross-cache-01.json freed only the completed final default
library ELF from the preceding checkpoint. Protected app/Niri/earlier restart
executables and all previous archives are unchanged. CARGO_INCREMENTAL=0,
TMPDIR=/tmp, --locked --offline -j1. Formatting completed before starting red01.

Before production edits reproduce red twice, show ranked falsifiable hypotheses,
and implement actual held source/parent/previous plus target parent/work/payload
original proofs, safe publication and explicit source cleanup phases. Do not bind
legacy migration to a cursor while retaining unsafe path mutation. Canonical
change/receipt and catalog source-retention lineage must cut over together. New
source occupants after a claimed cleanup, including same-inode namespace reuse,
never prove a return or grant deletion. Missing legacy proofs stay reviewable;
no fresh generation adoption. Preparing/Cleanup authority and exact task bindings
must be audited as part of full recovery; a passing final-result test is not full
cross-filesystem continuation acceptance.

Red01 and red02 both terminate101 with the same uncovered completed step at
operation#2. Both private source and destination roots remain; red-fixtures01
preserves 18 decoded/verified files. The exact red source reconstructs from the
verified schema43 source02 archive plus only the new cfgtest module/hook; its
transfer snapshot and 871-path red manifest are retained.

Ranked hypotheses were shown: missing exact task receipt causes false Interrupted;
legacy mutation ignores held descriptors and lacks original proof; cutover
loses canonical original ownership needed by later work. Receipt-only binding
would not fix the second/third problem and must not authorize legacy mutation.

First production foundation: Forward owns the original entry_move Selection;
Selection.copy_saved opens previous relative to that actual selected directory and
prepares its output relative to the retained destination parent. Every copy
checkpoint also checks the selected parents/container. fingerprint::prepare_copy
and its new prepare_pinned share one implementation with unchanged ordinary
checkpoint-before-source-open ordering. source::Opened::open_in preserves the
actual parent handle. No new serialization/schema or restart binding yet.
The six existing opened-source regressions pass (pinned-copy01). Formatted source
is pinned-inputs01 (871 paths), captured after that terminal run with no edits.
The original public restart probe must remain red until the actual safe migration
claims and exact task completion are implemented; red03 is running on foundation.

Foundation red03 reaches the actual completed migration and fails with the same
uncovered step (terminal101), confirming the pinned live-copy path executes but
does not pretend to fix restart. No Cargo process remains live from these runs.
The first archive reconstruction assertion was corrected before creating any
artifact: transfer has multiple saved_receipts.remove calls, so reconstruction
now removes only the exact cfgtest additions from the actual transfer snapshot
and proves equality to the archived source. Source/test/failure archives remain.

A second concrete safety gap is metadata restoration on the created copy by path.
Added a real OperationEngine saved-container cross Move regression with an old
raw-link timestamp. It verifies a successful descriptor timestamp roundtrip and
substitutes a regular foreign payload at the exact actual metadata boundary,
requiring its bytes and timestamps to stay untouched. The test hook operates only
on its own temporary stage. Both roots are kept on failure.

The descriptor implementation is preserved in with-metadata-fix.rs; for the
first metadata repro the call alone is temporarily the old pathname restore.
This is an intentional controlled baseline, not the final production path.
metadata-red-inputs01 captures its 871 formatted build inputs. metadata-red01 is
running. After reproducing, restore the preserved saved_stage.rs descriptor call,
format to completion, then rerun the same public regression. The source/staging
helpers restore only the held file/link; normal and reverse saved copy preparation
must both use that FD and verify the actual created payload before/after changes.
The overall completed-task restart test remains deliberately red until actual
safe migration claims and task receipts are implemented.

Metadata-red02 and red03 both fail at the intended timestamp invariant: the
replacement file receives (946684800,123456789) instead of its captured 2026
mtime. Red01 stopped earlier at an over-specific failures-list assertion; direct
root inspection already showed the same overwrite. The assertion was corrected
before the two timestamp repros; its original test source is preserved.
Restoring the one descriptor call passes metadata-green01, including the normal
raw-link timestamp path and unchanged substituted bytes/time. The existing public
cross-filesystem saved raw-link + successive older Undo regression also passes
(raw-link-undo01). metadata-fixed-inputs01 pins all 871 source hashes after separate
completed formatting; all remain unchanged after the integration check. No full
suite or complete cross-filesystem restart acceptance is claimed.

Next ranked probe: canonical target ownership is dropped at cutover. An actual
cross migrated saved container should remain eligible for a later same-filesystem
Move under its original created target-generation proof, and both Moves plus
older replacement Undo must succeed. The new public synchronous probe will
separate this ownership issue from the still-unimplemented active restart receipt.

Owner-red01 exposed a fixture callback error before the ownership probe: the
regular copy's Progress reply was Cancel. That run is not ownership-failure
evidence. The original fixture source and manifest are preserved; the callback
now Continues Verifying/Progress and cancels actual questions. The probe must be
rerun to reach the real second Move and classify its failure before any fix.

Owner-red02/03 reach the real second Move and both refuse its stale source-root
proof. The synchronous probe now lives in tests/undo_saved_stage_move.rs (public
seam and faster fixture iteration). owner-fixed-inputs01 preserves target proof
from the created payload as previous, removes disposable-payload rights and
updates all retained owners at cutover. Unknown leaf proof cannot be promoted;
an older target without proof cannot downgrade a strong owner to legacy scalar
cleanup rights. owner-green01 passes the cross Move, same-filesystem Move and
all three Undo steps. No new schema fields; strong copied task/cleanup journal
phases are still not implemented.

The metadata hook is being strengthened to the point after Prepared's final
require_current check. The checked-metadata baseline deliberately uses that
check followed by old path-based restoration. Its fixed staging file is preserved
in checked-metadata-fixed-staging.rs. After reproducing this remaining TOCTOU,
restore that file, format separately, then run metadata regression again. This
proves the FD use itself, not just an added precheck. checked-metadata-red-inputs01
captures this temporary baseline. Restore before broad checks or continuing work.

The after-final-name-check metadata baseline reproduces the same foreign mtime
overwrite twice (checked-metadata-red01/02). The descriptor-only staging file was
restored and separately formatted. foundation-inputs01 (871 paths) captures the
current source. Foundation library passes 152 tests/18 ignored, explicitly
filtering only the original completed-cross restart gate; that gate remains an
unignored #[test] in source and is still known red. This is not full library or
restart acceptance. The metadata parent passes with substitution after the
last require_current check, proving FD mutation rather than just an extra guard.
Full saved-container integration passes 24 tests/1 ignored on the same source,
including both killed legacy handoffs, prior schemas, owner move/three Undo,
raw links, history expiry and partial/reconnect workflows. Clippy and format
checks are in progress. All 871 captured build inputs remain unchanged.

Final foundation checks on foundation-inputs01: 152 backend-library tests pass,
18 ignored, with only the known unfinished completed-cross restart gate filtered;
24 saved-container integration tests pass, 1 ignored. All-target default Clippy
with warnings denied and workspace format pass. The current restart gate is
rerun unignored on this same final source and still fails with uncovered operation
#2 (current-restart-red01), retaining both private roots. No claim of full suite,
restart completion, native/device/workspace/release acceptance. Source hashes,
public metadata race and owner/three-Undo cases, failure archives and protected
binaries are checked in foundation-verification01. Goal stays active.
