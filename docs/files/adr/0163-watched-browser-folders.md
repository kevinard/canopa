# 0163: Displayed folders follow outside changes

Status: implemented with model-level verification, 2026-11-11. Native acceptance
of the Iced adapter is still pending. Restores the directory-monitor behavior
ADR0004 required of the GTK adapter, which the Iced port (ADR0057) had lost.

Every folder a Browser Window displays is observed: each tab's current folder
(including one still loading) and the active tab's ancestor columns. Files has
no tree view, so there are no expanded nodes to add. The renderer-independent
`folder_watch::FolderWatch` owns one inotify instance from the shared
`canopa_file::watch` watcher on its own thread. The frontend replaces the
observed set after each update; folders that are no longer displayed, including
those of closed tabs and windows, stop being watched and lose pending changes.

Events coalesce on the worker. A folder is published after a 200 ms quiet period,
or after at most one second while a burst continues, so a long copy into the
folder still refreshes periodically. The watcher names the affected targets, so
only the changed folders are re-read. One rescan runs per tab; further bursts
during it schedule a single follow-up.

A rescan is not a navigation. It keeps the tab's navigation revision and list
epoch, and installs the new snapshot through `Browser::refresh`. Selection,
focus, the range anchor and the scroll offset survive; only vanished entries are
pruned. Name dialogs, prepared Renames, drags, context menus and clipboard
origins captured before the change therefore stay current unless their own
entries vanished. Ancestor columns get a new request in place, keeping their old
listing and scroll position until the new snapshot arrives. A hidden tab marks
itself stale and rescans when it is shown. A change to a folder that is still
loading is replayed once its first snapshot arrives. A full scan (navigation,
F5, or the refresh after one of Files' own operations) supersedes outstanding
rescans. An open filename search keeps its results; the folder underneath it is
refreshed.

Watching never touches accepted operations. The operations engine continues to
refresh affected views when its own work completes (ADR0065). Its intermediate
changes, such as hidden `.canopa-files-stage-*` work folders, can appear in a
watched folder only while hidden files are shown, as they would after F5.

Degradation keeps F5 working. Without an inotify instance Files logs once and
keeps manual Refresh. When the kernel's per-user watch budget is exhausted, or
more than 256 folders are displayed, the remaining folders are not watched,
Files logs once on standard error, and Refresh still reads them. A rescan that
fails, for example because the folder was removed, shows its error until a later
rescan succeeds.

Verification: watcher unit tests in `canopa-file`; public integration cases in
`tests/folder_watch.rs` for per-folder attribution, burst coalescing, the
maximum delay, unwatching, recreated folders, the folder budget and Browser
state preservation; Desktop model tests in `iced_desktop/folder_watch_tests.rs`
for rescans, origins, hidden tabs, loading folders, columns and errors. The
installation between a navigation and its first read is asynchronous, so a
change in that small window is caught only by the next event or F5.
