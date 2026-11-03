# Launcher Quick Look admission

Date: 2026-11-03
Status: accepted

The Shell launcher can request Quick Look through the existing Files activation
owner using `--quick-look PATH_OR_FILE_URI`. This additive command uses raw-byte
path serialization and normalizes relative paths in the invoking process before
forwarding. It requires a display and admits one exact local path; invalid input
is refused before any browser is created. Older runtimes may refuse this additive
intent without relinquishing ownership or starting a second runtime.

Create a Browser Window with the parent and initial exact selection. Wait for
both the matching directory result and confirmed source geometry before opening
the existing transient Quick Look Window. If the file has disappeared, show an
error instead of previewing the browser's fallback selection. Subsequent navigation
or changing the active tab retires pending intent. Preserve ADR 0098's source-owned
preview dismissal/focus policy. No Shell-owned preview window is introduced.

This extends presentation admission only. Browsing, preview preparation, file
operations, recovery and Undo retain their existing owners. The shared contract
is recorded in [root ADR 0037](../../adr/0037-launcher-file-actions-and-settings-pages.md).
