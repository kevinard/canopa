# 0169 Live preferences, folder item counts and the Properties panel

Status: accepted. Builds on [0162](0162-files-browser-presentation.md) and the
root [ADR 0146](../../adr/0146-window-shape-and-dialog-elevation.md).

## Preferences apply as they change

Preferences no longer has Save and Revert buttons:

- Choices (segments, check boxes, toolbar and Places ordering, a recorded
  shortcut) save immediately.
- Typed fields (column width, bookmark name and path, shortcut text) save when
  submitted with Enter or Ctrl+S, so a half-typed value never saves. Closing the
  window saves a valid typed value; only an invalid one asks before discarding.
- A save runs without disabling the form or replacing the draft, so focus, page
  and selection survive it. A change made while a save is in flight is saved
  right after it.
- Reload appears only when the configuration changed outside the editor.
- Switching pages starts each page at its first item, as a fresh draft does.

## Folder item counts

In List view a folder's Size shows "—" until a background pass counts its
visible children; it then shows "N items", "Empty" or "10,000+ items". Counting
stops at 10,000 children. A count belongs to the folder's modification time and
hidden-file setting, so adding or removing a child counts the folder again.
Network listings keep the dash rather than reading every child folder over the
connection.

## Properties

A Properties command (Alt+Return, and the context menu) shows the selection, or
the current folder when nothing is selected. A new setting, `properties
"window"` or `"panel"`, chooses how:

- **Window**: the existing Properties window.
- **Panel**: a panel beside the listing that follows the selection, reading off
  the renderer thread as the selection changes. In Columns view it acts as a
  preview column. The command toggles it.

Both show Created where the filesystem records it, and the default application
with a Change… action that opens the Open With chooser. The owner of a file or
folder chooses each class's access from named levels: No access, Read only, or
Read and write for files, which keep their execute bit; No access, List files
only, Access files, or Create and delete files for folders. Changes are applied
off the renderer thread, and every open view of the item reads it again. Numeric
modes and identities stay behind a disclosure.

The configuration reader now recognizes `thumbnails` and `properties` beside
custom actions. Previously a saved thumbnail setting made the whole Files
configuration fail to load.

## Columns view

Inside Home, the column trail starts at Home, as browsing from Places does,
instead of showing `/` and `/home`. Outside Home it still starts at the root.
Ancestor rows share one neutral fill; the accent stays with the current
folder's selection and keyboard focus.

## Choosing views and column widths

- The toolbar no longer holds the view switcher. A compact button at the end of
  the status bar shows the current view's symbol and opens a menu of the three
  views above it (arrow keys, Enter and Escape as in other Files menus). The
  New folder button joins the toolbar from 860 logical pixels and is labelled
  from 1040.
- Dividers can be dragged: between Columns view columns, and between List view
  headings (Name, Size, Modified, Kind). A drag only repaints; on release the
  width is applied at once and saved like a Preferences change. Double-clicking
  a divider restores the default.
  - Columns share one width, the existing `view column-width` default, so
    Preferences shows it. Dragging also drops the current folder's own width,
    which would otherwise hide the new one. Each column takes its share of the
    pointer's travel, so the divider follows the pointer. Changing only the
    width keeps the loaded columns.
  - List widths are a new `list-columns name=… size=… modified=… kind=…` node,
    written only when chosen. Name is 160–1200 pixels, details 48–480. Without
    a chosen width, Name takes the room the details leave; a chosen Name width
    gives way before the details would leave the window. The divider after the
    last column appears only on hover.
  - A width save waits for any other configuration change in flight, and an
    open Preferences form adopts the saved widths while keeping its own edits.
- Icons view drops its folder heading, which repeated the toolbar's path.

## Quick Look

Quick Look opens at 880 × 620 logical pixels. Its subtitle formats dimensions as
Properties does. Its zoom controls sit below the
image, as two groups in focus order: Fit and 1:1, then zoom out, the live scale
and zoom in. The header has previous and next buttons, which step through the
source folder as the arrow keys do, and shows the item's position between them.
