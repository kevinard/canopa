# Files application ownership and state

Status: accepted under delegated implementation authority, 2026-10-16.

The application is named Canopa Files, with package and executable
`canopa-files`. It lives in `crates/canopa-files/` and uses `canopa-theme` and
shared configuration intake. The GUI uses the workspace's Iced stack and
`canopa-ui`; [the current toolkit decision](../../adr/0018-files-iced-and-shared-controls.md)
supersedes the intervening GTK implementation choice.
It has ordinary desktop windows; Shell's layer surfaces are not an application
window manager.
It does not put filesystem policy into `canopa-file`, which remains the shared
bounded local-read primitive.

[ADR0074](0074-renderer-independent-application-runtime.md) clarifies the intended
seam between domain models, renderer-independent application coordination and
the Iced adapter. That extraction remains incomplete; the historical GTK
adapter's application workflows must not simply be copied into Iced.

Browser state is independent of visible column count. A Tab retains its
location, selection, focus, scroll position, and navigation history; each folder
can restore its prior browsing state. Parent navigation selects the child just
left. Column visibility is derived from width, with 240 logical pixels as the
initial readable column width. Exact desktop thresholds remain subject to
native acceptance.

Directory enumeration, recursive filename search, preview preparation, and
transfers run off the UI thread. Native updates must reject stale directory,
search, and preview results. Search does not follow directory symlinks, so a
symlink cycle cannot turn a filename search into unbounded traversal. Explicit
navigation into a directory symlink remains supported.

Paths retain their original Unix bytes, including non-UTF-8 filenames. JSON
state uses ordinary strings for UTF-8 paths and a byte representation otherwise;
display strings never become operation targets or command arguments.

The initial keyboard policy uses Left/Right for parent/selected-folder
navigation, Up/Down for selection, Enter to open, F2 to rename, Space for Quick
Look, Ctrl+L for a location, Ctrl+F for filename search, and normal Ctrl-based
clipboard, tab, and window shortcuts. Editable fields retain text editing keys.
User shortcuts can override commands through preferences.

The public test seams are the browsing model, operation engine, Trash,
preferences, preview preparation, and custom-action executor. This seam choice
is made under the user's instruction to resolve implementation decisions and
record them without further interviews. Tests use real temporary files; native
acceptance verifies compositor behavior separately.

Product decisions remain in the product brief. New decisions and validation
evidence must be recorded as implementation proceeds; this ADR does not claim
the application is complete.
