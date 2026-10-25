# Opened cross-filesystem source claims, in progress

2026-10-22. Follows RESTART-LEAF-WIP.md and ADR0117. The complete five-area Files
goal remains active; global restarted Retry/Continue ownership remains closed.

Source-claim red01 reproduces both visible-parent adoption and deletion of an
unselected private payload at the actual syscall boundary. Actual private roots
are retained: /tmp/.tmpqIqmuI and crate output .canopa-source-claim-FyeSeK;
/tmp/.tmpG2k8MH and crate output .canopa-source-claim-6emaet. The old source is
parked in the first fixture; the second deleted the foreign payload while keeping
the original in the stage under original-user-source. The exact failed log stays.

Source::Opened now survives copy preparation in PreparedCopy.source. The new
source_removal module creates work through that held source parent and records
Ready/Claimed/Returning phases in Stage.source_claim. Original user data is never treated
as ordinary disposable copy staging. Copy/parent persistent facts are captured
from the held published copy. Source/parent/stage facts come from the original
held source and new private work. Schema32 gates the new interpretation.

Core01 first failed compilation on an Errno -> io::Error mapping. Core02 passes
the two original boundary regressions, with one unused getter warning (removed
afterward, not suppressed). Normalized completed/returned stages now clear
payload cleanup authority: any new occupant must remain unknown and preserved.

New real SIGKILL coverage tests before claim, after claim/before verification,
after verified claim/before unlink and after unlink/before append. It covers a
changed copied destination before restart; source data stays retained when it
cannot be safely removed, while already-finished unlink restores metadata only
and preserves newer copied bytes/inode across two opens. Both original boundary
regressions and these killed-process cases pass in Core03 (67 library parents,
3 ignored helpers). Source-remainder parents2 also pass.

Core03 exposes an existing placement acceptance regression: the first helper
returned a claim after platform availability had already been refused. The
existing losing_a_move_source_after_quarantine_preserves_its_payload_for_recovery
test correctly requires retaining that payload without further mutation. Both
before/after-claim checkpoint errors now preserve the journal when they are typed
placement failures. Core04 finished with130 passing parents across5 targets.
Core05/Clippy01 exposed the derived Undo SourceRemoved constructor after adding
completed original facts; that constructor now explicitly records None because
it did not remove original-location data. No current-path proof is fabricated.

Core06 passes130. Core07 passes161. On inputs02, Core08 passes172 across6
targets, Core09 passes111 across7 Undo/Recovery targets, and default all-target
Clippy02 passes with warnings denied. Inputs01 is a stale intermediate manifest;
inputs02 has843 entries including both new source-removal modules. Counterfactual
restart tests set both recorded copy scalars to identical replacement objects,
assert the full copy fingerprint also matches, and still require original-proof
refusal. The claim and normalized quarantine must match the recorded step.

The final Returning correction saves rollback direction and the actual return
subject before returning data. Kills before/after return, and a returned source
subsequently removed by the fixture, cannot become removal authority. Inspection
errors also remain errors rather than silently becoming absent sources. Inputs03
has843 entries and differs from02 only in the source-removal implementation and
its boundary tests. Core10 passes131 parents across5 targets on this final source
(68 library,28 application transfer,2 source remainder,2 stage recovery,31
transfer);6 inventory helpers ignored. The lost tool-session handle was followed
by complete terminal test summaries and an empty compiler process inventory;
reclamation03 then acquired both Cargo locks before reclaiming exact test ELFs.

Core11 finished exit101:63 passing parents across4 targets and the required sole
failure a_killed_stopped_batch_restores_both_exact_parts_and_keeps_completed_work.
It is unignored and still fails at restored Retry/Continue ownership. Keep actual
failed data /tmp/canopa-transfer-restart-Ph7iqu and crate output
.canopa-transfer-restart-output-IXqx4j. The repaired unselected child and newer
completed prefix remain preserved. Job checkpoints6, operation jobs28,
continuation10 and request execution19 pass;4 inventory helpers ignored.

The final desktop library finished exit0:136 passing parents,4 ignored helpers,
private D-Bus/HOME/XDG runner attempt1792639502, source inputs03, desktop-lib-01.
Its receipt proves843 unchanged inputs before/after. Formatting01 finished0 and
the schema32 fixture parses. Core12 finished0 with167 parents across10 Undo,
directory-permission and Recovery targets,10 ignored inventory helpers. Together
Core10/11/12 have361 passing parents across19 targets, the sole required restart
failure and20 ignored inventory entries; backend-results-01 parses only final
parent harness summaries. Neither subprocess nor desktop overlap is double-counted.

Default all-target Clippy03 stopped exit101 with ENOSPC while checking
device_runtime; this is an infrastructure failure, not a passing lint check.
The completed desktop-library ELF was audited/reclaimed (103,127,008 bytes);
all older completed Files logs were inspected and had no other unprotected test
ELFs remaining. Root free space became582MiB after rustc temporary files retired.
Clippy04 finished exit0 in59.52s with warnings denied, default features and all
targets. Final verification01 binds inputs03, the three current backend logs,
desktop receipt, lint/format/fixture checks and the still-failing required gate.
Source01 is the decoded-verified checkpoint archive; required-fixture01 preserves
the14 raw private fixture entries (the original roots also remain untouched).
Reclamation audits01..06 preserve exact hashes/inodes and exclude the application,
Niri, required gate ELF, source and proof fixtures/archives. The prior leaf
source03/841-input schema31 checkpoint remains intact. Protected application
b379372fb3f1 has not been rebuilt. Generic claims and full restarted execution
must not be replaced by review-only behavior or a weakened gate.
