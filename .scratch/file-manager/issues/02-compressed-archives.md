# Compressed archives: create and extract

Status: done

## Context

During the Files menu review (2026-11-13) the file menu was compared with other
file managers. "Open in new tab" stays a folder-only item and "Duplicate" was
left out to keep the menu short; compressed archives were confirmed as wanted.
Files has no archive support today: no menu item, no operation, no dependency
able to read or write archives.

## Scope to decide

- **Create**: a "Compress…" item in the menu for files, folders and mixed
  selections. It opens a small dialog with the archive name (preselected without
  its suffix) and a format choice. A single item is named after that item; several
  are named "Archive". The archive is written beside the items, under the next free
  numbered name.
- **Extract**: "Extract here" for an archive, plus "Extract to…" with a
  destination. An archive whose entries share one top folder is extracted as that
  folder; other archives are extracted into a new folder named after the archive.
- **Formats**: at least `.zip` and `.tar.gz`, likely `.tar.xz` / `.tar.zst`. Decide
  which formats are written versus only read, and whether to use Rust crates
  (`zip`, `tar` + `flate2`/`xz2`/`zstd`) or an external backend (libarchive).
  Record the choice and its licence review in an ADR.
- **Engine**: archive jobs run through the existing operation jobs, so they get
  progress, cancellation, the status summary, the Operations window and
  interrupted-job handling. Define what undo means: removing the created archive,
  or the extracted items when they are unchanged.
- **Safety**: refuse entries that leave the destination (absolute paths, `..`,
  symlinks pointing outside). Bound expansion against decompression bombs. Keep
  permissions without set-id bits. Resolve name conflicts with the existing
  replacement review.
- **Encrypted or damaged archives**: show a clear error. Password prompts are
  out of scope for a first version unless decided otherwise.

## Acceptance

- Menu items appear only where they apply and keep the grouped menu layout,
  with their shortcuts, if any, at the end of the row.
- Focused tests cover naming, conflicts, path traversal refusal, cancellation
  part-way, and recovery of an interrupted job.
- Never run against real user folders in tests; use temporary directories.

## Comments

### 2026-11-14 — implemented

Decided: Rust crates in-process, not libarchive. The written formats are
`.zip`, `.tar.gz`, `.tar.xz` and `.tar.zst`; `.tar` and `.tar.bz2` are also
read. There is no Undo in the first version, and no password prompt. Results
are published under the next free name with a no-replace rename, so the
replacement review is not used. See
[ADR 0172](../../../docs/files/adr/0172-compressed-archives.md).

Not done: native rendering checks of the Compress and Extract to dialogs; 7z
and rar; remembering the format across sessions.
