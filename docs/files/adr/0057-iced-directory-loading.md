# Iced directory loading and bounded row construction

Status: accepted under delegated implementation authority, 2026-10-18.
Implemented and verified for the Iced browsing adapter. The full GUI migration
and product acceptance remain in progress. Evidence is in
[the loading verification](../../../.scratch/file-manager/iced-loading-verification.json).

Deliver the requested folder independently of its ancestors. After installing
that snapshot, load only ancestor columns that the current window actually
displays, after the compositor confirms the actual size. The requested startup
size does not authorize ancestor reads. Each ancestor has its own worker,
cancellation token and request ID.
Resizing away a column or hiding its tab cancels its work and releases its snapshot; widening
requests a fresh snapshot. A late result cannot revive a hidden column, replace
a newer request for that path, or alter current selection and scroll state.

Navigation cancels the preceding folder scan. Closing a tab or browser cancels
its outstanding reads. Directory preparation checks cancellation between entries
and publishes either a complete snapshot or an Interrupted error. Search uses
the same cancellable reader so an abandoned search can stop inside a large
directory. Filesystem calls themselves remain subject to the underlying local
filesystem's response time.

Prepare metadata snapshots on workers, then construct widgets only for visible
rows and a small surrounding buffer. Empty space above and below retains the
full scroll extent. Row height derives from shared typography and MenuRow padding,
so scroll positions, keyboard reveal and configurable spacing use one geometry.
Ancestor scroll offsets are independent of the current Browser's folder state.
Visible row state is keyed by tab and original Unix path, so scrolling or a new
snapshot cannot reuse a different file's double-click state. A small transparent
row wrapper rebuilds its child state on identity changes and uses Tree::diff to
respect widget tags. The installed Iced keyed-column reconciliation could retain
stateless spacer state for a MouseArea when the visible range grew; two native
runs reproduced the resulting panic, and a deterministic widget-tree regression
reproduced the incompatible tags without workers or a compositor. The wrapper
fixes that seam without a shared dependency fork.

Verify the distinction against real native Iced windows: an inotify watch on a
private parent containing thousands of siblings sees no directory open when its
column is hidden, then sees an open when widening displays it. Retain ordinary
keyboard, raw-path, tab, resize/scroll and two-window native acceptance. The full
product and Iced migration gates remain open, including filename search UI.
