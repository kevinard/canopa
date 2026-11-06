# 0114: Per-application FileChooser folder memory

Status: implemented; focused, public Iced and real Flatpak checks pass, 2026-11-05.

When an application supplies neither current_folder nor current_file, open the
chooser in that application's last accepted folder. Explicit application hints
take precedence. A current_name alone supplies a filename and can still use the
remembered folder. Keep this preference in the backend, independent of Iced
helpers and the frontend's Documents access grants.

After a valid accepted response, remember the selected directory for a single
directory selection, or the common parent of selected file destinations.
Selections with different parents do not update it. Cancellation, invalid
selections and presentation failures do not update it. Anonymous callers have
no shared preference bucket. Use only the application identity supplied by the
trusted frontend; the backend's existing caller checks remain in force.

Store application IDs, original Unix folder bytes and an eviction counter in
`$XDG_STATE_HOME/canopa/portal-file-chooser.db`, falling back to
`$HOME/.local/state/canopa/portal-file-chooser.db`. Store no selected filenames,
document contents, filters, credentials or access grants. Limit the SQLite
database to 2 MiB and the last 128 applications by accepted update. Create the
file with mode 0600, and require an owned regular single-link private file.
Reject symlinks, unknown schema versions and foreign databases without rewriting
them. Reading absent state creates nothing. The database is opened per operation;
there is no persistent in-process folder cache.

Preferences are advisory. Unavailable, unsafe, corrupt, busy or stale state falls
back to the ordinary chooser defaults. A missing folder does not prevent opening
the chooser. Keep blocking filesystem/SQLite work off the async executor and
admit one operation at a time without queuing. Await it for at most 200 ms; the
operation retains its permit until its actual blocking work finishes. This is a
deadline for waiting on a preference, not a promise to interrupt kernel I/O.
Failed or skipped bookkeeping must not invalidate an accepted file choice.

`--no-file-chooser-history` disables both reads and writes. It does not erase an
existing preference file. To reset the preference, remove this one database while
the backend is stopped. Restarting the backend normally retains folder memory.
Remembering a location never grants the application access to that folder.

The focused tests cover persistence, independent applications, original path
bytes, eviction, unavailable folders, private state, foreign/future state and
contention. Wire tests cover explicit hints, valid acceptance, invalid responses
and cancellation. Native checks exercise actual Iced choices across backend
restart, application isolation and deleted-folder fallback. The real Flatpak
check separately verifies the stored authenticated application ID and that only
the accepted folder is recorded.

Acceptance on backend dfdf49c5: six storage tests, two FileChooser wire tests,
20 public native groups at dark/100%, and three real Flatpak Save/identity groups
pass. The full compiled Portal library passes 457 tests with 20 ignored. Normal
workspace binaries, strict workspace Clippy and formatting pass on the original
default feature graph. The latest full workspace test gate remains phase86;
these Portal results do not establish new full-workspace or universal client
acceptance. Existing phase88 light/125% evidence retains its original identity.
Receipts: target/portal-chooser-history-89/ and
target/portal-chooser-history-flatpak-89/native-dark-01/summary.json.
