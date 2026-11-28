# Quit from the taskbar menu

Date: 2026-11-27
Status: accepted; supersedes part of [ADR 0028](0028-auto-hiding-bars-and-hover-cards.md#taskbar-menu)

[ADR 0028](0028-auto-hiding-bars-and-hover-cards.md) kept the taskbar's
right-click menu to application actions and left choosing and closing windows to
the hover card. Reviews of the shell kept looking for a way to end a running
application from its menu, as every major desktop offers, while closing one
particular window from the hover card was judged enough.

## Decision

A running application's menu ends, after a divider, with **Quit**. It asks each
of the application's windows to close through Niri (`CloseWindow` per window),
exactly as each titlebar's close button would, so applications keep their own
confirmation for unsaved work. There is no per-window close and no window list
in the menu: the hover card stays the place to choose or close one window. Open
or New window, the desktop file's actions and Pin/Unpin are unchanged.

## Consequences

- Quitting is a request, not a kill: an application that asks to save, or that
  keeps a window open, stays running.
- An application without windows (only pinned) shows no **Quit**.
- Windows the taskbar does not group under the application (another app id) are
  not closed.
