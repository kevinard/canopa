# Layout commands in the taskbar menu

Status: implemented.

The application menu of Shell ADR 0026 gains, for a running application, a
layout section for its window in front (or its most recent window): One-third,
Half and Two-thirds width (`SetColumnWidth` 33/50/67 %), Show column as tabs or
stacked (`ToggleColumnTabbedDisplay`), Center column (`CenterColumn`), Float or
Tile window (`ToggleWindowFloating`), and Move to workspace above or below
(`MoveWindowToWorkspace` by ID, focus staying put). Floating windows get only
Tile and the moves. Column commands focus the window first, because Niri runs
them on the focused column; the actions are sent in order and stop at the first
failure. Applications without a desktop file still get no menu.

The tabs entry is a toggle because Niri does not report a column's display
mode; issue 12 proposes reporting it.

## Validation

- `cargo test -p canopa-shell -- taskbar`: 14 pass, including menu rows,
  neighbouring workspaces and the action sequences.
- Not checked in a live session.

## Comments
