# Protected retained versions and Recovery copies

Status: accepted under delegated implementation authority, 2026-10-16;
implementation in progress.

Replacement backups need an inspectable home before Undo can offer to displace
newer work. Recovery is an auxiliary native window available from the folder
menu and shared Operations. It also has a configurable command and toolbar
position, with no default shortcut that could conflict with existing actions.

Recovery lists both previous files still needed by Undo and protected data kept
after history expires. Rows show the original filename and location, operation
label, size, modification time and availability. A protected or unavailable
version remains visible. The original Unix path bytes stay in persistent state;
display strings are produced at the interface boundary. Older cleanup records
without filename context remain readable with an unknown-location label.

Space opens the selected retained version in source-bound floating Quick Look,
using the original filename as a format hint. Up and Down change the selection
and preview. Ctrl+R or Recover copy opens a native save chooser. The user chooses
a new filename/location; the engine refuses occupied paths, including broken
links, and paths inside that version's own retained-data folder. The chooser's
standard overwrite confirmation does not authorize overwriting through this
copy-only command. A failed copy is reported in Operations so the user can
choose a different path.

A recovery copy never consumes its source. The engine re-reads the catalog
record and validates its stage identity and observed file version before and
after preparing the copy. A changed or formerly unavailable observation requires
refreshing Recovery before another attempt. Regular files and symbolic links use
the existing staged, cancellable copy transaction and persistent Undo. Link
targets retain their original bytes. Undo of a recovery copy removes only that
new copy, subject to the normal newer-work checks. Unexpected directory or
special-file payloads remain inspectable but are not offered the file-copy
command by this initial adapter. [ADR 0050](0050-directory-recovery-copies.md)
extends copy-out to directories through shared recursive transfers; special
files remain unsupported.

The catalog reads bounded pages of 256 versions on workers, with Load more for
larger batches. Refresh cancels older requests and rejects late results. Jobs
refresh an open catalog after changes. F5 explicitly rechecks changes made by
external programs; this initial window does not continuously monitor every
retained file. The shared queue and application holds let copying outlive the
Recovery window. Its status, cancellation and Undo appear in Operations.

History describes paths as they were when each change happened. Reading an
active backup follows subsequent committed namespace renames, including Trash
and Restore, without rewriting historical Undo paths. Expiry archives the
resolved physical location with its filename context. Later namespace changes
update archived locations in the same transaction that records the operation or
its reversal. Keep both Undo also applies its durable path relocation. This
keeps protected backups discoverable when a containing folder moves, is renamed,
or returns under another name. Renaming or trashing the replacement file itself
does not move its sibling backup or replace its original filename with a storage
ID. Cross-filesystem movement of folders containing
retained stages needs its separate source-retention audit; a same-filesystem
namespace mapping does not establish that contract.

Public filesystem tests cover retained data changed through an open file
descriptor, history expiry and restart, active backup preservation, stale
observations, occupied/cancelled destinations, raw filenames, symlink bytes,
unavailable stages, folder moves across expiry, Keep both relocation of an
archived backup, original backup names after Trash and 257-version pagination.
Native acceptance covers actual replacement, catalog metadata, source-named
preview, save chooser copy-out, Undo of the copy, retained-file edits, 360-pixel
layout, a real process restart and original-name preview after the replacement
is trashed. Dark (`native-recovery-06`) and light at 1.25×
(`native-recovery-08`) pass; rendered inspection confirms readable narrow rows
and the retained text after restart and Trash. Both native application logs
have no GTK warnings. All 70 core tests, focused Clippy and the existing full
native regression (`native-acceptance-20`) pass.

Undo Replace now uses protected Recovery for newer file/symlink occupants of a
restoration path; [ADR 0014](0014-undo-replace.md) records the supported cases.
[ADR 0026](0026-undo-changed-saved-versions.md) adds an explicit review before
Undo restores an available saved version that has itself changed.
[ADR 0027](0027-history-readers-and-live-recovery.md) keeps catalog reads and
previews available while a live operation waits for an interactive answer.
[ADR 0017](0017-interrupted-operation-recovery.md) adds inspectable pending
history and independently journaled rescue copies when automatic reconciliation
cannot finish. The broader recovery/Undo requirement remains open. Next work
includes extending displacement to the remaining Undo cases, phase-specific
interrupted-operation choices, explicit disposal of archived protected
data, and directory recovery. No active Undo backup may be discarded by a
catalog cleanup command. The remaining Undo conflict, restart and filesystem
race work is tracked in the implementation ledger and ADR 0012.
