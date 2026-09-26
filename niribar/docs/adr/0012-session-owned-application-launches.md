# ADR 0012: Session-owned application launches

Status: Accepted

## Context

Configured Commands are bounded automation (ADR 0003). Using one to open a
terminal or browser kills the application at the action timeout. Spawning an
installed application directly also leaves it in `niribar.service`'s control
group, so restarting the bar can kill it.

## Decision

Add an explicit argv-only `launch "program" "argument"` Panel Action. It
hands application ownership to niri through its Spawn IPC request. When no niri
socket is advertised, use a separate systemd user scope. A failed niri handoff
is reported, never retried through another launcher: an ambiguous response must
not start the application twice. Only the handoff is bounded; applications have
no Niribar timeout. Owned commands and providers retain their existing cleanup.

Installed Desktop Applications retain the selected desktop file's path. Launch
ordinary Exec entries with GIO (`gio launch`) through the same session handoff.
GIO owns field-code expansion and Path. For DBusActivatable entries, send
`org.freedesktop.Application.Activate` directly on the session bus and await
acknowledgement with a bounded deadline. The bus owns auto-activation; errors
or timeouts do not trigger a second launch. Use the selected desktop filename
for the bus identity, preserving symlinks. Discovery retains desktop
ID precedence, GUI-only filtering and localized search metadata. Missing
executables, unavailable entries and handoff errors enter runtime diagnostics;
asynchronous application errors belong to the session launcher's diagnostics.

## Consequences

Existing `command` configuration remains bounded and compatible. Users opening
applications with it must migrate those actions to `launch`; the shipped menus
use the new action. Shell expansion is not implicit in `launch` arguments.
Exec desktop-entry launching requires GIO, and the fallback requires a
systemd user manager. Do not weaken the bar unit's KillMode or detach its owned
automation. Terminal desktop entries, desktop actions and activation tokens
remain future launcher features.
