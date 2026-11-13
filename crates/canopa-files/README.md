# Canopa Files

Canopa's native local file manager combines column navigation, floating Quick
Look, tiled browser windows, tabs and explicit custom actions. Implementation is
in progress; the [product brief](../../.scratch/file-manager/PRD.md) and
[implementation ledger](../../.scratch/file-manager/IMPLEMENTATION.md) describe
the complete scope and remaining work.

The GUI now uses the workspace's Iced stack and `canopa-ui`. The first migration
slice supports responsive columns, keyboard navigation, selection, editable
locations, hidden files, tabs, independent windows and filename search. Ctrl+F
starts a current-folder search; Include subfolders extends its scope. Results
show their containing locations, support Show in folder, and preserve the prior
folder's selection and scroll when closed. Stop keeps existing results; More
results extends the initial 50,000-match budget. Floating Quick Look now supports
read-only selectable text and source code with syntax colors, images with
zoom/Fit/100%, and PDF paging. It follows
its originating window's active tab and search selection. Space/Escape closes;
Tab enters its actions, and Space/Return activates the focused action. Default
preview keyboard input continues browsing in the source window at every width.
PDF previews need Poppler's `pdfinfo` and `pdftoppm`.

Audio/video previews now connect to a worker-owned GStreamer backend and start
paused. Inline Play/Pause, Seek, Mute and Volume remain available in narrow
previews. Tab enters controls; focused Seek and Volume use arrows and Home/End.
The backend owns transport state and reads the captured opened file. Closing a
preview or changing its source cancels playback; Files retains cleanup activity
until the decoder retires. Native media acceptance is still in progress.

Floating New Folder (Ctrl+Shift+N) and Rename (F2) dialogs now use Iced. Ctrl+Z
shares actual file Undo across windows; Operations includes filesystem work and
interactive Undo questions, with Cancel focused by default. Live GIO/GLib volume
observation runs in a private helper, without GTK widgets.

Transfers, Trash/Recovery, media playback and device controls are connected and
undergoing acceptance. The descriptions below include required migration targets;
the implementation ledger distinguishes verified Iced workflows from remaining
acceptance work.

Run from the Cargo workspace root:

```sh
cargo run -p canopa-files -- /path/to/folder
cargo test -p canopa-files --no-default-features
```

Launching Files opens a window on the existing runtime for its state directory,
sharing operations and Undo. For optional residency, start it from the graphical
session:

```sh
canopa-files --daemon &
canopa-files ~/Downloads
canopa-files --quit
```

`--daemon` runs in the foreground unless the shell or a supervisor backgrounds
it. It starts the backend without a browser and initializes Iced on the first
window request. `--quit` releases residency; Files exits after its windows and
accepted work finish, including pending decisions, clipboard ownership and
unsaved results. A start without display variables stays headless and refuses
browser requests until it is quit and started in the graphical session.
No resident service is installed automatically. Ordinary launches inherit the
running owner's configuration; an explicit conflicting selection is refused.

The owning backend implements FileManager1 for opening folders, revealing items
in grouped parent tabs and opening read-only Properties without a browser.
Properties uses a fixed selection, works in narrow tiles, and closes with
Escape/Ctrl+W; Ctrl+C copies escaped local locations. Explicit hidden reveals
show only the requested hidden files. The archive includes optional session-bus
activation metadata which starts `--daemon` on demand; see
[installation](../../docs/files.md). This is a window-independent host in the
same process as Iced. A separate frontend process for crash isolation and
production startup/idle performance acceptance remain open.

An update leaves the running owner on its existing implementation. Finish work,
close windows and release residency with `--quit` to use the new executable.

Left goes to the parent at every width. Explicitly opened hidden folders stay
visible in their hierarchy, and going up selects the folder just left while
other hidden files stay hidden. Ctrl+H toggles hidden files; explicitly hiding
them clears the temporary parent-selection visibility. Space toggles the source
window's independent Quick Look. Ctrl+N opens another browser, Ctrl+T opens a tab (drag
tabs to reorder them, onto another window to move them there, or anywhere else
to give one its own window),
Ctrl+F searches filenames and Ctrl+J opens shared Operations. Ctrl+C/X/V copy,
cut and paste files; Delete moves the selection to Trash. Shared Ctrl+Z undoes
the latest recorded file operation. If reversing a rename/move or Trash would
collide with a newer item, Keep both restores the incoming item under a new name.
Replace for a file or link preserves the newer occupant in protected Recovery,
then restores the incoming item at its original path. Keep both is the Return
default.

Operations adapts to narrow tiles: long paths wrap and available actions move
onto additional rows. Alt+R retries failed items, Alt+C continues remaining work,
and Alt+V reviews a deferred decision at any width.

Copied folders retain their Unix permissions. Continue and Undo support folders
copied as read-only. Existing folders in a Merge keep their own permissions.

Folder conflicts also offer Merge. Undo records individual returned children so
Skip, Cancel and reopening can continue unfinished work. If more source contents
remain after that snapshot, a separate review offers Merge remaining items or
Keep remaining items; Skip is its Return default. Merging additional work asks
about filename collisions again and preserves children already returned.
If either folder in a partial Merge moves, disappears, becomes unreadable or
is replaced, Undo pauses that branch. Restore the original folder or access and
retry, or choose Finish this Merge to leave files in place and protect saved
versions in Recovery. Skip remains the safe default for that review.
Merge child returns also support files, links, nested folders and empty folders
on another filesystem, with recorded progress across restart. Saved-data
containers currently pause that return and stay at their owned source location;
moving their history and data together remains in progress. Normal
cross-filesystem directory-move Undo ancestry is also still in progress.
Quick Look starts media paused. Tab enters its playback, seek and volume controls;
Space activates the focused control. Escape closes the preview. Distribution
GStreamer plugins supply codecs; see
[runtime dependencies and launcher installation](../../docs/files.md).
If a copied replacement has since changed, Undo can also restore its previous
destination: Keep both uses a new name, while Replace keeps the newer file in
Recovery. The dialog identifies the previous destination and compares metadata.
Skip leaves it available for a later Undo; Escape cancels remaining work.

If the saved previous destination itself has changed, Undo first shows its
current metadata. Restore this version explicitly uses the reviewed file or
link; Skip (the Return default) leaves it available for later. Escape cancels.
A newer occupant at the restoration path still gets its own conflict question.

If the saved previous destination is unavailable, Skip leaves the step retryable
and Escape cancels. Finish this step leaves current files in place and preserves
a Recovery record for the saved version when it becomes available again. For an
available moved item, Return moved item only returns it to its original location
and keeps restoration of the saved destination pending in Undo. A conflict at the
return location still needs its own choice. Operations reports saved versions
not restored separately from actual reversals.

Return moved item only also supports Merge when the current item is a folder.
If a saved version becomes unavailable during Merge, Continue without saved
version finishes returning the contents and keeps saved restoration pending.
Finish this Merge leaves the incoming folder in place and protects the saved
record in Recovery. Skip is the Return default; Escape cancels. Changes during
review require a fresh decision. A later Undo can restore the saved version
without returning completed children again.

For an edited copy with no previous destination, Undo offers Keep item (the
default), Move to Recovery, Skip and Cancel. Keeping finishes that history step
without touching the item. Move to Recovery preserves the newer file or link
before removing it from this folder. Kept children also keep the folders created
by that copy, including after cancellation and restart.

For a move or rename within one filesystem, if another item replaces it, Undo
offers Return this item, Keep item, Skip and Cancel. Skip is the safe default. Return explicitly
moves the reviewed replacement back; a conflict at that location still requires
its own choice. Keeping leaves the replacement here and preserves any saved
previous destination in Recovery. Edits to the same moved item return with it.

When Undo returns a move from another filesystem, newer work at its original
location receives the same Replace, Keep both, Skip and Cancel choices. Replace
preserves the newer file or link in Recovery. Skip keeps the destination copy
and its previous destination available for retry while other batch items continue.

For a move across filesystems, if the destination copy changes, Return this
version returns the shown file or link to its original location before cleaning
up that reviewed copy. Keep item leaves it here, finishes this part of Undo and
protects any saved previous destination in Recovery. Skip is the keyboard default.
If the destination cannot be inspected, Skip and Cancel keep it retryable. Finish
this step preserves current files and the saved version's Recovery record. When
the destination is missing, Restore previous destination can restore only its
saved predecessor, leaving unrelated original-location work untouched. The result
reports the incoming item as not returned.

If the moved item cannot be inspected, Undo shows its recorded location and the
lookup error. Skip keeps it retryable; Finish this step leaves the item wherever
it is and protects any saved previous destination in Recovery. If a saved file
or link is available and the incoming path is missing, Restore previous
destination restores that shown version without returning the incoming item.
Operations reports items not returned separately from changes actually undone.

If a folder created by an operation still contains items, Undo offers Keep
folder, Skip and Cancel. Keep folder preserves its remaining contents and any
created ancestor containers from that operation. Skip is the Return default and
keeps the step retryable; once new contents are moved elsewhere, an owned empty
folder can be removed normally. Kept folders are reported separately from
changes undone. Versions protected by Recovery inside a kept folder remain
available. If another item replaces a created folder, Keep item leaves it and any
original folder elsewhere in place; older Undo entries still review this replacement before changing it.
For a missing or unreadable folder, Skip keeps the step retryable. Finish this
step dismisses only that folder's removal step, leaves files untouched and reports
it as not removed. Created ancestor steps remain available for a later Undo.
Reappearance or a change during review requires a fresh decision.

Open Recovery from Places or Operations to inspect retained replacement
versions. Space previews a version; Enter or Ctrl+R recovers a copy with a chosen name and
location. The retained version stays available. F5 refreshes changes from other
programs; automatic observation also refreshes retained metadata. Load more
keeps earlier pages available. Recovery distinguishes backups still needed by Undo from protected
data requiring recovery, and keeps unavailable entries visible. Copies use the
shared Operations queue and have their own Undo. Saved folders can also be copied
to a new folder. Operations shows progress and lets you cancel, retry failed items
or continue remaining work while the application stays open. Continuing creates
its own Undo entry. Recovery never replaces newer output files; choose Keep both,
Skip or a folder Merge when a child conflicts. If the saved folder itself changes,
refresh Recovery before choosing another copy location. If the original output
folder becomes unavailable, restore it and use Continue. Unfinished work remains
in Operations while other jobs finish, including when older Undo history expires;
those older completed changes can still lose their Undo normally.
Recovery refresh and preview remain available while a file conflict or Undo
question waits for an answer. A recovery copy joins the shared operation queue.

When an interrupted operation needs attention, Recovery opens with its reason
and saved versions. Browsing and recovering fresh copies remain available while
normal operations wait. After clearing an occupied path or reconnecting its
drive, use Reconcile interrupted work. Copy-out leaves the interrupted work
available across restarts; its copies remain separately undoable after recovery.

Drag files between tiles or onto folders. Drops move on the same mounted
filesystem and copy between filesystems. Ctrl forces Copy; Shift forces Move.
The destination hint shows the action before release. Escape cancels a drag.

Ctrl+Shift+A opens the custom action editor. Add a name and command, optionally
restrict filename patterns and selection size, assign a shortcut or toolbar
button, then save. Actions also appear in a file's right-click/Shift+F10 menu.
`%f`/`%F` pass the first/all selected paths, `%n`/`%N` pass their basenames,
`%d` passes the current folder and `%%` is a literal percent. Multi-path
placeholders occupy a whole argument. Commands use quoted argument parsing;
shell syntax requires explicitly calling a shell. Commands own their effects.
Operations shows their output and cancellation controls.

Ctrl+comma opens Preferences. Edit global view defaults, add/remove/reorder
primary toolbar commands, organize local bookmarks and change keyboard shortcuts.
Ctrl+D starts a bookmark for the current folder. Save with Ctrl+S; Ctrl+R reloads.
The folder menu keeps Locations accessible when the sidebar hides in narrow
windows. Folder changes remember only the settings deliberately changed there;
Use defaults clears them. Other choices inherit current global defaults.

Devices appear after configured locations, including in the narrow Locations
menu. Open a mounted volume to browse it; activate an unmounted volume to mount
and open it. The adjacent control unmounts a mounted volume. Finish or cancel
active Files jobs before removal. New jobs wait while removal is pending.
Mounting and unmounting do not enter file Undo.

Saved configuration and external theme edits apply automatically. Invalid edits
keep the complete last valid settings and appearance and show a browser banner.
Open editor drafts stay intact; Ctrl+R explicitly reloads either editor with a
discard confirmation when needed. Stale main-file saves are refused. Theme edits
repaint existing windows without resetting Quick Look's page or zoom.

Actions and interface settings are stored in the shared Canopa configuration. For example:

```kdl
schema-version 1
files {
    view mode="columns" sort="name" descending=#false column-width=240 {
        details "size" "modified"
    }
    toolbar "back" "forward" "parent" "location" "new-folder"
    sidebar "auto"
    places {
        place "home"
        place "downloads"
        bookmark "Work" path="/home/me/Work"
        place "trash"
        place "recovery"
    }
    // Omit a shortcut to use its default. No bindings disables it.
    shortcut "search" "<Control>f" "F6"
    action "inspect-images" name="Inspect images" command="my-script %F" toolbar=#true {
        selection min=1 files=#true directories=#false
        patterns "*.png" "*.jpg" "*.jpeg"
    }
}
```

Toolbar IDs use command names such as `copy`, `new-folder`, `preferences` or
`action:inspect-images`; exactly one `location` field stays present. Search
is the location field's own button, and Quick Look has Space and the menus, so
`search` and `quick-look` are not toolbar buttons and are skipped if listed.
A toolbar without `new-folder` is written with `new-folder=#false`; one without
either, saved when New Folder was always shown, gets it at its end. Places are
`home`, `desktop`, `documents`, `downloads`, `music`, `pictures`, `videos`,
`templates`, `public`, `filesystem`, `trash` and `recovery`. Sidebar is
`auto`, `always` (when there is room) or `never`. View sort fields are `name`,
`size`, `modified` and `kind`; details are `size`, `modified` and `kind`. Add
`tree=#true` to `view` to open folders in place, as a tree, in List view, and
`folders-first=#false` to sort folders and files together. Both apply to every
folder rather than being remembered per folder.
Shortcut aliases are checked together with custom actions. Selection, focus,
Escape, context-menu and unmodified typing keys stay available. The editor lists
all command IDs through their labels and records shortcuts directly.

Use `canopa-files --check-config` to validate without opening the desktop.
`--config PATH` (also `--canopa-config PATH`) selects a shared configuration.
See [configuration](../../docs/configuration.md) and the
[Files decisions](../../docs/files/CONTEXT.md).
