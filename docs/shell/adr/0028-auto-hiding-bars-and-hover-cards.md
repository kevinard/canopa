# Auto-hiding bars and hover cards

Date: 2026-11-26
Status: accepted

Window management moved into the compositor's titlebars and window bar, so the
bar no longer needs its own layout commands, and people asked for a bar that
gives its space back to windows. Taskbar tooltips showed window thumbnails that
could not be clicked.

## Auto-hiding bars

A Bar takes `visibility "always" | "auto-hide"` (default `always`). An
auto-hiding Bar Surface keeps its size and layer but has no exclusive zone, so
windows use the whole output. The bar is drawn translated toward its edge,
clipped by its own surface, instead of resizing or re-margining the layer
surface: a slide costs redraws only, never a compositor configure per frame, and
the surface, its tooltips and its menus keep their placement rules. While the
bar is out of view the input region is a 2-pixel strip along its edge; while it
is shown, the region also covers the margin between the edge and the bar, so the
pointer never leaves the surface on its way in.

The bar slides in (170 ms, ease-out) when the pointer enters the strip, and
leaves (230 ms, ease-in-out) 500 ms after the pointer leaves it. It stays while
one of its panels or menus is open, while a tooltip or hover card of it is open,
while a pointer button is held on it, or while it has keyboard focus. A panel
opened by a shortcut brings its hidden bar along, so the panel never floats
beside empty space. Reduced motion jumps instead of sliding. Each Bar Surface
keeps this state; a policy pass after every update applies it, and a frame timer
runs only while a bar slides or waits to leave.

`canopa-shell --toggle-bar` (typed IPC request `toggle-bar`, bound to `Mod+B` in
the shipped Niri defaults) flips every bar between the two modes and changes its
exclusive zone in place. The switch is runtime state: the Shell never rewrites
KDL, so the next configuration install restores the configured mode.

## Hover cards

Hovering a running taskbar application or a workspace indicator opens a hover
card in the tooltip's layer surface, after the tooltip dwell. A card is a
tooltip with interactive content: it keeps the tooltip's placement, styling and
lifetime, gets pointer input, and never takes the keyboard.

- The taskbar card lists the application's windows in screen order (at most
  eight, then a count), each with a thumbnail, title and workspace. Clicking a
  row focuses the window; its close button asks it to close. Applications
  without open windows keep the plain tooltip.
- The workspace card draws the workspace's columns to scale with the same strip
  layout as the minimap module (`draw_strip`, with larger metrics), each tile
  filled with the window's thumbnail or, until one arrives, its application's
  icon. The focused window is ringed in the accent, the frame marks the part on
  screen, pinned columns stand apart, and floating windows are counted. Clicking
  a tile focuses the window, switching workspace as needed. An empty workspace
  says so.

A card is centered on its module's measured bounds, not on the last pointer
motion. It stays while the pointer travels from the module into it and closes
300 ms after the pointer has left both; moving to another module with a card
opens that one after 90 ms instead of the full dwell. Cards update in place
when windows, titles, focus or thumbnails change, resizing their surface only
when their size changes. Thumbnails come from `org.canopa.Niri.WindowPreview1`
at 320 pixels and are kept per open window. `tooltip #false` on the module
turns its card off.

## Taskbar menu

The taskbar's right-click menu keeps application actions only: Open or New
window, the desktop file's actions, and Pin or Unpin. The column width, tab,
center, float, column pin and move-to-workspace rows are removed; the
compositor's titlebars and window bar own window management, and the hover card
owns choosing and closing windows. This supersedes the layout rows that
[ADR 0027](0027-niri-layout-integration.md) listed for the taskbar menu.
[ADR 0029](0029-quit-from-the-taskbar-menu.md) adds **Quit** for a running
application; closing one window stays with the hover card.

## Consequences

- Hover cards are pointer-only and are not exposed to AT-SPI; the taskbar items,
  workspace indicators and the launcher's window search remain the accessible
  paths to the same windows.
- An auto-hiding bar needs layer `top` or `overlay`; configuration validation
  rejects `background` and `bottom`, where the revealed bar would sit under
  windows.
