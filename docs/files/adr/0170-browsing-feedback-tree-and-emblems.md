# 0170 Browsing feedback, List view's tree and emblems

Status: accepted. Builds on [0162](0162-files-browser-presentation.md) and
[0169](0169-live-preferences-folder-counts-and-properties-panel.md).

## Names, headings and column widths

- A file name that does not fit loses its middle, not its end, so its ending and
  extension stay visible (`Quarterly re…final.pdf`). Icons view keeps two lines
  and shortens the middle to fit them. Other single-line labels keep the end
  ellipsis.
- List view's column headings sort when clicked. Clicking the sorting heading
  reverses it; another heading starts ascending for Name and Kind, and newest or
  largest first for Modified and Size.
- Every column, Size included, starts at its leading edge, headings and values
  alike.
- While Name fills the list (following the window, or with a chosen width that
  reaches its edge), a divider moves the boundary between the two columns beside
  it: the detail after it gives what the column before it gains, so the divider
  stays under the pointer. Name's own divider trades with the first detail. With
  room after the last column, a detail grows into that room first and only then
  takes from the column after it; Name's divider resizes Name.

## Pointer feedback

- File rows, the chevrons of List view's tree and List headings show a hover
  wash. Quiet buttons in the shared `canopa-ui` style now lean their hover
  toward the text colour instead of using the surface colour, which menus and
  sidebars are drawn on, so menu items and Places show their hover. The lean
  stays small enough for muted text and links to keep their 4.5:1 contrast.
- A secondary click opens its menu at the pointer, turned back from the window's
  right and bottom edges. A secondary click on a folder's empty space clears the
  selection and opens the folder's menu there. Keyboard and toolbar menus keep
  their place.
- In Columns view, a single click on a folder in a parent column opens it; a
  file there is selected in its folder as before.
- A click on a folder's empty space clears the selection. The mouse's Back and
  Forward buttons go back and forward.
- Tooltips are drawn as a bubble with the text and background colours swapped.
- Context menus hold what fits their context: a selection's menu acts on the
  items; the folder's (empty space, or nothing selected) offers New Folder,
  Paste, Undo, Select All, Bookmark, View options and Properties. The toolbar's
  menu holds window, folder and application commands, without item actions.
- Unsaved-changes questions in Preferences and the custom actions editor dim
  the window and appear as a raised card that takes all input until answered.
- Search uses the toolbar's location field instead of a second field below it:
  the field's search button (where its edit button was) opens it, and a cross
  in the field closes it. The toolbar has no Search button; `search` in a
  configured toolbar is skipped. The current folder's name in the path still
  opens the location for typing, as Ctrl+L does. The search's options and
  actions sit beside the results' heading, and unreadable folders are listed
  under the toolbar only on request.
- A window with several tabs shows a tab bar: each tab names its folder, closes
  with its cross or a middle click, and takes dropped files into its folder; a
  plus button opens another. A folder's menu offers Open in New Tab, as does a
  middle click on it.
- Tabs are dragged as native drags of a Files-only type that carries the tab's
  identity. A picture of the tab, cut from a capture of its window when the
  drag starts, follows the pointer, and the tab left behind is ghosted. Every
  Files window accepts the drag anywhere and shows a pin where the tab would
  land (a one-tab window shows its bar meanwhile): on a bar, between the tabs
  it falls between; elsewhere in another window, after its tabs. The tab moves
  there and is shown, its folder read again; a window it leaves empty closes.
  A drag nothing accepts, ended outside every Files window, gives the tab a
  window of its own with its history and selection. The compositor grabs only
  the pointer, so Escape still reaches the source window, which gives the drag
  up without opening a window. Without native drag and drop, the bar carries
  the tab itself and a release outside the window (which keeps the pointer
  while its button is held) detaches it. Move tab left, Move tab right
  (Ctrl+Shift+Page Up/Down) and Move tab to new window (in the toolbar menu
  while a window has several tabs) do the same from the keyboard.
- The toolbar no longer starts with an application mark: the window title
  names the application, and the room goes to the toolbar.
- Every List detail column, Size included, starts half a gap after the divider
  before it.

## Drag and drop

- A drop into the dragged items' own folder (directly or through a folder
  alias), or onto a dragged folder itself, is refused when its hover is
  prepared, so nothing is moved or copied.
- A drag over a browser window shows a label beside the pointer with what is
  dragged and what a drop there does ("Move to Documents · report.pdf"), and
  outlines the target, only when a drop there would copy or move. Rows being
  dragged out fade. The dragged items' icons or thumbnails (up to three,
  stacked) follow the pointer as the native drag icon, at twice the logical
  size, through the winit and Iced forks' `start_with_icon` (winit#8, iced#9).

## Status bar and Properties

- One selected item is named in the status bar with its type description and
  size. Type descriptions come from the type-icon worker, never the renderer.
- The status bar no longer says "Recent activity" once the newest operation has
  finished; running, waiting and failed operations still show there. Operations
  stays in the menu and on Ctrl+J.
- The Properties panel's name is a field: Enter renames the one selected item
  through the same prepared, undoable operation as the Rename dialog; Escape
  gives the edit up. The open folder itself is not renamed from there.
- A regular file's Access section offers "Allow executing as a program". Turning it on
  adds execute permission for each class that may read the file (at least the
  owner); turning it off removes it for all.

## Special folders

Home and the XDG personal folders (Desktop, Documents, Downloads, Music,
Pictures, Public, Templates, Videos) ask the icon theme for `user-home`,
`user-desktop` and `folder-*` icons, falling back to the folder icon. A personal
folder defined as Home itself marks nothing.

## Folders first, and choices shared by every folder

A view setting, `folders_first` (`view … folders-first=#false`), sorts folders
and files together when off. It and List view's tree are one choice for every
folder: View options, the status bar's view menu (tree) and Preferences change
them everywhere and save them to the configuration. Preferences shows Folders
before files with the sort order, since it applies to every view. Folder view files may still
contain an older per-folder tree choice; it is read and ignored.

## List view's tree

A view setting, `tree` (`view … tree=#true`), lets List view open folders in
place. A chevron, Right and Left
open and close a folder; Left on a nested row goes to its folder. Open folders
are read on workers, flattened into the folder's rows, and read again when the
folder refreshes. Selection, keyboard movement, operations, drag sources and
targets work on nested rows. Closing a folder moves a selection inside it to the
folder. Another folder, search, or leaving List view closes the tree. Folder
watching still covers the open folder only; open subfolders follow its refreshes.

## Emblems

File icons carry emblems on small discs at their lower corners: a link, and
either a lock (this person may not read the item) or a crossed pencil (may read
but not change it). Access comes from the item's mode bits, or its target's for a
link, read on the listing worker; access-control lists are not consulted.
Entries saved before this was recorded show no access emblem. User-assigned
emblems (GIO `metadata::emblems`) are not read.

## Preferences lists

Preferences' Toolbar, Sidebar (formerly Locations) and Shortcuts pages give
each row its own controls instead of a selection with shared buttons:

- Toolbar shows a preview of the toolbar, then one row per item with earlier,
  later and remove buttons (the location field cannot be removed). "Add a
  button…" opens a filterable picker. New Folder, formerly always shown beside
  the toolbar menu in wide windows, is now an item like the others and in the
  default toolbar. A toolbar saved before then gets it at its end; one saved
  without it is written `toolbar … new-folder=#false` so it stays off.
- Sidebar lists its places in order, each with the same row buttons. Every
  XDG personal folder is a built-in place: Music and Videos join the default
  sidebar, and Templates and Public can be added. Places, in the sidebar and
  in Preferences, take their icons from the icon theme (`user-home`,
  `folder-download`, `drive-harddisk`, `user-trash`…; a bookmark takes its
  personal folder's icon or `folder`), prepared by the type-icon worker. The
  built-in glyph stays until then, when the theme has none, and for Recovery. A
  bookmark is a place added for one of your own folders; selecting its row
  opens its name and folder fields in place. "Add a place…" offers a new
  bookmark and the built-in places not yet listed. The toolbar menu's
  Places… lists the same places when the window is too narrow for the sidebar.
- Shortcuts is one table: each command shows its shortcuts with Record and
  Reset buttons; selecting a row opens its shortcuts for typing.
