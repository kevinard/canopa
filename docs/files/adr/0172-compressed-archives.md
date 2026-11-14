# 0172 Compressed archives: create and extract

Status: accepted. Closes `.scratch/file-manager/issues/02-compressed-archives.md`.
Archive jobs use the operation queue of
[0065](0065-iced-filesystem-operation-interfaces.md) and the name dialog of
[0066](0066-iced-name-dialogs-and-volume-observation.md).

Files could neither read nor write archives. People expect a file manager to
compress a selection and to extract a downloaded archive without a terminal.

## Commands

- The selection menu has its own group, just before Rename: **Extract here** and
  **Extract to…** when every selected item is a file named like an archive, then
  **Compress…** for any selection. The group appears only while the operation
  queue is ready, like the other commands that write.
- **Compress…** opens the name dialog with the archive's name and a format
  choice. A single folder is named after itself, a single file after its name
  without its extension, and several items "Archive". The dialog proposes the
  last format chosen in the session, `.zip` at first.
- **Extract here** submits one job per selected archive into the folder that
  shows it. **Extract to…** opens the same dialog with a destination folder
  field and the read-only folder browser used by Recovery.
- When the job finishes, the folder that asked for it is refreshed and its
  result is selected.

## Formats and backend

Written: `.zip` (Deflate), `.tar.gz`, `.tar.xz` and `.tar.zst`. Also read:
`.tar`, `.tar.bz2` and the short suffixes `.tgz`, `.txz`, `.tzst`, `.tbz2` and
`.tbz`. Reading uses each file's leading bytes first and its suffix only when
they are not recognised, so a misnamed archive still opens. Zip entries using
bzip2, zstd, LZMA, xz or Deflate64 are read as well.

The backend is Rust crates in-process: `zip`, `tar`, `flate2` (zlib-rs),
`lzma-rust2`, `bzip2` (libbz2-rs) and `zstd`, which builds its bundled C
library. libarchive was not chosen: it would add unsafe FFI and a runtime
package dependency, and Files would still have to check every entry path
itself. The 7z, rar and iso formats it would add are left out for now.

Licences: MIT or Apache-2.0 (zip, tar, flate2, bzip2), Apache-2.0
(lzma-rust2), Zlib (zlib-rs), bzip2-1.0.6 (libbz2-rs-sys), BSD-3-Clause and
BSD-2-Clause (zstd and its bundled library). All are permissive and compatible
with the workspace's MIT OR Apache-2.0 licence; their notices are collected in
`docs/dependency-attributions.md`.

## Publication

`crate::archives` is renderer-independent and knows nothing of jobs. A run
writes its result under a hidden partial name in the destination folder,
`.canopa-archive-<token>.partial` or `.canopa-extract-<token>.partial`, where
the token is random and saved in the request. When the result is complete, a
no-replace rename publishes it under the first free name: `Name.zip`, then
`Name (2).zip`, keeping a file's extension after the number.

So nothing is ever replaced, and no replacement review is needed; the issue's
proposal to reuse that review was dropped. A cancelled or failed run removes its
partial result before it reports.

Extraction publishes a single top-level entry as itself. Several entries are
published as a folder named after the archive without its suffix. An empty
archive is an error.

## Safety

- Entries with an absolute path or a `..` component are skipped and counted in
  the result's detail. Hard links are created only to a regular file extracted
  earlier, reached through real folders. Other hard links are skipped and
  counted too.
- Symbolic links are created after every other entry, so nothing is written
  through one. A link whose target is absolute, or climbs above the extracted
  folder, is skipped. `..` is accepted only before every named component of a
  target, so no step climbs out of a folder reached through another link. An
  entry whose folder would have to be a link is a conflict, and the extraction
  fails.
- Written bytes are bounded by the destination's free space, less a 64 MiB
  reserve, and entries by a count of one million. A zip with overlapping
  entries is refused. These limits stop decompression bombs with an error
  instead of filling the disk.
- File permissions keep their `rwx` bits; set-id and sticky bits are never
  restored. Folders keep their bits plus the owner's `rwx`, so the result can
  always be moved and removed. Modification times are restored.
- Devices, pipes and sockets are neither stored nor extracted; they are counted
  in the detail. Compression never follows symbolic links and stores them as
  links.
- `.zip` names must be UTF-8. A name that is not fails the job and suggests a
  `.tar` format, which keeps names as bytes.
- Encrypted zip entries fail the job with a clear message; there is no password
  prompt in this version. Damaged or unsupported archives fail likewise.

## Jobs, Undo and interruption

`Request::Archive` is an ordinary queued request: it has progress, cancellation,
the status summary and the Operations window. Its result is saved as
`ArchiveResult`: finished with its published path, stopped when cancelled, or
failed with an error.

Compressing reports bytes and entries against the scanned totals. Extracting a
zip reports against its entry count and the archive's size; extracting a tar
stream shows running counts, since its length is unknown until it ends.

Archive jobs are not undoable, like permanent deletion: the result is a new
item that can be moved to the Trash. They are never resumed. When history
reopens with an archive request still recorded as running, only that run's own
partial name is removed, never through a link. The job then reports
Interrupted. If the cleanup cannot be saved, the request stays recorded as
running and the next start repeats it.

## Verification

`tests/archives.rs` covers round trips in every written format, naming and
numbering, traversal, link and hard-link refusal, writing through a link,
the size and entry bounds, cancellation part-way, encrypted and damaged
archives, non-UTF-8 zip names, and job results. Its job tests cover
cancellation and the cleanup of an interrupted job after a restart. Desktop
tests cover the menu group, the Compress dialog's name, format memory and
refusal of an invalid name, Extract here and Extract to.

Native rendering of the dialogs was not verified in this change.
