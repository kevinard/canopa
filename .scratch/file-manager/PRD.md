# File manager product brief

Status: ready-for-agent

Stage: implementation in progress under delegated decision authority.

Started: 2026-10-15. Last updated: 2026-10-18.

Canopa's file manager should make everyday file work simple while offering
configuration and custom actions for more demanding workflows. Its defining
experience combines column navigation, a floating Quick Look preview, and
comfortable use in tiled windows.

## Product direction

Thunar inspires the simplicity, ease of use, extensibility, automation, and
ability to configure the interface. Mac OS X Snow Leopard's Finder inspires
column browsing and Quick Look, with nostalgia informing the feel of the
application. The interface should use Canopa's visual language.

The default interface should be understandable without configuration. Additional
power should be available through familiar controls, shortcuts, and custom
actions.

## Layout direction

The initial interactive layout study is the accepted visual baseline. Keep the
window chrome restrained, selections quiet but clear, and file rows readable.
Column visibility adapts to each browser's width. The floating preview has its
own useful size and remains attached to the browser that opened it.

## Navigation in tiled windows

Column view is the default. List and icon views remain easily accessible.

The column browser adapts automatically to the window width:

- Wide windows show several ancestor columns and the current folder.
- Medium windows show the immediate parent and current folder.
- Narrow windows give the current folder the available content width, with a
  compact path bar for navigating upward.

Columns should remain readable as the window narrows. Resizing changes the
visible context while preserving the navigation model. Keyboard commands keep
their meaning at every width; in particular, Left Arrow goes to the parent even
when its column is hidden.

Resizing should preserve location, selection, keyboard focus, and scroll
position. Widening the window reveals the surrounding hierarchy again. Returning
to a parent should select the folder just left so the user keeps their bearings.

## Floating Quick Look

Quick Look opens in a separate floating window whose size is independent of the
file manager tile. A narrow browser must still allow a comfortably sized preview.

Space opens Quick Look. Space or Escape dismisses it and returns to the same
browsing state. While the preview is open, the user can continue browsing with
the file manager's navigation keys. The preview follows the selected file.

Quick Look remains attached to the browser window that opened it. Selection
changes in that window update its preview; activating or browsing a different
window leaves it showing the originating window's selection. This lets users
keep a source file visible while choosing a destination in another tile.

The initial preview scope is:

- Images, with zoom and basic file information.
- PDFs, with page navigation.
- Text and source code, with readable formatting and syntax highlighting where
  appropriate.
- Audio and video, with playback controls.

These previews should work without user configuration. Other formats, including
office documents, can follow later.

## Custom actions and interface configuration

Custom actions come before automatic folder rules. Users should be able to run
commands or scripts for selected files and folders through context menus and
keyboard shortcuts. Example uses include resizing images, extracting archives,
and opening a terminal in the current location.

An action can declare when it applies, such as an image action applying to an
image selection. Frequently used custom actions can appear on the toolbar.

The first version includes a graphical custom action editor with:

- A name and optional icon.
- A command or script to run.
- Conditions such as file types and selection count.
- An optional keyboard shortcut and toolbar placement.

Basic actions should be approachable to configure. More elaborate actions can
call scripts maintained outside the application.

The application uses a familiar layout with substantial customization:

- Add, remove, and reorder toolbar actions.
- Organize sidebar locations and bookmarks.
- Adjust column widths and file details.
- Customize keyboard shortcuts.

### Folder view preferences

Remember view preferences per folder on top of global defaults. Columns remain
the global default, while a user can choose icons for Pictures or a detailed
list for a project folder.

Deliberate changes to the view, sorting, and visible details are remembered for
that folder. A Use defaults action clears its overrides and restores the global
defaults.

## Filename search

The first version searches filenames. Users can choose the current folder or
include its subfolders. File content search can follow later.

Typing to select a visible file remains distinct from explicitly starting a
search, preserving predictable keyboard navigation.

Search results temporarily use the current tab. Show results as a list with
each file's location, keep Quick Look available, and offer Show in containing
folder. Leaving search restores the previous folder, selection, and scroll
position.

## Windows and tabs

The first version supports separate browser windows and tabs. Users can tile two
windows side by side for work between locations, such as copying or comparing
files. Tabs keep several locations handy within one window.

Built-in split panes can follow later. The initial design uses Canopa's window
tiling to arrange simultaneous folder views while keeping room for columns.

Dragging files chooses Copy or Move automatically. Move between folders on the
same filesystem and copy between filesystems, such as onto a USB drive. Ctrl
forces Copy; Shift forces Move. Clearly show the chosen action before the drop.

Cut, Copy, and Paste remain available through keyboard shortcuts and context
menus.

## File operations and undo

Everyday file operations include opening files, creating folders, renaming,
copying, moving, and using Trash with restore. Copy and move operations need clear
progress and understandable conflict handling.

The first usable version includes Undo for renaming, moving, copying, and moving
files to Trash. Undo is available through Ctrl+Z and a visible menu action.

Undo uses one shared history across all file manager windows and tabs. A user
can undo a move between two windows while browsing in either window. The visible
command identifies the action, for example, "Undo move of 3 files to Canopa".

Undo respects subsequent changes. If reversal would overwrite newer work, the
application explains the conflict and lets the user decide how to proceed.

### Filename conflicts

When a destination already contains a file with the same name, ask the user how
to proceed. Show both files' sizes and modification dates, with three choices:

- Replace the destination file.
- Keep both by giving the incoming file a new name.
- Skip the incoming file.

Offer an optional "Apply to remaining conflicts" choice for batch operations.

Replacements are undoable. Retain the previous destination file for as long as
its Undo entry remains available. Undo restores that previous file; for a move,
it also returns the incoming file to its original location. If either location
has changed since, use the same conflict handling when undoing the replacement.

### Folder name conflicts

When an incoming folder has the same name as a destination folder, ask the user
to choose Merge, Keep both, or Skip.

- Merge combines the folders' contents. Individual filename collisions use the
  usual file conflict dialog.
- Keep both gives the incoming folder a new name.
- Skip skips the incoming folder.

For example, merging two Photos folders combines their contents while still
asking what to do about two files named coast.jpg.

### Transfers and operations

Long copies and moves continue even after every browser window closes.
Reopening the file manager gives access to a shared Operations view for
inspecting progress and cancelling transfers. Desktop notifications report
completion or anything needing the user's attention.

Cancellation stops the remaining work, cleans up unfinished copies, and keeps
completed results. The Operations view shows what finished and what remains,
with an explicit Undo option for the completed portion.

For example, cancelling after moving 30 of 100 photos leaves those 30 at the
destination and the remaining 70 at the source. Undo can reverse those 30 moves.

Individual file failures do not pause the whole batch. Continue the other files,
then show a clear summary with a Retry failed items action. If the destination
itself becomes unavailable, such as an unplugged external drive, pause the
transfer and ask for attention. Keep completed work recorded and retain source
files for failed or unfinished moves.

### Background runtime and resident mode

The application runtime owns operations independently of browser windows. Opening
Files again activates the same owning runtime and exposes the same operations.
Provide an optional resident mode that starts without a browser and stays
available while idle, with fast opening of new windows as a measured goal.

Keep the core hostable without GUI initialization so a headless mode can execute
and observe work. Windowless residency and a separate backend process are distinct
hosting decisions; see [ADR0076](../../docs/files/adr/0076-window-independent-and-resident-runtime.md).
Pending decisions remain available through Operations and desktop attention;
closing every window does not answer them. Resident mode should have low idle
CPU use and bounded memory.

## First usable version

The initial scope is local files and removable drives. Dedicated network browsing,
including SMB and SFTP locations, can follow later.

Column browsing, adaptation to tiled windows, floating Quick Look, and custom
actions are part of the initial product experience.

## First design test

Use sorting Downloads as the first workflow for validating the design:

1. Browse Downloads in a narrow tile and use Quick Look to inspect unfamiliar
   files. Keep navigating while the floating preview follows the selection.
2. Open a destination folder in a second tiled browser window.
3. Select and move files between the two windows.
4. Undo a mistaken move.
5. Run a custom action on selected images.

The walkthrough should check that navigation, floating previews, operations
between windows, recovery, and custom actions work comfortably together.

## Remaining design and implementation decisions

The user has authorized implementation and delegated these decisions. Record
chosen behavior in `docs/files/adr/` and progress in [the ledger](IMPLEMENTATION.md).

- Detailed selection and keyboard behavior, including navigation while the
  floating preview is open and interaction with preview controls.
- Transfer progress details and undo history lifetime.
- Custom action arguments, execution feedback, and interface preference controls.
- Sidebar defaults, default file details, and hidden file visibility.
- Application name and the acceptance boundary for the first implementation.

Window width thresholds and preview sizing need validation in representative
Canopa tiling layouts.
