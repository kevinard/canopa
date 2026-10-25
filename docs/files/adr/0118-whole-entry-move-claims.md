# 0118: Journal whole-entry Moves through the selected entry and parents

Status: implemented for ordinary whole-entry Moves without replacement or saved
container movement; scoped verification, 2026-10-22.
Continues ADR0117 and the unfinished full Files goal.

Two real rename-boundary tests replace either the source parent or destination
parent of a same-filesystem Move. Both report one completed item with the old
implementation. The first moves an unselected replacement file; the second moves
the selected file into an unselected folder. Retained red fixtures preserve both
results. Checks made before a pathname-based syscall do not bind its operands.

Capture the selected entry with O_PATH/NOFOLLOW and both followed parent folders
before conflict handling. Preserve the entry's version and available original
generation facts from those descriptors. Keep-both may change the destination
leaf, while both original parent bindings remain required. Claim, publish and
return through the held directories with NOREPLACE. Verify the actual claimed
entry and its version after callbacks. Directory claims use a hidden sibling in
the destination parent, avoiding an extra parent transition for readonly folders.

Schema33 gives whole-entry Move staging a distinct role. Ready permits return
of the proved selected object, Publishing records a verified publication intent,
and Returning records the actual return subject before rollback. Startup must
match the exact Rename step and Move facts, both original parent generations,
private stage generation and selected object generation. Unchanged Publishing
claims may finish publication; changed claimed versions return without replacing
a current source occupant. Already-published originals restore metadata while
preserving later edits. Missing return subjects and inspection errors retain
the journal. Cleanup receives empty-stage rights only.

Completed Move rows retain their descriptor-derived version and original facts.
Derived Undo, saved-container movements and ordinary Rename do not fabricate
those facts. General Undo consumption and relocation lineage remain unfinished.
The existing ordinary Rename review path cannot consume this Move claim role.
Unsupported original proof keeps restart mutation unavailable for that claim;
the live process retains its actual descriptors and journals empty cleanup.

A counterfactual publication test changes all duplicated object scalars while
leaving the captured generation facts intact. Recovery originally appended a
row with the wrong identity; require the actual published object's identity and
type as well as its original generation. Replacement payload, private directory,
source/destination parent and published file tests also preserve both originals
and foreign objects across two opens, even with matching replacement metadata.
A Move step with missing claim-role metadata is refused before legacy forward
inference. A fault test otherwise loses its journal and appends a foreign target
as completed while the original source still sits in private work.

Admission must constrain later preparation. A public observed-transfer test
replaces the source parent before execution and hard-links the original file
into that new parent. Its inode still matches the live pin, but the old code
moves the unselected new directory entry. Live selected-source checks now consume
available admitted entry/parent generation facts, and destination checks consume
their captured root facts. Preparation cannot turn fresh path observations into
authority over a replacement parent. Missing verified sources during a claimed
step retain their existing handling. Unknown proof still grants no restart owner.

The current path covers whole-entry same-filesystem Moves without replacement
or catalogued saved-container movement. Destination replacement, those saved
movements, decomposed empty-folder removal, legacy staging, complete step and
handoff lineage and actual scheduler ownership remain open. The unignored
killed-process Retry/Continue gate is still required for Copy and real
cross-filesystem Move in both part orders. Other restart families and all later
backend, product, reliability/performance and release work remain active.

Inputs02 freezes845 build inputs. Selected backend coverage passes367 parents
across19 targets, with21 ignored inventory helpers and the sole required unignored
restart ownership gate still failing. Copy/Failed-first is reached by that gate;
the later part orders and real cross-filesystem Move are not reached. The isolated
desktop library passes142,5 ignored; default all-target Clippy, formatting and
schema33 fixture syntax pass. Source01 and verification01 retain exact inputs,
terminal logs, the corrected hard-link-aware failure archive and scope. The
application and Niri binaries remain unchanged; native/full-workspace/release
acceptance is still required.
