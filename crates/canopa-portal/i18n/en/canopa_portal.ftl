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

# Who asked, when the requesting app has no ID.
portal-requested-by-application = Requested by an application

## Print

portal-print-title = Print
portal-print-save-title = Save printed document
# Default name of a document without a title, also used as a file name.
portal-print-document = Document
# The confirming button when the app gives no label.
portal-print-accept = Print
portal-print-preparing-picker = Preparing destination picker…
# Destination that saves to a file instead of a printer.
portal-print-to-file = Print to file
portal-print-choose-destination = Choose a destination
portal-print-destination = Destination
portal-print-refresh = Refresh
portal-print-loading = Loading printer information…
portal-print-no-printers = No printers are available. Configure a printer, then refresh.
portal-print-not-accepting = This printer is not accepting jobs.
portal-print-stopped = This printer is stopped. Submitted jobs may wait in its queue.
portal-print-busy = This printer is busy. Submitted jobs will be queued.
portal-print-idle = Print jobs are sent to this printer’s queue.
portal-print-printer-default = Printer default
# A paper size offered by the printer: $name is its IPP name (such as
# iso_a4_210x297mm), $width and $height are in millimetres.
portal-print-media = { $name } — { $width } × { $height } mm
portal-print-one-sided = One-sided
portal-print-two-sided-long = Two-sided, long edge
portal-print-two-sided-short = Two-sided, short edge
portal-print-color = Color
portal-print-monochrome = Monochrome
portal-print-custom-paper = Custom paper size
portal-print-width = Width (mm)
portal-print-height = Height (mm)
portal-print-check-paper = Check paper size
# Margins in millimetres the printer reported for a custom paper size.
portal-print-paper-confirmed = Paper confirmed. Margins (mm): top { $top }, bottom { $bottom }, left { $left }, right { $right }.
portal-print-custom-paper-hint = Enter millimetres with up to two decimal places, then check this size with the printer.
portal-print-paper = Paper
portal-print-sides = Sides
portal-print-check-settings = Check printer settings
portal-print-copies-range = Printer supports { $minimum }–{ $maximum } copies.
portal-print-copies-unknown = The printer did not report a supported copy count. Refresh to retry.
portal-print-file-format = File format
portal-print-choose-file-hint = Choose where to save the printed document.
portal-print-choose-file = Choose file…
# US paper sizes.
portal-print-paper-letter = Letter
portal-print-paper-legal = Legal
# Placeholder of the paper list when the dimensions match no listed size.
portal-print-custom-size = Custom size
portal-print-margin-top = Top margin (mm)
portal-print-margin-bottom = Bottom margin (mm)
portal-print-margin-left = Left margin (mm)
portal-print-margin-right = Right margin (mm)
portal-print-all-pages = All pages
portal-print-page-ranges = Page ranges
portal-print-current-page = Current page
portal-print-selection = Selection
portal-print-pages = Pages
portal-print-page-ranges-field = Page ranges (for example 1-3, 5)
portal-print-copies = Copies
portal-print-scale = Scale (%)
portal-print-collate = Collate copies
portal-print-reverse = Reverse order
portal-print-orientation = Orientation
portal-print-portrait = Portrait
portal-print-landscape = Landscape
portal-print-reverse-portrait = Reverse portrait
portal-print-reverse-landscape = Reverse landscape
# Which pages of the document are printed.
portal-print-page-set = Page set
portal-print-page-set-all = All
portal-print-even-pages = Even pages
portal-print-odd-pages = Odd pages
portal-print-pages-per-sheet = Pages per sheet
# Order of pages on a sheet with several pages per sheet.
portal-print-arrangement = Page arrangement
portal-print-arrangement-order = { $order ->
    [lrbt] Left to right, bottom to top
    [rltb] Right to left, top to bottom
    [rlbt] Right to left, bottom to top
    [tblr] Top to bottom, left to right
    [tbrl] Top to bottom, right to left
    [btlr] Bottom to top, left to right
    [btrl] Bottom to top, right to left
   *[lrtb] Left to right, top to bottom
}
portal-print-check-values = Check the copies, scale, page ranges and paper dimensions.
portal-print-format-unavailable = The previous file format is unavailable. Review the output format.
portal-print-printer-unavailable = The previous printer is unavailable. Choose a destination.
portal-print-preferences-unavailable = Some application print preferences are unavailable. Review the current printer settings.
portal-print-settings-changed = Printer settings changed. Check them again.
portal-print-information-changed = Printer information changed. Refresh the printer list.
portal-print-settings-unconfirmed = Couldn’t confirm these printer settings together. Change the paper or device options, then check again.
portal-print-information-failed = Couldn’t load printer information. Retry or save to a file.
# Accept button of the file picker opened from the print dialog.
portal-print-select-destination = Select destination
# File type filter; $format is the format's name in capitals, such as PDF.
portal-print-format-filter = { $format } document
portal-print-picker-failed = Couldn’t open the destination picker.
# Printer settings.
portal-print-paper-source = Paper source
portal-print-paper-type = Paper type
portal-print-quality = Print quality
portal-print-resolution = Resolution

portal-close = Close

## Print recovery: retained print files and unresolved printer jobs

portal-recovery-title = Print recovery
portal-recovery-status-busy = Still in use
portal-recovery-status-missing = Folder unavailable
portal-recovery-status-incomplete = Incomplete copy
portal-recovery-status-interrupted = Interrupted
portal-recovery-status-uncertain = Result unconfirmed
portal-recovery-status-published = Previously published
portal-recovery-status-unavailable = Couldn’t check files
portal-recovery-explain-busy = A print process may still be using these files. Refresh after it exits.
portal-recovery-explain-missing = The retained folder may have moved or become unavailable. Its last known location is shown below.
portal-recovery-explain-incomplete = A complete document could not be established. You can inspect any retained files in their folder.
portal-recovery-explain-interrupted = Printing was interrupted. You can save a copy of the retained data to a new destination.
portal-recovery-explain-uncertain = The print result could not be confirmed. Check the original destination before saving another copy.
portal-recovery-explain-published = The document was previously published. You can review the retained data and save another copy if needed.
portal-recovery-explain-unavailable = The recorded files changed or could not be checked. You can inspect their last known folder.
portal-recovery-preparing = Preparing recovered copy…
portal-recovery-files-stay = Retained files will stay in place.
portal-recovery-saving = Saving recovered copy…
portal-recovery-saving-hint = Retained files will stay in place. If you close this window, check your chosen destination before trying again.
portal-recovery-checking = Checking printer jobs…
portal-recovery-checking-hint = Only identified jobs without confirmed acceptance may be cancelled. Documents are never sent again.
portal-recovery-removing = Removing printer record…
portal-recovery-removing-hint = The printer job will be left unchanged.
# Title of a printer record whose document has no title.
portal-recovery-printed-document = Printed document
# Name of a retained file without a name.
portal-recovery-printed-file = Printed file
portal-recovery-remove-printer-question = Remove this printer record?
portal-recovery-remove-printer-explain = Check your printer before printing another copy. This removes only Canopa’s record; it does not cancel the printer job or send the document again.
portal-recovery-remove-printer-effect = Canopa will stop checking this record automatically. Removing it frees one place in the recovery list.
portal-recovery-keep = Keep in list
portal-recovery-remove-record = Remove record
portal-recovery-remove-record-ellipsis = Remove record…
portal-recovery-remove-work-question = Remove this work from the list?
portal-recovery-remove-work-explain = Its files will stay in place. Keep the folder location if you want to find them later.
portal-recovery-remove-from-list = Remove from list
portal-recovery-remove-from-list-ellipsis = Remove from list…
portal-recovery-original-destination = Original destination
portal-recovery-save-document = Save document copy…
portal-recovery-save-displaced = Save file moved aside…
portal-recovery-displaced-hint = A file moved aside may contain unrelated data. Saving a copy leaves it in place.
# $size is the largest copy that can be saved, such as "128 MiB".
portal-recovery-size-limit = Copies up to { $size } can be saved here. Use the folder for other files.
portal-recovery-retained-folder = Retained folder
portal-recovery-open-folder = Open folder
portal-recovery-loading = Loading retained work…
portal-recovery-unavailable = Retained work unavailable
portal-recovery-empty = No retained print work
portal-recovery-empty-hint = Interrupted print files will appear here when they can be found.
# Tabs with the number of entries in each.
portal-recovery-files-tab = Files ({ $count })
portal-recovery-printers-tab = Printers ({ $count })
portal-recovery-printer-unconfirmed = Print result unconfirmed
portal-recovery-printer-accepted = Previously accepted
portal-recovery-printer-unavailable = Couldn’t check record
portal-recovery-printer-explain-busy = A print process is still using this record. Refresh after it finishes.
portal-recovery-printer-explain-unconfirmed = Canopa could not confirm this print result. Check your printer before printing another copy.
portal-recovery-printer-explain-accepted = The printer service previously accepted this document. It may still be printing. Removing this record leaves the job unchanged.
portal-recovery-printer-explain-unavailable = This record could not be checked. Refresh before removing it.
portal-recovery-printer = Printer
portal-recovery-server = Print server
# $id is the printer's job number.
portal-recovery-job = Job { $id }
portal-recovery-job-unknown = Job number unavailable
portal-recovery-printers-loading = Loading printer records…
portal-recovery-printers-unavailable = Printer records unavailable
portal-recovery-printers-empty = No unresolved printer records
portal-recovery-printers-empty-hint = Printer records with an uncertain outcome will appear here.
portal-recovery-check-all = Check all printer records
portal-recovery-check-all-hint = Cleanup cancels only identified jobs whose acceptance was not confirmed. Previously accepted jobs are left alone. Documents are never sent again.
portal-recovery-check-cleanup = Check and clean up
# $count is the maximum number of records kept.
portal-recovery-record-limit = Up to { $count } records can be kept. Unconfirmed records stay until cleanup or your explicit removal.
portal-recovery-printers-failed = Couldn’t load printer records. Refresh to try again.
portal-recovery-record-removed = Record removed. The printer job was left unchanged.
portal-recovery-record-removal-unconfirmed = Couldn’t confirm removal. The record may have changed or still be in use. Refresh the list.
# Result of a cleanup: numbers of records in each state.
portal-recovery-cleanup-report = { $cleared } cleared · { $unconfirmed } unconfirmed · { $busy } still in use. Check your printer before printing again.
portal-recovery-cleanup-unconfirmed = Couldn’t confirm cleanup. Refresh the list and check your printer before printing again.
portal-recovery-files-failed = Couldn’t load retained work. Refresh to try again.
portal-recovery-save-title = Save recovered copy
portal-recovery-save-copy = Save copy
portal-recovery-verify-failed = Couldn’t verify this copy. The retained files are still available in their folder.
portal-recovery-choose-again = Choose the destination again.
portal-recovery-outside-folder = Choose a destination outside the retained folder.
# $path is the saved file's location.
portal-recovery-saved = Saved a copy to { $path }. Retained files are unchanged.
portal-recovery-save-unconfirmed = Couldn’t confirm the save. Check { $path } before trying again. Retained files are still available.
portal-recovery-removed = Removed from the list. Retained files were left in place.
portal-recovery-removal-unconfirmed = Couldn’t confirm removal. Refresh the list.
portal-recovery-open-folder-failed = Couldn’t open the retained folder. Its last known location is shown here.

## Printer jobs review

portal-jobs-title = Printer jobs
portal-jobs-stopped = Printer review stopped
portal-jobs-communication-failed = Couldn’t communicate with the printer review service.
portal-jobs-list-failed = Couldn’t check printer jobs. Refresh to try again.
portal-jobs-signed-in = Sign-in was sent to the printer service. Refresh to check the job; printing may still be in progress.
portal-jobs-sign-in-cancelled = Sign-in canceled.
portal-jobs-sign-in-unconfirmed = Sign-in couldn’t be confirmed. Refresh and check the job before trying again.
# $job is the printer's job number, $status one of the job states below.
portal-jobs-entry = Job { $job } · { $status }
portal-jobs-existing-document = The printer service already has this job. Sign-in uses its existing document.
portal-jobs-sign-in = Sign in…
portal-jobs-identity-unconfirmed = This job’s identity or sign-in method couldn’t be confirmed. Refresh to check again.
portal-jobs-unavailable = Printer jobs unavailable
portal-jobs-select-printer = Select a printer
portal-jobs-empty = No active printer jobs
portal-jobs-select-job = Select a printer job
portal-jobs-held-hint = Jobs held for sign-in can continue using the document already in the queue.
portal-jobs-status-receiving = Receiving document
portal-jobs-status-queued = Waiting to print
portal-jobs-status-held = On hold
portal-jobs-status-needs-sign-in = Sign-in needed
portal-jobs-status-printing = Printing
portal-jobs-status-stopped = Stopped
portal-jobs-status-cancelled = Canceled
portal-jobs-status-aborted = Couldn’t print
portal-jobs-status-completed = Completed by printer service

## Printer authentication

portal-print-auth-title = Printer authentication
portal-print-auth-closing = Closing authentication…
# $server is host:port; $realm is the server's authentication realm.
portal-print-auth-server-realm = Sign in to print server { $server } ({ $realm }).
portal-print-auth-server = Sign in to print server { $server }.
# $printer is the printer queue (with its instance), $server is host:port.
portal-print-auth-queue = Credentials for printer { $printer } on { $server }.
portal-print-auth-local = Connected through a local printer socket.
portal-print-auth-loopback = Connected to this computer. This connection is not encrypted.
portal-print-auth-tls = Connected using TLS encryption.
portal-print-auth-rejected = The previous credentials were not accepted. Try again or cancel.
portal-print-auth-domain = Domain
portal-print-auth-username = Username
portal-print-auth-password = Password
portal-print-auth-invalid = Check the username and credential lengths.
portal-print-auth-not-saved = Used only for this print request. Credentials are not saved by Canopa.
portal-print-auth-authenticate = Authenticate

## Account: sharing the person's profile

portal-account-window = Share profile
portal-account-image-window = Choose profile image
portal-account-title = Share your profile
portal-account-enter-information = Enter the information you want to share.
# Accept button of the image picker.
portal-account-select = Select
# File type filter of the image picker.
portal-account-images = Images
portal-account-picker-failed = Couldn’t open the image picker.
portal-account-image-failed = Couldn’t load that image.
portal-account-image-prepare-failed = Couldn’t prepare the image for sharing. Retry or turn off image sharing.
portal-account-preparing-picker = Preparing image picker…
portal-account-loading = Loading profile…
portal-account-no-image = No image
portal-account-choose-image = Choose image
portal-account-clear-image = Clear image
portal-account-share-image = Share profile image
portal-account-share-username = Share username
portal-account-share-name = Share name
portal-account-username = Username
portal-account-name = Name
portal-account-invalid-name = That name is too long or contains an unsupported character. Edit it before sharing.
portal-account-preparing-image = Preparing image…
portal-account-sharing = Sharing…

## USB device access

portal-usb-window = USB device access
portal-usb-question = Allow USB device access?
portal-usb-choose = Choose the devices this app may access.
# Accessible description; $requester is the app's name or ID.
portal-usb-description = { $requester }. Choose the devices this app may access.
# $count devices of $total are selected.
portal-usb-selected = { $count } of { $total } selected
portal-usb-select-all = Select all
portal-usb-clear-selection = Clear selection
# The same buttons with their keyboard shortcut.
portal-usb-select-all-shortcut = Select all (Alt+S)
portal-usb-clear-selection-shortcut = Clear selection (Alt+S)
portal-usb-read-write = Read-write access requested
portal-usb-read-only = Read-only access requested
portal-usb-serial = Serial: { $serial }
# $location is the device file, such as /dev/bus/usb/001/004.
portal-usb-location = Location: { $location }
portal-usb-use-read-only = Use read-only access instead
portal-usb-read-only-hint = Alt+R makes the selected devices read-only.
portal-usb-allow-selected = Allow selected
portal-usb-allow-selected-shortcut = Allow selected (Alt+A)
# Name of a device that reports none.
portal-usb-device = USB device

## Dynamic launcher installation review

portal-launcher-window = Install launcher
portal-launcher-install-application = Install application launcher?
portal-launcher-install-webapp = Install web app?
portal-launcher-name = Launcher name
portal-launcher-name-placeholder = Enter a name
portal-launcher-name-too-long = Use a shorter name on one line.
portal-launcher-name-missing = Enter a launcher name.
portal-launcher-web-address = Web address
portal-launcher-hint = The launcher will appear in your applications.
portal-launcher-install = Install
# Accessible descriptions; $requester is the app's ID, $address the web app's URL.
portal-launcher-description = Requested by { $requester }.
portal-launcher-description-webapp = Requested by { $requester }. Web address { $address }.

# A control's label with its keyboard shortcut, such as "Allow pointer input (Alt+P)".
portal-with-shortcut = { $label } ({ $shortcut })

## Input capture consent

portal-input-capture-window = Input capture
portal-input-capture-question = Allow input capture?
# $requester is the app's ID.
portal-input-capture-request = { $requester } wants to receive input from this desktop.
portal-input-capture-explanation = Moving the pointer across a screen edge chosen by the application activates capture of the input you approve.
portal-input-capture-description = { $requester } wants to receive input from this desktop. Moving the pointer across a screen edge chosen by the application activates capture of the input you approve.
portal-input-capture-pointer = Allow pointer input
portal-input-capture-keyboard = Allow keyboard input
portal-input-capture-clipboard = Share clipboard while input is captured
portal-input-capture-hint = Ctrl+Alt+Esc pauses capture. You can end this session from its control window.
portal-input-capture-allow = Allow input capture

## Wallpaper

portal-wallpaper-window = Set wallpaper
portal-wallpaper-question = Change wallpaper?
portal-wallpaper-preview = Wallpaper preview
portal-wallpaper-background = Desktop background
portal-wallpaper-lock-screen = Lock screen
portal-wallpaper-both = Desktop and lock screen
portal-wallpaper-lock-hint = The lock screen image is used by Canopa’s Lock action.
portal-wallpaper-apply = Apply
# Accessible description: $requester is the app's ID, $destination one of the
# destinations above and $size the image size in pixels.
portal-wallpaper-description = Requested by { $requester }. { $destination }, { $size }.

## Global shortcuts

portal-shortcuts-window = Global shortcuts
portal-shortcuts-configure = Configure global shortcuts
portal-shortcuts-question = Allow global shortcuts?
portal-shortcuts-explanation = Choose which shortcuts this app can use while other windows are focused.
portal-shortcuts-none-chosen = No shortcut chosen
# Placeholder of a shortcut field. Modifier names stay in capitals as typed.
portal-shortcuts-placeholder = CTRL+ALT+a or LOGO+F9
portal-shortcuts-cancel-recording = Cancel recording
portal-shortcuts-record = Record keys
portal-shortcuts-preparing = Preparing keyboard recording…
portal-shortcuts-release-to-begin = Release held keys to begin.
portal-shortcuts-press = Press the shortcut keys.
portal-shortcuts-release-to-finish = Release the keys to finish.
portal-shortcuts-finishing = Finishing keyboard recording…
portal-shortcuts-unavailable = This shortcut is unavailable. Choose another trigger.
portal-shortcuts-none-requested = This app requested no shortcuts.
# CTRL, ALT, SHIFT and LOGO are typed as shown.
portal-shortcuts-syntax-hint = Use CTRL, ALT, SHIFT or LOGO (Super), followed by a key name. Example: CTRL+ALT+a.
portal-shortcuts-applying = Applying…
portal-shortcuts-save = Save shortcuts
portal-shortcuts-unrepresentable = This key combination is unavailable in the current keyboard layout. Choose another shortcut.
portal-shortcuts-keymap-changed = The keyboard setup changed. Record the shortcut again.
portal-shortcuts-recording-interrupted = Keyboard recording was interrupted. Focus this window and try again.
portal-shortcuts-recording-gone = Keyboard recording is no longer available in this window.
portal-shortcuts-recording-unavailable = Keyboard recording is unavailable. Enter a shortcut or focus this window and try again.
portal-shortcuts-conflict = A selected shortcut is already in use. Choose another trigger.
portal-shortcuts-stale = Available shortcuts changed. Review your choices and try again.
portal-shortcuts-unresolved = A selected key combination is unavailable in the current keyboard layout.
portal-shortcuts-failed = These shortcuts could not be applied. Review the selected keys and try again.
portal-shortcuts-missing-key = Choose a key for every selected shortcut.

# Document title shown when signing in to check unresolved printer jobs.
portal-printer-recovery-title = Printer recovery
