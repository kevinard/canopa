# Browsing preferences and filename search

Status: accepted under delegated implementation authority, 2026-10-16.

Hidden files start hidden and can be toggled explicitly. Multi-selection uses
Ctrl to toggle and Shift to select a visible range. Folders sort before ordinary
files. Type-to-select is case-insensitive and distinct from filename search.
Directory refresh removes vanished selections while preserving scroll state;
an out-of-date snapshot for another folder cannot replace the current view.
Each Tab retains up to 256 history locations with per-folder browsing state.

Folder overrides contain view mode, sort field/direction, visible details, and
column width. The default is Columns, ascending Name, Size and Modified details,
and a 240-logical-pixel column width. Width customization is 160–640 logical
pixels. Use defaults removes the override, allowing later global changes to
apply again. Per-folder overrides are private application state, written
atomically with a 4 MiB read/write budget. Their JSON preserves original Unix
path bytes. Global defaults, custom actions and shortcuts belong to the common
Canopa configuration; implementing their editor must extend the shared envelope
instead of introducing a competing global configuration file.

Search matches filename substrings case-insensitively, includes matching folder
names, and respects hidden-file visibility. Current-folder and recursive scopes
share the same interface. Recursive search never follows directory symlinks and
deduplicates visited device/inode pairs to avoid mount cycles. Isolated read
failures are reported alongside results. Cancellation returns the results
already found with an explicit cancelled flag. A worker can publish progress
every 100 ms without moving filesystem work onto the UI thread.

The initial search result budget is 50,000 entries. Truncation is explicit;
the native interface must offer a way to continue with a larger budget rather
than implying that the result list is complete. Search state is separate from
regular browsing state, so leaving search restores its selection and scroll.
