# Canopa Files context

Canopa Files is the desktop's independent file manager. It owns directory
browsing, file selection, searches, previews, custom actions, preferences, file
operations, and Undo. It consumes Canopa's shared appearance and UI treatment;
Shell and Notify retain their established ownership and lifetimes.

## Language

- A Browser is one location-navigation session, hosted in a Tab.
- A Browser Window hosts Tabs and owns the origin of its Quick Look Window.
- A Directory is an immutable filesystem snapshot prepared off the UI thread.
- A Device is a local volume or standalone local mount exposed by the platform.
  Mount and unmount requests have their own lifetime and do not enter file Undo.
- A Device Request is a backend-owned mount or removal attempt. Its current
  question has an opaque answer token; descriptive results do not grant current
  root navigation or restart execution authority.
- Selection contains the focused file, selected files, and the range anchor.
- Quick Look follows its originating Browser Window, including its active Tab,
  independently of focus in other Browser Windows. The auxiliary Trash window
  and Recovery windows own their previews through the same source interface.
- An Operation is a user request such as a batch copy, move, rename, or trash.
- A Permanent Deletion is a confirmed, non-undoable Operation that removes a
  selection or Trash items through the same queue. Empty Trash is one over the
  listed Trash items.
- A New Document is a copy of a Templates file, or an empty file, under a new
  name. It is journalled as a copied item, so Undo removes it only unchanged.
- A Bulk Rename renames several items of one folder by one rule, as one
  Operation whose steps are ordinary Rename claims and whose Undo is one entry.
- An Archive Job compresses a selection or extracts an archive through the same
  queue. It publishes one new item under a free name and is neither undone nor
  resumed; an interrupted one only has its own partial result removed.
- A Transfer is the asynchronous execution of an Operation. It can outlive all
  Browser Windows and exposes progress, conflicts, failures, and cancellation.
- A Source Lease protects catalogued retained data while an owned clipboard
  selection or drag can still use it. It grants retention, not transfer execution.
- A Drag owns one captured selection and its live gesture state. Prepared hover
  destinations share that gesture; cancelling one hover does not cancel another.
- A Drop captures file references and a destination from a drag gesture; its
  resolved Copy/Move action becomes a normal Transfer request.
- An Undo Entry describes actual completed changes and the information needed
  to reverse them. Undo history is shared across Browser Windows and Tabs.
- An Undo Relocation records a restored item's new path so older history still
  refers to that item, including its descendants and retained replacement data.
- Recovery lists retained versions still needed by Undo and protected data that
  requires explicit recovery. A Recovery Copy creates an undoable copy at a new
  path without consuming the retained version.
- An Interrupted Operation retains a filesystem journal that cannot yet be
  reconciled. Recovery exposes its reason and owned files while normal mutations
  wait. Recovery copies have a separate journal and leave that record intact.
- A Custom Action is an explicitly invoked command or script with selection
  conditions. It does not run automatically as files arrive.

## Public test seams

Behavioral tests exercise Directory and Browser, the transfer engine, Trash,
preferences, the custom-action queue, portable storage requirements, durable scheduling records, preview
preparation, and custom-action execution through their public interfaces.
Scheduling-record tests cover persistence; crash-boundary tests kill a worker at a
journal step and reopen its history. Configuration tests use immutable snapshots and the serialized
reload runtime against actual isolated source/theme files. Native desktop
acceptance exercises window focus, tiling, keyboard input, clipboard, drag and
drop, and rendered appearance in the repository's private desktop harness.
Local-volume acceptance replaces the external GIO storage provider with a private
module, retaining real temporary files, native signals and asynchronous requests.
It never changes an actual user mount or block device.

See [the Files architecture](../architecture/files.md) for the design and the
decisions behind these terms.
