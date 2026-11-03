Status: ready-for-human

# Add the native launcher panel with application search

## Parent

[Unified Shell Launcher](../PRD.md).

## What to build

Present the Launcher Session through the existing Panel and Foreground owners,
initially using applications. Reuse the named Applications menu entry points and spine action.
Document the existing CLI/IPC command as the compositor-wide binding target.

## Acceptance criteria

- [ ] Spine and global invocation open the same existing Panel Menu on the focused
  Output, with deterministic fallback, immediate text focus, and bounded geometry.
- [ ] The bundled spine launcher retains its named-menu toggle action; custom menu
  actions and the existing Applications menu remain usable.
- [ ] Query field, result label/context/icon/category, loading state, empty state,
  provider failure, and additional named actions are host-rendered and accessible.
- [ ] Keyboard navigation, Enter, first-Escape clear, second-Escape close, outside
  click, focus loss, replacement, and Output removal follow the session contract.
- [ ] Installed applications preserve desktop-ID precedence, localized search
  metadata, visibility filtering, icons, and session-owned launch behavior.
- [ ] Blank queries show a stable application list; typed queries remain responsive
  during discovery and provider updates. Old-query rows cannot be activated.
- [ ] Successful launch dismisses the launcher; synchronous failure remains visible.
- [ ] Configuration candidates validate new references and retain the previous
  configuration on failure; changed installations retire obsolete sessions.
- [ ] Focused runtime/CLI tests and private native checks cover focus, replacement,
  both themes, a small Output, and fractional scale.

## Dependencies

[01](01-launcher-session.md).

## Implementation review — 2026-11-03

Implemented within the existing Panel Menu source path; see
[ADR 0023](../../../docs/shell/adr/0023-unified-launcher-providers.md), the
[launcher guide](../../../docs/shell/launcher.md), and
[validation evidence](../VALIDATION.md). Original proposal criteria remain as
historical scope; persistent extension sessions, dedicated CLI/action entry points,
and new placement were replaced by the documented implementation decisions.
Further native acceptance matrices are not implied by the focused checks.
