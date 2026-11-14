# 0178 Recording recent files

Status: accepted 2026-11-14. Amends the "Writing" decision of
[0176](0176-recent-files.md).

ADR0176 made Recent read the desktop's shared `recently-used.xbel` but left
adding to it to the opening application. Most applications launched through
`gio launch` do not record the file themselves, so documents opened from Files
did not appear in Recent. GNOME Files records the files it opens, and so does
every GTK file chooser.

## Decision

- **What is recorded.** A regular file opened with an application from a
  browser tab: the default application, Open With, or the application chooser.
  Folders, programs, scripts, desktop entries, links, Quick Look previews, and
  files opened from the Trash or Recovery windows are not recorded.
- **What an item holds.** As GLib's `GBookmarkFile` writes it: the `file:` URL;
  `added`, `modified` and `visited` times; the MIME type Files detected; and an
  application entry with the application's name, `'PROGRAM %u'` as its command,
  a `modified` time and a count. Opening a listed file again updates its
  `modified` and `visited` times and adds one to that application's count, or
  adds the application to the item.
- **Keeping other programs' data.** The list is edited as text. Only the changed
  attribute values and the inserted elements differ from what was read;
  groups, private flags, unknown metadata, comments and formatting are kept byte
  for byte. A list that does not parse, or is over 16 MiB, is not replaced.
- **Writing.** As `g_file_set_contents` does: a temporary file in the same
  folder with the list's mode (0600 for a new list), synced, then renamed over
  the list. A symlinked list is written at its target. There is no shared lock
  for this file among desktop programs. Before renaming, Files checks that the
  list is the version it read (device, inode, size, modification time); if
  another program replaced it meanwhile, Files starts again from that version,
  up to three times. Writes from Files itself are serialised.
- **Failure.** Recording runs on the launch worker after the application has
  started. A failure to record is written to the error log only: the document
  is already open.
- **Removing.** Removing items and clearing the list are still not offered.
- **Tests** never read or write the person's list: the desktop state has no
  list under `cfg(test)`, and Recent searches there are empty.

## Verification

- `recent::tests` cover creating a private list, updating a known item and its
  applications while keeping every other byte, foreign namespace prefixes, and
  leaving a malformed list alone.
- A list written by Files, then changed by GLib 2.90 `GBookmarkFile`, then by
  Files again, was read back by `GBookmarkFile` with the expected types,
  applications, counts and times.

Not verified: recording in a native session.
