# 0152. Use btrfs clones, shared extents and subvolume Trash in Files when available

Status: accepted

## Context

Files must work on any local filesystem, and its journal proves every step with
kernel identities and content fingerprints. On btrfs three things were left on
the table or went wrong:

- Every Copy moved all bytes through uutils, although btrfs can clone a file
  (`FICLONE`) without reading or writing its data and without using data space.
- A btrfs subvolume has its own device number, so a Move between two subvolumes
  of one disk (for example `~/Photos` and `~/Archive`) takes the cross-filesystem
  path: copy, verify, then hash the source and the copy twice each before removing
  the source.
- Trash chose its storage by mount ID only. A nested subvolume that is not mounted
  separately shares its mount with `$XDG_DATA_HOME`, so Files picked the main
  Trash, the rename failed with `EXDEV`, and Files reported the volume as having
  no Trash and offered permanent deletion instead.

btrfs gives userspace no multi-file transactions, so it cannot replace the journal.

## Decision

btrfs is an optional fast path, detected per descriptor with `fstatfs`. Nothing is
required of the filesystem, and every refusal falls back to the generic behavior.
The code is in `crates/canopa-files/src/btrfs.rs`.

- **Clones in the prepared-copy lifecycle.** Before uutils copies a regular file,
  the copy backend asks btrfs to clone the whole source into the new, empty stage
  output. On success the output shares the source's extents; on any failure the
  output is truncated and uutils copies the bytes. Every other step is unchanged:
  snapshot checks, the XXH3 output receipt, attributes, sync, journal and
  no-replace publication. Copy, Keep both, Merge, cross-subvolume Move, Recovery
  copies, copied Undo returns and New Document all use it. This is a reviewed
  exception to "uutils moves the bytes": a clone moves no bytes, and uutils
  remains the only copier whenever data is copied.
- **Shared-extent proof.** Two regular files on one btrfs filesystem (same
  filesystem UUID from `BTRFS_IOC_FS_INFO`) whose `FIEMAP` maps, flushed with
  `FIEMAP_FLAG_SYNC`, list the same logical and physical ranges have equal
  contents: btrfs never writes a shared extent in place, even for nodatacow
  files. Compressed, inline, encrypted, delayed, preallocated or unknown extents
  refuse the proof, as do a file compared with itself, empty files and maps over
  64 Ki extents. Source removal after a cross-filesystem Move uses
  `Fingerprint::read_opened_pair`: with a proof it hashes only the copy, then
  rechecks the source's metadata snapshot and the proof; otherwise it hashes both
  as before. The copy's publication is checked before and after.
- **Subvolume Trash.** Trash uses the main store only when the source shares both
  mount and device with `$XDG_DATA_HOME`. Otherwise its top is the highest folder
  of the mount still on the source's device, as GIO does, with the usual `.Trash`
  or `.Trash-$uid` rules. A top below a mount point is recorded in
  `$XDG_DATA_HOME/canopa-files/trash-tops.json`, written atomically before the
  item moves, and discovery reads it with the mount tops. The record lives beside
  the main Trash because Trash discovery is keyed by the data home, not by the
  Files state directory.
- **Recorded tops follow their subvolume.** Each record keeps the path and the
  btrfs filesystem UUID and subvolume ID. When the path no longer opens as that
  subvolume's root, discovery searches the filesystem's btrfs mounts for it with
  the unprivileged `GET_SUBVOL_ROOTREF` and `INO_LOOKUP_USER` requests (at most
  4,096 subvolumes). A subvolume inside Trash storage is a trashed item, not a
  top: below the main Trash, compared both as named and as resolved since the
  search reports physical paths, or below a `.Trash-$uid/files` or
  `.Trash/$uid/files` folder. Other names beginning with `.Trash` are ordinary. Renames of the subvolume
  or its ancestors, by Files or anyone else, and remounts elsewhere are followed
  without the engine tracking them.
- **Records are never dropped for being unreachable.** A top that cannot be found
  (a disconnected drive, an unreadable parent) keeps its record. With 256 records,
  a new top replaces only one that is present, whose Trash is empty, and whose
  storage no Undo entry refers to (Undo of a Restore moves an item back there),
  checked through the history's path index; otherwise the item fails before it
  moves, as an ordinary failure rather than an offer of permanent deletion.

## Consequences

- On btrfs, same-filesystem and cross-subvolume copies take no data space and
  write no data. The output receipt still reads the copy once; recording extent
  maps instead would need a new fingerprint format.
- Moving a large file between subvolumes reads it about three times (receipt and
  two verifications) instead of copying it and reading it five times.
- A failed record read or write fails that Trash item without the
  permanent-deletion offer. An unreadable record is never overwritten; until it
  is repaired, discovery lists no subvolume Trash and the stored items are
  untouched.
- A top on another filesystem with nested devices (bcachefs) has no subvolume
  identity and is found only at its recorded path.
- Other filesystems with clones (XFS, bcachefs) still copy bytes. Adding them
  needs their own proof rules, not just a magic number.
- Not done here: snapshots, clone-backed saved versions, user-created versions
  (Recovery has no way to discard protected data), space-based history limits
  and folder comparison.
- btrfs tests run when `CANOPA_FILES_TEST_BTRFS` names a writable btrfs folder;
  otherwise they return early.
