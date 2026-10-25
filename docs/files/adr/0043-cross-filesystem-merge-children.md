# 0043: Return Merge children across filesystems

Status: ordinary-child implementation verified; broader implementation in progress.
The full product goal remains active.

## Decisions

A Merge child whose destination is on another filesystem uses verified copying
before removing its source. Regular files and symbolic links reuse the existing
cross-filesystem Undo machinery: a source-return step creates the verified copy,
then a paired cleanup step removes only the owned, unchanged incoming version.
Both steps remain in the original operation and directory scope. Conversion
preserves the child's captured older-history bindings; returned copies acquire
new physical identities without losing older Undo ownership.

A disjoint subfolder needs a new owned destination. History schema 12 adds a
small destination-preparation journal. The journal, directory marker and child
snapshot rows commit in one transaction before the prepared folder is installed.
The snapshot uses individual history rows, so long filenames and larger folders
do not have to fit in one serialized journal record. Reopening can recognize the
installed folder and continue its recorded children. Only the owned empty source
folder is removed after its children finish. The destination keeps its existing
identity where a user explicitly merges into an existing folder.

Keep both for a colliding subfolder records its chosen destination and namespace
relocation with preparation. Older history follows that choice before the child
bindings are captured. The prepared destination uses its freshly captured
physical parent identity; an ordinary Rename step has no historical parent role
to resolve. File-return and cleanup member paths have separate source and
destination roles during later relocation.

If another item occupies a prepared destination before installation, it wins.
The preparation removes only its owned empty staging payload and rolls the
unstarted snapshot back to the original pending member for a fresh conflict
review. Unexpected staging contents are preserved instead of removed.

Private saved-data containers have physical ownership in Undo and Recovery.
They must not be copied as ordinary directories: doing so can leave their
catalog references pointing to vanished source containers. An interim guard
kept these containers at their owned source location. The dedicated migration
in [ADR0046](0046-saved-stage-cross-filesystem-returns.md) now moves identified
file/link backups and their references together; its broader scope is still in
progress.

## Current evidence and remaining work

Public filesystem tracers use actual private directories on two different
filesystems. Verified cases include files and raw-byte symlink targets, disjoint
nested and empty folders, preserving newer destination work, Skip/reopen/retry,
folder Keep both, older Copy Undo, and a long-filename snapshot exceeding the
old single-record budget. A process-kill test observes a newly installed folder
while source children remain, then verifies reconciliation and completion
without duplicate children. It proves that observed interruption boundary,
without asserting a private journal phase.

The Recovery invariant requires protected data to remain inspectable and
copyable whether the return finishes or is deferred. Its initial failure exposed
the private-container ownership problem described above. This invariant does
not prove that cross-filesystem container migration is implemented.

Validation passes240 core tests with17 parent-invoked child helpers, all-target
Clippy, desktop build, formatting, syntax, maintained inventory and whitespace.
Native runs75 (dark) and76 (light at1.25 scale) each pass17 checks using device
IDs35/2050, including private-destination cleanup. All12 final captures were
inspected; GTK logs are clean. Run74 stopped at a fixture preview-selection
mistake after the transfer assertions passed; correcting its Space keystrokes
required no application changes. Exact scope, source hashes and failure/green
logs are recorded in `merge-cross-verification.json`.

Also finish protected-container migration and active-backup visibility proof,
source/destination movement, stale and cancelled choices,
branch Finish and payload handling, physical-cleanup interruption checkpoints,
and normal cross-filesystem directory-move Undo ancestry. Do not infer complete
directory Merge or complete Undo from these tracers.
