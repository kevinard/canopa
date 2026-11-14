# 0173 Open in terminal, new documents and bulk Rename

Status: accepted. Closes
`.scratch/file-manager/issues/03-terminal-templates-bulk-rename.md`. New
documents and batches use the operation queue of
[0065](0065-iced-filesystem-operation-interfaces.md), the name dialog of
[0066](0066-iced-name-dialogs-and-volume-observation.md) and the Rename claims
of [0108](0108-opened-parent-rename-claims.md).

Files could only open a terminal through a custom action, create folders but
not files, and rename one item at a time. Other file managers do all three
from their menus.

## Open in terminal

- `open-terminal` is an ordinary command, without a default shortcut, so it
  can be given one and placed on the toolbar ("Terminal"). It is in the
  folder's menu and the toolbar menu, where it opens the shown folder, and in
  the menu of a single selected folder, where it opens that folder.
- It runs `xdg-terminal-exec --dir=FOLDER` with the folder as working
  directory too, for terminals whose entry lacks `--dir`. Files already relies
  on `xdg-terminal-exec` for `Terminal=true` programs
  ([0166](0166-open-with-and-launch-trust.md)); without it the tab shows the
  same "install xdg-terminal-exec" message. The activation token is passed as
  for any other launch. Folder paths keep their bytes.

## New document

- "New document" follows "New folder…" in the folder's menu and the toolbar
  menu, and opens a submenu beside its row: "Empty document", then the
  templates, then "Open Templates folder". The first menu stays open, inert,
  while the templates are read and behind the submenu; Escape or the back
  arrow returns to it, and a click on one of its rows acts on that row. The
  submenu opens after the menu's trailing edge, or before its leading edge
  near the window's edge. Open With likewise keeps its menu open until its
  applications arrive, then replaces it in place, as
  [0171](0171-menus-tabs-views-and-secondary-windows.md) describes.
- Templates are the files of the XDG Templates folder, read on a worker when
  the menu opens. Hidden files and `~` backups are left out, links count when
  they lead to a regular file, and one level of subfolders is read and shown
  as "Group › Name". At most 100 templates are listed. A Templates folder set
  to Home means none, as `xdg-user-dirs` documents.
- Choosing one opens the name dialog with the template's name, or "New
  document", numbered "Name (2).ext" when taken, with the part before the
  extension selected. A name already in the folder is refused in the dialog.
- `OperationRequest::CreateFile { path, template }` copies the template, or a
  private empty file, through the prepared-copy lifecycle of Copy, and
  journals it as a copied leaf (`Change::Copied` without source facts). So an
  interrupted run is reconciled like a Copy, and Undo removes the document
  while it is unchanged and asks before removing newer work. The document
  keeps the template's contents and permissions and is dated now.
- A taken name is refused before staging, as Copy reviews conflicts first. A
  refused publication keeps its staged data for Recovery, so only a race with
  another program reaches that path.

## Bulk Rename

- "Rename…" (F2) on several items of one folder opens a larger name dialog:
  Replace text, Add text (before and after the name) or Number (a name and a
  number counting from a start, zero-padded to the widest number). A preview
  lists every item with its new name and marks names that cannot be used.
- Rules change the part before the extension, counting `.tar.gz` and the
  other compressed tarball suffixes as one; folders change whole. Number
  counts in the order the browser shows. Names keep their Unix bytes; only
  labels are lossy.
- Rename is refused while a name is invalid, used by two items, used by an
  item of the folder that is not renamed, or when names swap (`a`→`b`,
  `b`→`a`), which would need a temporary name nobody chose. Chains are
  allowed: `1`→`2` and `2`→`3` rename `2` first.
- `OperationRequest::RenameBatch` is admitted like Rename: every selected
  entry and their folder are observed and their retained data is captured.
  The engine orders the steps and runs each as an ordinary journalled Rename
  claim under one history entry, "rename N items", so one Undo returns every
  name, newest step first.
- A step whose name was taken meanwhile fails alone and the others continue;
  the result lists what kept its name. A step that leaves a claimed entry for
  Recovery stops the batch there. Cancellation keeps the completed steps.

## Request bindings

Both operations open their history entry without binding it to the request,
as Trash does. Request bindings describe Transfer cursors and single-step
mutation receipts; a staged copy bound to a request without a Transfer cursor
is refused. An interrupted step is still reconciled from its journal intent on
the next operation, and an interrupted request reopens as interrupted rather
than as a proven completion.

## Verification

Unit tests cover the rules, extensions, byte names, problems, chains and
swaps, template listing and free names, the terminal command line, history
labels in both languages and the menu items. Desktop tests run both dialogs
against the operation queue: a template copy under a free name dated now, an
empty document, a refused taken name, a batch blocked by a problem and fixed,
and one Undo per operation. `tests/naming.rs` covers the engine: Undo of
unchanged and edited documents, a refused taken name without leftovers,
chains, a byte name, a step failing alone, swaps and cancellation.

Native rendering of the dialogs and a real terminal launch were not verified
in this change.
