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
