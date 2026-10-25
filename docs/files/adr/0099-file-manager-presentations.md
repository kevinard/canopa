# 0099: FileManager1 presentation and read-only Properties

Status: scoped core and native verification, 2026-10-20. Root ADR0025 records bus
ownership, cold activation, packaging and the remaining platform/release gates.

ShowFolders opens one Browser Window with a Tab per distinct requested folder,
matching ordinary multiple-location CLI launch. ShowItems groups requested items
by their parent folder, keeping first-requested folder order and selecting all
requested items in each Tab. A requested folder is selected in its parent rather
than entered. Revealing the filesystem root opens the root. Duplicate URIs are
coalesced; empty requests do nothing. Invalid/nonlocal URI batches are refused
as a whole. Requests are bounded to 64 input locations and the existing admission
budget; no filesystem authority comes from a URI or StartupId.

Explicit hidden selections become visible in that folder session without showing
other hidden siblings or changing global preferences. Selection and focus use the
shared Browser model at every width. Refresh preserves the context; deliberately
hiding files again clears it. An inactive Tab retains its initial selection until
its first scan. Later user navigation supersedes that initial request, and stale
scan results keep their existing revision guard.

ShowItemProperties opens an independent read-only Properties window for the
captured paths, including when no Browser exists. It can remain open after
Browsers close. The shared core prepares metadata off the GUI thread, inspects
symlinks as links, and keeps individual failures alongside successful items.
Show file byte size, type, modification time, numeric owner/group and Unix mode;
do not present a folder's metadata length as its total contents. This is a
snapshot when opened. Properties never reads file contents, recursively counts
folders, follows links for preview or edits permissions.

The window uses shared Canopa controls and appearance, wraps paths in narrow
tiles and scrolls its bounded item list. Home/End and Page Up/Down navigate the
metadata. Copy locations and Ctrl+C copy escaped local URIs, preserving names
that cannot be represented as UTF-8. Escape/Ctrl+W close Properties. Closing it
cancels inspection and releases its presentation lifetime without cancelling
accepted operations. Quick Look retains its separate source-following behavior.
