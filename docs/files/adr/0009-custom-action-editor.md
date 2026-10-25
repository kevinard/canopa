# Custom action configuration and native controls

Status: accepted under delegated implementation authority, 2026-10-16.

Custom actions live in the `files` namespace of the shared Canopa KDL file. Files
owns that namespace's schema; the shared envelope admits it without teaching
Shell, Notify or the appearance consumer its semantics. Files has no legacy
configuration fallback. Its headless `--check-config` validator participates in
`canopa config check`, alongside Shell and Notify. `--config` and
`--canopa-config` both select the shared format in Files.

The graphical editor opens through Ctrl+Shift+A, the folder menu or a file's
context menu. It edits a draft catalog, with explicit Save and Reload. Switching
between actions retains their drafts; closing or reloading a changed draft asks
whether to discard it. Adding, removing and moving actions changes their order
in menus and the custom toolbar. Each action exposes its name, optional theme
icon, command, filename patterns, allowed file/folder kinds, minimum/maximum
selection, shortcut and toolbar placement. A maximum of zero in the editor means
no maximum. A minimum of zero permits commands for the current folder with no
selection. Pattern alternatives are separated by semicolons.

Save validates every action and publishes the complete catalog to all browser
windows. It writes the shared file off the UI thread through a private temporary
file and atomic rename, retaining other namespaces and their comments. Existing
configuration symlinks remain symlinks; the editor replaces the selected target.
The editor compares its source bytes and target before writing and refuses stale
drafts. A cooperating Files editor lock prevents two Files writers from racing.
A final comparison narrows, but cannot eliminate, races with an unrelated text
editor that does not honor this lock. There is no claim of cross-process atomic
configuration installation. Other Canopa processes retain their existing reload
contracts. Explicit built-in defaults are not a writable configuration source.

Applicable actions appear in the file context menu. Shift+F10/Menu opens that
menu for the current selection; right-clicking an unselected row selects it,
while right-clicking within a selection preserves the selection. The menu also
provides Cut, Copy, Paste, Rename and Trash. Toolbar actions remain visible and
are disabled when their conditions do not apply. Their row scrolls horizontally
to preserve the browser's narrow layout. Applicability uses cached listing data
for presentation; the command worker rechecks actual metadata before execution.

The editor can record shortcuts. [ADR 0011](0011-interface-preferences.md)
now defines shared canonical validation against configurable navigation commands. Native
text fields keep their editing keys, and popovers and other controls retain
their own keyboard focus. Quick Look navigation explicitly routes to its source
browser independently of the browser's focused control.

Each invocation captures its paths and working folder. It appears in Operations
with status, bounded stdout/stderr, exit code and Cancel. Commands run independently
of the serialized file-operation queue and retain an application hold if windows
close. They do not depend on writable Undo storage and never create fabricated
Undo entries. The argument and process-group contract remains in ADR 0003.

Remaining acceptance includes long-running action cancellation/background
lifetime through the native UI, condition changes and error feedback, shortcut
recording, concurrent action/transfer presentation, and editor conflict/reload
flows. General toolbar/sidebar/shortcut preferences are now described in ADR 0011.
Live external configuration reload remains separate unfinished work.
