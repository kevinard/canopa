# Trash and restore

Status: accepted; implementation and remaining acceptance are tracked in the
[ledger](../../../.scratch/file-manager/IMPLEMENTATION.md).

Files uses the [freedesktop Trash storage format](https://specifications.freedesktop.org/trash/1.0/).
The main store is `$XDG_DATA_HOME/Trash`. Other mounted filesystems use their own
Trash storage: a validated sticky `.Trash` with a private user directory, or
private `.Trash-$uid` storage. Unsafe shared directories are not followed. Each
item has a reserved `.trashinfo` file before its payload moves, with a byte-safe
escaped original path and local deletion time. Volume paths are relative to the
mount root. Storage names are opaque and short, so long original names still fit.

Delete and the visible Move to Trash command initiate recoverable operations.
There is no implicit permanent-delete fallback. Explicit, confirmed permanent
deletion and Empty Trash were added later; see
[ADR0164](0164-permanent-deletion-and-empty-trash.md). Trashing is an atomic rename on
the source filesystem; if suitable storage is unavailable, the source stays put
and the operation reports failure. Mount identifiers from `statx` and
`/proc/self/mountinfo` distinguish mounts, including bind mounts that share a
device number. A volume location also checks its root identity before use.

A batch produces one shared Undo entry, including only actual completed changes.
Cancellation stops subsequent items; individual failures are reported and can be
retried in Operations. Trashed payloads remain in Trash when ordinary Undo history
expires: history expiry never empties Trash.

Trashing and restoring have distinct journal changes. Their durable intents cover
the payload move and associated metadata. Recovery reconciles completed moves
and preserves ambiguous work. Undo of Trash returns the current trashed contents
to the original location; Undo of Restore returns the item to Trash and recreates
its information. Restore validates the selected item and its volume inside the
operation lock, after pending recovery. Concurrent requests cannot each record
one already completed move as their own change. Metadata that differs from the selected snapshot blocks removal.
Restoring never overwrites an occupied destination. Missing original parents can
be handled by choosing another folder instead of silently recreating a hierarchy.

The Trash window is an auxiliary list view available from the sidebar, overflow
menu and the application's `trash` action. It shows original names, locations and
deletion dates, supports Restore/Ctrl+R and Restore to another folder/Ctrl+Shift+R,
and shares Ctrl+Z and Operations with ordinary browsers. Actions use a body
toolbar so the title bar does not impose an excessive minimum tile width.
Reads run off the GTK
thread; filesystem monitors refresh its contents. All locations includes existing
stores on mounted filesystems; Main Trash limits the view to the primary store.
Changing the scope clears the old selection and rows before loading, preventing
commands from applying to an entry left over from another scope.

Items without usable information remain visible with a recovery explanation;
their opaque storage name is not mistaken for the original name. Copy can recover
their stored data to an ordinary folder. Ordinary restore is disabled for those
rows. File content is never erased because information is missing or invalid.

Quick Look uses a source interface shared by Browser Windows and the Trash
window. Each source retains its own preview and keyboard navigation. Trash
previews use original names for presentation and a filename hint for format
classification; reads still use the actual stored path. This keeps media and
source-code detection useful without giving a display name control over I/O.
Model refreshes coalesce selection updates so a stable selected item does not
unnecessarily lose its preview while rows are replaced. Native folder choosers
use the app's process-local GTK dark/light preference; their accept button has a
keyboard mnemonic.

Native acceptance explicitly selects Main Trash before any restore. Its
`XDG_DATA_HOME`, payloads and metadata belong to the private fixture, so mounted
stores outside the fixture are never selected for mutation. Interoperability is
checked by restoring a private file trashed with GIO.

Remaining work before complete file-operation acceptance includes graphical
restore/Undo conflict choices, batch restoration, restore across filesystems,
unavailable-volume
acceptance, retained-data recovery, and the shared traversal/race audit. Restore
to another folder currently requires a rename-compatible destination filesystem.
The standard's recommended directory-size cache is not needed for the initial list:
opening Trash does not recursively scan payloads just to calculate their sizes.
