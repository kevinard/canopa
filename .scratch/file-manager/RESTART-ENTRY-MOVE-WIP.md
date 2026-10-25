# Whole-entry same-filesystem Move claims, in progress

2026-10-22. Previous goal turn was verified progress (source-removal implementation,
361 selected backend passes and one required gate failure, desktop136 pass,
Clippy/format pass,843-input decoded source archive). The full goal stays active.
All843 inputs and the source01 archive were checked unchanged before this work.

Red01 is a test compilation error (nonexistent Result::is_err_or), not a bug
reproduction. Red02 then reproduces both actual pathname rename failures:
source-parent replacement and destination-parent replacement each return
completed1 with no failure. Keep actual private roots:
/tmp/canopa-entry-move-boundary-p9Wo6i and
/tmp/canopa-entry-move-boundary-wjWk3u. The first moves unselected replacement
bytes, the second installs original selected bytes in the replacement folder.

Ranked probes: pathname re-resolution at syscall; late source capture around
conflict; accepting syscall success without a verified publication. The new
entry_move module holds the entry/parents before conflict, exposes selection,
prepare and commit, and owns durable Ready/Publishing/Returning reconciliation.
Source versions and available actual object/parent facts persist in completed
Rename.move_facts. Stage.entry_move is protected from generic disposable cleanup
and ordinary Rename review. Existing constructors explicitly retain absent facts.
Schema33 gates the role; exact migration/desktop fixture assertions were updated.

Core01 exposed one exhaustive Undo pattern and an unused import; both fixed.
Core02 passes the two original boundary regressions. Core03 finished0,
with real SIGKILL matrix added for file, raw final link and folder; before claim,
Ready, Publishing, Published and Returning/returned, plus newer work and missing
returned source. The command's module filter also applies to its three selected
integration targets: it compiles them but does NOT execute their broad coverage.
Core04 ran them unfiltered:171 passing parents across4 targets,7 ignored helpers.
Do not count Core03's zero-match targets as broad coverage.

The six-case generation/step-coherence probe passes replacement payload, private
stage, source/destination parents and published generation cases, including
rewritten scalars and matching moved-version metadata. Red03 then reproduces a
remaining published-scalar error: recovery appends a row with the wrong identity
and clears the journal. Keep /tmp/canopa-entry-move-restart-EjkqbX. Final publication
and returned-selected checks now require matching actual identity/type as well as
original facts. Core05 passes176 across6 targets,8 ignored helpers.

Red04 is the public observed-transfer handoff: after admission, replace the
source parent and hard-link the exact old selected inode into it. The source
pin matches, but execution adopts the new folder and reports completed1. Keep
/tmp/canopa-admitted-move-parent-4wHxVn. Available captured entry/parent facts now
constrain live source checks, and captured destination facts constrain its root.
Core06 finished0 with the invalid-operand capture guard and this correction.
Move selection now precedes UI metadata read and is rechecked immediately, so a
changed source type cannot be approved using stale display metadata. Final
source/verification follows after freezing the new845-input graph.

Inputs01 freezes845 inputs after moving capture before source UI metadata read.
Core07 passes178 parents across6 targets,8 ignored inventory helpers. Red05 then
strips a claim's stage role and creates a matching foreign target: generic forward
inference appends a bogus completion and loses the original private-source journal.
Keep /tmp/canopa-entry-move-restart-fiUfvr. Forward/Rename.move_facts now requires
the whole-entry role before reaching legacy inference. Inputs02 differs from01
only in that dispatch guard and the additional fault case; use02 for final checks.
Red03's read-only observation confirms one committed row with a different identity
from the actual published object and zero pending rows; no authority was inferred
from reading that retained fixture.

Scoped final source checks are recorded below; native/full-product verification
remains open. Inputs02 is the current845-input manifest. Core08 finished101:
74 library parents plus63 queue/schema/continuation parents pass; the sole failure
is the required unignored killed-process Retry/Continue ownership test. Keep
/tmp/canopa-transfer-restart-57QFMt and crate output
.canopa-transfer-restart-output-EPCTXk. Copy/Failed-first is reached; later restart
orders and real cross-filesystem Move are not reached by that failing gate.
Core09 finished0 with113 parents across6 integration targets,5 ignored helpers.
Core10 finished0 with117 parents across8 remaining Undo/Recovery targets,8 ignored
inventory helpers. Core08/09/10 together pass367 across19 targets, with the sole
required gate failure and21 ignored inventory helpers. Final parent summaries
are in backend-results-01; child helpers and desktop overlap are not double-counted.
The isolated desktop library finished0 with142 pass/5 ignored, runner1792639503,
inputs02 unchanged before/after. All-target Clippy01 finished0 with warnings denied
in1m40s; its session63870 is terminal. Verification01 and decoded source01 preserve
this unfinished schema33 checkpoint. All845 inputs remain unchanged. Full native,
workspace/release acceptance and actual restarted execution ownership stay open.
Reclamation01..05
holds both Cargo locks and records exact completed test ELF hashes/inodes; app,
Niri, required gate, source and fixtures/archives remain excluded.

The exact two inputs01 members changed by02 are reconstructed and hash-verified
as operations-source01.rs and boundary-source01.rs. Source01 broad runs are
predecessor evidence; use current02 runs in final totals. Failure archive01
preserves all five retained red roots and both required-gate roots with decoded
member verification. Reading or archiving those roots grants no execution rights.
The first archive verifier rejected a valid hard-link member; resolving internal
tar hard links verifies all58 entries and the one preserved hard link without
extracting data. Fixture syntax verification caught and corrected indentation
in the schema33 assertion; final syntax01 passes. Formatting13 passes. Protected
app/Niri hashes were checked unchanged; neither has been rebuilt.
Previous protected app b379372fb3f1 and Niri63e39bae3473 remain intact.
Covered probes include replacement stage/payload/target generations, return and
publication deaths, directory-permission compatibility, claim-role loss and exact
object-value coherence. More cleanup deaths, blocked publication/return, full
operation/slot binding and unsupported proofs remain. Destination replacement and saved-stage/empty-folder
claims still need implementation. The required runtime Retry/Continue gate must
remain unignored and failing until actual executable ownership is safely restored.
