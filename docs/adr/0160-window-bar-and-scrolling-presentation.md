# 0160. Present the scrolling layout through a window bar

Status: accepted

## Context

[0159](0159-desktop-window-chrome-and-overview.md) kept window chrome and the
overview with the compositor and gave the scrolling layout a window strip with
width-scaled tabs, an **In view** outline, destination previews and a **Next
view** row. In use, that strip asked people to decode several overlapping
states, and neighbouring windows clipped at the panels needed a clearer frame.
The default layout also left empty wallpaper or half-visible neighbours at rest.

The framed overview of 0159, which laid every window out in a grid, showed all
windows at a glance but felt less fluid than Niri's own overview of zoomed-out
workspaces. Window management (column width, pinning, floating, tabs) lived in
the taskbar's application menu, which should only offer application actions.

## Decision

Niri still owns the window outline, shadows, titlebar, movement and overview.
The overview is Niri's own: the workspaces zoom out and stack, without frames,
captions or a separate grid.

Above the tiles, a window bar lists every window of the workspace in layout
order. Each entry carries the application's icon and title, sized to its name,
with a count when a column holds stacked windows. Floating windows form a
separate group beside it, with the control that shows or hides the floating
layer. One solid accent fill marks focus: it slides to a neighbouring entry and
crossfades when focus jumps further, and a name the fill only partly covers is
drawn in the focused style inside the fill and its own style outside it. Windows
at least half on screen carry a faint accent tint and outline, neutral while a
floating window has focus. Arrows at the ends appear when windows lie out of
view, show how many with a `+N` badge, name that count on hover, and bring the
next window into view. Pointing at an entry or arrow for a window out of view
shows its thumbnail.

The bar is a choice (`layout { window-bar }`, **Settings → Personalization →
Window layout**): shown on demand, the default, always shown in a row of its
own, or off. Shown on demand, it reserves no row and slides down over the
windows when the pointer touches the top edge of the work area, while keyboard
navigation moves between windows, or with Super+Shift+B, and slides away once
the pointer moves below it, the moves stop or a window is chosen. Shown on
demand, it appears over floating windows too, and the bar and the other controls
the compositor draws over windows take the pointer, so titlebar buttons beneath
them do not react. While it is hidden, and when it is off, a slim handle appears
in the gap at an edge with windows beyond it as the pointer nears; moving the
pointer into the gap opens it into an arrow naming the next window. Arrows that
came out beside the pointer over the windows took clicks meant for the
applications, so they only open from the gap.

A column that only partly fits beside a panel stays partly in view, cut one gap
from the panel, so it reads as the next window rather than disappearing; sliding
such columns out of view at rest was tried and hid windows people expected to
see. Columns open at half the view width and the view scrolls only as far as
needed, so two windows fill the work area. Proportional widths stay relative to
the whole work area, so pinning a column does not narrow the others; a column
narrows only when it would not fit.

Each titlebar carries a layout button beside maximize and close. Clicking it, or
resting the pointer on it, opens a menu of tiles that each show a small diagram:
a third, half, two thirds or the full width, pin left or right (or unpin), float
or tile, and tabs. A floating window offers only tiling. The taskbar menu keeps
application actions.

## Consequences

The bar no longer previews the next view or labels the visible group; the
arrows' counts, tooltips and thumbnails replace them. While a bar shown on
demand is hidden, windows out of view are marked only by the partly visible
neighbours and the arrows near the edges. When a panel appears or hides, the
leftmost window in view keeps its place beside it rather than the focused one
keeping its screen position. The bar's appearance and the layout menu follow the
titlebar colors Settings generates, and `niri-custom.kdl` keeps priority for
user overrides. Small outputs without room for the bar keep the scroll cue
cards. The compositor's menu labels are English only, like its other captions.
