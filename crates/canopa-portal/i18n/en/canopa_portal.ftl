# Canopa Portal dialogs. English is the source language.

## Shared

# A portal dialog's window title. $title is the title the requesting app passed.
portal-window-title = { $title } — Canopa
portal-cancel = Cancel
portal-loading = Loading…
# Accessible description of a drop-down's current option, such as "2 of 5".
portal-combo-position = { $position } of { $count }

## File chooser

portal-place-home = Home
portal-place-documents = Documents
portal-place-downloads = Downloads
portal-place-pictures = Pictures
# The root folder, "/".
portal-place-filesystem = Filesystem
# Subtitle when the requesting app is not identified.
portal-chooser-purpose = Choose from your local files and folders
# Toolbar button that opens the parent folder.
portal-chooser-up = Up
portal-chooser-location = Location
# Toolbar button that opens the folder typed in the Location field.
portal-chooser-go = Go
portal-chooser-new-folder = New folder
portal-chooser-navigation = Navigation
# Heading of the side list of folder shortcuts.
portal-chooser-places = Places
# Accessible name of a row's Open button. $name is the file name.
portal-chooser-open-entry = Open { $name }
# Small button on each row.
portal-chooser-open-row = Open
portal-chooser-no-matches = No matching files
portal-chooser-loading-folder = Loading folder…
# Announced number of rows in the folder.
portal-chooser-item-count = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
# Accessible name of the file list.
portal-chooser-files = Files
# Column heading.
portal-chooser-name-column = Name
portal-chooser-search = Filter names in this folder
# Label before the file name field when saving.
portal-chooser-name-label = Name
portal-chooser-file-name = File name
# Saving several files at once; $count is the number of files.
portal-chooser-save-many = { $count ->
    [one] Choose a folder for { $count } file
   *[other] Choose a folder for { $count } files
}
portal-chooser-file-type = File type
portal-chooser-read-only = Open read-only
portal-chooser-show-hidden = Show hidden files
# Default accept buttons when the app gives none.
portal-chooser-accept-open = Open
portal-chooser-accept-save = Save
portal-chooser-folder-name = Folder name
portal-chooser-creating = Creating…
portal-chooser-create = Create
portal-chooser-overwrite-title = Replace existing files?
portal-chooser-overwrite-question = The destination already exists. Allow the application to replace it?
# Accessible description of the replacement question. $names lists up to three
# file names separated by commas; $more is how many others are not listed.
portal-chooser-overwrite-names = { $more ->
    [0] { $names }
   *[other] { $names } and { $more } more
}
# Below the first three listed names; $count is how many others exist.
portal-chooser-overwrite-more = and { $count } more
portal-chooser-keep-choosing = Keep choosing
portal-chooser-replace = Replace
# Accessible descriptions of a file row.
portal-chooser-kind-folder-link = Link to folder
portal-chooser-kind-link = Link
portal-chooser-kind-folder = Folder
portal-chooser-kind-file = File
# $extension is the file's extension in capitals, such as PDF.
portal-chooser-kind-typed-file = { $extension } file
# $kind is one of the descriptions above.
portal-chooser-kind-hidden = { $kind }, hidden
portal-chooser-choose-existing-folder = Choose an existing folder
portal-chooser-select-file = Select a file
portal-chooser-select-type = Select the requested file type
portal-chooser-one-file-name = Enter one file name without a folder path
portal-chooser-choose-folder = Choose a folder
portal-chooser-invalid-name = Invalid file name
portal-chooser-distinct-names = Save file names must be distinct
portal-chooser-special-destination = The destination is a folder or special file
portal-chooser-destination-changed = The destination folder changed. Choose it again.
portal-chooser-absolute-folder = Choose an absolute folder
portal-chooser-folder-changed = The folder location changed. Choose it again.
portal-chooser-folder-name-rule = Enter one folder name, without a slash, up to 255 bytes

## Application chooser

# Window and dialog title.
portal-app-chooser-title = Open with
# Caption of an offered application the system does not know.
portal-app-chooser-caption = Application
# $item is the file name or content type being opened.
portal-app-chooser-heading = Choose an application for { $item }
portal-app-chooser-heading-any = Choose an application for this item
portal-app-chooser-loading = Loading applications…
portal-app-chooser-none = No applications available
portal-app-chooser-no-matches = No matching applications
portal-app-chooser-count = { $count ->
    [one] { $count } application
   *[other] { $count } applications
}
portal-app-chooser-search = Search applications
portal-app-chooser-applications = Applications
portal-app-chooser-open = Open

## Access

# Title when the requesting app gives none.
portal-access-title = Permission
# Default buttons when the requesting app gives no labels.
portal-access-deny = Deny
portal-access-allow = Allow

# Who asked, when the requesting app has no ID.
portal-requesting-app = The requesting app
# $requester is the requesting app's ID.
portal-requested-by = Requested by { $requester }
# Image dimensions; $width and $height are numbers of pixels.
portal-pixel-size = { $width } × { $height } pixels
portal-share = Share

## Screenshot

portal-screenshot-window = Screenshot
portal-screenshot-color-window = Choose a color
portal-screenshot-title = Share a screenshot
portal-screenshot-color-title = Choose a color
portal-screenshot-screen-hint = All displays will be shared.
portal-screenshot-window-hint = Only this window will be shared.
portal-screenshot-area-hint = Drag to select an area. Use arrows and Space for keyboard selection.
portal-screenshot-color-hint = Click a pixel, then use the color. Arrow keys adjust it; Shift moves ten pixels.
# Mode buttons.
portal-screenshot-mode-screen = Screen
portal-screenshot-mode-area = Area
portal-screenshot-mode-window = Window (Alt+W)
# The picked pixel: $color is its hexadecimal value (#RRGGBB), $x and $y its
# coordinates and $display the display's name.
portal-screenshot-pixel = { $color }  ·  { $x }, { $y }  ·  { $display }
portal-screenshot-next-display = Next display
portal-screenshot-too-large = Choose a smaller area; this image exceeds the size limit.
portal-screenshot-select-area = Select an area to share.
portal-screenshot-use-color = Use color

# Name of a display that has none; $number counts displays from 1.
portal-display-numbered = Display { $number }

# Who asked, when the requesting app has no ID.
portal-an-application = An application

## Screen sharing and remote desktop consent

# Dialog title; $kind says what is asked.
portal-screencast-title = { $kind ->
    [screenshot] Choose a screenshot window
    [clipboard] Allow clipboard sharing
    [desktop] Allow desktop sharing
    [keyboard] Allow keyboard control
    [pointer] Allow pointer control
    [control] Allow desktop control
    [share-control] Share and allow control
   *[screen] Share your screen
}
# The confirming button; $action says what it does.
portal-screencast-action = { $action ->
    [capture-window] Capture window
    [share-clipboard] Share clipboard
    [allow-sharing] Allow sharing
    [share-screen-clipboard] Share screen and clipboard
    [share-control] Share and allow control
    [allow-control] Allow control
   *[share] Share
}
# Keyboard hint for the confirming button, in the status line.
portal-screencast-shortcut = { $action ->
    [capture-window] Ctrl+Enter to capture the window
    [share-clipboard] Ctrl+Enter to share the clipboard
    [allow-sharing] Ctrl+Enter to allow sharing
    [share-screen-clipboard] Ctrl+Enter to share the screen and clipboard
    [share-control] Ctrl+Enter to share and allow control
    [allow-control] Ctrl+Enter to allow control
   *[share] Ctrl+Enter to share
}
portal-screencast-keyboard = Allow keyboard control of your desktop
portal-screencast-keyboard-hint = Type and use keyboard shortcuts.
portal-screencast-pointer = Allow pointer control of your desktop
portal-screencast-pointer-hint = Move the pointer, click, drag and scroll.
portal-screencast-touch = Allow touch control (Alt+T)
portal-screencast-touch-hint = Touch and swipe within the screen or window you share.
portal-screencast-clipboard = Share clipboard contents (Alt+C)
portal-screencast-clipboard-hint = Read and replace copied text, images and other data.
portal-screencast-choose-display = Choose a display to share.
portal-screencast-choose-displays = Choose the displays to share.
portal-screencast-choose-window = Choose a window to share.
portal-screencast-choose-windows = Choose the windows to share.
portal-screencast-choose-source = Choose a display or window to share.
portal-screencast-choose-sources = Choose the displays or windows to share.
portal-screencast-search = Search displays and windows
# Status line parts, joined with " · ".
portal-screencast-desktop-unavailable = Desktop unavailable. Retrying…
portal-screencast-sources-unavailable = Sources unavailable. Retrying…
portal-screencast-clipboard-allowed = Clipboard sharing allowed
portal-screencast-clipboard-undecided = Choose whether to share your clipboard
portal-screencast-keyboard-allowed = Keyboard control allowed
portal-screencast-pointer-allowed = Pointer control allowed
portal-screencast-keyboard-pointer-allowed = Keyboard and pointer control allowed
portal-screencast-control-undecided = Choose whether to allow control
# $count displays or windows are selected.
portal-screencast-selected = { $count } selected
# $count are selected, out of at most $max.
portal-screencast-selected-of = { $count } selected · up to { $max }

# Offered capture sources.
portal-screencast-display = Display
# Subtitle of a display; $model is its model name.
portal-screencast-display-subtitle = Display · { $model }
portal-screencast-untitled-window = Untitled window
portal-screencast-application-window = Application window

## Sharing controls, shown while a screen, the input or the clipboard is shared

portal-sharing-title = { $kind ->
    [input-capture] Input capture
    [clipboard-sharing] Clipboard sharing
    [desktop-sharing] Desktop sharing
    [remote-control] Remote control
   *[screen-sharing] Screen sharing
}
portal-sharing-heading = { $kind ->
    [input-capture-is-allowed] Input capture is allowed
    [clipboard-sharing-is-active] Clipboard sharing is active
    [desktop-sharing-is-active] Desktop sharing is active
    [keyboard-control-is-active] Keyboard control is active
    [pointer-control-is-active] Pointer control is active
    [keyboard-and-pointer-control-are-active] Keyboard and pointer control are active
    [sharing-and-control-are-active] Sharing and control are active
   *[screen-sharing-is-active] Screen sharing is active
}
portal-sharing-stop = { $kind ->
    [stop-input-and-clipboard-sharing] Stop input and clipboard sharing
    [stop-input-capture] Stop input capture
    [stop-all-sharing] Stop all sharing
    [stop-control] Stop control
    [stop-sharing-and-control] Stop sharing and control
   *[stop-sharing] Stop sharing
}
portal-sharing-hint = { $kind ->
    [capture-clipboard] Ctrl+Alt+Esc pauses input and clipboard sharing. Stop or close ends both.
    [capture] Ctrl+Alt+Esc pauses capture. Stop or close ends this session.
    [clipboard] Esc or closing this window stops clipboard sharing
    [all] Esc or closing this window stops all sharing
    [control] Esc or closing this window stops control
    [sharing-control] Esc or closing this window stops sharing and control
   *[sharing] Esc or closing this window stops sharing
}
# What is controlled and where.
portal-sharing-control = { $kind ->
    [keyboard-input-screen-edges] Keyboard input · Screen edges
    [pointer-input-screen-edges] Pointer input · Screen edges
    [keyboard-and-pointer-input-screen-edges] Keyboard and pointer input · Screen edges
    [keyboard-control-desktop] Keyboard control · Desktop
    [pointer-control-desktop] Pointer control · Desktop
    [touch-control-shared-screen-or-window] Touch control · Shared screen or window
    [keyboard-and-touch-control] Keyboard and touch control
    [pointer-and-touch-control] Pointer and touch control
    [keyboard-pointer-and-touch-control] Keyboard, pointer and touch control
   *[keyboard-and-pointer-control-desktop] Keyboard and pointer control · Desktop
}
portal-sharing-clipboard-capture = Clipboard · Only while input is captured
portal-sharing-clipboard = Clipboard · Read and replace copied content
# $requester is the app's ID.
portal-sharing-input-to = Input may be sent to { $requester }
portal-sharing-control-to = Control granted to { $requester }
portal-sharing-with = Sharing with { $requester }

## Background activity

portal-background-window = Background activity
portal-background-question = Allow background activity?
portal-background-explanation = This app is running without open windows. Allow once applies only to its current run. Block stops the current run.
# Accessible description; $name is the app's name.
portal-background-description = { $name }. This app is running without open windows. Allow once applies only to its current run. Block stops the current run.
portal-background-closing = Closing this question keeps the current run without changing its permission.
# Buttons of the dialog and of the notification. "Allow once" and "Block" are
# also named in the explanations above.
portal-background-block = Block
portal-background-allow-once = Allow once
portal-background-allow = Allow
# Notification title; $name is the app's name.
portal-background-notification-title = { $name } is running in the background
# Notification body; $app is the app's ID.
portal-background-notification-body = Allow { $app } to keep running without open windows? Choose Allow once for its current run only.
