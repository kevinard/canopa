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

## Actions menu (toolbar menu and context menu)

files-menu-open = Open
files-menu-open-with = Open With…
files-menu-new-folder = New folder…
files-menu-rename = Rename…
files-menu-delete-permanently = Delete permanently…
files-menu-edit-custom-actions = Edit custom actions…
files-menu-preferences = Preferences…
files-menu-bookmark-folder = Bookmark this folder…
files-menu-locations = Locations…
files-menu-view-options = View options…
files-menu-other-application = Other Application…
# The first application in the Open With menu.
files-menu-default-application = { $application } (default)
# $action is a custom action's name.
files-action-not-applicable = { $action } does not apply to this selection.

## Devices: removal and menu items ($device is a device name)

files-device-power-off = Power off
files-device-eject = Eject
files-device-unmount = Unmount
files-device-open-named = Open { $device }
files-device-mount-named = Mount { $device }
files-device-power-off-named = Power off { $device }
files-device-eject-named = Eject { $device }
files-device-unmount-named = Unmount { $device }

## Devices: sidebar and requests

files-devices = Devices
files-devices-unavailable = Devices are unavailable.
files-devices-error = Devices: { $error }
files-cancel = Cancel
files-review = Review
files-open-folder = Open folder
files-device-cancelling = Cancelling…
files-device-needs-decision = Needs a decision
files-device-mounting = Mounting…
files-device-powering-off = Powering off…
files-device-ejecting = Ejecting…
files-device-unmounting = Unmounting…
files-device-connecting = Connecting…
files-device-mounted = Mounted
files-device-connected = Connected
files-device-connected-remote = Connected without a local folder: GVFS’s FUSE daemon (gvfsd-fuse) is not running, so only a read-only listing is available
files-device-removed = Removed
files-device-cancelled = Cancelled
files-device-failed = Could not finish: { $error }
files-device-finished = Finished

## New folder and Rename dialog

# The name a new folder starts with.
files-new-folder-name = New folder
files-create = Create
files-name-full-name = Full name
files-name-folder-name = Folder name
files-name-invalid = Enter one name, without a slash. The names . and .. are reserved.
files-name-dialog-failed = Could not open the name dialog.
files-rename-worker-stopped = The operation worker stopped before preparing Rename.
files-operation-queue-unavailable = The operation queue is unavailable.
files-accepting-operation = Accepting operation…
# A folder inside the home folder, such as "Home / Documents / Work".
files-home-path = Home / { $path }

## Permanent deletion confirmation

files-delete-title = Delete permanently?
files-delete-title-empty-trash = Empty Trash?
files-delete-title-no-trash = Can't move to Trash
files-delete-one = Permanently delete “{ $name }”? This can't be undone.
files-delete-many = { $count ->
    [one] Permanently delete { $count } item? This can't be undone.
   *[other] Permanently delete { $count } items? This can't be undone.
}
files-delete-no-trash-one = “{ $name }” is on a volume without a usable Trash, so it cannot be moved to Trash. Delete it permanently instead? This can't be undone.
files-delete-no-trash-many = { $count ->
    [one] { $count } item is on a volume without a usable Trash, so it cannot be moved to Trash. Delete it permanently instead? This can't be undone.
   *[other] { $count } items are on a volume without a usable Trash, so they cannot be moved to Trash. Delete them permanently instead? This can't be undone.
}
files-delete-empty-trash = { $count ->
    [one] Permanently delete all { $count } item in Trash? This can't be undone.
   *[other] Permanently delete all { $count } items in Trash? This can't be undone.
}
files-delete-empty-main-trash = { $count ->
    [one] Permanently delete all { $count } item in Main Trash? This can't be undone.
   *[other] Permanently delete all { $count } items in Main Trash? This can't be undone.
}
files-delete-from-trash-one = Permanently delete “{ $name }” from Trash? This can't be undone.
files-delete-from-trash-many = { $count ->
    [one] Permanently delete { $count } item from Trash? This can't be undone.
   *[other] Permanently delete { $count } items from Trash? This can't be undone.
}
files-empty-trash = Empty Trash
files-delete-items = { $count ->
    [one] Delete { $count } item
   *[other] Delete { $count } items
}
files-confirmation-failed = Could not open the confirmation.

## Dates

# A date and time with seconds: "Nov 11, 2026 · 2:30:05 PM".
files-date-time = { $date } · { $time }
files-unknown = Unknown

## Operation questions: conflicts and Undo decisions

files-question-undo = Undo needs a decision
files-question-recovery = Recovery copy needs a decision
files-question-folder-conflict = Folder conflict
files-question-file-conflict = File conflict
files-question-destination-unavailable = Destination unavailable
files-question-failed = Could not open the operation question. Review the operation to try again.
files-conflict-folder-exists = A folder named “{ $name }” already exists in the destination.
files-conflict-item-exists = An item named “{ $name }” already exists in the destination.
files-conflict-merge-explanation = Merge combines these folders and asks about individual collisions. Keep both renames the incoming folder. Skip leaves it untouched.
files-conflict-recovery-explanation = “Keep both” copies the saved item under a new name. “Skip” leaves both items untouched.
files-conflict-replace-explanation = Replace saves the existing file for Undo. Keep both renames the incoming file. Skip leaves both untouched.
files-conflict-kind-explanation = This name is already used by a different kind of item. Keep both gives the incoming item another name. Skip leaves both items untouched.
files-conflict-saved-version = Saved version in Recovery
files-conflict-incoming = Incoming
files-conflict-existing = Existing
files-conflict-replace = Replace
files-conflict-merge = Merge
files-conflict-keep-both = Keep both
files-conflict-skip = Skip
files-conflict-apply-folders = Apply to remaining folder conflicts
files-conflict-apply-files = Apply to remaining file conflicts
files-transfer-paused = The transfer is paused
files-transfer-unavailable-explanation = Restore access to the destination, then choose Retry. Cancel keeps the unfinished source files.
files-destination = Destination
# Technical details of an error, from the system.
files-system-details = System details · { $details }
files-kind-special = Special item
# A file's kind, size in bytes and modification date, on two lines.
files-entry-metadata = { $bytes ->
    [one] { $kind } · { $bytes } byte
   *[other] { $kind } · { $bytes } bytes
}
    Modified { $modified }
files-entry-metadata-folder = { $kind }
    Modified { $modified }
files-undo-keep-item = Keep item
files-undo-finish-step = Finish step
files-undo-move-remaining = Move remaining items
files-undo-keep-remaining = Keep remaining items
files-undo-finish-merge = Finish this Merge
files-undo-move-to-recovery = Move changed item to Recovery
files-undo-use-saved-version = Use saved version
files-undo-return-item = Return item
files-undo-restore-previous = Restore previous destination
files-undo-restoring-from-trash = Restoring from Trash
files-undo-saved-previous = Saved previous destination
files-undo-existing-item = Existing item
files-undo-unavailable = Unavailable · { $error }
files-undo-destination = Destination · { $location }
files-undo-return-to = Return to · { $location }
files-undo-explain-current = Undo would return this item to a name that is already in use. Review both versions before choosing. Replace retains the displaced item in Recovery; Keep both returns under a new name. Skip leaves this step available for later.
files-undo-explain-trash = An item from Trash needs to return to a name that is already in use. Review both versions. Keep both restores it under another name; Skip keeps it available for later.
files-undo-explain-previous-destination = Undo is restoring a saved previous destination, but its original name is in use. Review both versions. Replacements remain protected in Recovery.
files-undo-explain-nonempty-folder = This created folder now contains later work. Keep item leaves the folder and its contents in place and finishes its creation step. Skip keeps the step available for later.
files-undo-explain-changed-created-folder = The created folder has changed, moved or been replaced. Keep item leaves the current item untouched and finishes this creation step. Skip lets you decide later.
files-undo-explain-unavailable-created-folder = The created folder cannot be inspected. Restore its location or access before trying again. Finish step leaves files untouched and gives up this removal step; Skip keeps it for later.
files-undo-explain-merge-remainder = Items remain in the merged destination. Move remaining items continues the return while handling new conflicts. Keep remaining items leaves them here and finishes this branch. Skip lets you decide later.
files-undo-explain-merge = A folder used by this partial Merge has moved, changed or cannot be inspected. Restore its original location or access to continue. Finish this Merge leaves files in place and preserves saved previous destinations in Recovery. Already returned items stay where they are.
files-undo-explain-changed-copy = This copied item has changed since the copy. Move changed item to Recovery preserves its newer contents before undoing the copy. Keep item leaves it here and finishes its copy step; Skip lets you decide later.
files-undo-explain-changed-retained = The saved version has changed. Review the saved and current items before choosing to use it. Keep item preserves the current item and finishes this step, retaining protected versions in Recovery.
files-undo-explain-changed-move = The moved item or its destination has changed. Return item returns the reviewed current item and preserves any saved previous destination. Keep item leaves the current item in place and finishes this step. Skip keeps it for later.
files-undo-explain-unavailable-move = The moved item cannot be inspected. Restore its location or access before continuing. Restore previous destination returns only that saved version when available. Finish step leaves the moved item untouched and preserves protected data in Recovery.
files-undo-explain-unavailable-retained = A saved previous version cannot be inspected. Restore its location or access to retry. Return item performs only the available return when offered. Finish step leaves current files untouched and retains protected saved data in Recovery.

## Operation queue messages

files-jobs-admission-full = The operation admission queue is full while device removal is in progress. Try again after removal finishes.
files-jobs-cancelled-before-start = Cancelled before starting
files-jobs-startup-stopped = Operation startup was stopped.
files-jobs-queue-full = The operation queue is unavailable or full. Try again.
files-jobs-volume-worker-stopped = The volume worker stopped before its first observation.
files-jobs-recovery-busy = The Recovery reader is unavailable or busy. Try again.
files-jobs-interrupted-needs-review = Interrupted work needs Recovery review before more changes.
files-jobs-recovery-cancelled-removal = Recovery inspection cancelled while waiting for device removal.
files-jobs-drag-cancelled-removal = Drag preparation cancelled while waiting for device removal.
files-jobs-close-worker-busy = Finish or cancel active work, resume or abandon stopped transfers, and save every result before closing the operation worker.
files-jobs-needs-reconciliation = An operation needs reconciliation before more changes.
files-jobs-kept-in-recovery = Entry kept in Recovery. The interrupted work is preserved; recover a copy when needed.
files-jobs-retry-interrupted = Retry interrupted operation
files-jobs-journals-rechecked = Interrupted operation journals rechecked.
files-jobs-worker-stopped = The operation worker stopped unexpectedly
files-jobs-reconciliation-retained = journal reconciliation completed; recorded result retained
# $operation is an operation number, $steps a count of completed steps.
files-jobs-interrupted-steps = Filesystem operation #{ $operation }: { $steps } confirmed steps remain in history. Review its completed work before starting a new request; this saved request will not run again.
files-jobs-interrupted-journal = Filesystem operation #{ $operation }: { $steps } confirmed steps remain in history; its journal still needs Recovery review. Review its completed work before starting a new request; this saved request will not run again.
files-jobs-interrupted-no-history = Filesystem operation #{ $operation }: { $steps } confirmed steps remain in history; its Undo history is no longer available. Review its completed work before starting a new request; this saved request will not run again.
files-jobs-some-work-cannot-continue = Some saved work cannot yet continue: { $error }
files-jobs-transfer-cannot-continue = Saved transfer cannot yet continue: { $error }
files-jobs-reconciliation-not-saved = Reconciliation result not saved: { $error }
files-jobs-result-not-saved = Result not saved: { $error }
files-jobs-save-failed = Could not save operation: { $error }
# What an Undo did, as counts.
files-undo-outcome = { $reversed } reversed · { $kept } kept · { $skipped } skipped · { $dismissed } steps dismissed · { $unrestored } previous destinations not restored · { $unremoved } items not removed · { $remaining } remaining
files-undo-outcome-cancelled = { $outcome } · cancelled

## Operation titles ($name is a file name, $destination a folder name)

files-job-keep-in-recovery = Keep { $name } in Recovery
files-job-recover = Recover { $name }
files-job-create-folder = Create folder { $name }
files-job-rename = Rename { $source } to { $destination }
files-job-restore = Restore { $name }
files-job-trash = { $count ->
    [one] Move { $count } item to Trash
   *[other] Move { $count } items to Trash
}
files-job-delete = { $count ->
    [one] Delete { $count } item permanently
   *[other] Delete { $count } items permanently
}
files-job-empty-trash = { $count ->
    [one] Empty Trash ({ $count } item)
   *[other] Empty Trash ({ $count } items)
}
files-job-delete-from-trash = { $count ->
    [one] Delete { $count } item from Trash permanently
   *[other] Delete { $count } items from Trash permanently
}
files-job-copy = { $count ->
    [one] Copy { $count } item to { $destination }
   *[other] Copy { $count } items to { $destination }
}
files-job-move = { $count ->
    [one] Move { $count } item to { $destination }
   *[other] Move { $count } items to { $destination }
}

## Operation history records (src/history_labels.rs)
## Lower-case phrases that name a recorded operation; $operation is one of them.

files-history-undo = Undo { $operation }
files-history-interrupted = Interrupted { $operation }
files-history-preserved-undo = Preserved by Undo { $operation }
files-history-preserved-kept-item = Preserved by keeping an item during Undo { $operation }
files-history-preserved-unavailable-item = Preserved after finishing an unavailable item during Undo { $operation }
files-history-preserved-unavailable-saved = Preserved after finishing an unavailable saved version during Undo { $operation }
files-history-operation = operation
files-history-restore-permissions = Restore folder permissions
files-history-kept-undo-folder = Kept interrupted Undo work folder
files-history-create-folder = create folder { $name }
files-history-rename = rename { $source } to { $destination }
files-history-copy = copy to { $destination }
files-history-move = move to { $destination }
files-history-trash = { $count ->
    [one] move { $count } item to Trash
   *[other] move { $count } items to Trash
}
files-history-restore = restore { $name }
files-history-recover = recover { $name }

## Desktop notifications

files-notifications-unavailable = Desktop notifications are unavailable.
files-notification-timed-out = Desktop notification timed out.
files-notification-not-sent = Desktop notification could not be sent.
files-notifications-busy = Desktop notifications are busy or unavailable.
# The application name the notification server shows.
files-app-full-name = Canopa Files
files-notification-decision = Operation needs a decision
files-notification-worker-stopped = Desktop notification worker stopped before replying.
files-completed-detail = Completed.
# Body of the notification for an operation that waits for a decision.
files-notification-decision-body = { $operation } · Open Operations to review.

## File operations window

files-recovery-reader-stopped = The Recovery reader stopped before replying.
files-operation-queue-unavailable-short = The operation queue is unavailable
files-operation-worker-stopped-acceptance = The operation worker stopped before confirming acceptance.
files-abandon-not-saved = The operation queue stopped before abandonment was saved.
files-dismissal-not-saved = The operation queue stopped before dismissal was saved.
files-history-heading = File history
files-history-latest = Latest
files-history-older = Older
files-history-preparing = Preparing file operations…
files-history-empty = No file operations yet
files-history-empty-description = File changes made in Files will appear here.
files-history-moved-to-trash = Moved to Trash
files-history-renamed = Renamed
files-history-created-folder = Created folder
files-details = Details
files-retry-save = Retry Save
files-retry-failed-items = Retry failed items
files-continue-remaining = Continue remaining work
files-abandon-unfinished = Abandon unfinished work
files-dismiss-request = Dismiss request
files-dismiss-request-description = Dismiss releases this saved request. Completed work and Undo stay available.
files-retry-failed-recovery = Retry failed Recovery copies
files-continue-recovery = Continue Recovery copy
files-phase-completed = Completed
files-recent-activity = Recent activity
files-phase-queued = Queued
files-phase-running = Running
files-phase-awaiting-review = Not started · review needed
files-phase-interrupted = Interrupted · review needed
files-phase-failed = Could not finish
files-phase-stopped = Stopped · review needed
files-phase-start-save-failed = Not started · could not save
files-phase-save-failed = Result not saved
# $status is a phase such as "Running", $operation the operation's name.
files-operation-summary = { $status } · { $operation }
files-operation-announcement = { $operation }: { $status }
files-operation-announcement-error = { $operation }: { $status }. { $error }
files-history-renamed-to = Renamed to { $name }
files-history-previously = Previously { $name }
files-history-accepted = Request accepted { $date }
files-history-count = { $count ->
    [one] { $count } operation
   *[other] { $count } operations
}
files-history-count-latest-first = { $count ->
    [one] { $count } operation  ·  Latest first
   *[other] { $count } operations  ·  Latest first
}

## Transfer details and progress

files-details-completed = Completed destinations ({ $count })
files-details-failed = Failed items ({ $count })
files-details-skipped = Skipped items ({ $count })
files-details-remaining = Remaining items ({ $count })
files-details-sources = Source folders to review ({ $count })
files-details-completed-short = Completed ({ $count })
files-details-failed-short = Failed ({ $count })
files-details-skipped-short = Skipped ({ $count })
files-details-remaining-short = Remaining ({ $count })
files-details-sources-short = Sources ({ $count })
# A failed item: its source path, destination path and the error, on separate lines.
files-details-failure = Source
    { $source }
    Destination
    { $destination }
    { $error }
# Appended to a label cut to its maximum length.
files-details-truncated = … (truncated)
files-details-failed-to-open = Could not open transfer details. Try Details again in Operations.
files-review-decision = Review decision
files-details-section-empty = No items in this section.
# A numbered item of a details list.
files-details-entry = { $number }. { $entry }
files-details-page = Items { $first }–{ $last } of { $count }
files-details-gone = This operation is no longer available.
files-previous = Previous
files-next = Next
# $name is the file being transferred.
files-progress-paused-at = Paused at { $name }
files-progress-checking = Checking { $name }
files-progress-transferring = Transferring { $name }
# $bytes and $total_bytes are sizes such as "2.0 KiB".
files-progress-counts = { $total ->
    [one] { $done } of { $total } item · { $bytes } of { $total_bytes }
   *[other] { $done } of { $total } items · { $bytes } of { $total_bytes }
}
files-progress-counting = { $total ->
    [one] { $done } of { $total }+ item · { $bytes } · counting…
   *[other] { $done } of { $total }+ items · { $bytes } · counting…
}
files-progress-running-counts = { $done ->
    [one] { $done } item · { $bytes }
   *[other] { $done } items · { $bytes }
}
# A transfer speed; $size is a size such as "12.0 MiB".
files-progress-rate = { $size }/s
# $duration is a duration such as "2 min".
files-progress-time-left = about { $duration } left
files-progress-few-seconds-left = about a few seconds left
files-progress-rate-and-time = { $rate } · { $left }

## Operations window and custom commands

files-phase-waiting-for-device = Waiting for device removal…
files-output-stdout = Standard output
files-output-stderr = Standard error
files-output-empty = (empty)
files-output-truncated = [Output was truncated]
files-actions-runtime-unavailable = The custom-action runtime is unavailable.
files-actions-runtime-stopped-admission = The custom-action runtime stopped before replying to admission.
files-new-browser = New browser
files-operations-subtitle = File changes, transfers, and command results
files-custom-commands = Custom commands
files-latest-commands = Latest commands
files-older-commands = Older commands
files-hide-output = Hide output
files-show-output = Show output
files-previous-page = Previous page
files-next-page = Next page
files-copy-all-output = Copy all output

## Custom action commands

files-action-folder-changed = The action's original folder changed. Return to that folder and invoke the action again.
files-action-command-terminated = Command terminated
files-action-runtime-stopped = The custom-action runtime has stopped.
files-action-queue-full = The custom-action queue is unavailable or full. Try again.
files-action-close-busy = Finish or cancel accepted commands and hand off completion notices before stopping the custom-action runtime.
files-action-invalid-request = A custom action requires absolute selected paths and its original working-folder binding.
files-action-too-many = Too many unfinished commands or undelivered results. Finish or cancel work and collect its completion notices before starting another.
files-action-sequence-exhausted = The custom-action sequence is exhausted.
files-action-worker-stopped = The command worker stopped unexpectedly.
# Heading of a command's output stream, followed by its text.
files-output-heading = { $stream }:
files-custom-actions-error = Custom actions: { $error }
# A custom command's folder and how many items it was given.
files-command-job-folder = { $folder }  ·  { $count } selected
# $code is a process exit code.
files-action-exit-code = Failed · exit code { $code }
files-action-could-not-run = Could not run: { $error }

## Properties window

files-properties = Properties
files-properties-title-reading = Properties — Reading…
files-properties-title = { $count ->
    [one] Properties — { $count } item
   *[other] Properties — { $count } items
}
files-properties-reading = Reading properties…
files-properties-cancelled = Properties inspection cancelled.
files-properties-copy-location = Copy location
files-properties-copy-locations = Copy locations
files-properties-link-target = Link target
files-properties-access = Access
files-properties-owner = Owner
files-properties-group = Group
files-properties-others = Others
# $who is Owner or Group, $identity the user or group name.
files-properties-access-identity = { $who } · { $identity }
# $mode is an octal mode such as 0644.
files-properties-mode = Mode { $mode } · UID { $uid } · GID { $gid }
files-unavailable = Unavailable
files-kind-symlink = Symbolic link
files-kind-special-file = Special file
files-size-bytes = { $bytes ->
    [one] { $bytes } byte
   *[other] { $bytes } bytes
}
files-date-out-of-range = Outside supported date range
files-permission-none = none
files-permission-x = execute
files-permission-w = write
files-permission-wx = write and execute
files-permission-r = read
files-permission-rx = read and execute
files-permission-rw = read and write
files-permission-rwx = read, write, and execute

## Trash window and Restore to folder

files-trash-restore = Restore
files-trash-restore-to = Restore to…
files-trash-copy = Copy to clipboard
files-trash-empty = Empty Trash…
files-trash-all-locations = All locations
files-trash-main = Main Trash
files-trash-original-unknown = Original location unknown
# $date is when the item was moved to Trash.
files-trash-deleted = Deleted { $date }
files-trash-date-unknown = Deletion date unknown
files-trash-item-description = { $location }, { $date }
files-trash-items = Trash items
files-trash-items-heading = Items in Trash
files-trash-loading = Loading Trash…
files-trash-original-unavailable = Original information is unavailable. Copy this item to recover its contents.
files-trash-count-select = { $count ->
    [one] { $count } item · Select an item to see restore options
   *[other] { $count } items · Select an item to see restore options
}
files-select-item-for-restore = Select an item to see restore options
files-trash-empty-title = Trash is empty
files-trash-empty-description = Items moved to Trash will appear here.
files-trash-subtitle = Restore deleted files to their original location or another folder, or delete them permanently.
files-restore-dialog-failed = Could not open the Restore destination dialog.
files-operation-queue-not-ready = The operation queue is not ready. Try again.
files-enter-destination-folder = Enter a local destination folder.
files-folder-path = Folder path
files-location-line = Location · { $location }
files-destination-folder = Destination folder
files-restore-name-taken = If this name is already taken, the item stays in Trash.
files-restore-here = Restore here

## Recovery window, Recover a copy and the folder chooser

files-recovery-recover = Recover a copy…
files-recovery-keep = Keep in Recovery
files-recovery-load-more = Load more
files-recovery-reconcile = Reconcile interrupted work
files-recovery-view-operations = View Operations
files-recovery-held = Held for Undo
files-recovery-held-modified = Modified { $date }  ·  Held for Undo
files-recovery-saved-copies = Saved copies
files-recovery-loading = Loading Recovery…
files-recovery-count = { $count ->
    [one] { $count } retained version · Recovery creates a copy and keeps the saved version.
   *[other] { $count } retained versions · Recovery creates a copy and keeps the saved version.
}
files-recovery-count-loaded = { $count ->
    [one] { $count } retained version loaded · Recovery creates a copy and keeps the saved version.
   *[other] { $count } retained versions loaded · Recovery creates a copy and keeps the saved version.
}
files-recovery-empty = No saved versions
files-recovery-empty-description = Saved versions from file replacements appear here. Operations shows your recent file changes.
files-recovery-subtitle-interrupted = Saved versions and interrupted operations
files-recovery-subtitle = Recover a copy of an earlier version.
files-recovery-saved-versions = Saved versions
files-recovery-select = Select a version to choose where to recover it
# An interrupted operation and why it stopped.
files-recovery-interrupted-notice = { $operation }: { $message }
files-recovery-destination-invalid = Enter a local destination folder and a valid filename.
files-recovery-name-invalid = Enter a filename, or a full local path or file URI.
files-recovery-no-copy = No copy was made. Review the destination and try again.
files-recovery-dialog-failed = Could not open the Recovery destination dialog.
files-recovery-keep-name = Keep the saved filename
files-file-name = File name
files-recovery-new-name = New filename (optional)
files-recovery-submitting = Submitting Recovery copy…
files-recovery-in-progress = Copy request in progress. Closing this window keeps accepted work running.
files-recovery-hint = ~ is your Home folder. Leave the filename blank to keep the saved name.
files-recovery-recover-here = Recover here
files-destination-up = Up
files-destination-no-subfolders = No subfolders
files-destination-reading = Reading folders…
files-destination-use = Use this folder
files-browse = Browse…

## Open With and launch confirmation ($name is a file name)

files-open-could-not-open = Could not open “{ $name }”: { $error }
files-open-default-failed = Could not make { $application } the default: { $error }
# $name is an application or file name.
files-open-start-failed = Could not start { $name }: { $error }
files-open-start-file-failed = Could not start “{ $name }”: { $error }
files-open-link-failed = Could not open the link: { $error }
files-open-run-failed = Could not run “{ $name }”: { $error }
files-open-terminal-missing = Running in a terminal needs xdg-terminal-exec; install it to choose your terminal.
files-open-failed = Opening the file failed: { $error }
files-open-with-title = Open With
files-open-run-script-title = Run this script?
files-open-run-program-title = Run this program?
files-open-untrusted-title = Untrusted application launcher
files-open-choose-one = Choose an application for “{ $name }”.
files-open-choose-many = Choose an application for { $count } files.
files-open-script-message = “{ $name }” is an executable script. Run it only if you trust where it came from.
files-open-program-message = “{ $name }” is an executable program. Run it only if you trust where it came from.
# $application is the launcher's name, $file its file name.
files-open-untrusted-message = “{ $application }” ({ $file }) is not installed in an applications folder and is not marked as executable. Launch it only if you trust where it came from. Marking the file executable makes Files trust it.
files-open-file-failed = Could not open the file.
files-open-list-failed = Applications could not be listed.
files-open-interrupted = The launch was interrupted.
files-open-dialog-failed = Could not open the dialog.
files-open-search = Search applications
files-open-looking = Looking for applications…
files-open-no-match = No matching applications
files-open-recommended = Recommended
files-open-other = Other applications
files-open-applications = Applications
# $mime is a file type such as text/plain.
files-open-set-default = Set as default for { $mime }
files-open-in = Open in { $application }
files-open-run-in-terminal = Run in Terminal
files-open-run = Run
files-open-as-text = Open as Text
files-open-launch = Launch

## Device and server questions (passwords, choices)

files-device-q-connect = Connect to server
files-device-q-unlock = Unlock device
files-device-q-busy = Device is busy
files-device-q-host-identity = Check the server's identity
files-device-q-server = Server question
files-device-q-device = Device question
files-device-q-failed = Could not open the device question. Review it in Operations to try again.
files-device-q-too-long = The field exceeds its input limit.
files-device-q-pim-invalid = Enter a whole number for PIM.
files-device-q-username = Username
files-device-q-domain = Domain
files-device-q-password = Password
# PIM: the VeraCrypt personal iterations multiplier.
files-device-q-pim = PIM (optional)
files-device-q-anonymous = Connect anonymously
files-device-q-hidden = Hidden volume
files-device-q-system = System volume
files-device-q-remembered = A remembered password is kept in your keyring by GVFS, not by Files.
files-device-q-this-request = Files uses these credentials for this request only.
files-device-q-unlock-button = Unlock
files-device-q-close-app = Close the application before retrying.
files-device-q-host-explanation = Continue only if you expect to connect to this server, for example on your own network. SSH remembers its identity for next time; if it changes later, Files cannot connect until it is checked.
files-remember-never = Forget the password immediately
files-remember-session = Remember it until you log out
files-remember-keyring = Remember it in the keyring

## Network: sidebar section and Connect to Server

files-network = Network
files-network-title = Connect to Server
files-network-bookmarks-failed = Could not update the GTK bookmarks: { $error }
files-network-wait-for-folder = Wait for the folder to load, then connect again.
files-network-unavailable = Network locations are unavailable.
files-network-dialog-failed = Could not open Connect to Server.
files-network-window-closed = Its Files window was closed.
files-network-open-failed = Could not open the file: { $error }
files-network-location = Network location
files-network-phone = Phone
files-network-camera = Camera
files-network-disconnect = Disconnect { $device }
files-network-remove-bookmark = Remove bookmark { $name }
files-network-other-locations = Other Locations…
files-network-address = Server address
files-network-address-hint = smb://, sftp://, ftp://, ftps://, dav://, davs:// or nfs://. Files asks for a password when the server needs one.
files-network-add-bookmark = Add to bookmarks
files-network-bookmarked = Bookmarked servers
# A server's name and address.
files-network-server = { $name } — { $uri }
files-network-on-this-network = On this network
files-network-looking = Looking…
files-network-no-servers = No servers were found. Enter an address above.
files-network-empty = This location is empty.
files-network-truncated = Only the first entries are shown.
files-network-connect = Connect
