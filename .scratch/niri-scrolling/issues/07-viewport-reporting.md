# Viewport reporting and control

Status: reporting implemented (fork PR kevinard/niri#14, pinned, now `b4fc5ea7`);
view control without focus declined.

## Niri fork (`kevinard/niri`)

Branch `canopa/viewport-ipc` (`f1dd795d` on `canopa/base`, PR #14):
"layout: Report scrolling tile positions in the workspace view over IPC".

- `WindowLayout.tile_pos_in_workspace_view` is now filled for scrolling tiles:
  the column's x minus the view position, plus the tile's offset in its column,
  rounded to physical pixels. Off-screen columns have a negative x or one past
  the view's width. Tiles of a tabbed column share a position; windows being
  moved interactively still report `None`. JSON is unchanged.
- It reports the settled view position: the animation's target, and during a
  gesture the position before it until the gesture ends. Animated scrolling
  therefore sends no per-frame events; each change of view target sends one
  `WindowLayoutsChanged` for the tiles whose position changed. A focus change
  that scrolls the view now also sends that event.
- Four new tests in `src/layout/tests/ipc_layout.rs`. `cargo test -p niri`
  passed (385 tests on the branch, 387 on the integration branch below).

A scroll action that keeps focus was not added: Niri keeps the focused column in
view, so the view would snap back, or it needs a new detached-view state through
much of the layout code. `FocusColumn { index }` scrolls but also moves focus.

## Shell

The minimap reads these positions with the output width from Niri's `Outputs`:
a frame marks the part of the strip on screen when some of it is off screen, and
columns outside the view are dimmer. With the packaged Niri (no positions for
tiled windows) it draws as before.

## Validation

- `cargo test -p canopa-shell -- minimap`: 12 pass, including the frame span
  for clipped and off-screen columns and no frame without positions.
- Private Weston/Niri session with the integration build
  (`target/niri-fork/canopa-integration/niri`, `v26.04-264-g232a4e72`): Niri
  reported x = 52 for the first column (40 px bar + 12 px gap) and 1280 for an
  off-screen one; the frame covered the first two columns with the first one
  focused, and the last one plus the visible part of the third with the last one
  focused. Evidence in `.scratch/artifacts/niri-scrolling/minimap-4`.

## Comments
