# 0160. Present the scrolling layout through a window bar

Status: accepted

## Context

[0159](0159-desktop-window-chrome-and-overview.md) kept window chrome and the
overview with the compositor and gave the scrolling layout a window strip with
width-scaled tabs, an **In view** outline, destination previews and a **Next
view** row. In use, that strip asked people to decode several overlapping states,
and neighbouring windows clipped at the panels read as broken rather than
continuing. The default layout also left empty wallpaper or half-visible
neighbours at rest.

## Decision

Niri still owns the window outline, shadows, titlebar, movement and overview
geometry, and the framed overview keeps the behaviour 0159 describes: previews
clear of the reserved area, captions at the output scale, floating previews beside
the strip, and one selection for pointer and keyboard.

Above the tiles, a window bar lists every window of the workspace in layout order.
Each entry carries the application's icon and title, sized to its name, with a
count when a column holds stacked windows. Floating windows form a separate group
beside it, with the control that shows or hides the floating layer. One solid
accent fill marks focus: it slides to a neighbouring entry and crossfades when
focus jumps further, and a name the fill only partly covers is drawn in the focused
style inside the fill and its own style outside it. Windows at least half on
screen carry a faint accent tint and outline, neutral while a floating window has
focus. Arrows at the ends appear when windows lie out of view, show how many with
a `+N` badge, name that count on hover, and bring the next window into view.
Pointing at an entry or arrow for a window out of view shows its thumbnail.

At rest, a column that only partly fits beside a panel slides out of view rather
than being clipped; the focused column never moves for this. Columns open at half
the view width and the view scrolls only as far as needed, so two windows fill the
work area.

Opening the overview moves the windows that were in view straight to their grid
slots. Windows that were out of view wait in their slots and fade in behind them
during the second half, so no preview is ever seen through another.

## Consequences

The bar no longer previews the next view or labels the visible group; the arrows'
counts, tooltips and thumbnails replace them. The bar's appearance follows the
titlebar colors Settings generates, and `niri-custom.kdl` keeps priority for user
overrides. Small outputs without room for the bar keep the scroll cue cards.
