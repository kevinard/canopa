# 0164: Permanent deletion and Empty Trash

Status: implemented with engine, queue and Desktop model verification,
2026-11-11. Native acceptance of the Iced dialogs is still pending. Continues
ADR0008, ADR0084 and ADR0085, which deliberately had no permanent deletion.

Files offers three explicit, confirmed ways to remove data permanently:

- **Delete permanently** for a Browser selection: Shift+Delete (the configurable
  `delete-permanently` command) and the context menu.
- **Delete permanently…** for the selected Trash item: its button, Delete or
  Shift+Delete in the Trash window.
- **Empty Trash…** in the Trash window, for every item the current scope lists
  (All locations or Main Trash).

Each opens a confirmation that names what it removes: the file's name for one
item, otherwise the count, and states that it cannot be undone. Cancel is the
default focus and Escape cancels. One confirmation is open at a time, and it
closes with the window that asked for it. Confirming submits the exact captured
selection or Trash items; Empty Trash never includes items trashed after the
dialog opened.

Removal always goes through the existing serial operation queue as
`Request::Delete` or `Request::DeleteTrash`, never through ad-hoc calls from the
renderer. It is cancellable while queued or running and publishes progress
(entries and bytes). Its durable result is a `DeleteReport`: completed, failed and
unstarted items. Reopening never replays it. It adds nothing to Undo and keeps no
journal: an interruption leaves the remaining entries in place, and a stopped
folder is reported as possibly partially deleted. It waits behind interrupted
journals like every other mutation.

The engine removes entries relative to opened directory descriptors and never
follows symbolic links: a link is unlinked and its target is preserved. It does
not descend into another mounted filesystem, and it refuses trees deeper than 384
levels rather than exhausting descriptors. A browser selection must still be the
entry observed at admission, as for Trash; a replacement at the same path is
preserved. Items that contain data Recovery or an unfinished request still needs
are refused.

Trash deletion follows the freedesktop Trash specification for every
discovered store: `$XDG_DATA_HOME/Trash`, `$topdir/.Trash/$uid` and
`$topdir/.Trash-$uid`. The engine checks the volume and private storage. It
requires the stored payload's identity, removes the payload first and then its
`.trashinfo`, which readers ignore once it is orphaned. An orphaned payload
(invalid or missing information) is removed together with a leftover
information file. Changed information belongs to someone else and stays.
Matching `directorysizes` entries are dropped by an atomic rewrite.

ADR0008 still holds that Delete and Move to Trash never fall back to permanent
deletion on their own. Instead, a Trash result now lists the sources that failed
because their volume has no usable Trash: no store can be found or created, or
the store is on another filesystem than the source. When such a result returns
to its Browser, Files offers **Delete permanently** for exactly those still
existing items, with the same confirmation. That list is presentation data and
authorizes nothing; deletion is a new confirmed request.

Verification: `tests/deletion.rs` covers trees and links, cancellation, the queue
result and reopening, preserved replacements, home and per-volume stores,
orphans, size-cache cleanup, refused replaced payloads, Empty Trash over a
captured list, and storage-unavailable reporting through the queue and engine.
`iced_desktop/delete_tests.rs` covers the Shift+Delete confirmation, Escape,
confirmation-only execution, the Trash-unavailable offer and Empty Trash counts.
The unit tests never use the Trash window's reader, because it follows the
session's real data home. The retained-data refusal and the mount boundary have
no automated fixture yet.
