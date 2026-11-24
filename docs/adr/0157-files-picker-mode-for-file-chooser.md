# 0157. Files picker mode for the FileChooser portal

Status: accepted

## Context

Sharing the browsing model and individual widgets left the portal in charge of
assembling its own window. Files navigation features such as configured Places,
Recent and mounted devices still diverged.

## Decision

The portal starts the sibling `canopa-files` executable for each FileChooser
request and exchanges one JSON request and result over private pipes. Files
runs its regular browser in picker mode with request options, filters, a
selection footer and target confirmation. The process is independent of the
resident Files owner and observes mounted devices using in-memory state.

The portal keeps request cancellation, validates results and supplies the
frontend with the accepted paths. The frontend owns access grants. Account,
Print and Print Recovery keep their embedded browser and use the same target
preparation and folder creation code.

## Consequences

Changes to the Files window, Places and browsing interactions reach the public
FileChooser through the same code. The two binaries ship in the same directory.
Save returns a destination for the receiving application to write. File
operations, custom actions and global preference writes belong to normal Files
windows.
