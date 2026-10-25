# 0050: Copy saved directories through the shared transfer engine

Status: accepted under delegated authority; verified for the scope below.
Broader Recovery and the entire file-manager product brief remain in progress.

Recovery can encounter a directory where an older operation expected a file. The
saved bytes remain protected, including after the owning history entry expires.
The user should be able to copy this directory to a chosen new location without
consuming its source or displacing work created at the output location.

Recovery now uses the existing recursive transfer executor. Its opaque
`RecoveryCopyContinuation` carries the selected catalog record and observed
version, exact remaining targets, captured directory identities and the original
destination-parent identity. It cannot be passed to ordinary transfer resume.
Both initial and continued requests re-fetch the selected record and check its
saved container and observed version. A committed identified container rename
can relocate source paths without changing the selected physical version.

The chosen root must be new. An occupied root, including a racing arrival, does
not offer Merge or Replace. A continuation may use its own captured output
folder only while that folder and its original destination parent still match.
Child conflicts offer Keep both, Skip and, for folders, Merge. Replace is refused
by the engine and omitted from Recovery's native questions. A different folder
at the same path never becomes the old request's destination automatically.

Progress, byte counts, cancellation, per-child failures, Retry failed items,
Continue remaining items and Undo use the shared Operations interface. Partial
success is actual completed work; continuing creates a separate chronological
Undo entry. Unsupported children fail without blocking valid siblings. Source
names in displayed progress and conflict metadata use original-location hints;
those display paths never enter a continuation or filesystem check. Placement
preflight resolves the selected saved record again rather than trusting a stale
catalog row's old path.

Recovery copies retain their independent journal lane and do not replay the
original interrupted operation as a copy-out side effect. Destination checks
exclude the source's saved container and every original journal-owned path,
including resolved aliases. Schema 17 permits created-folder forward intents in
the Recovery lane; schema 16 readers must refuse this newer journal format.
Startup reconciles incomplete child-copy bytes while retaining completed prefix
files and their Undo history. It does not promise persisted exact continuation
of an unfinished directory job after process exit.

The selected root must still have the observed version. Changing that root
requires Refresh and a newly chosen output, while an existing partial output
remains untouched and undoable. This is not an atomic snapshot of every child.
Encountered files use the existing staged-copy fingerprint checks and directory
identity checks; full pinned-directory and original-media authority remains
separate work. Explicit protected-data disposal, unsupported special-file
copy-out, durable jobs, and broader cleanup interruption/orphan audits also stay
open. The narrow window keeps the same navigation and floating preview model.

Public tests use real Copy-replace backups and temporary files. They cover raw
names and link bytes, history expiry/reopen, cancellation and exact continuation,
newer output work, replaced output/parent identities, source observation changes,
identified ancestor rename, unsupported children and root collisions. Additional
process tests terminate a directory Recovery copy at a public byte-progress
boundary, check unavailable-output startup, reconcile incomplete bytes and Undo
the completed prefix. Another real interrupted Undo leaves its original journal
unresolved while its saved directory is copied through the separate lane.

Native 105 dark and 106 light at 1.25 scale each pass 13 checks on the same
executable. All 16 captures were inspected, including Keep both as the conflict
default, the absence of Replace, floating text Quick Look above a 360-pixel
browser, and a nonempty-folder Undo that preserves newer work. Both GTK logs are
clean. The existing directory Cut/Retry regression (native 107) passes 20
functional checks plus private-volume cleanup on actual device IDs 35/2050. Its
11 captures were inspected, including newer Copy/Cut/external clipboard ownership
and floating preview beside a narrow browser. This totals 47 native checks.

The final source passes 300 core tests with 20 parent-invoked helper definitions,
Clippy with warnings denied, the desktop build, formatting, Python syntax and the
maintained inventory. All 100 Rust/build source hashes remain unchanged through
these checks. Packaged documentation retains 24 files with 39 resolving offline
links. Exact evidence is in `.scratch/file-manager/recovery-copyout-verification.json`.

[ADR 0051](0051-recovery-continuation-ownership.md) adds identified selection
resolution and unfinished-task retention after reproducing the history-expiry
and output-root refusal failures. The root-install/read-error interruption
boundary remains unaudited. These
limits do not turn this scoped verification into completion of all Recovery or
the file-manager product brief; remaining observations are maintained in
`.scratch/file-manager/DIRECTORY-RECOVERY-WIP.md`.
