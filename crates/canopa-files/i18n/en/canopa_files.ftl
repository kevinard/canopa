# Canopa Files. English is the source language.

## Commands: toolbar buttons, menu items, shortcut names (src/interface.rs)

files-command-back = Back
files-command-forward = Forward
files-command-parent = Parent folder
files-command-enter-folder = Enter folder
files-command-open = Open selection
files-command-quick-look = Quick Look
files-command-search = Search filenames
files-command-search-subfolders = Include subfolders in search
files-command-stop-search = Stop search
files-command-more-search-results = More search results
files-command-reveal-search-result = Show in containing folder
files-command-search-problems = Unreadable search locations
files-command-location-focus = Edit location
files-command-refresh = Refresh
files-command-show-hidden = Show hidden files
files-command-select-all = Select all
files-command-new-folder = New folder
files-command-rename = Rename
files-command-trash = Move to Trash
files-command-delete-permanently = Delete permanently
files-command-show-trash = Open Trash
files-command-connect-to-server = Connect to Server…
files-command-undo = Undo
files-command-copy = Copy
files-command-cut = Cut
files-command-paste = Paste
files-command-columns = Columns view
files-command-list = List view
files-command-icons = Icons view
files-command-new-tab = New tab
files-command-new-window = New window
files-command-close-tab = Close tab
files-command-next-tab = Next tab
files-command-previous-tab = Previous tab
files-command-operations = Operations
files-command-recovery = Recovery
files-command-custom-actions = Custom actions
files-command-preferences = Preferences
files-command-bookmark-folder = Bookmark this folder

## Built-in places in the sidebar

files-place-home = Home
files-place-desktop = Desktop
files-place-documents = Documents
files-place-downloads = Downloads
files-place-pictures = Pictures
files-place-filesystem = Filesystem
files-place-trash = Trash
files-place-recovery = Recovery

## Thumbnail and icon size choices (Preferences)

files-thumbnails-local = Local files only
files-thumbnails-always = All files
files-thumbnails-never = Never
files-icon-size-small = Small
files-icon-size-medium = Medium
files-icon-size-large = Large
files-icon-size-huge = Huge

## Toolbar and location bar

# Caption beside the toolbar's search icon.
files-toolbar-search = Search
# The toolbar's overflow menu button.
files-toolbar-actions = Actions
# Narrow windows: the button that opens the sidebar's places as a menu.
files-toolbar-places = Places
# Placeholder and accessible name of the location field and breadcrumb.
files-location = Location
files-crumb-current-description = Current folder. Activate to edit the location

## Shared window chrome

files-close-window = Close window

## Browser window

# The application's name, shown at the top of the sidebar.
files-app-name = Files
# Accessible names of window regions.
files-toolbar = Toolbar
files-tabs = Tabs
files-sidebar = Sidebar
files-view = View
files-configuration-not-applied = Configuration was not applied: { $error }
files-view-preferences-not-saved = View preferences were not saved: { $error }
files-retry = Retry
files-close = Close
files-searching-filenames = Searching filenames…
files-no-matching-filenames = No matching filenames
files-empty-folder = Empty folder
files-loading-folder = Loading folder…
files-folder-unavailable = Folder unavailable
# Heading of the column that lists search results.
files-search-results = Search results
# Accessible descriptions of the current folder's list.
files-column-view = Column view
files-list-view = List view
files-icon-view = Icon view
# Captions of the view switcher.
files-view-columns = Columns
files-view-list = List
files-view-icons = Icons
# Status bar facts, joined with " · ".
files-status-items = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
files-status-results = { $count ->
    [one] { $count } result
   *[other] { $count } results
}
# $count is the number of files a search has looked at.
files-status-inspected = { $count } inspected
files-status-selected = { $count } selected
files-status-stopping = Stopping…
files-status-searching = Searching…
files-status-stopped = Stopped
files-status-more-results = More results available
# Spoken summaries of the folder state.
files-announce-items-selected = { $count ->
    [one] { $count } item, { $selected } selected
   *[other] { $count } items, { $selected } selected
}
files-announce-results = { $count ->
    [one] { $count } result
   *[other] { $count } results
}
files-announce-results-selected = { $count ->
    [one] { $count } result, { $selected } selected
   *[other] { $count } results, { $selected } selected
}
files-announce-results-stopped = { $count ->
    [one] { $count } result, stopped
   *[other] { $count } results, stopped
}
files-announce-results-stopped-selected = { $count ->
    [one] { $count } result, stopped, { $selected } selected
   *[other] { $count } results, stopped, { $selected } selected
}

## Search bar

files-search-include-subfolders = Include subfolders
files-search-stop = Stop
files-search-more = More results
files-search-reveal = Show in folder
# $count folders or files could not be read; the button shows or hides them.
files-search-unreadable-details = { $count } unreadable · Details
files-search-unreadable-hide = { $count } unreadable · Hide
# One unreadable search location and why.
files-search-failure = { $location } — { $error }
# A search failure in the folder being searched itself.
files-this-folder = This folder

## Preferences window: pages and status line

files-preferences-page-views = Views
files-preferences-page-toolbar = Toolbar
files-preferences-page-locations = Locations
files-preferences-page-shortcuts = Shortcuts
files-preferences-status-initial = Changes apply to all Files windows when saved.
files-preferences-too-many-locations = At most 256 locations are supported.
files-preferences-column-width-invalid = Column width must be a whole number from 160 to 640.
files-preferences-bookmark-path-invalid = A bookmark needs an absolute local path, ~/ path or local file URI.
# $command is a command name, $error why its shortcut is not valid.
files-preferences-shortcut-invalid = { $command }: { $error }
# $max is a number of bytes.
files-preferences-field-too-long = This field is limited to { $max } bytes.
files-preferences-location-required = The location field must remain available.
files-preferences-recording = Press a shortcut. Escape cancels; Backspace disables this command.
files-preferences-recording-cancelled = Shortcut recording cancelled.
files-preferences-wait = Wait for the configuration change to finish.
files-preferences-another-change = Another configuration change is still in progress.
files-preferences-worker-unavailable = The configuration worker is unavailable.
files-preferences-worker-stopped = The configuration worker stopped before replying.
files-preferences-saving = Saving…
files-preferences-reloading = Reloading…
files-preferences-saved = Saved. Preferences apply to all Files windows.
files-preferences-reloaded = Reloaded shared Files preferences.

## Preferences window: controls

files-preferences-subtitle = Choose how Files looks and behaves.
files-preferences-read-only = Built-in defaults are read-only. Start Files with a writable configuration to save preferences.
files-preferences-changed-outside = Configuration changed outside this editor. Your draft is kept; Reload before saving.
files-preferences-discard-question = Discard unsaved preference changes?
files-preferences-reload-question = Reload and discard unsaved preference changes?
files-preferences-keep-editing = Keep editing
files-preferences-discard = Discard
files-preferences-reload = Reload
files-preferences-save = Save preferences
# Keyboard hints under the Preferences window's status line.
files-preferences-keys = Alt+1–4 pages · Ctrl+S Save · Ctrl+R Reload
files-preferences-default-view = Default view
files-preferences-sort-by = Sort by
files-preferences-descending = Descending order
files-preferences-column-layout = Column layout
files-preferences-column-width = Column width
files-preferences-column-width-range = 160–640 pixels
files-preferences-list-details = Details in List view
files-preferences-modified-date = Modified date
files-preferences-thumbnails = Thumbnails
files-preferences-icon-size = Icon size
files-preferences-views-note = Folders inherit these settings unless they have their own view options. “Local files only” skips network shares and removable media.
# The toolbar item that is the location field.
files-preferences-location-field = Location field
# A toolbar item that runs the custom action $name.
files-preferences-toolbar-action = Action: { $name }
files-preferences-toolbar-note = Select a toolbar item to move or remove it.
files-preferences-earlier = Earlier
files-preferences-later = Later
files-preferences-remove = Remove
files-preferences-find-command-to-add = Find a command to add
# $item is a command, action or place name.
files-preferences-add-item = Add { $item }
files-preferences-toolbar-footer = The location field stays available. Explicitly placed custom actions appear once; unpositioned toolbar actions keep their separate row.
files-preferences-sidebar-auto = Automatic
files-preferences-sidebar-always = When there is room
files-preferences-sidebar-never = Hidden
files-preferences-locations-and-bookmarks = Locations and bookmarks
files-preferences-bookmark-name = Bookmark name
files-preferences-bookmark-folder = Folder (absolute path, ~/ path or local file URI)
files-preferences-add-bookmark = Add bookmark…
files-preferences-locations-footer = Missing folders remain bookmarked. Narrow tiles retain location navigation in the folder menu.
files-preferences-find-command = Find a command
# Heading of the selected command's shortcuts.
files-preferences-command-shortcuts = { $command } shortcuts
files-preferences-shortcuts-field = Shortcuts (separated by ;, empty disables)
files-preferences-record = Record…
files-preferences-use-defaults = Use defaults
files-preferences-shortcuts-footer = Selection, focus and typing keys stay available. Save checks conflicts with every command and custom action.

## File listing: columns and kinds

files-column-name = Name
files-column-size = Size
files-column-modified = Modified
files-column-kind = Kind
# Name column heading when the list is sorted by a column that is not shown.
# $field is that column's name with its sort arrow.
files-column-name-sorted-by = Name  ·  Sorted by { $field }
files-kind-link = Link
files-kind-link-to-folder = Link to folder
files-kind-folder = Folder
files-kind-file = File
# A file's kind from its extension, such as "PDF file".
files-kind-extension = { $extension } file
# Parts of a row's spoken description ("PDF file, 2.0 KiB, modified …, hidden").
files-row-modified = modified { $date }
files-row-hidden = hidden

## Window titles

# Every Files window title; $title names the window's content.
files-window-title = { $title } — Canopa Files
files-choose-folder = Choose a folder
files-recover-copy-title = Recover a copy
files-restore-to-folder-title = Restore to folder
files-transfer-details-title = Transfer details
files-custom-actions-title = Custom Actions
# A browser window searching for $query in the folder $folder.
files-title-search = Search: { $query } — { $folder }
files-title-searching = Searching: { $query } — { $folder }
files-title-search-stopping = Stopping search: { $query } — { $folder }
files-title-search-stopped = Stopped search: { $query } — { $folder }
files-operations-save-before-closing = Save the operation result before closing the last window.
