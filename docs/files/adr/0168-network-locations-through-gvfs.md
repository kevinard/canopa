# 0168: Network locations, phones and cameras through GVFS

Status: implemented with protocol, Desktop model and native GVFS verification,
2026-11-11. Native acceptance of the Iced window and sidebar in a session, and
hands-on checks against real SMB, SFTP and MTP devices, are still pending.
Extends the device helper of ADR0020, ADR0091 and ADR0093, which kept
network-only volumes and mounts without a local folder out of scope, and
integrates with ADR0163 to ADR0167.

## Decision

GIO and GVFS stay the only platform seam, as the distribution plan (W9) asks.
Every mount, credential question and listing runs in the existing GIO helper
process; Files gains no second stack, no service and no credential storage.

### Connect to Server

**Connect to Server…** (the `connect-to-server` command, and Other Locations…
in the sidebar) opens one window per session with an address field, an
**Add to bookmarks** choice, the bookmarked servers and what the network
announces. Typing an address in the location bar does the same without the
window.

`crate::network::ServerAddress` accepts `smb://`, `sftp://` (and `ssh://`,
rewritten to `sftp://`), `ftp://`, `ftps://`, `dav://`, `davs://` and `nfs://`,
plus Windows `\\server\share` paths. It refuses a password in the address (it
would end up in bookmarks and history), queries, fragments, control characters
and addresses over 2 KiB, and explains `http(s)://` (use `dav(s)://`). The
helper reparses the address and requires the exact normalized URI, so the parent
cannot hand GVFS anything the parser did not accept.

The helper mounts with `g_file_mount_enclosing_volume` and the request's own
`GMountOperation`, so it is one ordinary device request: one at a time, cancellable,
holding a LocalActivity work hold, reported in Operations and in notifications.
An already mounted location succeeds at once. The result names the mount root
and the requested folder, both GVFS FUSE paths. The helper waits up to two
seconds for the volume monitor to list a new mount before it publishes the
outcome, so the parent sees the mount among its prepared Storage observations;
the Browser tab then navigates as for a mounted device (ADR0093), only while its
navigation intent is unchanged. The window closes when connecting starts: its
questions and the folder belong to the Browser window that asked.

### Questions and remembered passwords

Password questions keep ADR0091's flow. When GVFS sets
`G_ASK_PASSWORD_SAVING_SUPPORTED`, the dialog offers **Forget the password
immediately**, **Remember it until you log out** and **Remember it in the
keyring**; the answer passes `G_PASSWORD_SAVE_NEVER`, `_FOR_SESSION` or
`_PERMANENTLY`. GVFS then keeps the password in the Secret Service through its
own keyring integration. Files never writes it anywhere, and refuses a remember
choice for a question that does not support saving, in the parent and again in
the helper. An anonymous login always uses `NEVER`. Domain and user name fields
appear when GVFS asks for them. Wrong credentials make GVFS ask again; the new
question replaces the old one.

Other `ask-question` callbacks are choices, as before. A server question is
titled **Server question**, and an SSH host-key question (its message mentions
the identity, host key or fingerprint) **Check the server's identity**, with a
line explaining what confirming means. The provider's own cancel choice
("Cancel", or SSH's "Cancel Login") folds into the shared Cancel button.

### Browsing: FUSE first, GIO listing as a fallback

Files browses network locations through GVFS's FUSE folder
(`$XDG_RUNTIME_DIR/gvfs/…`), where `g_file_get_path` puts them when
`gvfsd-fuse` runs (the `gvfs` package starts it by default). This choice keeps
every other part of Files unchanged: the Browser, the operations engine with its
descriptor checks, Undo, Recovery, Quick Look, Open With, custom actions,
drag and drop and the clipboard all work on Unix paths, and other applications
receive paths they can open.

Without `gvfsd-fuse` (no `/dev/fuse` in a container, for example) a mounted
location has no local folder. It is still listed (`Device::remote_only`), and
opening it shows a **read-only GIO listing** in the Connect to Server window:
the helper enumerates the URI (`Enumerate`, at most 2,000 entries, four
listings at once, 30 s each), folders can be entered and files opened with
their default application through `gio open URI`. Files deliberately does not
make a URI-backed Browser: every operation would need a second, weaker
implementation, and a URI held as a relative `PathBuf` could resolve against
the working directory.

### Sidebar and bookmarks

The catalog now classifies devices by their URI scheme
(`crate::network::LocationKind`): local storage, network locations, phones
(`mtp://`, `afc://`) and cameras (`gphoto2://`). GVFS's MTP and gPhoto2 volumes,
which were filtered out because their activation root has no path, now appear
under **Devices** with their own icons and mount through the same Open path.
A **Network** section lists mounted network locations with **Disconnect**
(the ADR0091 unmount, never forced), the bookmarked servers (opening one
connects, which is immediate when it is already mounted; each can be removed),
and **Other Locations…**.

Other Locations lists `network:///`, GVFS's merged view of Avahi/DNS-SD,
WS-Discovery and SMB browsing, through the same helper listing. An SMB host is
entered to list its shares; a share or any other server fills the address.
Missing discovery backends are reported by name.

Files reads `$XDG_CONFIG_HOME/gtk-3.0/bookmarks`, the file GTK file choosers and
other file managers use, with the shared `canopa_file::gtk_bookmarks`. Local
`file://` lines join Places after the configured places (deduplicated); server
lines join Network. **Add to bookmarks** appends one line after a successful
connection, and removing a server deletes only its own line; every other line
is kept byte for byte and the file is replaced atomically. Files' own Ctrl+D
bookmarks stay in its KDL configuration.

### Policies for remote folders

A folder is remote when its mount is a network filesystem in
`/proc/self/mountinfo` (`fuse.gvfsd-fuse`, `cifs`, `nfs`, `fuse.sshfs` and the
other types `crate::mounts` lists), read on the scan worker.

- **No Trash on GVFS locations.** GVFS exposes every share, phone and camera
  below one FUSE mount, so a `.Trash-UID` at its top would mix them, and no
  GVFS backend keeps a freedesktop Trash. `TrashLocation::for_source` refuses
  `fuse.gvfsd-fuse` explicitly and Trash discovery skips it. Move to Trash
  therefore reports those items as having no usable Trash and ADR0164's
  confirmed **Delete permanently** is offered for exactly them. Kernel network
  mounts keep the specification's per-volume Trash when one can be created.
- **No thumbnails by default.** ADR0167's `local` policy already treats network
  placements (GVFS included) as foreign; `always` still allows them.
- **No inotify.** inotify sees only this machine's own changes on a network
  mount, so remote folders are left out of the watched set. Instead the active
  tab of each window showing a remote folder is re-read every 10 seconds with
  ADR0163's rescan (selection, scroll and origins survive). Hidden tabs are not
  polled. F5 still refreshes at once.
- **No executable trust** (ADR0166) on network placements, as before.

### The operations engine on GVFS FUSE

Verified against a real `gvfsd-fuse` (below): new-item Copy and Move into a
GVFS folder, Move out of it, Undo of the copies, Trash refusal and permanent
deletion all work through the existing engine. Its primitives behave as
follows on `gvfsd-fuse` 1.62:

- `renameat2(RENAME_NOREPLACE)` is honoured (`EEXIST`), so publication never
  overwrites a file that appeared on the server.
- `name_to_handle_at` and unique mount IDs work, so restart identities can be
  captured; they are libfuse node IDs, so they detect changes made through
  this machine, not a file replaced at the same path by another client.
- Opening an existing file read-write fails (`EOPNOTSUPP`); the engine only
  writes to files it creates (`O_CREAT|O_EXCL`), which works. Symlinks fail on
  some backends; that item fails and the rest continue.
- `gvfsd-fuse` implements neither `fsync` nor `syncfs`; the kernel reports
  both as successful. Every `write` is passed synchronously to the GVFS backend
  and returns its error.

ADR0165's batched barrier is therefore unchanged and costs nothing here. What
makes "done" mean done for a remote target is the existing receipt: after
writing, the engine reads the whole output back to fingerprint it, and through
GVFS that read closes the upload stream and reopens the file on the server.
Copy now also requires the read-back length and the destination's reported
size to equal the unchanged source size, on every filesystem, so a truncated or
failed upload is a failed item instead of a published copy. A finished Copy to
a share means the server returned the complete file; whether the server's own
disk has flushed it is the server's guarantee, as for `cp` to a mounted share.

**Disconnection.** When a share disappears during a transfer, the kernel keeps
answering for the destination folder from FUSE's attribute and name caches for
a while, so its descriptor check passed and each remaining item failed with
`ENOENT`. A failed item now also confirms the destination root with
`statx(AT_STATX_FORCE_SYNC)`, which asks the filesystem itself; when the root
is gone the transfer stops for attention with ADR0165's continuation and the
interrupted item stays pending, instead of being failed. The device helper's
`mount-removed` observation also marks the root unavailable in Storage, so
queued work waits for the share to return (ADR0021). Local filesystems ignore
the forced synchronization; the extra `statx` runs only after a failure.

## Errors

- **Offline server:** GIO's host-not-found, unreachable, refused and timed-out
  errors, and backends' generic failures with those messages, read "Files cannot
  reach SERVER. Check that the server is on and connected to the network…".
- **Wrong credentials:** GVFS asks again; a final permission error reads "SERVER
  refused the login. Check the user name, domain and password". Cancelling a
  question (`G_IO_ERROR_FAILED_HANDLED`) is a cancellation, not an error.
- **Disconnected during an operation:** the transfer pauses for attention as
  above; Retry resumes once the share is mounted again.
- **Missing backends:** a scheme GIO does not list (`g_vfs_get_supported_uri_schemes`)
  or a mount failing with `G_IO_ERROR_NOT_SUPPORTED` names the package:
  `gvfs-smb`, `gvfs-nfs`, `gvfs-mtp`, `gvfs-gphoto2`, `gvfs-afc`, `gvfs-dnssd`,
  `gvfs-wsdd`, or `gvfs` for SFTP, FTP and WebDAV.

Failures appear on the originating tab, in the sidebar and in Operations.

## Verification

- `network` tests: address parsing and validation, aliases, UNC paths, IPv6,
  refused passwords, package names and failure wording, location kinds.
- `canopa-file` `gtk_bookmarks` tests: exact local bytes, labels, adding and
  removing without disturbing other lines, no temporary files left.
- `local_volumes` tests against a fake helper on the real pipe protocol: Connect
  sends only the normalized URI and navigates to its folder, the share is a
  Storage root and its loss blocks work; remember choices only with
  `SAVING_SUPPORTED`; a repeated password question after wrong credentials; an
  SSH host-key choice; failure and retry; bounded listings; remote-only devices.
  `interaction` tests apply `G_PASSWORD_SAVE_*` to the real `GMountOperation`.
- `mounts` tests: GVFS FUSE paths are network placements.
- Desktop model tests (`iced_desktop/network_tests.rs`): typed addresses connect
  instead of navigating and never echo a password, remote folders are polled and
  not watched, F5 still works, the sidebar model, and the Connect window.
- `tests/network_gvfs.rs` (ignored; a private D-Bus session, `gvfsd` and
  `gvfsd-fuse` with GVFS's `localtest` backend): Copy, Undo, Move out, no Trash,
  permanent deletion and placement on a real FUSE location, and a Copy that
  pauses for attention when the location is unmounted mid-transfer.
- `tests/network_runtime.rs` (opt-in with `CANOPA_FILES_NATIVE_NETWORK=1`):
  the production Application and helper connect to a private `sshd` through
  GVFS's SFTP backend, answer the host-identity question, copy through the
  queue, refuse Trash, disconnect, and report an offline server and the missing
  `gvfs-nfs` backend; `network:///` discovery settles.

## Not done

- Packaging: `gvfs`, `gvfs-smb`, `gvfs-mtp` and `gvfs-gphoto2` are still not
  dependencies of the Arch package.
- A GVFS backend's own trash, and per-share choices for kernel network mounts.
- Hands-on checks against a real SMB share, SFTP host and Android phone, and
  native acceptance of the window, sidebar and dialogs.
- Remembering a password needs a Secret Service provider (GNOME Keyring or
  KeePassXC); without one GVFS cannot keep it.
- Polling hidden tabs and ancestor columns of remote folders.
