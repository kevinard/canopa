# 0085: Iced Trash catalog and window

Status: scoped Main Trash workflow verified, 2026-10-19. Recovery and full product
acceptance remain incomplete. Continues ADR0008/0059/0084.

Trash discovery, scope, sorting, warnings and filesystem observation belong to
a renderer-independent catalog. Its reader publishes prepared snapshots from a
backend thread, coalesces refreshes and rejects superseded reads. It must be
usable without Iced. Dropping a view stops its reader without cancelling any
accepted filesystem operation.

Install watches before reading. Observe existing stores and the writable volume
roots or existing shared Trash directories where a new user store can appear.
The mounted-volume owner triggers rediscovery on mount changes. Main Trash does
not discover or watch other mounted stores. Failed watches remain visible as
warnings with manual Refresh available.

The Iced auxiliary window owns selection, scroll position and a Restore-to
destination draft. All locations and Main Trash use the same catalog. Switching
scope clears rows and selection before loading. Refresh preserves the selected
raw stored path when it still exists. Restore captures the actual TrashItem;
it does not rebuild metadata or a target from a display label. A chosen folder
is joined with the original raw basename. Occupied destinations remain ordinary
engine refusals. No implicit overwrite or permanent deletion is added.
The destination dialog uses an editable local folder path or file URI, including
byte-safe escaped URIs. It retains the draft if durable admission is refused.

Use body controls that wrap in narrow tiles. Provide Restore/Ctrl+R, Restore to
another folder/Ctrl+Shift+R, Copy/Ctrl+C, Quick Look/Space, Refresh/F5, shared
Undo/Ctrl+Z and Operations/Ctrl+J. The browser's Delete command submits Trash to
the existing operation queue. Trash is accessible from Places and the overflow
menu independently of a browser's file selection.

Quick Look follows the Trash selection independently of browser selections.
It reads the stored raw path and uses the original name only as a classification
hint and title. Closing the Trash window closes its preview. Invalid metadata
leaves the payload visible and copyable, with Restore disabled.
The catalog supplies an opaque payload version containing identity and prepared
metadata. A changed version invalidates the prepared preview. Replacing a file
while preserving its size and modification date must still invalidate it;
unrelated refreshes preserve the selected preview's page and zoom.

Verify through the established public Trash/catalog/operation interfaces and
the private native desktop harness. Native restore explicitly selects Main
Trash, whose data home and files belong to the fixture. This decision does not
complete Recovery, batch/conflict restoration, cross-filesystem Restore,
execution/restart authority or the full product brief.

## Scoped verification

The renderer-independent catalog tests preserve raw names and orphan payloads,
avoid creating storage during reads, and distinguish a replacement whose size
and modification date are unchanged. The headless reader observes store creation,
metadata removal/repair and actual Restore. A private child supplies a synthetic
mount inventory while using real files, GIO monitors and the existing engine;
it reproduces a new volume store being missed, then verifies automatic discovery
without manual Refresh. No actual mount or user Trash is changed.

Final checks pass 67 focused core parents, three catalog parents with the
local-runtime feature and both display variables removed, and 47 unfiltered Iced
library tests. Private native acceptance passes six Trash checks in each theme,
12 existing operation checks and ten clipboard checks on binary `6565ad76`.
Delete verifies stored bytes, the decoded original raw path and actual Undo.
Preview checks copy the rendered text before and after a payload edit. Restore
checks original/chosen locations, an exact occupied-destination refusal,
raw basenames and a raw folder URI, shared Undo and orphan Copy/Paste. External
GIO changes populate a long list; End selects its last row in a narrow tile and
opens its own floating preview. Representative captures were inspected.

The first occupied-refusal oracle accepted any newer notification; it now
requires this Restore's title and existence error, with both payloads preserved.
Focus diagnostics also showed a title change preceding native dialog retirement.
The fixture waits for the actual window to disappear and Trash to regain focus.
The adapter requests owner focus before closing the dialog, only while that
dialog still has focus. Temporary diagnostic instrumentation was removed.

All-target Clippy passes with the three existing library warning groups;
formatting, C fixture, Python syntax and native matrix checks pass. Both scoped
review axes have no unresolved findings. A clipboard attempt timed out before
its first Browser appeared; an unchanged fresh attempt passes the workflow but
does not fix the independent startup timeout. Full workspace, release,
performance and product acceptance remain open. Exact attempts and source/binary
identities are in [the verification record](../../../.scratch/file-manager/trash-ui-verification.json).
