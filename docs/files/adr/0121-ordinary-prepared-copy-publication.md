# 0121: Verify and resume ordinary prepared-copy publication

Status: scoped verification; complete restart ownership remains unfinished, 2026-10-22.
Continues ADR0120. The full Files goal remains active.

Actual public transfers killed at publication reveal two ordinary Copy gaps.
A replacement parent linking the same published inode is accepted into history
before stage cleanup rejects that parent. A prepared file edited after a crash
is deleted as partial work. The independent Forward binding, captured facts,
weak identities and fingerprint are valid before either filesystem change.

Use the original parent/work/payload facts before accepting a Copy step. Reopen
and verify the actual objects, then finish unchanged fully prepared output with
NOREPLACE through the held directories. Keep a changed payload or any destination
occupant intact for Recovery. Already-published output requires its original
object and parent; restore only metadata, preserving later contents and modes.
Missing work is acceptable only with independently proved publication. Sync the
publication directories before appending its history step.

Record Copied.publication before live publication. An older bound opened-copy
intent already contains the same original facts in its stage; completion may
retain those facts without observing a new pathname as authority. Missing original
facts, a mismatched work directory or a replacement output preserve the journal.
Cleanup receives only the completed step's empty-directory rights (ADR0120).
Schema36 reserves this interpretation from older executables.

The regression loop uses real public Copy and actual cross-filesystem Move with
SIGKILL on both sides of the publication syscall. Raw filenames and dangling
links, newer prepared/published bytes, preserved modes, occupied destinations,
missing/replaced work and changed parents are checked across two opens. Current
scope passes34 killed-worker cases and the library78 parents,6 ignored. Selected
no-default-feature backend coverage passes372 parents across19 targets, with the
required restarted Retry/Continue test the sole failure. Default all-target
Clippy, formatting and schema36 fixture syntax pass. The isolated desktop
library passes159 parents,7 ignored, on unchanged captured inputs. No native
application rebuild or complete backend/workspace/release acceptance is claimed.

Application execution ownership remains unfinished. Complete journal/cleanup
binding, empty-folder/saved-container claims, source-removal continuation proof
and typed handoff/abandon lineage remain necessary; keep the required public
restarted Retry/Continue gate unignored. General Undo/expiry must consume the new
published-object facts, and the remaining four goal areas remain active.
