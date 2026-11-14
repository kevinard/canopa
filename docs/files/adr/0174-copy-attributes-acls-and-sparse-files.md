# 0174 Copies keep extended attributes, ACLs and holes

Status: accepted 2026-11-14. Changes the copy policy of
[0153](0153-uutils-prepared-copy.md); the prepared-copy lifecycle, receipts and
durability of [0155](0155-trust-upstream-copy.md) and
[0165](0165-batched-copy-durability-and-progress.md) are unchanged.

Copies used to drop every extended attribute and ACL and to allocate the full
logical size of sparse files. Tags, download origins (`user.xdg.origin.url`)
and the access lists of shared folders were lost silently, and copying a
virtual-machine image could fill the destination. Hard links became separate
files, as they still do.

## Extended attributes and ACLs

- A regular file's copy gets its source's `user.*` attributes and its POSIX
  access ACL (`system.posix_acl_access`). A folder's copy also gets its
  default ACL (`system.posix_acl_default`), so new files in the copied folder
  inherit what they would have inherited in the original.
- Other namespaces are deliberately not copied: `security.*` labels and file
  capabilities follow the destination's policy, and `trusted.*` needs
  privileges Files does not use. Symbolic links carry no user attributes on
  Linux.
- Attributes are read through the descriptors the copy already holds (`O_PATH`
  folders through their `/proc/self/fd` link), never through a user path
  again. They are written to the staged copy before publication.
- ACLs are written before the permission restore: an access ACL rewrites the
  group bits, and the later `chmod` to the source mode then sets the ACL mask to
  the source's mask, so the copy ends with the source's exact ACL. A folder's
  owner entry may deny writes, so its staged mode is restored right after its
  attributes and Files can still fill it; the folder's final mode is applied
  at the end as before.
- When a source has no ACL, the copy keeps whatever the destination folder's
  default ACL gave it, like any new file.

Attributes never fail a copy. When the destination refuses them (a FAT or exFAT
drive, a share without ACLs) or the source cannot be read, the copy completes
and its path is added to the transfer's `attributes_omitted` list. The
Operations result then says "N copies missing extended attributes". The list is
part of the run's report, not of the restart cursor: a resumed transfer reports
only what it copied itself.

Attributes are not part of the fingerprint, so Undo and source-removal checks do
not compare them.

## Sparse files

uutils still copies every byte (ADR0153 keeps it as the only data mover). Before
copying a regular file whose allocated blocks are fewer than its size, Files lists
its holes with `SEEK_DATA`/`SEEK_HOLE`. While the copy runs, each part of a hole
that the copy has passed is punched (`FALLOC_FL_PUNCH_HOLE | KEEP_SIZE`) before
the 32 MiB write-behind sync. The zeros are dropped from the page cache instead
of being written to the device. The copy keeps the logical size and contents
and allocates about what the source allocates.

- Only holes the source reports are punched. Allocated zeros stay allocated.
- A destination that cannot punch holes keeps a full copy. That costs space, not
  data, and is not reported.
- A source written during the copy fails the existing snapshot check, so a hole
  that gained data is never punched into a published copy.
- Reading the holes still costs memory bandwidth: a 100 GiB image with 10 GiB of
  data copies 10 GiB to the device but streams 100 GiB of zeros through the copy.

## Hard links

Hard links stay independent copies, as in GNOME Files, Thunar and the other
GIO-based file managers. Recreating a link group would make a published copy
depend on another item's publication, which is a new journal state for Undo and
restart reconciliation. Not worth that for a file-manager copy.

## Verification

- `operations::attributes` unit tests: user attributes and access ACLs reach the
  copy; a refused attribute is reported without failing; a folder ACL denying
  its owner writes does not lock the staged copy.
- `uutils_copy::shared_copy_retains_the_documented_metadata_and_link_policy`
  now asserts kept file and folder attributes, an 8 MiB file with one 64 KiB
  extent copying to less than 1 MiB allocated with identical contents, and
  independent hard links.
- The transfer, Recovery, resume, interrupted and Undo integration cases.

Not verified: a FAT destination in a native session (the "missing extended
attributes" row), and SELinux systems.
