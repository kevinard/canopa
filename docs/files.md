# Canopa Files

Canopa Files is the desktop's file manager. It browses local folders, removable
drives, network shares, phones and cameras, and it is built for tiled windows:
column browsing adapts to narrow tiles, Quick Look floats beside the window that
opened it, and every command works from the keyboard. Copies, moves, renames and
Trash share one Undo that survives restarts.

Developers: see the [Files architecture](architecture/files.md).

## Starting Files

- Open **Canopa Files** from the launcher, or press Super+E with Canopa's default
  Niri bindings.
- From a terminal: `canopa-files ~/Downloads`. Several folders open as tabs in one
  window. Local `file://` URIs work too.
- `canopa-files --quick-look FILE` opens a browser on the file's folder with the
  file selected and shows it in Quick Look. The launcher's Quick Look uses this.

Only one Files process runs per account. Later launches, and other applications'
"Show in folder" requests (when Files is the FileManager1 provider), open windows in
that process, so all windows share operations, Undo and the clipboard. Files exits
when its last window closes and its work is finished, unless it is resident (see
[Command line](#command-line)).

## Browsing

### Windows, tabs and views

- **Views**: Columns (default, Ctrl+1), List (Ctrl+2) and Icons (Ctrl+3). The
  toolbar's view button switches the current tab. List view has sortable, resizable
  column headings (click a heading to sort) and can show folders as an expandable
  tree (Preferences > Views, or `tree=#true`). In List view, folders show their
  item count, up to 10,000+.
- **Columns**: Left goes to the parent and Right enters the selected folder at
  every window width. Going up selects the folder you just left.
- **Tabs**: Ctrl+T opens a tab, Ctrl+W closes it, Ctrl+Tab and Ctrl+Shift+Tab
  switch, and Ctrl+Shift+Page Up/Page Down move it. Drag a tab to reorder it, onto
  another window to move it there, or anywhere else to give it its own window. The
  tab's menu also offers Duplicate tab, Close other tabs and moving it to a new window.
- **Windows**: Ctrl+N opens another browser window. Each window has its own tabs and
  its own Quick Look.
- **Type to select**: typing a name's first letters selects the first match.
- **Hidden files**: Ctrl+H shows or hides them. A hidden folder you open explicitly
  stays visible in its hierarchy without revealing its hidden siblings.
- Folders refresh by themselves when other programs change them. F5 refreshes on
  demand (network folders refresh every 10 seconds).

### Location bar

The location bar shows the path as breadcrumbs. Ctrl+L edits it: type a path, a
`file://` URI or a server address such as `sftp://host/` to connect. Escape gives up
the edit.

### Sidebar and places

The sidebar lists places, bookmarks, devices and network locations. When the window
is too narrow for it, the toolbar's Places button opens the same list.

Built-in places: Recent, Home, Desktop, Documents, Downloads, Music, Pictures,
Videos, Templates, Public, Filesystem, Trash and Recovery. Templates and Public are
not shown until you add them in Preferences > Sidebar. Personal folders follow your
XDG user directories (`~/.config/user-dirs.dirs`), including localized names and
folders outside Home; Files reads them at launch.

Ctrl+D bookmarks the current folder. Edit, reorder or remove places and bookmarks in
Preferences > Sidebar.

### Folder view options

The folder menu's **View options…** changes the view, sort order and columns of the
current folder only. Files remembers only the settings you change there; **Use
defaults** clears them. The global defaults are in Preferences > Views.

## Quick Look

Space opens Quick Look for the selection and closes it again; Escape or Ctrl+W also
closes it. It follows the window's active tab and selection, including search
results, while the keyboard keeps navigating the browser. Tab moves into the
preview's own controls; Space or Return activates the focused control.

| Content | Preview |
|---|---|
| Images | Fit, zoom and 100% |
| PDF | One page at a time; Page Up and Page Down turn pages |
| Text and source code | Selectable text with syntax colors; large files are truncated |
| Audio and video | Opens paused; Play/Pause, Seek, Mute and Volume. Focused Seek and Volume use the arrows, Home and End. |
| Folders | Item count |
| Other files | A short explanation, with Open in the default application |

PDF previews need Poppler (`pdfinfo`, `pdftoppm`). Audio and video use the
installed GStreamer plugins.

## Search and Recent

Ctrl+F searches the current folder by name. Turn on **Include subfolders** to search
everything below it. Results show their folder; **Show in folder** reveals one, and
closing the search (Escape) returns to the folder with its selection and scroll.
**Stop** keeps the results found so far; **More results** continues past the first
50,000.

Search filters:

| Filter | Choices |
|---|---|
| Type | Any type, Folders, Documents, Images, Audio, Video, Archives |
| Modified | Any time, Today, Past week, Past month, Past year |
| Size | Any size, Under 1 MiB, 1 to 100 MiB, Over 100 MiB |
| Search contents | Also match text inside text files (up to 16 MiB) |

A filter works with an empty query, so Type: Images alone lists every image.

**Recent** (first in the sidebar) lists the files recently used on the desktop,
newest first, from the shared `recently-used.xbel` list. It behaves like a search,
so the query and filters narrow it. Files adds the documents it opens with an
application to that list.

## Opening files

Return (or double-click) opens the selection:

- Folders open in the browser.
- Documents open with their default application. If there is none, the Open With
  dialog asks; it can also **Set as default** for that file type.
- Programs and scripts always ask first: Run, Run in Terminal, Open (for scripts) or
  Cancel. Files treats a file as a program only when it is executable, on a local
  disk (not removable, network or `noexec`), and starts like a program.
- Application launchers (`.desktop` files) outside an applications folder that are
  not marked executable ask: Launch, Open as Text or Cancel.

The context menu's **Open with** lists the default application, other suitable
applications and **Other application…**. **Open in Terminal** opens your terminal in
the folder through `xdg-terminal-exec`.

## Working with files

### Menus

Right-click, Shift+F10 or the Menu key opens the context menu.

- On a selection: Open, Open in new tab and Open in Terminal (one folder), Open with,
  Quick Look, your custom actions, Copy, Cut, Extract here and Extract to… (archives
  only), Compress…, Rename…, Move to Trash, Delete permanently… and Properties.
- On the folder background: New folder…, New document, Paste, Undo, Select all, Open
  in Terminal, Bookmark this folder…, View options… and Properties.
- The window menu also has New window, New tab, Places, Show hidden, Trash,
  Operations, Connect to Server…, Edit custom actions… and Preferences….

### Copy, move and paste

Ctrl+C, Ctrl+X and Ctrl+V copy, cut and paste files. The file clipboard works with
GTK and KDE file managers. Pasting into the same folder creates "Name (2)".

Drag files onto a folder or another window. A drop moves within the same drive and
copies between drives; hold Ctrl to copy or Shift to move. The drop hint shows the
action before you release. Escape cancels the drag.

Copies keep permissions, modification and access times, `user.*` extended
attributes, ACLs and sparse files. If the destination cannot store some attributes
(a FAT drive, for example), the copy still completes and the result says how many
copies are missing them. Hard links are copied as separate files. A Move between
drives removes the originals only after the copies are complete and verified.

On a btrfs drive, a copy within the drive is a clone: it finishes almost at once
and uses no extra space until one of the two files is edited. This includes moves
between subvolumes such as a separate `~/Photos`, which btrfs treats as different
filesystems. Other filesystems copy the data as usual.

### Name conflicts

When a name is already taken, Files asks:

- **Replace** (files): the existing file is kept as a saved version, so Undo can
  bring it back.
- **Merge** (folders): combine the folders, asking again for each colliding item.
- **Keep both**: give the incoming item a numbered name, such as `report (2).txt`.
- **Skip**: leave both untouched.

Check **Apply to remaining file conflicts** or **Apply to remaining folder
conflicts** to reuse the answer. Each question starts on its safe answer, so Return
keeps both (or skips the item when keeping both is not offered); Escape cancels.

### Operations window

Ctrl+J opens Operations, shared by all windows. It shows progress, remaining time,
questions waiting for an answer and results. Use it to cancel, Retry failed items,
Continue remaining work or review a decision you postponed. Alt+D opens the details
of the running transfer. Work continues when you close browser windows, and Files
stays running until it finishes. Finished operations also send a desktop
notification.

If a drive disappears during a transfer, the transfer pauses. Reconnect the same
drive and choose Retry; completed work stays recorded.

### New folders and documents

Ctrl+Shift+N creates a folder. **New document** creates an empty document or a copy
of a file from your Templates folder (`XDG_TEMPLATES_DIR`, one level of subfolders);
**Open Templates folder** opens it. Both can be undone.

### Rename and bulk Rename

F2 renames the selection. With several items of one folder selected, Rename opens
bulk Rename:

- **Replace text**: replace Find with Replace with.
- **Add text**: add text before or after the name.
- **Number**: a base name and a counter from Start at, in the order shown.

Extensions are kept; folders are renamed whole. The preview flags invalid names,
duplicates, names already used in the folder and swaps (rename those in two steps).
The whole bulk Rename is one Undo step.

### Archives

**Compress…** creates a zip (default), tar.gz, tar.xz or tar.zst archive.
**Extract here** and **Extract to…** read zip, tar, tar.gz, tar.xz, tar.zst and
tar.bz2 archives. An archive with one top-level item extracts to that item; otherwise
it extracts to a folder named after the archive. Results never replace an existing
item: they get the next free name. Extraction skips entries that would land outside
the destination and never restores set-id bits. Archive jobs run in Operations and
can be cancelled, but they are not undoable.

## Undo

Ctrl+Z undoes the latest file operation, from any window, even after Files restarts.
It covers copy, move, rename, bulk Rename, new folders and documents, Trash and
Recovery copies. Files keeps the latest 100 operations. There is no Redo.

Undo never destroys newer work. When something changed since the operation, it asks
first, for example:

- The original name is taken: **Keep both** restores under a new name; **Replace**
  restores it and keeps the newer item in Recovery.
- A copied file was edited: **Keep item** leaves it; **Move changed item to
  Recovery** keeps the edited version safe before undoing the copy.
- A created folder now contains other files: **Keep item** leaves the folder and its
  contents.
- An item cannot be inspected (for example, its drive is disconnected): **Skip**
  keeps the step for later; **Finish step** gives it up and keeps any saved version
  in Recovery.

**Skip** always leaves the step available for a later Undo, and Escape cancels the
rest. Permanent deletion, Empty Trash, archives, custom actions, permission changes
in Properties, and mounting or unmounting are not undoable.

## Trash

Delete moves the selection to Trash. Open the Trash place to see it:

- **Restore** (Ctrl+R) puts items back; **Restore to…** (Ctrl+Shift+R) chooses
  another folder. Restore never overwrites.
- **Copy** (Ctrl+C) puts the items on the clipboard so you can paste copies elsewhere.
- **Delete permanently…** (Delete or Shift+Delete) and **Empty Trash…** remove items
  for good after a confirmation.
- Ctrl+0 shows every drive's Trash, Ctrl+1 only the main Trash. Space previews.

Shift+Delete in a browser deletes the selection permanently after confirmation. When
a drive has no usable Trash, Files offers to delete the items permanently instead,
again after confirmation. Network locations have no Trash.

Inside a btrfs subvolume, items go to a `.Trash-UID` folder at the top of that
subvolume and appear in Trash with the rest.

## Recovery

Recovery keeps versions that Replace and Undo set aside, and the files of
operations interrupted by a crash or power loss. Open it from the sidebar or from
Operations.

- Space previews a version. Enter or Ctrl+R recovers a copy under a name and folder
  you choose; the saved version stays in Recovery. Recovery copies are undoable.
- Versions still needed by Undo are marked as such. Protected versions stay until
  you recover them; other saved versions are discarded when their operation leaves
  the 100-operation history.
- After an interruption, Recovery opens at the next start and explains what
  happened. Ordinary file changes wait until you choose **Reconcile interrupted
  work** (after reconnecting a drive or clearing an occupied name, for example) or
  **Keep in Recovery**. Browsing and recovering copies keep working meanwhile.

Saved versions live in hidden `.canopa-files-stage-*` folders beside the original
items. Do not delete those folders by hand; use Recovery.

## Properties

Alt+Return shows the selection's properties: size, dates, type, the application it
opens with (**Change…**), image dimensions, link target, owner, group and access
levels. You can rename from the name field (Enter), change access levels and toggle
**Allow executing as a program**; access changes apply immediately and are not
undoable. They apply only to the item shown: a symbolic link is never followed, and
an item replaced since Properties read it is refused. A Properties window another
application opened (FileManager1 `ShowItemProperties`) only shows the items. **Copy locations** (Ctrl+C) copies the items' `file://` URIs.

Properties opens in its own window by default. Choose **In a side panel** in
Preferences > Views (or `properties "panel"`) to show it beside the listing,
following the selection.

## Devices

Drives and partitions appear in the sidebar. Select an unmounted device to mount
and open it. The eject button powers off, ejects or unmounts, whichever the device
supports. Files waits for its own operations and custom actions before removing a
device; new work waits until the removal finishes. Password questions for encrypted
devices appear in their own window and are never saved. Mounting and unmounting are
not undoable.

## Network locations

Choose **Connect to Server…** or **Other Locations…** in the sidebar, or type an
address in the location bar. Supported addresses: `smb://`, `sftp://` (`ssh://`),
`ftp://`, `ftps://`, `dav://`, `davs://`, `nfs://` and `\\server\share`. Files asks
for a password when the server needs one; never put a password in the address.
**Add to bookmarks** saves the server in the GTK bookmarks file, shared with GTK
applications. Servers discovered on the local network are listed under **On this
network**. Phones (MTP) and cameras (PTP) appear like devices.

Network locations come from GVFS. When a backend is missing, Files names the package
to install (see [Optional runtime tools](#optional-runtime-tools)). Network folders
have no Trash, refresh every 10 seconds and show no thumbnails unless thumbnails are
set to `always`.

## Custom actions

Ctrl+Shift+A opens the custom action editor. Give an action a name and a command,
optionally an icon, file patterns, a selection size, whether it applies to files
and folders, a shortcut and a toolbar button, then save. Applicable actions appear
in the selection's context menu.

| Placeholder | Expands to |
|---|---|
| `%f` / `%F` | The first / all selected paths |
| `%n` / `%N` | The first / all selected file names |
| `%d` | The current folder |
| `%%` | A literal `%` |

`%F` and `%N` must be a whole argument. Commands are split like a shell would split
them, but never run through a shell; call `sh -c '...'` explicitly when you need shell
syntax. Commands run only when you invoke them. Operations shows their output and can
cancel them. Their effects are not undoable, and queued actions do not survive
quitting Files.

## Preferences

Ctrl+comma opens Preferences:

- **Views**: default view, sort order, columns, column width, folders first, List
  tree, thumbnails and where Properties appears.
- **Toolbar**: add, remove and reorder toolbar buttons, including custom actions.
- **Sidebar**: places and bookmarks.
- **Shortcuts**: record shortcuts for every command.

Choices save immediately; typed fields save on Enter or Ctrl+S. Preferences and the
action editor change only the `files` section of `config.kdl`. Edits you make to the
file yourself, and theme changes, apply to open windows automatically.

## Keyboard shortcuts

Browser shortcuts are configurable (see [Configuration](#configuration)). Defaults:

| Command ID | Action | Keys |
|---|---|---|
| `back` / `forward` | Back / Forward | Alt+Left / Alt+Right |
| `parent` | Parent folder | Left, BackSpace, Alt+Up |
| `enter-folder` | Enter folder | Right |
| `open` | Open | Return |
| `quick-look` | Quick Look | Space |
| `search` | Search | Ctrl+F |
| `location-focus` | Edit location | Ctrl+L |
| `refresh` | Refresh | F5 |
| `show-hidden` | Show hidden files | Ctrl+H |
| `select-all` | Select all | Ctrl+A |
| `new-folder` | New folder | Ctrl+Shift+N |
| `rename` | Rename | F2 |
| `trash` | Move to Trash | Delete |
| `delete-permanently` | Delete permanently | Shift+Delete |
| `undo` | Undo | Ctrl+Z |
| `copy` / `cut` / `paste` | Copy / Cut / Paste | Ctrl+C / Ctrl+X / Ctrl+V |
| `columns` / `list` / `icons` | View | Ctrl+1 / Ctrl+2 / Ctrl+3 |
| `new-tab` / `close-tab` | New tab / Close tab | Ctrl+T / Ctrl+W |
| `next-tab` / `previous-tab` | Switch tab | Ctrl+Tab / Ctrl+Shift+Tab |
| `move-tab-left` / `move-tab-right` | Move tab | Ctrl+Shift+Page Up / Ctrl+Shift+Page Down |
| `new-window` | New window | Ctrl+N |
| `operations` | Operations | Ctrl+J |
| `custom-actions` | Edit custom actions | Ctrl+Shift+A |
| `preferences` | Preferences | Ctrl+comma |
| `properties` | Properties | Alt+Return |
| `bookmark-folder` | Bookmark this folder | Ctrl+D |

These commands have no default key: `search-subfolders`, `stop-search`,
`more-search-results`, `reveal-search-result`, `search-problems`, `open-terminal`,
`show-trash`, `show-recent`, `connect-to-server`, `move-tab-to-window` and `recovery`.

Fixed keys: Shift+F10 or Menu opens the context menu; arrows, Home, End, Page Up and
Page Down move the selection (with Shift to extend it); in a List tree, Left and Right
collapse and expand folders; Escape closes search, menus and dialogs. In the Trash,
Recovery, Operations, Properties and Quick Look windows, Escape or Ctrl+W closes the
window. In Trash and Recovery, Ctrl+J opens Operations and Ctrl+Z undoes. In
Operations, Ctrl+N opens a browser window.

## Command line

```text
canopa-files [OPTIONS] [PATHS]...
```

| Option | Effect |
|---|---|
| `PATHS` | Local folders or `file://` URIs; several open as tabs in one window. |
| `--quick-look PATH` | Open a local file (or `file://` URI) in Quick Look, in a new browser window on its folder. |
| `--daemon` | Stay resident without opening a window; later launches open windows instantly in this process. Runs in the foreground. |
| `--quit` | Release resident mode. Files exits once its windows, accepted work and pending questions are finished; nothing is cancelled. |
| `--config PATH` | Use this `config.kdl` (alias `--canopa-config`). |
| `--defaults` | Use only built-in defaults; no file is read or watched. |
| `--check-config` | Validate the `files` section and appearance without opening a window. Prints `{"component":"files","source":...}` or `{"component":"files","error":...}` and sets the exit status. |
| `--show-source` | With `--check-config`, also print the error to standard error. |
| `--version`, `--help` | Version and help. |

A launch without `--config` or `--defaults` uses the running process's
configuration. An explicit selection that differs from it is refused.

Resident mode:

```sh
canopa-files --daemon &     # start from the graphical session
canopa-files ~/Downloads    # opens a window in the resident process
canopa-files --quit         # release residency
```

Start `--daemon` from the graphical session: a process started without a Wayland or
X11 display stays headless and refuses windows until you quit it.

## Configuration

Files reads the `files` section of the shared `~/.config/canopa/config.kdl`; see
[configuration](configuration.md) for the file, `canopa config check` and the other
sections. The section is strict: unknown nodes or properties, duplicates and type
annotations are errors. Each node may appear once, except `shortcut` and `action`.
Edits apply automatically; an invalid edit keeps the last valid settings and shows a
banner in Files windows.

```kdl
files {
    view mode="columns" sort="name" descending=#false column-width=240 folders-first=#true {
        details "size" "modified"
    }
    toolbar "back" "forward" "parent" "location" "new-folder" "view" "action:inspect-images"
    sidebar "auto"
    properties "window"
    thumbnails show="local" size=96
    places {
        place "recent"
        place "home"
        place "downloads"
        bookmark "Work" path="/home/me/Work"
        place "trash"
        place "recovery"
    }
    // Omit a command to keep its defaults; a shortcut with no keys disables it.
    shortcut "search" "<Control>f" "F6"
    action "inspect-images" name="Inspect images" command="my-script %F" toolbar=#true {
        selection min=1 files=#true directories=#false
        patterns "*.png" "*.jpg" "*.jpeg"
    }
}
```

| Node | Values | Default |
|---|---|---|
| `view mode=` | `columns`, `list`, `icons` | `columns` |
| `view sort=` | `name`, `size`, `modified`, `kind` | `name` |
| `view descending=` | boolean | `#false` |
| `view column-width=` | 160 to 640 | 240 |
| `view tree=` | boolean; List view opens folders in place | `#false` |
| `view folders-first=` | boolean | `#true` |
| `view { details ... }` | any of `size`, `modified`, `kind` | `size` `modified` |
| `toolbar` | command IDs, `location` (exactly once), `view`, `action:<id>`; at most 64 | `back forward parent location new-folder view` |
| `sidebar` | `auto` (shown in wide windows), `always` (whenever there is room), `never` | `auto` |
| `properties` | `window`, `panel` | `window` |
| `thumbnails show=` | `local` (skip network and removable media), `always`, `never` | `local` |
| `thumbnails size=` | pixels, 48 to 256 | 96 |
| `list-columns` | `name=` 160 to 1200; `size=`, `modified=`, `kind=` 48 to 480 | follow the window |
| `panes` | `sidebar=` 160 to 420; `properties=` 260 to 640 | built-in widths |
| `places { place "<id>" }` | `recent`, `home`, `desktop`, `documents`, `downloads`, `music`, `pictures`, `videos`, `templates`, `public`, `filesystem`, `trash`, `recovery` | all except `templates` and `public` |
| `places { bookmark "Name" path="/abs" }` | name of 1 to 128 characters; absolute `path=`, or `path-bytes=` (hex) for non-UTF-8 paths; at most 256 entries | none |
| `shortcut "<command-id>" "<keys>"...` | GTK-style keys such as `<Control>f`, `<Shift>Delete`, `F6`; at most 8 | see [Keyboard shortcuts](#keyboard-shortcuts) |
| `action "<id>"` | `name=`, `command=`, `icon=`, `shortcut=`, `toolbar=`; children `selection min= max= files= directories=` and `patterns` | `selection min=1 files=#true directories=#false` |

Notes:

- `quick-look` and `search` are never toolbar buttons and are dropped if listed. A
  toolbar without `new-folder` is saved as `toolbar ... new-folder=#false`.
- `tree` and `folders-first` apply to every folder. The other `view` settings are
  defaults that a folder's View options can override.
- Shortcuts may not take keys that typing or selection need, and custom-action
  shortcuts need a modifier such as Ctrl.
- `thumbnails size` takes a number; words such as `"medium"` are rejected.

Validate without opening a window:

```sh
canopa-files --check-config --show-source
```

## Default file manager

Installing Canopa adds `org.canopa.Files.desktop`, which makes Files available as a
folder handler without changing your default. To make it the default for folders:

```sh
xdg-mime default org.canopa.Files.desktop inode/directory
```

Other applications' "Show in folder" requests use the separate
`org.freedesktop.FileManager1` D-Bus service. Canopa ships its activation file without
enabling it, so it never takes over from another file manager. The
[installation guide](installation.md) shows how to enable it for your account. Once
enabled, the first request starts `canopa-files --daemon`, which stays resident until
`canopa-files --quit`. Files does not take the name while another file manager owns it.

## Optional runtime tools

The Arch `canopa` package installs what Files needs: GStreamer with the base, good and
libav plugins, Bubblewrap, `xdg-terminal-exec`, GLib (`gio`) and GVFS. Optional:

| Package | Enables |
|---|---|
| `poppler` | PDF previews and PDF thumbnails |
| `gvfs-smb` | Windows and Samba shares |
| `gvfs-nfs` | NFS shares |
| `gvfs-mtp` | Phones |
| `gvfs-gphoto2` | Cameras |
| `gvfs-afc` | Apple devices |
| `gvfs-dnssd`, `gvfs-wsdd` | Discovering servers on the local network |
| other GStreamer plugin packages | More audio and video formats |

Other thumbnail types use the thumbnailers installed on the system; they run only
when `/usr/bin/bwrap` is available.

## Troubleshooting

- **No window appears.** A Files process may be running without a display, for
  example `--daemon` started outside the graphical session. Run `canopa-files --quit`,
  wait for it to exit, and start Files again from the session.
- **"Files is already running with a different configuration selection."** The
  running process uses another `--config`/`--defaults` selection. Launch without the
  option, or quit the running process first.
- **Still the old version after an update.** The running process keeps the old
  executable. Finish your work, close the windows and run `canopa-files --quit`.
- **A banner says the configuration is invalid.** Run
  `canopa-files --check-config --show-source` to see the error. Files keeps using the
  last valid settings until you fix it.
- **File changes are refused after a crash.** An interrupted operation needs review.
  Open Recovery, follow its explanation, then choose Reconcile interrupted work or
  Keep in Recovery.
- **A PDF does not preview.** Install `poppler`.
- **Audio or video does not play.** Install the GStreamer plugins for the format.
  Run Files from a terminal to see `[canopa-files media]` messages.
- **No thumbnails.** Check Preferences > Views; the default `local` policy skips
  network and removable media. Thumbnails other than common images need `bwrap`.
- **A network address does not open.** Install the GVFS package the message names.
- **Run in Terminal or Open in Terminal does nothing.** Install `xdg-terminal-exec`
  and set your preferred terminal.
- **"Show in folder" opens another file manager.** Enable the FileManager1 activation
  file as described in the [installation guide](installation.md), then quit the other
  file manager's service or log in again.

## Known limitations

- There is no Redo, and Undo keeps the latest 100 operations.
- Queued custom actions and their results are lost when Files quits or crashes.
- A crash of the Files window process also stops running transfers; they are reviewed in Recovery at the next start.
- Network folders have no Trash, refresh by polling and need GVFS FUSE to browse.
- Recent items cannot be removed or cleared from Files.
- Changes to XDG user directories apply after restarting Files.
- Hard links are copied as separate files.
