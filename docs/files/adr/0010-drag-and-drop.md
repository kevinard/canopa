# File drag and drop

Status: accepted under delegated implementation authority, 2026-10-16.

Files exports native GDK file lists and `text/uri-list`, preserving Unix path
bytes through GIO URI encoding. Local file URI input follows the clipboard's
admission rules; remote locations are outside the initial product scope.
Incoming lists are bounded to 16 MiB and a five-second read/check budget.
Filesystem inspection runs off the UI thread and stale hover work is cancelled.

A drop on the same mounted filesystem defaults to Move; other drops default
to Copy. Mount identifiers distinguish bind mounts as well as different devices.
Symlinks belong to the filesystem containing the link, rather than its target.
A selection spanning filesystems has one Copy default for the complete batch.
Ctrl forces Copy and Shift forces Move; Ctrl takes precedence if both are held.
The negotiated action must be supported by the source.

On the verified GTK/Niri stack, drop-event and keyboard-device modifier state
can be empty even when the source press carries Ctrl or Shift. Local drags
therefore capture the source press and follow native modifier notifications.
They use the source's declared supported actions rather than mistaking the
currently negotiated action for its complete capabilities. Hover feedback follows
this state without requiring another pointer movement. External offers retain
their negotiated capability checks.

Current/ancestor column backgrounds, folder rows and sidebar locations can be
destinations. A highlighted destination and persistent pre-drop hint identify
the action, count and target. The hint overlays the body so it does not move
file rows under the pointer. A folder row's feedback takes precedence over its
containing column. Checking an unavailable or slow target does not block the
browser. Selection is captured before the native view's press handling so
dragging one member of a multiple selection retains the complete selection.

Accepted drops use the same transfer queue, conflict dialogs, cancellation,
notifications and shared Undo as clipboard transfers. A queued/running drop has
an application hold. The destination finishes the native drag with its action
only after the whole transfer succeeds; cancelled, skipped or failed portions
finish without a success action. Successful filesystem changes remain recorded
independently of the protocol result.

Escape marks a local drag cancelled before ending its source controller. Native
testing reproduced a later drop event after that source-side end; the destination
rejects the cancelled gesture and explicitly finishes the offer with no action.
This prevents a release after Escape from starting a transfer. Every drop event
claimed by this adapter is finished, including rejected/stale destinations, so
the native lifecycle cannot remain in its dropping state.

The receiving file manager owns file-URI mutations. Files never deletes sources
again in response to the generic source-side `delete_data` flag: the destination
may already have moved them, and a replacement may now occupy the old path.
This follows the file-reference ownership used by
[Thunar's source handler](https://github.com/xfce-mirror/thunar/blob/master/thunar/thunar-standard-view.c).
Native integration uses GTK's asynchronous drop lifecycle and status negotiation:
[DropTargetAsync](https://docs.gtk.org/gtk4/class.DropTargetAsync.html),
[Drop status](https://docs.gtk.org/gdk4/method.Drop.status.html) and
[Drop finish](https://docs.gtk.org/gdk4/method.Drop.finish.html).

Three public transfer tests verify default and forced actions, actual transfers,
Undo, cancellation, symlink classification on distinct real filesystems, and
isolation of a missing source item. An unknown source filesystem uses a Copy
default and remains in the request so the normal per-item failure policy applies.

Native runs `native-drops-15` (dark/1×) and `native-drops-16` (light/1.25×) pass
tiled-window transfers, column/list/icon sources, folder targets, initial Ctrl/Shift
overrides, Escape cancellation, foreground filename conflicts with Keep both and
Undo, actual cross-filesystem Copy/Move, multiple selection with raw Unix names,
and inbound/outbound transfers with an independent GTK file-URI application.
The fixture checks that nothing changes before release and waits for committed
completion before requesting Undo. Temporary debug instrumentation is removed.

Modifier changes during a held gesture, additional external source conventions,
and direct background DND lifetime acceptance remain follow-up checks. Mounted-volume
unplug/race acceptance remains part of the larger operation audit. The complete
file-manager implementation is still in progress.
