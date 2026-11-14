# Open in terminal, new documents from templates, bulk Rename

Status: done

## Context

Requested 2026-11-14: an "open terminal in this folder" command, New document
actions based on the XDG templates, and a bulk rename feature. Files could
only open a terminal through a custom action, could create folders but not
files, and renamed one item at a time.

## Scope

- **Terminal**: a command for the shown folder and for a selected folder,
  through the user's terminal (`xdg-terminal-exec`).
- **New document**: the XDG Templates folder's files, plus an empty document,
  created under a name chosen in the name dialog, undoable.
- **Bulk Rename**: several selected items of one folder renamed by one rule
  with a preview, refusing conflicts, undone in one step.

## Acceptance

- Items appear only where they apply and keep the grouped menu layout.
- New documents and batches run through the operation queue with Undo and
  interrupted-step reconciliation; nothing is replaced.
- Tests use temporary folders only.

## Comments

### 2026-11-14 — implemented

See [ADR 0173](../../../docs/files/adr/0173-terminal-new-documents-and-bulk-rename.md).
Modes are Replace text, Add text and Number; extensions are kept. Swapping
names is refused rather than done through temporary names.

Not done: native rendering checks of the dialogs and a real terminal launch;
case-insensitive matching and regular expressions in Replace; template
subfolders deeper than one level.
