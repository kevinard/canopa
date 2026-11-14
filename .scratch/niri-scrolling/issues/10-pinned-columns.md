# Pinned columns

Status: implemented. Fork PR kevinard/niri#20, pinned (`b4fc5ea7`); minimap,
taskbar menu and default bindings in Canopa.

## Niri fork

Branch `canopa/pinned-columns` (`25dd5a42` on `canopa/base`): "layout: Add
pinned columns". At most one pinned column per side; it keeps its index at the
end of the strip, takes no strip width (the other columns get the working area
minus it and a gap), draws at its edge, and stays put while the rest scrolls.
Moves within the workspace are refused; moving it elsewhere, closing it or
floating it unpins it; fullscreen and maximize keep the pin. Actions
`PinColumn { side }`, `UnpinColumn {}`, `TogglePinnedColumn { side }`; window
rule `open-pinned`; IPC `WindowLayout.pinned` (`"Left"`, `"Right"`, `null`).
Wiki page `Pinned-Columns.md`. The merge into `canopa/main` with
`canopa/viewport-ipc` reports pinned columns at their edge in
`tile_pos_in_workspace_view`.

## Canopa

- Shipped bindings: `Mod+P` toggles a left pin, `Mod+Shift+P` a right pin
  (Settings labels them).
- Minimap: a pinned column is outlined and set apart from the strip; its
  description says "pinned to the left edge".
- Taskbar menu: "Pin column to the left edge", "… right edge", or "Unpin
  column".
- Window session restore does not restore pins (`SpawnPlaced` has no pin field).

## Validation

- Fork: 15 pinned-column tests, pin ops in the layout fuzzers plus a biased
  fuzzer (4000 cases each, and the exhaustive operation tests); at its branch
  niri 263 pass; on `canopa/main` niri 458 pass. A nested session checked
  widths, edge position while focus scrolls, the rule, fullscreen and unpin.
- Canopa: `cargo test -p canopa-shell` (minimap gaps and pinned state, taskbar
  pin and unpin rows); `niri validate` of the defaults with the pinned build.
- Private Weston/Niri session with the pinned build and the Shell:
  `pin-column left` on the last of three columns moved it to column 1 at x = 12
  with `pinned: "Left"`, and the minimap drew it outlined at the left, apart from
  the narrowed strip (`.scratch/artifacts/niri-scrolling/preview-pin/pinned.png`).

## Comments
