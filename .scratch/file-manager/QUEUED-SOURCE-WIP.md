# Queued source execution work

Status: remainder regression fixed;51 focused tests pass; broader validation in progress.
The entire product brief remains active. Previous goal turn was progress:
Selected-source observations, descriptor-budget acceptance and ADR0076 daemon direction changed authoritative state; this turn fixes the existing remainder regression.

## Current process

All known compiler/native/archive handles are terminal. Full-core9204 exits0:
447 summary checks (446 parent plus one isolated helper). Iced-library42599
passes51; focused14396 passes51 parent tests; Clippy7821, Fmt42942 and default
build94088 exit0. Both scoped reviews report0 findings. Archive26064 exits0:
source05 and candidate d2f3a1a8 are preserved and decoded/member hash-verified.
Source06 changes only ledger and ADR0075 verification prose. No new native
acceptance is claimed. The next slice registers and runs the ordinary clipboard
identity regression before changing its production workflow.

## Implemented core seam

The transfer core owns opaque ObservedTransfer: an immutable request plus opened
selected-entry observations, captured before queue admission is acknowledged.
O_PATH/NOFOLLOW/CLOEXEC preserves symlink identity and avoids opening FIFO/device
contents. The engine rechecks selected roots and selected-directory ancestors,
including after conflict answers. Ordinary capture failures remain isolated.
No Iced code or source-safety sequencing was added. See ADR0074/0075.

The shared process budget is min(4096, descriptor soft limit/4). Clones share
ownership; failed partial capture closes entries before returning reservations.
Budget and system descriptor exhaustion reject whole admission. Observations
release after settlement; restored/stopped plans remain review-only.

## Evidence

- Original queued Move regression:22545 exit101, genuine unrelated-replacement
  consumption, queued-source-identity-red-01.log and source01.
- Initial fix:30817 exit0, one public regression; source02.
- Focused76928 exit0:22 runtime tests and24 transfer tests, one deliberate child
  helper ignored. Cross-filesystem Move/Undo, source quarantine, cancellation and
  killed-copy recovery pass on source02.
- Standards review identified unbudgeted descriptors. Private RLIMIT128 child
  regression2894 exit101 genuinely admitted an oversized selection. Its inputs
  are source02 production plus the newly added budget test (budget-red-source).
- Budget fix81925 exit0: parent and child pass; verifies whole refusal, shared
  reservations, partial cleanup, live-question usability and later successful work.
- Source03 adds public engine coverage for changed folders/links/missing-then-
  created sources, unaffected file and dangling-link Move/Undo, plus source
  replacement during a live conflict question. Focused55777 passes all26 engine
  tests. Full34811 passes all23 runtime tests (one child helper ignored), then
  stops at the existing completed-folder remainder regression described above.
- Final Standards review resolves the P2 and finds no further scoped findings.
  Updated Spec review also found none. The subsequent full-suite failure remains
  an open correctness finding regardless of those scoped review results.

Verification details: queued-source-verification.json. Source28/c2b9e792 remains
the prior fully verified production checkpoint; no desktop binary is newly
claimed. Regenerable Files incremental caches (851599190 bytes) were removed
under Cargo locks with all known handles terminal; evidence and binaries remain.

## Remaining scope

The observation does not freeze contents or make syscalls descriptor-relative.
A missing original after successful verification still relies on engine journal,
payload identity and fingerprint proof for deliberate quarantine transitions.
Copy/Cut-time ordinary-source observation, request/journal association, original
media, restart reconciliation and safe Retry/Continue remain open. No saved
payload or source-retention lease grants execution authority. Preserve exact
stopped plans, isolated failures and completed-prefix Undo while continuing.

All remaining Iced Trash/Recovery/devices/media/DnD/activation/Downloads,
accessibility/performance/shared-stack/workspace/release/packaging and final
review/commit gates stay active. No full product checkbox is complete.

The user then proposed daemon/headless operation. ADR0076 and the updated PRD
record optional residency, a host independent of Iced and activation of one
owning runtime. Separate-process layout/transport remain implementation choices;
no daemon feature is implemented. ADR0074 now acknowledges that hosting value.
