# Launcher file actions and Settings pages

Date: 2026-11-03
Status: accepted

Shell's unified launcher searches native windows, files, controls and applications
within the existing Panel Menu. Shell owns matching, selection and its private
bounded ranking history. Niri owns focusing the selected live window. Controls
open the existing provider panels; Settings shortcuts launch `canopa-settings
--page displays|input|personal|power`. Selecting a page writes no preferences.

Reveal sends the exact local URI to standard FileManager1.ShowItems, retaining
[ADR 0025](0025-files-file-manager-interface.md)'s admission and owner boundaries.
Open with remains a Shell Panel Menu child mode populated off-thread through GIO with file-capable installed applications and MIME
recommendations first;
the selected desktop entry receives the original URI using the established session
launch handoff or org.freedesktop.Application.Open. MIME defaults are unchanged.

Quick Look is owned by Files. Add `canopa-files --quick-look PATH_OR_FILE_URI` and
an additive QuickLook intent to Files' existing acknowledged activation channel.
Normalize relative paths in the caller before forwarding. Preserve original Unix
bytes and reject malformed/nonlocal URIs before admission. An existing owner is
reused; an older owner may refuse the new command, without a duplicate runtime or
blind retry. This does not change activation protocol framing or ownership.

Files creates a Browser Window at the parent, selects the exact path, waits for
its directory result and initial geometry, then opens the existing source-owned
Quick Look Window. Navigation or an unavailable exact selection retires preview
intent. Dismissal returns focus to that Browser Window under existing policy.
Shell introduces no new placement or preview renderer. See
[Files ADR 0161](../files/adr/0161-launcher-quick-look-admission.md).
