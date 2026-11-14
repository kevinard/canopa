# Placement on spawn

Status: implemented. Fork PR kevinard/niri#16, pinned (now `b4fc5ea7`); session
restore and templates in the Shell.

Branch `canopa/spawn-placement` (`36ccdb7c` on `canopa/base`, PR #16).

New IPC action `SpawnPlaced`; every placement field is optional:

```json
{"Action":{"SpawnPlaced":{"command":["foot","-e","htop"],"placement":{"workspace":{"Name":"dev"},"column_index":2,"width":{"SetProportion":50.0},"display":"Tabbed","floating":false}}}}
```

CLI: `niri msg action spawn-placed --workspace dev --column 2 --width 66.667% --display tabbed --floating false -- foot -e htop`.

- `column_index` is 1-based like `FocusColumn`; past the end appends. `width`
  is `SetFixed` (logical px) or `SetProportion` (percent, as `SetColumnWidth`);
  adjustments, column 0, non-positive widths and empty commands are refused.
- Matching: Niri already gives spawned programs an activation token
  (`XDG_ACTIVATION_TOKEN`, `DESKTOP_STARTUP_ID`). The placement is bound to it
  and applied to the first toplevel that presents it before mapping. PIDs were
  rejected: Niri double-forks, and wrappers, Flatpak and D-Bus-activated or
  single-instance applications open windows from other processes, while GTK and
  Qt forward the token. A program that never presents the token opens normally.
- A token presented before the first configure shapes that configure; between
  configure and map the window is reconfigured; after mapping the placement is
  dropped.
- Workspace references resolve when Niri handles the request (an index means
  the focused monitor at that moment), so session restore should use names or
  IDs. A missing workspace falls back to normal placement; the other fields
  still apply. Column index is ignored for floating windows.
- One use per placement, 60-second expiry, at most 32 pending (oldest dropped).
  A token with a pending placement stays valid until it expires, but no longer
  grants focus after Niri's usual 10 seconds.

Risks: programs that ignore the token (some Electron applications, X11 through
xwayland-satellite) are not placed; a window that maximizes before mapping is
sized against the target monitor's active workspace, though it still opens in
the right place.

## Validation (fork)

- `cargo test --locked -p niri -p niri-config -p niri-ipc --no-fail-fast`: niri
  396, niri-config 36, niri-ipc 5 + 1 doctest pass. New: nine end-to-end tests
  in `src/tests/spawn_placement.rs` (workspace and column, inactive workspace,
  past the end, missing workspace, unrelated windows, single use, expiry, late
  token, floating), five placement-store unit tests, a layout test and a fuzzer
  option.
- Not run in a nested or live session.

## Shell

- Session restore, `windows { restore #true; }` (off by default):
  `crates/canopa-shell/src/app/session_restore.rs` records application, output,
  workspace index and name, column and window width in
  `$XDG_STATE_HOME/canopa/shell/windows.json`, one second after a change and
  eight after a close (ending a session closes every window). At the first
  window list after connecting, if no recorded application has a window yet, it
  starts the recorded ones one by one with `SpawnPlaced`, by workspace name or
  index on the focused output, columns renumbered from 1, and a fixed width:
  a proportion would shrink each session because Niri applies it to the
  working area less its gaps, which the Shell cannot see. Saving pauses while
  restored applications open. Stacked windows after the first of a column,
  dialogs and applications without a desktop entry are not recorded;
  single-main-window applications start once.
- Workspace templates, `templates { template "dev" { app "code" width=66.667 } }`,
  opened with `canopa-shell --open-template dev` (new IPC request
  `OpenTemplate`): the applications open one column each, at the given
  proportion, on the empty workspace after the focused output's last one, which
  is then focused.
- Applications start through the app-scope helper and `gio launch`, like other
  launches; D-Bus-activated applications may open their window without Niri's
  activation token, and then unplaced.

## Validation (Shell)

- `cargo test -p canopa-shell`: record contents and order, placements, record
  round trip and refusal of foreign files, the restore waiting for Niri's window
  list (so a Shell restart does not start applications twice), template parsing,
  template workspace choice and placements, and the CLI flag.
- Private Weston/Niri session with the pinned build, two sessions sharing state:
  two fixture applications at 410 and 833 px in columns 1 and 2 were restored in
  the same columns at 410 and 833 px; `canopa-shell --open-template pair` opened
  them in columns 1 and 2 of workspace 2 at half width (622 px) and focused it.

## Comments
