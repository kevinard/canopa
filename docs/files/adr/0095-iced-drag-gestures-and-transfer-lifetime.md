# 0095: Iced drag gestures and transfer lifetime

Status: scoped native workflow verified, 2026-10-20. Extends ADR0010/0074/0076/0094
and root ADR0023. GTK exchange, clipboard coexistence and windowless receipts
have scoped native verification, as do cross-filesystem choices and additional
source/target presentations and mounted-device drops. Broader interop and the full
product brief remain in progress. These stages received primary review only.

## Ownership and interaction

The Iced adapter captures the selected group before a row's ordinary click can
collapse it. An unselected current-folder/search row applies its normal selection
mode once; a selected row defers its short-click behavior until release. An
ancestor row can be dragged without navigating first. A six-logical-pixel motion
starts a drag only after backend source preparation and the original native
press capability are available. Late preparation cannot borrow a newer click.
Window, Tab, listing revision and editing context invalidate an unstarted gesture.
Preparation errors do not break an ordinary short click.

OperationJobs still owns source observation, storage, retention, action choice,
single-use admission and filesystem mutation. Native source bytes do not replace
the clipboard. Automatic action follows the backend's placement policy; Ctrl
forces Copy and Shift forces Move only when supported. A receiver advertises no
accepted action while metadata is pending or a destination is refused. At Drop
it freezes the already prepared destination and the final negotiated native
action. It does not resolve a new destination from a late reply.

Actual row, column and sidebar geometry defines the targets. Folder rows take
priority over their containing column; headers/backgrounds refer to that column.
Mounted device targets use the current backend token/root and cover only Open,
not removal buttons. Hover never mounts a device. Listing, icon and search rows
share the source marker. Feedback uses a border and the existing status line,
so it does not shift the rows beneath the pointer.

Geometry collection traverses Iced's actual scrollable state, accumulating nested
translations and viewport clips. Draw-time child layouts alone are insufficient:
Scrollable translates the renderer, not those layouts. Collection runs after
scroll updates and during fresh root layout. Iced can rebuild and draw a new view
without sending that new view RedrawRequested; keeping only the earlier event's
regions can leave an old listing context after Undo. The fractional-scale native
run exposed a refused source; layout-time collection closes this refresh gap.

## Incoming references and accepted work

Same-client offers share the PreparedDrag identified by the native source on the
actual client/seat. An unknown local source is refused. Foreign URI lists are
read asynchronously under the native byte/time limits, decoded without losing
Unix path bytes, and prepared through OperationJobs. A foreign drop reads again
after Drop and requires the same decoded local paths before submitting. Empty,
changed or failed final reads refuse admission; they cannot refresh the original
source observation or destination proof. Actual GTK/Files acceptance now verifies
this safeguard, including a cancelled source with prefetched hover data.

An empty native read ends the gesture without a persistent Operations error. A
withdrawn source can return an empty pipe even when a residual Drop is delivered;
no transfer was admitted in that case. The first external workflow was safe but
left a misleading clipboard-empty error in Operations. A rendered before/after
regression reproduced that row. The adapter now represents the empty result as
an ended gesture; other malformed data and I/O failures remain explicit.

Admitted drops use the existing serial queue, conflicts, attention, result saving
and shared Undo. The native receipt waits for the actual terminal backend report;
failed, cancelled, skipped or unfinished work does not report native success.
Source completion never performs another deletion. Accepted receipts and their
application holds remain owned beyond browser closure. Actual foreign receipts
now have zero-window evidence: an incoming conflict remains unanswered, the same
owner reopens and completes it on explicit Replace, and Undo restores both
versions. An outgoing Move waits for GTK completion after every Files window
closes, then releases the ordinary owner. A closed window never answers an
operation question.

## Escape ordering

A stopped-process test queued Escape before physical button release. The earlier
application-only handler saw the native Drop already latched and admitted a real
Move. Its private filesystem snapshot and wire log preserve that failure.

The shared Wayland adapter now handles the converted logical Escape during native
keyboard dispatch, before a later Drop can overtake the queued Iced key message.
It revokes that seat's held press and ends only its undropped sources. Already
dropped receipts and admitted backend work keep their lifetime. The Files adapter
invalidates the original PreparedDrag when its undropped source ends. An unrelated
logical F12, mapped by the fixture to the same synthetic native keycode as Escape,
still permits the Move. The rule therefore follows keyboard mapping rather than
assuming a physical evdev code.

This prevents the verified internal Files mutation after Escape. It does not
resolve the separate strict transport gate: the independent peer can still
observe a native Drop after acknowledged source destruction. Foreign hover bytes
alone remain insufficient authority. Selected clipboard coexistence workflows
now pass; broader retained-offer interaction remains under root ADR0023.

## Verification and remaining work

Eighteen real Files checks pass: nine in dark/100% and nine in light/125%. They
cover pre-release non-mutation, same-filesystem Move and shared Undo from another
Browser, live Ctrl-Copy and Undo, folder-row priority, the original selected group
with raw filename bytes, scrolled source/destination columns, ordinary Escape,
Escape buffered while the private process is stopped, and 360-pixel column
browsers. The first Move also survives the unrelated logical key. Light scrolled
feedback and dark narrow-window captures were inspected.

All-target Files Clippy with warnings denied, the default build and all-target
Shell/Notify/layer-adapter compilation pass. Exact source and executable archives,
failed attempts and native results are recorded in
[the evidence](../../../.scratch/file-manager/dnd-controls-verification.json).
No implementation or review was delegated during this stage.

A further sixteen checks pass on the quiet-cancellation candidate, eight per
appearance. They cover exact raw-byte GTK-to-Files Move, the same offer read before
and after Drop, shared Undo, exact GTK source destroy acknowledgement and refusal
of a cancelled foreign drop, and unchanged rendered Operations facts after
cancellation. They also cover Iced text paste after those drags, retention of an
owned file clipboard while dragging another row to GTK Copy, a foreign conflict
through zero-window deferral/reactivation/Replace/Undo, and outgoing GTK Move
completion with zero Files windows and orderly owner exit. Clipboard acquisition
readiness and expected ownership-loss exits are handled explicitly by the fixture.
Per-check native/notification/namespace snapshots and final private-file inventories
are retained. The resumed conflict, zero-window source and quiet Operations
captures were inspected. All-target Files Clippy and the default build pass;
there is no shared dependency change in this external stage. See
[the external evidence](../../../.scratch/file-manager/dnd-exchange-verification.json).

Twelve further native gesture checks pass, six per appearance, on the search
shortcut candidate: actual different-filesystem Copy by default, Shift-forced
cross-filesystem Move, icon source to the current column header, a recursive
search result's actual nested source, a live-configured sidebar bookmark without
navigation, and an ancestor-column folder source while its open view becomes
unavailable. Each uses the same transfer queue and shared Undo. Actual filesystem
device IDs differ; hover preserves the private namespace and notification count.
Both cases also verify secondary-directory cleanup after the private session
shuts down. That adds two cleanup checks, not two more gestures.

The focused query initially swallowed Alt+S as text. The presentation-only fix
and broader search regression are recorded in ADR0096. Earlier fixture failures
are retained: a parent-column header is a valid destination, a 512px tile hides
the sidebar below its documented threshold, a narrow source wraps its toolbar,
and Niri's center-visible-columns centers only already fully visible columns.
The fixture now allocates enough target width, locates the selected source's
painted band, and reveals both fitting tiles before calculating coordinates.
It keeps the actual native-start/action, exact destination and no-hover-mutation
requirements. Light ancestor feedback and the bookmark captures were inspected.
All-target Clippy and 77 library tests pass. The aggregate combines dark Native13
and light Native15; Native13's overall failure is not recorded as a pass. See
[the choices evidence](../../../.scratch/file-manager/dnd-choices-verification.json).

A later held-folder test exposed an actual authority refresh bug: cancelling and
re-preparing a hover after the view refreshed captured a replacement destination.
The backend now preserves the original successful observation for that pathname
throughout its PreparedDrag and checks it on re-preparation. Ten native checks,
five per appearance, verify refusal of destination/source replacements, ordinary
contents refresh and fresh reviewed Move/Undo. The earlier dark controls and
fractional light choices also pass on that candidate. See
[ADR0097](0097-drag-destination-observations-survive-refresh.md) and
[the evidence](../../../.scratch/file-manager/dnd-stale-verification.json).
This is a backend correction; the GUI and shared transport are unchanged.

## Mounted-device targets

The existing device Open row accepts a drop only through the backend's current
token/root lookup. The separate removal control is not a drop target. Hover or
release on an unmounted row does not mount the device; explicit Open remains the
mount command. An open mounted folder remains an ordinary destination in a
narrow tile where the sidebar is hidden. All successful drops use the same
OperationJobs admission, conflicts, saved result and shared Undo.

Eight new native checks pass per appearance (dark/100% and light/125%) on the
unchanged Downloads focus candidate. They cover same-filesystem default Move,
Ctrl Copy, shared Undo and original drive contents; removal-control rejection;
disconnect/replacement during a held gesture at the same root pathname and
directory inode; a fresh reviewed gesture on that replacement; no implicit
mounting on an unmounted-row drop; explicit GIO remount and a subsequent transfer;
and the mounted root in a 340px tile. The target browser stays at its own local
folder during device-row drops. Hover changes neither files nor notifications,
and refused drops create no operation scheduling row.

The private native GIO provider supplies real asynchronous mount/catalog signals
and real temporary files; these checks do not exercise a physical removable
device or emulate a second filesystem. Device replacement changes the provider's
storage UUID while retaining the directory inode. The existing light device UI
workflow passes nine further checks, including pending questions through zero
windows. Total scoped coverage is twenty-five checks. Representative replacement
refusal and narrow-root captures were inspected. No application, Cargo, shared
dependency or provider implementation changed. Primary review only.

The first fixture attempt passed Move/Undo then assumed the returned source was
still selected. The corrected fixture uses ordinary Home input to select that
one source before each fresh gesture. This is a fixture correction, not an
application defect. See [the device-drop evidence](../../../.scratch/file-manager/dnd-devices-verification.json).

Additional delayed/stale geometry, full retained-offer coexistence, the strict
native cancellation gate, physical devices, accessibility and performance remain
open. The complete Downloads design-test workflow is now verified under ADR0098.
Original namespace/journal and restart execution authority, general Retry/Continue,
remaining renderer extraction and full workspace/native/release acceptance
remain part of the original goal.
