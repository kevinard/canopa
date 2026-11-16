# Canopa Files architecture

Canopa Files (`canopa-files`, crate [`crates/canopa-files`](../../crates/canopa-files/))
is the desktop's file manager. It owns browsing, selection, search, Quick Look,
file operations and their Undo, Trash, Recovery, devices and network locations,
custom actions and its own preferences. It consumes Canopa's shared appearance,
configuration and Iced controls (`canopa-ui`); Shell and Notify keep their own
ownership and lifetimes.

User-facing behavior is described in the [Files guide](../files.md). Domain terms
(Browser, Operation, Transfer, Undo Entry, Recovery, Source Lease, ...) are defined
in the [Files glossary](../files/CONTEXT.md).

Earlier decision records were consolidated into this document; the originals are
in the git history at commit cb300936 under `docs/adr/` (root ADRs 0015 to 0018,
0022 and 0025) and `docs/files/adr/`.

## Product direction

Files takes its simplicity, extensibility and configurable interface from Thunar,
and column browsing and Quick Look from the Snow Leopard Finder. The defining
experience is column navigation, a floating Quick Look attached to the browser
that opened it, and comfort in tiled Niri windows: columns adapt to wide, medium
and narrow tiles, Left always goes to the parent, and returning to a parent selects
the folder just left. The reference workflow is sorting Downloads in a narrow tile
with Quick Look, opening a destination in a second tile, moving files between the
windows, undoing the move and running a custom action on selected images.

Out of scope by design: office-document previews, automatic folder rules (custom
actions run only when invoked), and built-in split panes (Niri tiles windows).

## Layers and features

The crate has three layers. Each lower layer builds and tests without the one above.

| Layer | Cargo feature | Owns |
|---|---|---|
| Domain | none (always built) | Filesystem and browsing semantics: directories, search, previews, preferences, the operation engine and its journal, Trash, archives, opening rules. No threads of its own beyond workers it is handed. |
| Runtime | `local-runtime` (= `background-actions` + `background-operations` + GIO/GLib) | Admission, queues, cancellation, progress, questions, attention, persistence, reconciliation, clipboard intent, activation, FileManager1, the volume helper, and the lifetime of accepted work. |
| Frontend | `desktop` (= `local-runtime` + `media-playback` + Iced + `canopa-ui` + `wayland-clipboard`) | Windows, focus, drafts, geometry, rendering and native transports. It turns input into typed commands and renders bounded snapshots. |

The binary requires `desktop`; default features are `desktop` and
`accessibility-testing`. `media-playback` links GStreamer core/app/video directly.
`wayland-clipboard` adds the native file clipboard connection. Core tests run with
`--no-default-features`.

The XDG desktop portal's FileChooser (`crates/canopa-portal`) depends on
`canopa-files` with `default-features = false` and reuses its domain browsing model
(`browse::{Browser, Directory, Entry, SelectionMode}`, `personal_folders`,
`interface::PlaceKind`, `Cancellation`). The lower-level crate
[`canopa-file`](../../crates/canopa-file/src/lib.rs) is shared by most of the
workspace; Files uses its bounded reads, shared-mime-info database (`mime`),
inotify watcher (`watch`) and byte-preserving GTK bookmarks editor (`gtk_bookmarks`).

## Module map

Paths are relative to `crates/canopa-files/src/`.

| Module | Purpose |
|---|---|
| `main.rs`, `copy_check.rs` | clap entry point; hidden `--internal-check-copy` packaged copy self-test and `--internal-volume-monitor` helper mode. |
| `application.rs` | `Application`: starts configuration, volume monitor, operation queue, action queue and lazy media playback before any window; state-directory paths. |
| `activation.rs` | Single owner per state directory, `runtime.lock`, private launch socket and frames. |
| `file_manager.rs`, `presentation.rs` | `org.freedesktop.FileManager1` on the session bus; renderer-independent window intents (browsers, Quick Look, Properties). |
| `browse.rs` | `Browser` (location, selection, focus, history) and immutable `Directory` snapshots. |
| `folder_watch.rs` | One inotify worker for displayed folders (via `canopa_file::watch`). |
| `search.rs`, `recent.rs` | Filename/content search with filters; `recently-used.xbel` reading and recording. |
| `preview.rs`, `preview/`, `syntax.rs`, `playback.rs`, `playback/` | Quick Look preparation (image, PDF, text with highlighting, folder, binary) and the GStreamer playback worker. |
| `thumbnails.rs` | freedesktop thumbnail cache, in-process image decoding and Bubblewrap-confined external thumbnailers. |
| `opening.rs`, `mounts.rs` | What activating a file means (browse, open, run, launch) and the mount facts that decide program trust. |
| `actions.rs`, `process.rs`, `action_jobs.rs`, `local_actions.rs` | Custom-action catalog, argument expansion, process groups, and the bounded action queue. |
| `btrfs.rs` | Optional btrfs facts and fast paths: detection, whole-file clones, shared-extent proofs (ADR 0152). |
| `operations.rs`, `operations/` | `OperationEngine`: serialized filesystem mutations, the SQLite journal and history (`operations/store/`), transfers (`operations/transfer/`), Undo (`operations/undo/`), Recovery, Trash, deletion, copy backend, fingerprints. |
| `operation_jobs.rs`, `operation_jobs/` | `OperationJobs`: the shared operation queue, questions, progress, clipboard and drag preparation, Retry/Continue, archives, attention. |
| `operation_attention.rs`, `desktop_notifications.rs` | Unread attention and standard desktop notifications for finished work. |
| `recovery_catalog.rs` | Read-only, coalescing Recovery catalog reader. |
| `local_volumes.rs`, `local_volumes/`, `local_activity.rs`, `storage.rs` | GIO helper process, device requests and questions, network mounts, the global activity gate for device removal, and the persisted known-volume roots. |
| `network.rs` | Server address parsing, GVFS backend packages, FUSE paths. |
| `file_clipboard.rs`, `file_clipboard/` | File clipboard formats and the private Wayland `ext-data-control-v1` connection. |
| `archives.rs`, `bulk_rename.rs`, `templates.rs` | Archive create/extract, bulk rename rules and preview, Templates folder. |
| `configuration.rs`, `configuration/`, `interface.rs`, `interface_config.rs`, `preferences.rs` | The `files` config namespace, live reload, commands and shortcuts, global and per-folder view preferences. |
| `personal_folders.rs`, `file_properties.rs`, `history_labels.rs`, `i18n.rs` | XDG user directories, Properties metadata, operation labels, Fluent strings (`i18n/en`, `i18n/fr`). |
| `iced_desktop/` | The Iced frontend, compiled as the `desktop` module (`#[path]` in `lib.rs`): browser windows, views, toolbar, sidebar, menus, dialogs, Operations, Trash, Recovery, Properties, Preferences, action editor, drag and drop, accessibility. |

## Process model

### One owner per state directory

All Files state lives in `$XDG_STATE_HOME/canopa-files` (default
`~/.local/state/canopa-files`):

| File | Content |
|---|---|
| `runtime.lock` | Exclusive `flock` held by the owning process for its lifetime. |
| `operations.sqlite3` | Operation journal, Undo history, durable requests, Recovery records (WAL mode). |
| `views.json` | Per-folder view overrides. |
| `volumes.json` | Known volume roots with raw path, canonical path and device identity. |

The first `canopa-files` process takes the lock and becomes the owner. Later
launches find the lock held and forward their request over a private Unix socket,
`$XDG_RUNTIME_DIR/canopa-files-<hash>.sock`, whose name is derived from the
canonical state-directory bytes. `HOME` and `XDG_RUNTIME_DIR` must be absolute and
the runtime directory owner-only; a process that cannot reach the endpoint fails
instead of starting a second history writer. Stale endpoints are recovered only
under the lock, and a non-socket is never removed.

The socket carries launch intent only (Open, Quick Look, Resident, Quit), never
filesystem commands. Frames are versioned, bounded (64 KiB, at most 64 locations)
and keep raw path bytes; relative paths are resolved in the caller. A launch that
omits `--config`/`--defaults` inherits the owner's configuration selection; an
explicit different selection is refused. The acknowledgement means "accepted",
not "window mapped". Only an explicit "stopping" refusal is retried; a lost
acknowledgement is not, so a request is never duplicated.

### Lifetime

- The owner exits when it has no windows and no holds, unless it is resident.
  Holds come from accepted work, pending questions, clipboard ownership, unsaved
  results, undelivered attention and pending openings.
- `--daemon` makes the owner resident (starting one if needed); it runs in the
  foreground. `--quit` releases residency; the owner exits once the holds are gone.
  Neither answers questions nor cancels work.
- Iced is initialized on the first browser request, on the main thread, and keeps
  its session connection afterwards. A resident start needs no display connection.
- An owner started without `WAYLAND_DISPLAY`, `WAYLAND_SOCKET` or `DISPLAY` stays
  headless and refuses browser requests; it still accepts Quit. Display variables
  are never copied from a later caller into the running process.
- An update replaces the executable on disk; the running owner keeps its old code
  until it exits. It is never killed or replaced to upgrade it.

### Startup order

`Application::start` brings up configuration coordination, the volume helper, the
operation queue and the action queue before any window exists. The operation
queue opens history only after the first complete storage observation, and
rechecks ownership afterwards so that a shutdown racing readiness cannot create
history. Losing the volume helper invalidates storage and refuses further
filesystem changes until Files restarts.

The volume helper is the same executable re-run as
`/proc/self/exe --internal-volume-monitor`, because GIO's volume monitor needs its
own main context. It speaks bounded, versioned frames over a pipe and exits on EOF.

### FileManager1

The owner registers `org.freedesktop.FileManager1` at
`/org/freedesktop/FileManager1` with `DoNotQueue`: it never replaces or queues
behind another file manager, and failing to get the name does not stop Files.

| Method | Effect |
|---|---|
| `ShowFolders` | One browser window with one tab per distinct folder. |
| `ShowItems` | Items grouped by parent, one tab per parent, items selected. Explicitly requested hidden items are shown for that session without revealing their hidden siblings. |
| `ShowItemProperties` | A Properties window for the items, without a browser. |

A request is decoded and validated whole before anything is presented: at most
64 local `file://` URIs within 64 KiB, and a bounded `StartupId`. Invalid batches
are refused whole. The interface never runs commands, transfers or actions.
`packaging/file-manager/org.freedesktop.FileManager1.service` starts
`canopa-files --daemon`, so bus activation opts into residency.

### Desktop entry and launcher

`packaging/applications/org.canopa.Files.desktop` uses normal `Exec=canopa-files %U`
launching with `MimeType=inode/directory`; it does not declare `DBusActivatable`.
Installing it makes Files available as a folder handler; making it the default is
the user's choice. The Shell launcher's Quick Look uses
`canopa-files --quick-look PATH`, which opens a browser on the file's folder with
the file selected and then its Quick Look, through the same activation owner.

## Browsing

- A `Directory` is an immutable snapshot read on a worker. Each request carries a
  revision; a late result never replaces a newer view.
- Paths keep their raw Unix bytes end to end. Display strings are lossy only at the
  UI edge and never become operation targets or command arguments.
- A `Browser` keeps location, selection (focus, selected set, range anchor), scroll,
  per-folder state and a history of up to 256 locations, independently of how many
  columns the window shows. The requested folder loads first; ancestor columns load
  only once the window is wide enough to show them, and hiding a column cancels its
  read. Widgets are built only for visible rows.
- Going to the parent selects the child just left. An explicitly opened hidden
  folder stays visible in its own hierarchy while its hidden siblings stay hidden;
  toggling hidden files clears that exception.
- Displayed folders are watched: each tab's current folder and the active tab's
  ancestor columns, at most 256 folders, on one inotify instance. Events publish
  after 200 ms of quiet, or every second during a burst. A rescan keeps selection
  and scroll and prunes only vanished entries. GVFS folders are polled every 10 s
  instead. Beyond the watch budget, F5 still refreshes.
- List view folder sizes count children off-thread, capped at 10,000+, and are not
  counted on network folders.

## Operations engine

### Queue and engine

Every filesystem change goes through one queue, `OperationJobs`, which serializes
work on the `OperationEngine`. This covers Copy, Move, Rename, bulk Rename, New
Folder, New Document, Trash, Restore, Delete permanently, Empty Trash, archive jobs,
Recovery copies and Undo. Device removal waits for this queue and for running
custom actions through one global activity gate (`local_activity.rs`).

Accepted requests are durable: each is stored in `operations.sqlite3` with its
phase (Queued, Running, Failed, Remaining, FailedWithRemaining, Finished) and an
optimistic revision. At most 1,024 unfinished requests, 4 MiB per payload and
64 MiB in total are admitted. The Operations window shows running, retryable,
continuable and question rows plus the latest 100 finished rows.

Store rules:

- One writer per history: an advisory `flock` (`history_lock.rs`) shared by the
  engine and its lease-release worker; the last logical owner unlocks it explicitly,
  so a forked child's inherited descriptor cannot keep it alive.
- Every mutating transaction is `BEGIN IMMEDIATE`. No transaction is held across a
  question or a filesystem mutation. Busy handling is bounded (2 s).
- Catalogs, Undo labels and interrupted-operation summaries use a separate
  read-only WAL connection, so a waiting question never blocks inspection.

### Copy

All content copies share one prepared-copy lifecycle (`operations/prepared_copy.rs`):
Copy, Merge, Retry, cross-filesystem Move, Recovery export, saved-container
migration, Undo returns and New Document.

1. Register the private stage and payload in the journal.
2. Copy the bytes with `uucore::buf_copy::copy_with_progress` on held descriptors
   (`operations/copy_backend.rs`), checking that the copied count equals the output
   length. Large outputs are synced every 32 MiB (write-behind) so progress follows
   the device. On btrfs the whole source is first cloned into the empty output
   (`FICLONE`, `btrfs.rs`): no data is read or written and the copy shares the
   source's extents. Any refusal truncates the output and uutils copies instead.
3. Fingerprint the output once: size, mode, mtime, type and an XXH3-128 hash of the
   content or raw symlink target (`operations/fingerprint.rs`).
4. Apply attributes and metadata: permission bits, access and modification times,
   `user.*` extended attributes, the POSIX access ACL (and default ACL on folders).
   Holes reported by the source are kept. Attributes the destination refuses are
   listed in the result instead of failing the copy. `security.*` and `trusted.*`
   attributes follow the destination; hard links become independent files.
5. Sync, record a receipt, then publish with `renameat2(RENAME_NOREPLACE)`.

A copy that finished natively but has no host receipt stays "Preparing" and cannot
publish. Files never falls back to an overwriting rename.

### Batched small-file copy

Copying many small files one journal intent at a time cost about five SQLite
transactions and several syncs per file. Two mechanisms reduce that without
weakening the proofs:

- Durability spans (`operations/durability.rs`). Plain new-item Copy steps run with
  `synchronous=NORMAL` and recorded, not performed, filesystem syncs. A barrier runs
  `syncfs` on each destination filesystem and then syncs the database. It fires at
  1 s or 32 MiB, before any question, Replace or failure handling, and when the
  transfer returns. Move, Replace, Recovery, Undo and Trash keep a full sync on
  every step.
- Batches (`operations/copy_batch.rs`, `operations/store/copy_batch.rs`). At least
  8 consecutive plain Copy items (regular files or symlinks, same source folder,
  same destination folder, no reviewed replacement, no name clash when gathered)
  are copied as one batch of up to 64 files, 4 MiB of content or about 96 KiB of
  encoded intent, under one `Pending::CopyBatch` journal intent. Payloads are
  prepared in one stage folder, then published in order with `NOREPLACE`, stopping
  at the first failure. The batch takes five transactions instead of five per file.
  Each published file still gets its own cursor receipt and its own Undo step.

Crash semantics of a batch: in Preparing, restart removes only registered payloads
and the batch is copied again; in Forward, restart appends the provable published
prefix (anything that is not a prefix stops for attention); in Cleanup, it finishes.
A name taken during the batch sends the rest of the batch back to the single-item
path, which asks the usual question; a failing source fails only its item.
Batched files appear in the destination together. Fewer than 8 files keep exact
per-file visibility.

The target for this design is copying 10,000 files of 4 KiB on ext4 within 5x the
time of `gio copy` followed by `sync` (`scripts/measure-files-copy.py` measures it).
A power loss inside an open span can lose up to about 1 s or 32 MiB of new copies;
sources are never at risk and every outcome stays consistent.

### Move and Rename

- Same filesystem: the entry is pinned (`O_PATH|O_NOFOLLOW`), both parents are
  opened, and the entry is moved with descriptor-relative `renameat2` `NOREPLACE`.
  Directories pass through a hidden stage in the destination parent, which avoids
  a `..` update that a read-only directory would refuse.
- Across filesystems: commit and verify the destination copy, rename the source
  into an owned quarantine on its own filesystem, re-read and verify it against the
  copied snapshot, then remove it. Two btrfs subvolumes count as two filesystems
  (separate devices); there the copy is a clone, and when `FIEMAP` proves the
  source and copy share every extent their contents are read once instead of twice
  (`Fingerprint::read_opened_pair`). A mismatch restores the source; if its name is
  taken meanwhile, the work is kept for Recovery. The source is never removed
  before the destination is committed.
- Rename authority is prepared before the name dialog opens (`PreparedRename`): the
  leaf and its parent are pinned and a storage snapshot is captured, so a
  replacement put at that name while the dialog is open is not renamed.
- Bulk Rename (`operations/rename_batch.rs`) runs ordinary journalled Rename claims
  under one history entry; swaps are refused and chains are ordered.

### Trash and deletion

- Trash follows the freedesktop specification (`operations/trash.rs`): the main
  store is `$XDG_DATA_HOME/Trash`, used when the source shares both mount and
  device with it; other filesystems use a validated sticky `.Trash/$uid` or
  `.Trash-$uid` at their top. Inside a btrfs subvolume the top is the subvolume's
  own top (the highest folder of the mount on the source's device, as in GIO).
  Such tops are recorded in `$XDG_DATA_HOME/canopa-files/trash-tops.json` before
  the item moves, so Trash discovery lists them with the mount tops. A record
  keeps the btrfs subvolume's identity, so a renamed or remounted subvolume is
  found again; an unreachable top keeps its record, and when 256 are recorded a
  new top replaces only an empty one that no Undo entry refers to, or fails
  before moving anything. The `.trashinfo` is reserved before the payload
  moves, and trashing is an atomic rename on the source filesystem. Trash never
  falls back to deletion; when a volume has no usable Trash, Files offers a new,
  confirmed permanent deletion of exactly those items. GVFS FUSE mounts get no Trash.
- Restore never overwrites and validates inside the operation lock. Items with
  missing info stay listed and can be copied out. One Trash batch is one Undo
  entry; expiring history never empties Trash.
- Delete permanently and Empty Trash (`operations/deletion.rs`) are confirmed
  queue jobs with Cancel focused. Removal is descriptor-relative, never follows
  links or crosses mounts (depth limit 384). For Trash items the payload goes first,
  then the `.trashinfo`, then `directorysizes` is rewritten atomically. They are not
  journalled, not undoable and never replayed.

### Conflicts

| Incoming vs existing | Choices |
|---|---|
| File or link over file or link | Replace, Keep both, Skip, Cancel |
| Folder over folder | Merge, Keep both, Skip, Cancel |
| Different kinds | Keep both, Skip, Cancel |
| Recovery copy | Keep both, Skip (and Merge for eligible folders); never Replace |

Keep both picks a numbered free name (`report (2).txt`); pasting into the same
folder does too. "Apply to remaining" is separate for file and folder conflicts.
The destination is rechecked after the answer, including in-place edits. Replace
keeps the existing item in a private stage as a saved version for Undo. Every
question focuses Cancel first, so Return never picks a destructive answer.

### Placement checks

Storage placement is captured when a request is made: sources at Copy/Cut (or at
drag start), the destination at Paste or Drop. Checkpoints before each item, after
answers, during the copy, before commit and around source quarantine compare the
current mount and device identity with the captured one. A changed destination
pauses the job with an exact continuation; a changed source fails only that item.
Retry keeps the original media requirement, so replacement media or a retargeted
alias never satisfies an old request. On GVFS, a forced-sync `statx` of the
destination root after a failure tells a disconnection apart, and the transfer
pauses instead of failing every remaining item.

## Journal invariants

The journal exists so that a crash at any instant leaves every byte either where
the user put it or in Recovery, and so that Undo never acts on something it did
not create. These rules apply to every operation:

1. **Durable intent before mutation.** Each filesystem step writes and syncs its
   intent (and its parent directory) before the system call. The intent carries a
   digest binding it to the history identity, the journal lane and the original
   history file, so a copied or replaced database cannot authorize work.
2. **No overwrite, ever.** Commits use `RENAME_NOREPLACE` through held parent
   descriptors. An occupied name keeps both the data and the journal.
3. **Identity, not pathnames.** Each step records path, logical identity and
   expected physical identity. Restart reopens recorded paths only as lookups and
   requires them to match the recorded originals: a kernel file handle, a unique
   mount ID (`STATX_MNT_ID_UNIQUE`) and the boot ID (`operations/original.rs`).
   Inode numbers and ordinary mount IDs are reused, so replaying by path could
   overwrite later work or adopt a replacement.
4. **Ownership is per history reference.** Undo may remove or move only what the
   recorded operation created or moved, as verified by identity and fingerprint.
   If Copy created A at a path and another program then put B there, the older Undo
   must not delete B.
5. **Each name is consumed once.** Multi-step work persists its phase before the
   single live call (Prepared, Taking or Returning, Consumed, ...). A restarted
   phase never repeats its call; an uncertain outcome is kept for Recovery. A
   failure that is proven not to have moved anything (held-descriptor proof after
   `EACCES`/`EPERM`) may be retried.
6. **Cleanup is empty-only.** Files never removes anything recursively on its own
   initiative. It removes only original empty roots through their original parent.
   Completed output belongs to the user and is never reopened or rewritten. A
   reversal and its cleanup right commit in one transaction.
7. **Disposal requires reproducibility.** Before discarding a partial copy, Files
   reopens the original source and compares the whole partial prefix. Edited
   partials, missing sources and replacement sources are kept.
8. **Exact completion evidence.** Filesystem success and the saved result are
   separate. Only a live successful return writes a completion receipt; an empty
   journal or a path that looks right never implies success. If saving the result
   fails, an in-memory witness allows "Retry Save" and keeps the application alive.
9. **Transactions check their own effects.** Deletes and inserts are read back;
   any mismatch rolls back the whole handoff.

There are two journal lanes. Recovery copies use their own lane, so salvaging data
never clears the original intent; the rescue lane is reconciled first. History
expiry and garbage collection wait while either lane is pending.

## Undo

- Undo is shared by every window and tab and survives restart. It reverses the
  latest completed entry; Ctrl+Z waits for active work. History keeps the latest
  100 operations (`HISTORY_LIMIT`). There is no Redo.
- Covered: Copy, Move, Rename, bulk Rename, New Folder, New Document, Trash,
  Recovery copies, Merge and Replace. Not covered: permanent deletion, Empty Trash,
  archive jobs, custom actions, Properties permission changes, mount and unmount.
- Undo is interactive. Whenever the current state differs from what the entry
  recorded, it asks instead of guessing, and every answer rechecks the observation
  it was shown:

| Situation | Choices | Why |
|---|---|---|
| Original name now taken | Keep both, Replace (files and links; the newer item goes to Recovery), Skip | Newer work at the original path is never destroyed. |
| A copied item changed since the copy | Keep item, Move changed item to Recovery, Skip | Undo of a copy may only delete what it created. |
| A moved item was replaced by another | Return item, Keep item, Skip | The replacement is not the moved object. |
| A moved item or saved version cannot be inspected | Skip, Finish step, Restore previous destination (when available) | Unreadable is never treated as empty. |
| A saved previous version changed | Use saved version, Keep item, Skip | Approving the version does not authorize overwriting the current occupant. |
| A created folder now has contents, changed or vanished | Keep item, Skip (Finish step when unavailable) | Folders are removed only empty, by atomic `rmdir` of the owned identity. |
| A partial Merge folder moved or changed | Finish this Merge, Skip | Returned children stay; saved versions are protected. |
| New items in a merged destination | Move remaining items, Keep remaining items, Skip | New contents are never silently added to the old snapshot. |

- Merge Undo returns each recorded child individually inside the original entry;
  unfinished Merges are exempt from history expiry. Cross-filesystem Undo rebuilds
  and verifies the item on the original filesystem before displacing anything.
- Keep both during Undo relocates older history to the new name in the same
  transaction, so older entries still find the item.
- Noninteractive callers get refusals for every ambiguous case.

## Restart recovery and the Recovery catalog

Startup never replays a destructive action. It reconciles pending intents against
actual filesystem identities, records what committed, and cleans only owned empty
stages. Unexpected contents are kept.

- Exact completed or stopped results are restored as metadata without touching files.
- Durably stopped transfers whose parts reopen with verified originals get their
  Retry, Continue and Abandon controls back. A running transfer whose traversal
  cursor and history suffix match the receipts bound before mutation becomes a
  stopped result that can continue; completed paths are never re-walked.
- Cut and clipboard intent and custom actions are never rebuilt. Requests without
  proof show as Interrupted with no controls. Proof is same-boot only, so after a
  reboot interrupted work is reviewed in Recovery.
- An unresolved journal keeps the engine available for browsing and Recovery but
  refuses ordinary mutations until it is reconciled. Recovery opens once at startup
  when review is needed. Reconcile is an explicit request that rechecks both lanes
  under the engine lock and never authorizes overwriting or deleting.
- "Keep in Recovery" turns an interrupted task into protected Recovery data in one
  transaction: it archives the available leaves and whole work folders, retires
  only the captured journal binding and moves nothing.

The Recovery catalog (`recovery_catalog.rs`, `operations/recovery.rs`) lists saved
versions still needed by Undo, protected data that needs explicit recovery, and
interrupted work, including unavailable entries. It is one read-only, coalescing
reader per owner (150 ms), in pages of 256. Saved versions live in hidden
`.canopa-files-stage-*` folders beside the original item, on the same filesystem.

- When an Undo entry leaves the 100-entry history, its saved versions are archived.
  Unprotected archives are later removed by a journalled expiry that checks the
  parent and stage identities; protected data never expires. Shared saved data is
  removed only by its last eligible owner.
- A Recovery copy re-resolves the saved source, never consumes it, refuses an
  occupied path, uses its own journal lane and becomes an ordinary Undo entry.
- Restricted folders that an operation created are entered by borrowing owner
  read and search access only, through recorded identities, with a durable grant saved
  before each `chmod` and restored in reverse order.

## Clipboard

Copy and Cut publish `x-special/gnome-copied-files`, `text/uri-list` and
`application/x-kde-cutselection` on Files' own Wayland connection using
`ext-data-control-v1`, separate from Iced's text clipboard
(`file_clipboard/wayland/`). This interoperates with GTK and KDE file managers.
Reads accept local file URIs only, up to 16 MiB within 5 s; `..` spellings and
unescaped whitespace are refused.

Copy/Cut prepares the selection before publishing (`PreparedSelection`): each entry
is pinned, its storage binding captured and a process-owned source lease taken, so
a later Paste uses those entries and never adopts a replacement placed at the same
path. External selections are observed at Paste. Only the newest Copy/Cut publishes;
a Cut's remainder only shrinks, and only after a saved result. Source leases are
dropped when history opens, so clipboard authority never survives a restart.
Descriptor pins are capped at min(4096, soft limit/4), with temporary credit for a
replaced selection.

## Drag and drop

The drag transport comes from the kevinard/iced and kevinard/winit forks
(ordinary-window drag handles on Winit's Wayland connection); Files owns the
policy in `operation_jobs/drag_drop.rs` and the gesture in `iced_desktop/drag_ui.rs`.

- The default action is Move on the same mount and Copy otherwise; a mixed or
  unknown selection is copied. Ctrl forces Copy, Shift forces Move.
- A symlink belongs to the filesystem holding the link.
- Each destination path's first capture is kept for the whole gesture (up to 256
  per gesture), so a view refresh cannot redirect a hover to a replacement folder.
  Dropping onto the items' own folder is refused.
- Foreign drops are read again after Drop and must decode to the same paths.
- Files never deletes a source because a drag peer asks; the receiver owns the
  Move. A native drop is acknowledged only after the backend result.
- Escape is handled in native keyboard dispatch, so a latched drop cannot overtake it.
- Tabs can also be dragged within and between windows or out to a new window.

## Devices

- The GIO helper reports drives, volumes and mounts. Mounting happens only on an
  explicit request (opening an unmounted device); there is no autorun and no force flag.
- Removal prefers Power off (safe drive stop), then Eject, then Unmount. It waits
  for the operation queue and all running custom actions, because scripts can touch
  any path; new work waits while removal is pending. A lost helper reply is
  reported as uncertain, never as success.
- Devices are addressed by opaque tokens that expire with each observation; labels,
  UUIDs and paths are never targets. Media without a UUID requires the original
  mount epoch.
- Credentials go only into the helper's pipe; they never appear in snapshots,
  history or logs, and Files stores none. Each question has its own token.
- A completed mount navigates only if the window, tab and navigation state that
  asked are unchanged.

## Network locations

GIO/GVFS is the only network seam (`network.rs`, `local_volumes/`,
`iced_desktop/network_ui.rs`). Connect to Server accepts `smb`, `sftp` (and
`ssh` as an alias), `ftp`, `ftps`, `dav`, `davs` and `nfs` URIs plus
`\\server\share`, up to 2 KiB; addresses containing a password are refused.
Mounting and password or host-identity questions run through the helper. Browsing
uses the GVFS FUSE path under `$XDG_RUNTIME_DIR/gvfs`; without FUSE there is only a
read-only listing (at most 2,000 entries). Phones and cameras come from the GVFS
`mtp`, `gphoto2` and `afc` backends. Missing backends are reported with the Arch
package that provides them (`gvfs-smb`, `gvfs-nfs`, `gvfs-mtp`, `gvfs-gphoto2`,
`gvfs-afc`, `gvfs-dnssd`, `gvfs-wsdd`). Server bookmarks are kept in the GTK
bookmarks file through `canopa_file::gtk_bookmarks`, byte-preserving.

GVFS locations get no Trash, no inotify, no thumbnails by default, no folder item
counts and no program trust. Copies check that the read-back length equals the
source size.

## Archives

`archives.rs` and `operation_jobs/archive.rs` use pure-Rust crates (zip, tar,
flate2, lzma-rust2, bzip2, zstd). Extraction reads zip, tar, tar.gz/tgz,
tar.xz/txz, tar.zst/tzst and tar.bz2/tbz2/tbz, detected by content first, then
suffix; creation writes zip, tar.gz, tar.xz and tar.zst.

- The result is written under a hidden `.canopa-archive-*.partial` or
  `.canopa-extract-*.partial` name and published with a no-replace rename to the
  first free name, so nothing is ever replaced and there is no conflict question.
  An archive with one top-level item extracts to that item; otherwise extraction
  produces a folder named after the archive.
- Entries that would escape the folder (absolute paths, `..`, symlinks leaving it,
  hard links to anything but an earlier regular file) are skipped; symlinks are
  created last. Output is bounded by free space minus a 64 MiB reserve and by
  1,000,000 entries. Set-id and sticky bits are never restored.
- Archive jobs are cancellable queue jobs. They are not undone and not resumed;
  after a crash only the job's own partial result is removed.

## Thumbnails

`thumbnails.rs` uses the freedesktop thumbnail cache. PNG, JPEG, GIF and WebP are
decoded in-process with the Quick Look limits. Other types use the installed
thumbnailers, and PDFs use Poppler's `pdftoppm`, inside Bubblewrap
(`/usr/bin/bwrap`): all namespaces, a cleared environment, the input read-only from
a held descriptor and only `/output` writable, with limits of 15 s wall time, 30 s
CPU, 2 GiB address space, 32 MiB output, 256 descriptors and no core dumps. Output
is always re-encoded. Without `bwrap`, no external thumbnailer runs. Failures are
recorded in the cache's `fail/` folder. Three workers serve a queue of 512. The
default `show "local"` policy skips network and removable media. Rows in List and
Columns show thumbnails for pictures and videos only.

## Quick Look and previews

One Quick Look window per browser window. It is a transient window that follows its
browser's active tab and selection (including search results), and closes with its
source. Trash and Recovery windows own their previews the same way. Default keyboard
focus stays on browsing in the source window; Tab enters the preview's controls.

The file is opened once on a worker (non-blocking, validated as a regular file) and
read through that descriptor. Limits: 128 MiB encoded input, images up to 16,384 px
per side and 256 MiB of decoder memory, PDF pages rendered up to 1,800 px through
`pdfinfo` and `pdftoppm` from a private snapshot, text up to 1 MiB with syntax
highlighting up to 128 Ki characters.

Audio and video use one GStreamer `playbin` per preview, fed from the prepared
reader through `appsrc` rather than the pathname, with video frames to an RGBA
appsink (at most 1280x720, one buffer). Leading ID3 tags are skipped for the decoder
to avoid a GStreamer abort. Playback initializes lazily, opens paused and stops when
the selection changes or the preview closes; ordinary browsing and `--daemon`
create no pipeline. Decoder errors are logged with the `[canopa-files media]` prefix.

## Search and Recent

`search.rs` matches names case-insensitively in the current folder, or recursively
with Include subfolders. It respects hidden-file visibility, never follows directory
symlinks and deduplicates visited (device, inode) pairs. Filters run before content
reads: Type (from the name's MIME type, no file read), Modified, Size, and Search
contents (text files up to 16 MiB with no NUL byte in their first 8 KiB). The
initial budget is 50,000 results; More results extends it, Stop keeps what was
found. There is no indexer.

Recent (`recent.rs`) reads `$XDG_DATA_HOME/recently-used.xbel`: local `file:` items
only, newest first, deduplicated, at most 500; a missing, malformed or oversized
(over 16 MiB) list shows nothing. It is presented as a search scope, so the query
and filters narrow it. Files records regular files it opens with an application,
editing the list as text, writing it atomically and retrying up to three times if
another program replaced it. Recording failures are only logged.

## Opening, Open With and custom actions

### Launch trust

`opening.rs` classifies an activated item on a worker before anything runs:

- Folders are browsed.
- Documents open with the `mimeapps.list` default (`canopa_apps::mime_apps`, types
  from `canopa_file::mime`); without one, the Open With dialog appears.
- A file counts as a program only if it is regular, owner-executable, passes
  `access(X_OK)`, lives on a local mount that is not `noexec`, removable or a
  network share, and starts with an ELF header or `#!`. Programs always ask: Run,
  Run in Terminal, Open (scripts) or Cancel.
- A desktop entry is trusted if it lives in an XDG `applications` directory or
  passes the program rules. Untrusted entries ask: Launch once, Open as Text or Cancel.
- Launches use `gio launch` or `gio open` with the activation token and never pass
  through a shell. Terminal programs and Open in Terminal use `xdg-terminal-exec`.
  Trash and Recovery only open files; they never run a stored program.

### Custom actions

Custom actions (`actions.rs`, `process.rs`, `action_jobs.rs`) are commands the user
invokes on a selection. They never run automatically.

- The command is split into an argument vector with quoting rules and never run
  through a shell; the executable is literal. `%f`/`%F` expand to the first/all
  paths, `%n`/`%N` to basenames, `%d` to the current folder, `%%` to a percent sign;
  `%F` and `%N` must be a whole argument.
- Each invocation captures the action, the selected paths, the working folder and
  its canonical path, plus a storage snapshot. Before running, the worker checks that
  the folder still resolves to the same place and that the folder, each entry and
  its referent are still on the captured storage.
- Commands run in their own process group; Cancel sends SIGTERM, then SIGKILL after
  250 ms. At most 4 run at once, with 1,024 admitted slots, 100 finished summaries
  and 64 KiB of output per stream.
- Commands own their effects: they create no Undo entries and are never replayed
  after a restart.

## Preferences and configuration

Settings live in the `files` namespace of the shared `config.kdl` (`configuration.rs`,
`interface_config.rs`), validated strictly by Files' own schema; `canopa config check`
calls `canopa-files --check-config`. Per-folder view overrides are private state in
`views.json` and store only the fields deliberately changed; Use defaults clears them.

Live reload (`configuration/jobs.rs`, `configuration/reload.rs`):

- One serialized worker polls the selected source every 250 ms and needs two equal
  observations before loading a candidate. Settings and appearance are prepared as
  one immutable candidate.
- An invalid candidate keeps the complete last valid settings and appearance and
  shows a banner in browser windows. Open editor drafts are not reset.
- `--defaults` never watches. The worker is bound to its original source selection;
  an editor cannot redirect it.
- Graphical saves (Preferences, the action editor) rewrite only the `files`
  namespace, keep other namespaces, comments and symlinks, write atomically under
  `.canopa-files-edit.lock`, and refuse a stale draft.

## Accessibility and input

- Every window publishes an AT-SPI tree through the shared `canopa-ui` bridge
  (`iced_desktop/accessibility_ui.rs`); nothing is collected while no assistive
  technology listens. Tree keys use lossless path components, not translated labels.
- Every feature is reachable from the keyboard, including in narrow tiles: menus
  open with Shift+F10 or the Menu key, the Places button replaces a hidden sidebar,
  focus rings appear after keyboard use, and arrow semantics are mirrored for
  right-to-left languages. Unmodified keys that typing or selection need cannot be
  bound to commands.
- Strings are Fluent (`i18n/en`, `i18n/fr`).

## Design decisions

**Iced and `canopa-ui`, not GTK.** Files uses the same Iced stack and shared
controls as Shell and Notify. Sharing palette values through a GTK adapter would not
give the same controls, focus treatment or maintenance model. Consequence: Wayland
drag and drop, which upstream Iced did not support, is implemented in the
kevinard/iced and kevinard/winit forks; Files windows are ordinary toplevels, not
layer surfaces.

**Renderer-independent core and runtime.** Workflow policy (admission, retention,
journal, decisions) lives below the frontend, behind coarse typed interfaces
(`OperationJobs`, `ActionJobs`, `ConfigurationJobs`, `DeviceRequests`). Reason:
mixing policy with widgets made it untestable and fragile. Consequence: the core
builds and tests without Iced, dropping a frontend handle never cancels accepted
work, and snapshots are bounded and coalesced.

**One resident-capable owner per state directory.** A single process owns history
and activates later launches over a private socket carrying launch intent only.
Reason: two history writers would break every journal invariant, and a resident
owner opens new windows quickly. Consequence: the frontend shares the process; there
is no crash isolation between the UI and running transfers. A separate frontend
process remains possible if crash isolation justifies the transport.

**FileManager1 on the owner, opt-in activation.** Reason: other applications'
"Show in folder" should reach the same owner as CLI launches. Consequence: the name
is requested without replacing or queueing; the Arch package keeps the activation
file under `/usr/share/canopa/dbus-1/services/` so installing Canopa does not take
over another file manager. Activation starts `--daemon` and therefore opts into
residency.

**Configuration in the shared envelope.** Files owns the `files` namespace of
`config.kdl` with its own strict schema and headless validator; there is no second
configuration file and no legacy fallback. Per-folder views and history stay private
state.

**Distribution libraries, no bundled codecs.** GStreamer, Poppler, GIO and GVFS come
from the distribution. Reason: codec licensing and security updates belong to the
distribution. Consequence: decoding is a runtime property that `--check-config` or
`--help` cannot verify.

**Raw path bytes everywhere.** Paths are never converted through display strings.
Reason: non-UTF-8 names exist and a lossy round trip could target the wrong file.

**Durable intent, identity-bound history, no overwrite.** See
[Journal invariants](#journal-invariants). Reason: a file manager must never lose
data on a crash, and Undo must never reverse someone else's later work. Consequence:
Undo is interactive, sometimes refuses, and keeps more data in Recovery than a
path-based design would.

**SQLite journal.** History, requests and Recovery share one WAL database. Reason:
rewriting a growing JSON file per completed item made transfers quadratic.

**Interactive Undo with protected Recovery.** Undo asks whenever the current state
differs from the record, and every destructive alternative keeps the displaced data
in Recovery. Protected data survives history expiry. Reason: users expect Undo to be
safe even after other programs touched the files.

**Restart restores proven state, never replays.** Reason: replaying from pathnames
could overwrite later work or adopt replacements. Consequence: proof needs the
recorded kernel identities of the same boot; anything else is review-only.

**uutils moves the bytes; Files keeps Move.** Content copy uses
`uucore::buf_copy::copy_with_progress` on held descriptors (uucore pinned to the
kevinard/coreutils fork in the root `Cargo.toml`). `uu_cp::copy` was rejected: its
options take only paths (a `/proc/self/fd/N` operand is copied as a symlink) and it
has no progress, cancellation or conflict hooks. Move is not delegated to `uu_mv`,
which performs the rename and its cross-device copy-and-remove itself with no hook
between the copy and source removal, while Files must commit the destination first.
uucore also keeps exit status in one process-global atomic, which cannot isolate
concurrent operations.

**btrfs as an optional fast path.** Clones replace copied bytes, shared extents
prove equal contents, and subvolumes get their own Trash, all detected per
descriptor with generic fallbacks ([ADR 0152](../adr/0152-files-btrfs-clones-and-subvolume-trash.md)).
A clone moves no bytes, so it is not a second data mover. btrfs offers no
multi-file transactions; the journal and its proofs stay.

**Trust the upstream copy, fingerprint once.** The source is not re-read to compare
a finished Copy; the output is fingerprinted once with XXH3-128. Reason: the check is
non-adversarial and SHA-256 dominated large-file CPU. Destructive steps
(cross-filesystem source removal, cleanup, Undo returns) still compare full content.
Journal and binding digests stay SHA-256.

**Batched small-file Copy.** See [Batched small-file copy](#batched-small-file-copy).
A rejected alternative used one virtual journal lane per batch item: the journal is
lane-parametric, so it would reuse every single-item check, but it would amortize only
commits, WAL frames and shared-memory locks (about 20% of the measured cost) while
per-item binding, digest and cursor verification (about 30%) would still run per file.
It remains the fallback if the batch design proves intractable. Cached SQL statements
and single-observation checkpoints (`operations/proofs.rs`) cut the remaining cost
without changing the number of checkpoints per file.

**Copies keep attributes, not hard links.** `user.*` attributes, ACLs and holes are
kept and never fail a copy. Hard links become separate files, because preserving a
link group would need a new journal state.

**Archives publish, never replace.** Pure-Rust formats, a hidden partial and a
no-replace publish mean there is no archive conflict question and no partial result
under a visible name. Archives are not undone or resumed.

**Permanent deletion is explicit.** Trash never silently falls back to deletion;
deletion is a separately confirmed, non-undoable request.

**Sandboxed thumbnailers.** Third-party thumbnailers parse untrusted files, so they
run in Bubblewrap with tight limits, and their output is re-encoded.

**Program trust from mount facts.** Executables and desktop entries run only after an
explicit choice, unless they are installed applications or local executables that
pass every rule; removable, network and `noexec` mounts never confer trust.

**Custom actions are argument vectors, run on demand.** No shell, no automatic
folder rules, no Undo, no replay.

**GVFS for network, phones and cameras.** Reason: GVFS already implements the
protocols and credentials. Consequence: browsing needs GVFS FUSE, and network folders
lose Trash, inotify and thumbnails.

**Recent through the shared xbel list.** Files reads and records the same list as GTK
and Qt applications instead of keeping its own.

**Search without an indexer.** Filters run before reads and the result budget is
explicit, so search stays bounded without a background index.

**Displayed folders follow outside changes.** Watching is limited to what is shown
and never changes selection or interrupts operations.

**Live preferences.** Choices save immediately and typed fields on Enter or Ctrl+S;
an invalid external edit keeps the last valid state.

## Limits

| Limit | Value | Code |
|---|---|---|
| Undo history | 100 operations | `operations.rs` |
| Unfinished requests | 1,024; 4 MiB each, 64 MiB total | `operations/store/requests.rs` |
| Finished Operations rows | 100 | `operation_jobs.rs` |
| Copy batch | 8 to 64 files, 4 MiB, ~96 KiB intent | `operations/copy_batch.rs` |
| Durability barrier | 1 s or 32 MiB | `operations/durability.rs` |
| Progress updates | every 250 ms | `operation_jobs/transfer.rs` |
| Tab history | 256 locations | `browse.rs` |
| Watched folders | 256; 200 ms quiet, 1 s maximum | `folder_watch.rs` |
| Folder item count | 10,000+ | `iced_desktop/folder_counts.rs` |
| Search | 50,000 results per step; contents up to 16 MiB | `search.rs` |
| Recent | 500 items; list up to 16 MiB | `recent.rs` |
| Clipboard and drop payload | 16 MiB, 5 s | `file_clipboard.rs` |
| Drag destinations | 256 per gesture | `operation_jobs/drag_drop.rs` |
| Launch and FileManager1 requests | 64 locations, 64 KiB | `activation.rs`, `presentation.rs` |
| Archive extraction | 1,000,000 entries; free space minus 64 MiB | `archives.rs` |
| Templates | 100 | `templates.rs` |
| Custom actions | 4 running, 1,024 slots, 64 KiB output per stream | `action_jobs.rs`, `process.rs` |
| Deletion depth | 384 | `operations/deletion.rs` |

## Testing

Run focused tests from the workspace root:

```sh
bash scripts/check.sh --files --lib [FILTER]
bash scripts/check.sh --files --test CASE [--test CASE ...] [FILTER]
```

Ordinary cases (`tests/*.rs`) share the `integration` executable, selected by module.
Crash-boundary tests kill a child worker at a journal step and reopen the history.
btrfs cases (`btrfs.rs` unit tests, `cases::btrfs`, and the clone cases in
`operations::source_removal_boundary`) run when `CANOPA_FILES_TEST_BTRFS` names a
writable btrfs folder and return early otherwise.
Filesystem tests use private temporary directories; device tests replace the GIO
storage provider with a private module. Native window, clipboard and drag acceptance
runs in the private desktop harness (`scripts/run-desktop-acceptance.py`,
`scripts/verify-files*.py`). See [testing](../testing.md).

## Known limitations

- There is no Redo.
- Undo keeps only the latest 100 operations; protected Recovery data outlives them.
- The frontend and the operation engine share one process; a UI crash stops running transfers until Recovery on the next start.
- Custom-action requests and results are kept in memory only; quitting or a crash loses queued actions and their summaries.
- Custom actions check paths against the captured storage but do not pin the selected entries; a file replaced at the same path on the same storage is used.
- Undo of an ordinary Move is not proven crash-safe if Files is killed right after the return and the original name is recreated before restart.
- Restart continuation needs Linux 6.8 or newer (`STATX_MNT_ID_UNIQUE`) and the same boot; otherwise interrupted work is review-only in Recovery.
- A power loss during a batched Copy can drop up to about 1 s or 32 MiB of new copies.
- Hard links are copied as separate files; `security.*` and `trusted.*` attributes are not copied.
- Properties access changes and "Allow executing as a program" are applied directly by path and are not in Undo.
- Permanent deletion, Empty Trash, archive jobs, mount and unmount are not undoable.
- Network (GVFS) folders have no Trash, no live watching (polled every 10 s) and no thumbnails by default; browsing them needs GVFS FUSE.
- Recent items cannot be removed or cleared from Files.
- XDG user directories are read at launch; restart Files after changing them.
- `StartupId` from FileManager1 is carried but not used as an activation token for focus.
- On Niri without the Smithay fix shipped in `canopa-compositor`, cancelling a drag may still deliver a drop.
- A running owner keeps its old executable after an update until it is quit.
