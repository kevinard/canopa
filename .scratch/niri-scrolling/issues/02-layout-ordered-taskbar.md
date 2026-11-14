# Layout-ordered taskbar

Status: implemented

Order each application's windows, and running applications after the pinned
ones, by workspace index then `pos_in_scrolling_layout`, so the taskbar reads
in the same order as the screen. Window cycling (Shell ADR 0026) visits windows
left to right instead of by window ID. Floating windows follow their workspace's
tiled windows. Needs `WindowLayoutsChanged` in the Shell's Niri state, added by
issue 01.

## Validation

- `cargo test -p canopa-shell -- taskbar`: 11 pass, including a new case with
  two workspaces, a stacked column and a floating window.
- Not checked in a live session.

## Comments
