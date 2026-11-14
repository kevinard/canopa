# Making the most of Niri's scrolling layout

Status: implemented. Every issue is done except the parts each issue file lists
as left out; the compositor parts are tracking PRs kevinard/niri#14–#20, merged
into `canopa/main` and pinned at `b4fc5ea7`.

Canopa runs on Niri, but the desktop treats it like an ordinary window manager.
The Shell sends Niri three requests (`FocusWindow`, `FocusWorkspace`, `Spawn`)
and reads window, workspace and focus events. The taskbar treats windows as an
unordered set. Each window's place in the strip (`pos_in_scrolling_layout`)
arrives with every layout update, but only Notify reads it
(`crates/canopa-notify/src/compositor_suppression.rs`). The shipped
`packaging/niri/defaults.kdl` binds column focus and movement, maximize and the
overview. It has no preset widths, no stacking or tabbed columns, no gestures,
no hot corner and no window switcher. The Canopa Niri fork changes only input,
capture, shortcut-lease, drag-and-drop and screencast behavior, not the layout.

The main weakness of a scrolling layout is that the windows off screen are
invisible. This PRD makes the strip visible and controllable from the desktop,
first with what Niri 26.4 already reports, then with small compositor additions.

## Goals

- People can see how many windows each workspace holds, in what order, and
  which column is focused, without opening the overview.
- The common layout operations (width presets, stacking, tabs, centering) are
  reachable from the defaults, the taskbar and Settings.
- The compositor reports enough for the Shell to draw the visible area exactly
  and to place restored windows deterministically.

## Non-goals

- Replacing Niri's overview or drawing window contents in the Shell before
  window previews exist (issue 08).
- Supporting compositors other than Niri for these features. The Shell keeps
  degrading to nothing when Niri IPC is unavailable (Shell ADR 0002).

## Part 1: no compositor changes

| Issue | Feature | Summary |
| ----- | ------- | ------- |
| [01](issues/01-strip-minimap.md) | Strip minimap | A bar module draws the active workspace's columns and stacked windows to scale; a click focuses a window, scrolling moves between columns. |
| [02](issues/02-layout-ordered-taskbar.md) | Layout-ordered taskbar | Order running windows by workspace and column, so the taskbar matches the screen. |
| [03](issues/03-taskbar-layout-menu.md) | Layout commands in the taskbar menu | Width ⅓/½/⅔, tabbed, center, float, move to workspace, through existing Niri actions. |
| [04](issues/04-overview-integration.md) | Overview integration | React to `OverviewOpenedOrClosed` (typing searches the launcher); wallpaper uses `place-within-backdrop`. |
| [05](issues/05-niri-defaults.md) | Defaults pass | Preset widths, consume/expel, tabbed display, center, first/last column, wheel scrolling, gestures, hot corner, recent windows, numbered workspaces, per-app rules, blur. |
| [06](issues/06-settings-layout-page.md) | Settings Layout page | Gaps, presets, centering and per-app widths, as changes over shipped defaults (ADR 0135). |

## Part 2: compositor additions (Canopa Niri fork)

Each change follows `docs/vendor-maintenance.md`: one tracking PR per change in
`kevinard/niri`, upstream attribution kept, pinned through `packaging/forks.json`.
Generic actions and events (07, 09, 10) are good upstream candidates and should be
proposed there too, to keep the fork small. Features that need Canopa to grant
authority (08) follow the fork's private D-Bus service pattern (`RemoteInput1`,
`GlobalShortcuts1`).

| Issue | Feature | Summary |
| ----- | ------- | ------- |
| [07](issues/07-viewport-reporting.md) | Viewport reporting and control | Report tiled windows' positions in the view (today only floating windows have `tile_pos_in_workspace_view`), throttle the events, add an action to scroll the view without changing focus. Gives the minimap an exact visible-area frame and drag-to-scroll. |
| [09](issues/09-placement-on-spawn.md) | Placement on spawn | `Spawn` carries a workspace, column index, width and display mode, bound to the activation token. Enables session restore and workspace templates. |
| [10](issues/10-pinned-columns.md) | Pinned columns | A column that stays on screen while the rest of the strip scrolls. |
| [08](issues/08-window-previews.md) | Live window previews | Per-window capture for the Shell (reusing ADR 0118/0041 capture), for taskbar hover previews, a richer switcher and a minimap with content. |
| [11](issues/11-dialog-parents.md) | Dialog parents in IPC | Report a dialog's parent window, so the taskbar and minimap group dialogs. |
| [12](issues/12-layout-announcements.md) | Screen-reader layout announcements | "Column 3 of 5, Files, tabbed" on layout moves, voiced through the AccessKit layer (ADR 0144). |
| [13](issues/13-touchscreen-gestures.md) | Touchscreen gestures | Strip scrolling and overview from the touchscreen; Niri's gestures are mostly touchpad today. |

## Order

1. Issue 01 (minimap) and 02 (taskbar order): no compositor work, largest gain.
2. Issue 05 (defaults).
3. Issue 07, which completes the minimap, then 09, which session restore and
   templates build on.

## Comments
