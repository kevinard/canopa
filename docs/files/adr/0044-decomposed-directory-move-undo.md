# 0044: Undo a decomposed directory move as a directory return

Status: recorded directory-move scope verified; broader implementation in progress.
The full product goal remains active.

## Decisions

A directory moved across filesystems has recorded file returns and cleanup
steps. Undo must coordinate those existing steps when another folder occupies
the original location. It offers Merge, Keep both, Skip and Cancel at the root.
Merge preserves the newer destination folder and returns only recorded moved
children. A forward Merge leaves the preexisting destination and its unrelated
files in place. Missing original folders are recreated without a conflict.

History schema 13 records each decomposed directory's destination path and
identity, the beginning of its traversal, and its optional owned destination
cleanup step. The directory return adopts its existing journal children instead
of scanning and claiming all files currently in the destination. Original
postorder steps can precede their return marker, so member ordering no longer
requires a child's ordinal to be greater than its parent's ordinal.

Keep both persists its chosen return name before installing an owned, empty
destination. It reuses the small destination journal from ADR 0043, while
retaining the original child history. Older history follows the new location
and physical identities. A newly recreated or renamed return folder retains
older directory ownership; merging into a newer folder adopts only captured
parent references, without claiming that newer folder.

Multiple source folders in one batch can merge into the same destination.
Each pending directory keeps a reference to that destination. An owned cleanup
waits until the other directory returns release their references. A return into
a preexisting destination releases its history without removing that folder.
The source reference index changes in the same transactions as its history rows.

Skip covers the entire recorded directory traversal for the current attempt.
Branches blocked by skipped work are deferred for that attempt, so the ready
marker pass cannot repeatedly reconsider them. The next Undo attempt starts
with fresh deferrals. Changes to either reviewed folder invalidate Merge and
Keep both before preparation proceeds. Cancellation preserves pending children.
Completing an owned frame counts both the marker and the consumed source-folder
cleanup step in the public Undo outcome.
Undo keeps the recorded reverse execution order. A child conflict can therefore
appear before other branches return; Skip still lets independent branches finish.

## Evidence and limits

Public OperationEngine tests use actual private files on two different
filesystems. Eight parents cover root Merge and Keep both, older file and
directory Copy ownership, owned recreation, forward Merge into preexisting
directories, nested and empty folders, a symbolic link, mode preservation,
shared destination Skip/reopen/retry, cancellation and stale original-folder
review. A child process is killed after observing the owned return folder while
source children remain. Reopening returns 128 recorded children without
duplicates and preserves older Copy Undo. This proves the observed interruption
boundary rather than a private journal phase.

Five upgrade parents include schema 12 and authentic older member constraints;
their results are observed through history, Undo and actual files. An older
directory move without the new association retains its old record format. A
newer original-location conflict in such an entry still needs a deliberate
legacy recovery policy; unrelated history rows must not be guessed into a frame.

Validation passes 248 core tests with 18 parent-invoked helpers, all-target
Clippy, desktop build, formatting, Python syntax, acceptance inventory and
whitespace checks. Native runs 78 (dark) and 79 (light at 1.25 scale) each pass
21 checks using device IDs 35/2050, including private destination cleanup.
All 16 final captures were inspected. An apparent ancestor-heading discrepancy
was subsequently withdrawn: Quick Look covered the labels that had been misread.
Three dedicated native runs show correct visible headings and exact agreement
with a fresh-window reference after navigation and preview closure. No rendering
change was needed; `column-heading-verification.json` records that diagnosis.
The application logs have no panic, GTK critical or
GTK warning markers. Run 77 stopped because the fixture expected siblings to
return before the first child question; moving its check after Skip required
no application change. Exact evidence is in `directory-move-verification.json`.

Also finish unavailable and replaced incoming roots, source/destination movement,
unstarted preparation races and rollback proof, additional physical cleanup
interruption points, and dedicated migration of private Undo/Recovery containers.
This decision does not claim complete directory Merge, Undo or product delivery.
