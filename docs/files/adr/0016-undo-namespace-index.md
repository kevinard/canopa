# Index affected Undo and Recovery namespaces

Status: accepted under delegated implementation authority, 2026-10-16.

Keeping both during Undo relocates older history to the restored item's new
name. Scanning and decoding every older step for every file made a 500-file
previous-destination reversal take 48.5 seconds after restart. The public crash
fixture now enforces a generous 20-second bound on reopening and completing both
reversals; it failed against the original implementation.

History schema version 2 adds derived namespace indexes for changes, archived
Recovery records and identity bindings. Each relevant path contributes its
component-normalized spelling and ancestors. A relocation reads only owners in
its namespace, in ordered pages of at most 256 records, then rewrites their
bodies and index entries in the same transaction as history completion. Foreign
keys remove derived entries when history, archives or bindings are deleted.
Bindings continue to retain their original path bytes for identity resolution.

Keys preserve Unix filename bytes. Normalization uses `Path::components`, which
matches the equality and prefix rules used by relocation: repeated separators
and interior `.` are equivalent, `..` remains a component, and similarly named
siblings remain distinct. Indexing never canonicalizes a path against the live
filesystem or follows a symlink. The same path visitor supplies indexing and
relocation, including retained stages, Trash information and source quarantine.

Opening a v1 database builds all three indexes and advances its version in one
transaction. Existing change bodies, pending intents, archive formats and
bindings retain their formats. Legacy JSON import also indexes its imported
changes transactionally. A database from a newer unsupported schema is refused.
There is no version downgrade path; older binaries reject schema version 2.

The unchanged 500-file killed-process fixture now completes in about 3.5 seconds
including setup, rather than about 50 seconds. A separate supported-v1 fixture
is opened through the public engine and verifies actual Keep both, restart,
older Undo through an inode binding, similarly named siblings, raw filenames,
equivalent path spellings, and archived Recovery preview data/copy-out. The
fixture constructs historical input; assertions do not query the database.
All 82 core tests, the desktop build and all-target Clippy pass. The 500-file
fixture also passes during the full core suite in 3.9 seconds.
`native-undo-15/summary.json` passes all 16 native Undo checks against the rebuilt
application. `native-acceptance-23/summary.json` passes the full existing native
workflow in light appearance at 1.25×, including narrow navigation, previews,
clipboard, background operations, notifications and Trash. Rendered inspection
confirms newer copy edits remain previewable in Recovery. Neither application
log contains GTK warnings.

This bounds relocation work by affected history rather than the whole store.
Large active Recovery catalogs still resolve retained locations through later
namespace operations; that separate traversal, unresolved pending inspection,
and remaining Undo behaviors stay open in the implementation ledger.
