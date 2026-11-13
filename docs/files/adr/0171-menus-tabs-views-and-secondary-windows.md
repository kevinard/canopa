# 0171 Menus, tabs, views and secondary windows

Status: accepted. Refines [0162](0162-files-browser-presentation.md) and
[0170](0170-browsing-feedback-tree-and-emblems.md); shared parts are in root
[ADR 0147](../../adr/0147-shared-switches-scrollbars-and-dialog-shadow.md).

Files worked, but its chrome read as unfinished next to other desktop file
managers: flat menus without symbols or shortcuts, tabs that stretched across
the window, a tiny view switch in the status bar, outline folder glyphs, and
dialogs that each closed and titled themselves differently. This pass changes
presentation only. Commands, paths, focus ids, keyboard order and operation
authority are unchanged.

## Menus

- Every menu row has a 16 px symbol, its name, and its first shortcut at the
  end (`KeyChord::display`, with translated key names: "Ctrl+Shift+N").
  Related items are grouped and the groups parted by a hairline. Rows that open
  another menu ("Open with", "Places") end with a chevron instead.
- The menu is as wide as its longest name and shortcut need, within bounds, on a
  raised surface with a soft shadow. Hover and keyboard focus fill the row with
  the selection tint; a menu opened by the pointer shows no focused row until
  the keyboard moves. "Delete permanently" is drawn in the danger role.
- A folder's menu keeps Paste in place, unavailable (dimmed, inert) while there
  is nothing to paste.
- A secondary click on a tab makes it current and opens its menu below the tab
  bar: New tab, Duplicate tab, Move tab to new window, Close tab and Close
  other tabs. Open With keeps the first menu's place when its applications
  arrive.

## Tabs

- The tab strip heads only the folder area, beside the full-height sidebar.
  Tabs share one width, the width the longest folder name needs (150–260 px),
  narrowing together when many are open. The strip takes the toolbar's tone.
  The current tab takes the folder's surface, without a shadow, so it opens into
  its folder. A short, straight accent bar runs along its top, inset clear of the
  rounded corners; an edge that followed the corners tapered into a crescent.

## Tones

- A browser window keeps three tones: the toolbar's, shared by the tab strip
  and the status bar; the sidebar's; and the folder's surface, shared by every
  column, List's headings and the Properties side panel. Headings and parent
  columns are set apart by hairlines and selections, not by tints of their own.

## Views

- The view button is a toolbar item, `view`, in the default toolbar and the
  Toolbar preferences like any other: the current view's symbol and a chevron,
  opening the view menu below it. The view's name is its tooltip and accessible
  name; a caption beside the symbol only repeated what the menu lists. A toolbar without it has no view button; the status
  bar never carries one, and the view menu and shortcuts still work.
- The view button, its menu and the view shortcuts change the view of the
  current tab only, for as long as the tab is open, across the folders it
  visits. They never save a folder's view options; View options does, and
  choosing a view there ends the tab's temporary one. New tabs and windows start
  from the saved views. The view button and View options both show the view the
  tab shows. Opened from the keyboard, the view menu drops from the toolbar's
  end; from the pointer, below the button pressed.
- List and search headings share one height. Columns view has no headings at
  all: the path and the column before already name each folder, and titleless
  heading bands read as empty blocks. Icons view has none either.
- List view: Name is a bounded share of the list (38 % of it, 280–440 px) unless
  a width was dragged, and the details follow it, each just wide enough for its
  widest likely value and heading; the room after the last column stays empty.
  Every divider is drawn and resizes only the column before it, following the
  pointer, from the column's narrowest width up to where the list meets the
  window's edge; the columns after it move with it. This replaces dividers that
  traded width with their neighbours, whose effect could not be predicted.
  Sizes end at the trailing edge so their units align (this replaces 0170's
  leading-edge rule for Size).
- Columns view: the current folder is a column of the column width like its
  parents, so its chevrons stay beside their names; the space after it is still
  the folder's for clicks, menus and drops, and the last column has no divider
  after it. A selected file is not previewed in that space; the Properties side
  panel does that. Ancestor columns list their folders in those folders' own
  order, as they show when opened. Pressing a parent column's highlighted
  folder, or the empty space below its list, goes back to that folder.
- Dividers take the pointer above the content beside them, so a press within a
  divider's reach resizes and never selects or navigates; they do not while a
  menu or View options lies over the window. The sidebar's and the Properties
  side panel's dividers resize them too; their widths are saved in the `panes`
  configuration node (`sidebar=160–420`, `properties=260–640`), and a
  double-click restores the default.
- Icons view: tiles are wider than tall, so landscape pictures fill them;
  pictures get a hairline mat and soft shadow, and they, folders and type
  symbols stand on the tile's lower edge; a symbol's empty rows below its
  drawing are trimmed so its lowest pixel meets that edge. Names break after spaces, underscores or
  hyphens, never just before the extension. The icon size is any number of
  pixels from 48 to 256, set with a slider; cells follow it, so small icons
  pack many to a row. Only a number of pixels is read from the configuration;
  the earlier size names are not.
- View options is a popover in the menus' surface: a heading naming the folder
  with the round close button, then Preferences' Views settings in the same
  groups, order and names ("Browsing": view, sort, reverse order, folders
  first; "Columns and lists": column width, List's details, the tree), with
  "Use defaults" in its footer. View choices carry the toolbar's view symbols,
  in both places. The column width steps by 20 px with round −/+ buttons, to the
  next multiple of 20, so a dragged 287 steps to 300 or 280; Preferences keeps
  its typed value between them.
- The pointer shows the grabbing hand while items are dragged. During the
  Wayland drag itself the compositor draws the pointer, so this needs the
  compositor's grabbing cursor for drags (a Niri fork change).
- The current place in the sidebar is lit inside its folders, except Home and
  the Filesystem.

## Operations

- Pasting items into the folder they are in copies them beside the originals
  under the next free numbered name (`Report (2).txt`); a cut pasted there, or a
  drop on the same folder, does nothing.
- An operation's failure is shown in its tab's status bar until that operation
  is no longer failing (dismissed, retried or gone) or the person selects or
  runs something else; Operations keeps the record. The status summary only
  shows operations that are under way or need the person.

## Secondary windows

- The Properties side panel shows one item's picture as wide as the panel
  allows, its name as a title that becomes a field when pointed at or focused,
  and in its narrow width the application that opens the item is itself the
  button that changes it.
- Properties, Preferences and Quick Look close from a round button in their
  heading; their headings lead with the item's picture where there is one.
  Rename, New folder and Delete share one width, a title and a subtitle.
  Renaming a file selects its name without the extension (a compressed
  tarball's two suffixes count as one).
- Preferences' Views page is a list of settings grouped under titles, each line
  with its name and note at the start and its control at the end. Choices are
  equal-width segmented controls; on/off settings are switches.
- Quick Look fits a picture to the stage without enlarging it past its own
  pixels, inside an even margin; stepping through the folder is one pager.
- Keyboard focus rings appear once the keyboard moves focus and hide at the next
  click, so a dialog's default button opens without one; Enter and Space still
  act on it. High contrast keeps its rings. Sliders take focus like any control
  and step with the arrow keys, Page Up/Down by larger steps, and Home/End to
  their ends.
- Every Files window over a browser dims it alike, lightly; light windows dim
  toward their own ink rather than grey.
